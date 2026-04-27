#![deny(missing_docs)]

//! Runtime contracts for DICOMweb startup policy, persistence preflight,
//! recovery visibility, worker/queue backpressure behavior, route classification,
//! interop policy, diagnostic redaction, and environment variable parsing.

use dicom_env_contract::{
    parse_bool, parse_optional_string, parse_u64, parse_usize, NumericBounds,
};
use dicom_web::{dicomweb_status_for_error, TlsPolicy, TransportSecurity};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::Error as IoError;
use std::path::{Path, PathBuf};

/// Re-export of [`std::io::ErrorKind`] for convenience in error-kind matching.
pub use std::io::ErrorKind as IoErrorKind;

/// Re-export of [`dicom_core::Error`] for structured error handling.
pub use dicom_core::Error;

/// Re-export of [`dicom_core::ErrorKind`] for structured error-kind matching.
pub use dicom_core::ErrorKind;

/// Re-export of [`dicom_core::Limits`] for storage and parsing limits.
pub use dicom_core::Limits;

/// Re-export of [`dicom_storage::IngestOutcome`] for STOW result handling.
pub use dicom_storage::IngestOutcome;

/// Re-export of [`dicom_storage::Storage`] for durable DICOM object persistence.
pub use dicom_storage::Storage;

/// Re-export of [`dicom_web::HttpMethod`] for HTTP method classification.
pub use dicom_web::HttpMethod;

const DICOM_WEB_SERVICE_NAME: &str = "dicom-web-server";
const DEFAULT_WEB_QIDO_P95_LATENCY_MS: u64 = 250;
const DEFAULT_WEB_QIDO_P99_LATENCY_MS: u64 = 500;
const DEFAULT_WEB_WADO_P95_LATENCY_MS: u64 = 400;
const DEFAULT_WEB_WADO_P99_LATENCY_MS: u64 = 800;
const DEFAULT_WEB_STOW_P95_LATENCY_MS: u64 = 1200;
const DEFAULT_WEB_STOW_P99_LATENCY_MS: u64 = 2400;

/// Startup diagnostics for environment-derived runtime policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupPolicyDiagnostics {
    /// Effective TLS policy.
    pub tls_policy: TlsPolicy,
    /// Effective auth mode label.
    pub auth_mode: &'static str,
    /// Effective request transport classification.
    pub transport_security: TransportSecurity,
    /// True when `DICOM_WEB_TLS_POLICY` had an invalid value and fell back.
    pub tls_defaulted: bool,
    /// True when `DICOM_WEB_AUTH_MODE` had an invalid value and fell back.
    pub auth_defaulted: bool,
    /// True when `DICOM_WEB_TRANSPORT_SECURITY` had an invalid value and fell back.
    pub transport_defaulted: bool,
}

impl StartupPolicyDiagnostics {
    /// Return whether the resulting policy posture is fail-closed by default.
    pub fn fail_closed(&self) -> bool {
        self.auth_mode == "deny_all"
            || (self.tls_policy == TlsPolicy::RequireTls
                && self.transport_security == TransportSecurity::Insecure)
    }
}

/// Resolve startup policy values and fallback indicators from optional raw inputs.
pub fn startup_policy_diagnostics(
    tls_policy_raw: Option<&str>,
    auth_mode_raw: Option<&str>,
    transport_raw: Option<&str>,
) -> StartupPolicyDiagnostics {
    let (tls_policy, tls_defaulted) = match tls_policy_raw {
        Some("allow_insecure") => (TlsPolicy::AllowInsecure, false),
        Some("require_tls") | None => (TlsPolicy::RequireTls, false),
        Some(_) => (TlsPolicy::RequireTls, true),
    };
    let (auth_mode, auth_defaulted) = match auth_mode_raw {
        Some("allow_all") => ("allow_all", false),
        Some("deny_all") | None => ("deny_all", false),
        Some(_) => ("deny_all", true),
    };
    let (transport_security, transport_defaulted) = match transport_raw {
        Some("tls") => (TransportSecurity::Tls, false),
        Some("insecure") | None => (TransportSecurity::Insecure, false),
        Some(_) => (TransportSecurity::Insecure, true),
    };
    StartupPolicyDiagnostics {
        tls_policy,
        auth_mode,
        transport_security,
        tls_defaulted,
        auth_defaulted,
        transport_defaulted,
    }
}

/// Preflight diagnostics for file-backed runtime persistence artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistencePreflightDiagnostic {
    /// Human label for the artifact.
    pub label: String,
    /// Artifact file path.
    pub path: String,
    /// Whether the parent directory had to be created.
    pub parent_created: bool,
    /// Whether file-open readiness checks completed.
    pub file_ready: bool,
    /// Whether rollover rotation was performed due to `max_bytes` overflow.
    pub rotated: bool,
    /// Configured retained rotation depth.
    pub max_rotated_files: usize,
    /// Whether rollover used zero-retention destructive mode.
    pub destructive_rollover: bool,
}

/// Ensure persistence file readiness and return deterministic preflight diagnostics.
pub fn prepare_persistence_file_with_diagnostics(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<PersistencePreflightDiagnostic> {
    let path = Path::new(path);
    let mut parent_created = false;
    if let Some(parent) = path.parent() {
        parent_created = !parent.exists();
        fs::create_dir_all(parent)?;
    }

    let mut rotated = false;
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_dir() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                format!(
                    "{label} path must reference a file, not a directory; choose a file target path"
                ),
            ));
        }
        if meta.len() > max_bytes {
            rotate_file(path, max_rotated_files)?;
            rotated = true;
        }
    }

    let file = OpenOptions::new().create(true).append(true).open(path)?;
    file.sync_all()?;

    Ok(PersistencePreflightDiagnostic {
        label: label.to_string(),
        path: path.display().to_string(),
        parent_created,
        file_ready: true,
        rotated,
        max_rotated_files,
        destructive_rollover: rotated && max_rotated_files == 0,
    })
}

/// Ensure persistence file readiness, discarding diagnostic details.
pub fn prepare_persistence_file(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<()> {
    let _ = prepare_persistence_file_with_diagnostics(path, max_bytes, max_rotated_files, label)?;
    Ok(())
}

/// Rotate a persistence artifact with bounded suffix depth.
pub fn rotate_file(path: &Path, max_rotated_files: usize) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if max_rotated_files == 0 {
        fs::remove_file(path)?;
        return Ok(());
    }

    let oldest = path_with_suffix(path, max_rotated_files);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }

    for index in (1..=max_rotated_files).rev() {
        let src = if index == 1 {
            path.to_path_buf()
        } else {
            path_with_suffix(path, index - 1)
        };
        let dst = path_with_suffix(path, index);
        if src.exists() {
            fs::rename(src, dst)?;
        }
    }
    Ok(())
}

/// Build a deterministic rotation suffix path (`.1`, `.2`, ...).
pub fn path_with_suffix(path: &Path, suffix_index: usize) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(format!(".{suffix_index}"));
    PathBuf::from(os)
}

/// Deterministic worker/accept-queue contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerQueueContract {
    /// Number of request workers.
    pub workers: usize,
    /// Accept queue depth.
    pub queue_depth: usize,
}

/// Validate worker/queue sizing and reject invalid zero values.
pub fn worker_queue_contract(
    workers: usize,
    queue_depth: usize,
) -> std::io::Result<WorkerQueueContract> {
    if workers == 0 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WEB_WORKERS must be >= 1",
        ));
    }
    if queue_depth == 0 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WEB_ACCEPT_QUEUE_DEPTH must be >= 1",
        ));
    }
    Ok(WorkerQueueContract {
        workers,
        queue_depth,
    })
}

/// Deterministic accept-queue saturation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueBackpressureState {
    /// Queue has available capacity.
    Healthy,
    /// Queue is saturated.
    Saturated,
}

/// Classify queue occupancy against queue capacity.
pub fn classify_accept_queue_state(
    queued_connections: usize,
    queue_depth: usize,
) -> QueueBackpressureState {
    if queue_depth == 0 || queued_connections >= queue_depth {
        QueueBackpressureState::Saturated
    } else {
        QueueBackpressureState::Healthy
    }
}

/// Deterministic operator guidance for queue backpressure state.
pub fn queue_backpressure_guidance(state: QueueBackpressureState) -> &'static str {
    match state {
        QueueBackpressureState::Healthy => "accept queue healthy",
        QueueBackpressureState::Saturated => {
            "accept queue saturated; allow workers to drain before retrying"
        }
    }
}

/// Deterministic storage recovery summary for startup diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRecoveryDiagnostic {
    /// Whether persistence is durable (`path`-backed).
    pub durable: bool,
    /// Persistence artifact path when durable mode is active.
    pub persistence_path: Option<String>,
    /// Number of recovered datasets available after open/restart.
    pub recovered_records: usize,
}

/// Build a recovery summary from an opened storage instance.
pub fn storage_recovery_diagnostic(
    storage: &Storage,
) -> std::io::Result<StorageRecoveryDiagnostic> {
    let recovered_records = storage
        .datasets()
        .map_err(|err| IoError::other(format!("failed to read recovered datasets: {err}")))?
        .len();
    Ok(StorageRecoveryDiagnostic {
        durable: storage.persistence_path().is_some(),
        persistence_path: storage
            .persistence_path()
            .map(|path| path.display().to_string()),
        recovered_records,
    })
}

// ---------------------------------------------------------------------------
// Types and functions moved from main.rs for integration-test visibility
// ---------------------------------------------------------------------------

/// Web server log verbosity levels.
#[derive(Debug, Clone, Copy)]
pub enum WebLogLevel {
    /// Suppress all log output.
    Off,
    /// Log only errors.
    Error,
    /// Log warnings and above.
    Warn,
    /// Log informational messages and above.
    Info,
    /// Log debug-level messages and above.
    Debug,
    /// Log everything including trace-level messages.
    Trace,
}

impl WebLogLevel {
    /// Parse a log level string into a [`WebLogLevel`].
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "error" => Some(Self::Error),
            "warn" | "warning" => Some(Self::Warn),
            "info" => Some(Self::Info),
            "debug" => Some(Self::Debug),
            "trace" => Some(Self::Trace),
            _ => None,
        }
    }

    /// Return whether a message at `level` should be emitted given the current threshold.
    pub fn should_log(&self, level: Self) -> bool {
        (*self as u8) >= (level as u8)
    }
}

impl From<WebLogLevel> for u8 {
    fn from(value: WebLogLevel) -> Self {
        match value {
            WebLogLevel::Off => 0,
            WebLogLevel::Error => 1,
            WebLogLevel::Warn => 2,
            WebLogLevel::Info => 3,
            WebLogLevel::Debug => 4,
            WebLogLevel::Trace => 5,
        }
    }
}

/// Web authentication mode for the DICOMweb server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebAuthMode {
    /// Allow all requests without authentication.
    AllowAll,
    /// Deny all requests (fail-closed default).
    DenyAll,
}

impl WebAuthMode {
    /// Parse an optional raw string into a [`WebAuthMode`].
    pub fn parse(raw: Option<&str>) -> std::io::Result<Self> {
        match raw.unwrap_or("deny_all") {
            "allow_all" => Ok(Self::AllowAll),
            "deny_all" => Ok(Self::DenyAll),
            _ => Err(web_env_parse_error(
                "DICOM_WEB_AUTH_MODE",
                raw.unwrap_or(""),
                "supported values are allow_all | deny_all",
            )),
        }
    }

    /// Return the string label for this auth mode.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllowAll => "allow_all",
            Self::DenyAll => "deny_all",
        }
    }
}

/// Classification of an incoming DICOMweb request by service class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DicomWebRouteClass {
    /// QIDO-RS query (search for studies/series/instances).
    Qido,
    /// WADO-RS retrieve (read instance/series/study data).
    Wado,
    /// STOW-RS store (upload DICOM objects).
    Stow,
    /// DELETE operation.
    Delete,
    /// Any other route (health checks, unknown paths).
    Other,
}

impl DicomWebRouteClass {
    /// Return the metric label for this route class.
    pub fn as_metric_label(&self) -> &'static str {
        match self {
            Self::Qido => "qido",
            Self::Wado => "wado",
            Self::Stow => "stow",
            Self::Delete => "delete",
            Self::Other => "other",
        }
    }
}

/// Latency budget thresholds for a single DICOMweb route class.
#[derive(Clone, Copy, Debug)]
pub struct RouteLatencyBudget {
    /// Maximum allowed p95 latency in milliseconds.
    pub p95_ms: u64,
    /// Maximum allowed p99 latency in milliseconds.
    pub p99_ms: u64,
}

/// Combined latency budgets for all DICOMweb route classes.
#[derive(Clone, Copy, Debug)]
pub struct RoutePerformanceBudgets {
    /// QIDO-RS latency budget.
    pub qido: RouteLatencyBudget,
    /// WADO-RS latency budget.
    pub wado: RouteLatencyBudget,
    /// STOW-RS latency budget.
    pub stow: RouteLatencyBudget,
}

impl RoutePerformanceBudgets {
    /// Read route performance budgets from environment variables.
    pub fn from_env() -> std::io::Result<Self> {
        Ok(Self {
            qido: parse_route_latency_budget(
                "QIDO",
                DEFAULT_WEB_QIDO_P95_LATENCY_MS,
                DEFAULT_WEB_QIDO_P99_LATENCY_MS,
            )?,
            wado: parse_route_latency_budget(
                "WADO",
                DEFAULT_WEB_WADO_P95_LATENCY_MS,
                DEFAULT_WEB_WADO_P99_LATENCY_MS,
            )?,
            stow: parse_route_latency_budget(
                "STOW",
                DEFAULT_WEB_STOW_P95_LATENCY_MS,
                DEFAULT_WEB_STOW_P99_LATENCY_MS,
            )?,
        })
    }

    /// Return the latency budget for the given route class, if applicable.
    pub fn for_route(&self, route: DicomWebRouteClass) -> Option<RouteLatencyBudget> {
        match route {
            DicomWebRouteClass::Qido => Some(self.qido),
            DicomWebRouteClass::Wado => Some(self.wado),
            DicomWebRouteClass::Stow => Some(self.stow),
            DicomWebRouteClass::Delete => Some(self.stow),
            DicomWebRouteClass::Other => None,
        }
    }
}

/// Sliding window of latency samples for percentile computation.
#[derive(Debug)]
pub struct RouteLatencyWindow {
    samples: VecDeque<u128>,
}

impl RouteLatencyWindow {
    /// Create a new empty window.
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
        }
    }

    /// Return the number of recorded samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Record a latency sample, evicting the oldest when the limit is exceeded.
    pub fn record_sample(&mut self, latency_ms: u128, sample_limit: usize) {
        while self.samples.len() >= sample_limit {
            let _ = self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
    }

    /// Compute the p95 and p99 latencies from the current sample window.
    pub fn p95_p99_ms(&self) -> Option<(u128, u128)> {
        if self.samples.is_empty() {
            return None;
        }
        let mut ordered = self.samples.iter().copied().collect::<Vec<_>>();
        ordered.sort_unstable();
        let p95 = ordered[(ordered.len() - 1) * 95 / 100];
        let p99 = ordered[(ordered.len() - 1) * 99 / 100];
        Some((p95, p99))
    }
}

/// Multi-route performance tracker that records latencies and detects budget violations.
#[derive(Debug)]
pub struct RoutePerformanceTracker {
    sample_limit: usize,
    qido: RouteLatencyWindow,
    wado: RouteLatencyWindow,
    stow: RouteLatencyWindow,
}

/// A detected route latency budget violation.
#[derive(Debug)]
pub struct RouteLatencyViolation {
    /// The route class that violated its budget.
    pub route: DicomWebRouteClass,
    /// Number of samples in the window at evaluation time.
    pub sample_count: usize,
    /// The sample latency that triggered evaluation.
    pub sample_ms: u128,
    /// Computed p95 latency.
    pub p95_ms: u128,
    /// Computed p99 latency.
    pub p99_ms: u128,
    /// Budgeted p95 threshold.
    pub budget_p95_ms: u64,
    /// Budgeted p99 threshold.
    pub budget_p99_ms: u64,
}

impl RoutePerformanceTracker {
    /// Create a new tracker with the given per-route sample limit.
    pub fn new(sample_limit: usize) -> Self {
        Self {
            sample_limit,
            qido: RouteLatencyWindow::new(),
            wado: RouteLatencyWindow::new(),
            stow: RouteLatencyWindow::new(),
        }
    }

    /// Record a sample and evaluate whether the budget is violated.
    pub fn record_and_evaluate(
        &mut self,
        route: DicomWebRouteClass,
        latency_ms: u128,
        budgets: &RoutePerformanceBudgets,
    ) -> Option<RouteLatencyViolation> {
        let budget = budgets.for_route(route)?;
        let window = match route {
            DicomWebRouteClass::Qido => &mut self.qido,
            DicomWebRouteClass::Wado => &mut self.wado,
            DicomWebRouteClass::Stow => &mut self.stow,
            DicomWebRouteClass::Delete => &mut self.stow,
            DicomWebRouteClass::Other => return None,
        };
        window.record_sample(latency_ms, self.sample_limit);
        let (p95_ms, p99_ms) = window.p95_p99_ms()?;
        if p95_ms <= u128::from(budget.p95_ms) && p99_ms <= u128::from(budget.p99_ms) {
            return None;
        }
        Some(RouteLatencyViolation {
            route,
            sample_count: window.len(),
            sample_ms: latency_ms,
            p95_ms,
            p99_ms,
            budget_p95_ms: budget.p95_ms,
            budget_p99_ms: budget.p99_ms,
        })
    }
}

/// Cloud/provider profile determining which DICOMweb operations are supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProfile {
    /// Microsoft Azure Health Data Services.
    Azure,
    /// AWS HealthImaging.
    AwsHealthImaging,
    /// Orthanc open-source DICOM server.
    Orthanc,
    /// dcm4chee open-source DICOM server.
    Dcm4chee,
    /// Generic provider (all operations supported).
    Generic,
}

impl ProviderProfile {
    /// Parse a provider profile string.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "azure" => Some(Self::Azure),
            "aws_health_imaging" | "aws" | "ahi" => Some(Self::AwsHealthImaging),
            "orthanc" => Some(Self::Orthanc),
            "dcm4chee" => Some(Self::Dcm4chee),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    /// Return the canonical label for this provider profile.
    pub fn label(self) -> &'static str {
        match self {
            Self::Azure => "azure",
            Self::AwsHealthImaging => "aws_health_imaging",
            Self::Orthanc => "orthanc",
            Self::Dcm4chee => "dcm4chee",
            Self::Generic => "generic",
        }
    }

    /// Return the capability map for this provider profile.
    pub fn capabilities(self) -> ProviderCapabilityMap {
        match self {
            Self::Azure => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: false,
                workitem: true,
                metadata: true,
                frame: true,
                rendered: true,
                bulkdata: true,
                wado_uri: false,
            },
            Self::AwsHealthImaging => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: false,
                workitem: false,
                metadata: false,
                frame: false,
                rendered: false,
                bulkdata: false,
                wado_uri: false,
            },
            Self::Orthanc => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: false,
                metadata: true,
                frame: true,
                rendered: true,
                bulkdata: false,
                wado_uri: true,
            },
            Self::Dcm4chee => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: false,
                metadata: true,
                frame: true,
                rendered: true,
                bulkdata: true,
                wado_uri: true,
            },
            Self::Generic => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: true,
                metadata: true,
                frame: true,
                rendered: true,
                bulkdata: true,
                wado_uri: true,
            },
        }
    }
}

/// Capability flags for a specific cloud/provider profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilityMap {
    /// Supports Retrieve (WADO-RS).
    pub retrieve: bool,
    /// Supports Search (QIDO-RS).
    pub search: bool,
    /// Supports Store (STOW-RS).
    pub store: bool,
    /// Supports Delete.
    pub delete: bool,
    /// Supports Workitem operations.
    pub workitem: bool,
    /// Supports metadata retrieval.
    pub metadata: bool,
    /// Supports frame-level retrieval.
    pub frame: bool,
    /// Supports rendered retrieval.
    pub rendered: bool,
    /// Supports bulkdata retrieval.
    pub bulkdata: bool,
    /// Supports WADO-URI legacy access.
    pub wado_uri: bool,
}

impl ProviderCapabilityMap {
    /// Check whether a given route is unsupported, returning the operation name if blocked.
    pub fn supports_web_route(self, method: &HttpMethod, path: &str) -> Option<&'static str> {
        match classify_provider_operation(method, path) {
            ProviderOperation::Search if !self.search => Some("search"),
            ProviderOperation::Retrieve if !self.retrieve => Some("retrieve"),
            ProviderOperation::Metadata if !self.metadata => Some("metadata"),
            ProviderOperation::Frame if !self.frame => Some("frame"),
            ProviderOperation::Rendered if !self.rendered => Some("rendered"),
            ProviderOperation::Bulkdata if !self.bulkdata => Some("bulkdata"),
            ProviderOperation::Store if !self.store => Some("store"),
            ProviderOperation::Delete if !self.delete => Some("delete"),
            ProviderOperation::WadoUri if !self.wado_uri => Some("wado_uri"),
            _ => None,
        }
    }
}

/// Fine-grained provider operation classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOperation {
    /// QIDO-RS search.
    Search,
    /// WADO-RS retrieve.
    Retrieve,
    /// WADO-RS metadata.
    Metadata,
    /// WADO-RS frame retrieval.
    Frame,
    /// WADO-RS rendered retrieval.
    Rendered,
    /// WADO-RS bulkdata retrieval.
    Bulkdata,
    /// STOW-RS store.
    Store,
    /// DELETE operation.
    Delete,
    /// WADO-URI legacy access.
    WadoUri,
    /// Any other operation.
    Other,
}

/// Classify the fine-grained provider operation from method and path.
pub fn classify_provider_operation(method: &HttpMethod, path: &str) -> ProviderOperation {
    if matches!(method, HttpMethod::Get | HttpMethod::Head) && path == "/wado" {
        return ProviderOperation::WadoUri;
    }
    if is_delete_write(method, path) {
        return ProviderOperation::Delete;
    }
    if is_stow_write(method, path) {
        return ProviderOperation::Store;
    }
    if is_wado_read(method, path) {
        if path.contains("/bulkdata") {
            return ProviderOperation::Bulkdata;
        }
        if path.contains("/rendered") {
            return ProviderOperation::Rendered;
        }
        if path.contains("/frames/") {
            return ProviderOperation::Frame;
        }
        if path.ends_with("/metadata") {
            return ProviderOperation::Metadata;
        }
        return ProviderOperation::Retrieve;
    }
    if is_qido_read(method, path) {
        return ProviderOperation::Search;
    }
    ProviderOperation::Other
}

/// Reason an interop feature request is blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteropBlockReason {
    /// The feature is administratively disabled.
    FeatureDisabled(&'static str),
    /// The provider profile does not support the requested operation.
    ProviderUnsupported {
        /// The provider profile that lacks support.
        profile: ProviderProfile,
        /// The unsupported operation name.
        operation: &'static str,
    },
}

/// Runtime policy for DICOMweb interoperability features.
#[derive(Debug, Clone, Copy)]
pub struct InteropRuntimePolicy {
    /// Whether QIDO-RS (search) is enabled.
    pub qido: bool,
    /// Whether WADO-RS (retrieve) is enabled.
    pub wado: bool,
    /// Whether STOW-RS (store) is enabled.
    pub stow: bool,
    /// Whether DELETE is enabled.
    pub delete: bool,
    /// Active provider profile.
    pub provider_profile: ProviderProfile,
}

impl InteropRuntimePolicy {
    /// Read the interop runtime policy from environment variables.
    pub fn from_env() -> std::io::Result<Self> {
        let provider_profile = match parse_optional_string(
            DICOM_WEB_SERVICE_NAME,
            "DICOM_WEB_PROVIDER_PROFILE",
        )? {
            Some(raw) => ProviderProfile::parse(raw.trim()).ok_or_else(|| {
                web_env_parse_error(
                    "DICOM_WEB_PROVIDER_PROFILE",
                    raw.trim(),
                    "supported values are azure | aws_health_imaging | orthanc | dcm4chee | generic",
                )
            })?,
            None => ProviderProfile::Generic,
        };
        Ok(Self {
            qido: parse_bool(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_ENABLE_QIDO", true)?,
            wado: parse_bool(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_ENABLE_WADO", true)?,
            stow: parse_bool(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_ENABLE_STOW", true)?,
            delete: parse_bool(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_ENABLE_DELETE", false)?,
            provider_profile,
        })
    }
}

/// Classify an incoming DICOMweb request by its service class.
pub fn classify_dicomweb_route(method: &HttpMethod, path: &str) -> DicomWebRouteClass {
    if is_wado_read(method, path) {
        DicomWebRouteClass::Wado
    } else if is_delete_write(method, path) {
        DicomWebRouteClass::Delete
    } else if is_qido_read(method, path) {
        DicomWebRouteClass::Qido
    } else if is_stow_write(method, path) {
        DicomWebRouteClass::Stow
    } else {
        DicomWebRouteClass::Other
    }
}

/// Determine whether an interop feature request should be blocked.
pub fn blocked_interop_feature(
    method: &HttpMethod,
    path: &str,
    policy: &InteropRuntimePolicy,
) -> Option<InteropBlockReason> {
    match classify_dicomweb_route(method, path) {
        DicomWebRouteClass::Wado if !policy.wado => {
            return Some(InteropBlockReason::FeatureDisabled("wado"));
        }
        DicomWebRouteClass::Qido if !policy.qido => {
            return Some(InteropBlockReason::FeatureDisabled("qido"));
        }
        DicomWebRouteClass::Stow if !policy.stow => {
            return Some(InteropBlockReason::FeatureDisabled("stow"));
        }
        DicomWebRouteClass::Delete if !policy.delete => {
            return Some(InteropBlockReason::FeatureDisabled("delete"));
        }
        _ => {}
    }

    let capabilities = policy.provider_profile.capabilities();
    capabilities
        .supports_web_route(method, path)
        .map(|operation| InteropBlockReason::ProviderUnsupported {
            profile: policy.provider_profile,
            operation,
        })
}

/// Resolve the worker/queue contract from environment variables.
pub fn resolve_worker_queue_contract(worker_default: usize) -> std::io::Result<WorkerQueueContract> {
    let worker_count_raw = parse_env_usize("DICOM_WEB_WORKERS", worker_default)?;
    let queue_depth_raw = parse_env_usize(
        "DICOM_WEB_ACCEPT_QUEUE_DEPTH",
        worker_count_raw.saturating_mul(8),
    )?;
    worker_queue_contract(worker_count_raw, queue_depth_raw)
}

/// Parse a `u64` environment variable with a minimum bound of 1.
pub fn parse_env_u64(name: &str, default: u64) -> std::io::Result<u64> {
    parse_u64(
        DICOM_WEB_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

/// Parse a `usize` environment variable with a minimum bound of 1.
pub fn parse_env_usize(name: &str, default: usize) -> std::io::Result<usize> {
    parse_usize(
        DICOM_WEB_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

/// Parse the log level from an environment variable.
pub fn parse_log_level(name: &str) -> std::io::Result<WebLogLevel> {
    match parse_optional_string(DICOM_WEB_SERVICE_NAME, name)? {
        Some(raw) => WebLogLevel::parse(&raw).ok_or_else(|| {
            web_env_parse_error(name, raw.trim(), "must be off/error/warn/info/debug/trace")
        }),
        None => Ok(WebLogLevel::Info),
    }
}

/// Extract the `Content-Length` value from an HTTP head section.
pub fn content_length(head: &str) -> Option<usize> {
    for line in head.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value.trim().parse::<usize>().ok();
            }
        }
    }
    None
}

/// Build a JSON response body for STOW-RS ingestion outcomes.
pub fn stow_json_body(outcomes: &[IngestOutcome]) -> String {
    let mut json = String::from("{\"outcomes\":[");
    for (index, outcome) in outcomes.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        match outcome {
            IngestOutcome::Inserted { hash } => {
                json.push_str("{\"outcome\":\"inserted\",\"hash\":\"");
                json.push_str(&escape_json(hash));
                json.push_str("\"}");
            }
            IngestOutcome::Duplicate { hash } => {
                json.push_str("{\"outcome\":\"duplicate\",\"hash\":\"");
                json.push_str(&escape_json(hash));
                json.push_str("\"}");
            }
        }
    }
    json.push_str("]}");
    json
}

/// Map a DICOM error to an HTTP status code and reason phrase.
pub fn status_for_error(error: &Error) -> (u16, &'static str) {
    dicomweb_status_for_error(error)
}

/// Redact potentially sensitive tokens from a diagnostic message.
pub fn redact_diagnostic_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Parse a boolean environment variable with a default value.
pub fn parse_bool_env_with_default(name: &str, default: bool) -> std::io::Result<bool> {
    let raw = parse_optional_string(DICOM_WEB_SERVICE_NAME, name)?;
    let Some(raw) = raw else {
        return Ok(default);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(web_env_parse_error(name, raw.trim(), "must be true/false")),
    }
}

/// Escape a string for safe embedding in a JSON value.
pub fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

/// Escape a string for safe embedding in a JSON key or value (backslash and double-quote only).
pub fn json_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------
// Internal helpers (not pub)
// ---------------------------------------------------------------------------

fn web_env_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!("service={DICOM_WEB_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}"),
    )
}

fn parse_route_latency_budget(
    route: &str,
    p95_default: u64,
    p99_default: u64,
) -> std::io::Result<RouteLatencyBudget> {
    Ok(RouteLatencyBudget {
        p95_ms: parse_u64(
            DICOM_WEB_SERVICE_NAME,
            &format!("DICOM_WEB_{route}_P95_LATENCY_MS"),
            p95_default,
            NumericBounds::at_least(1),
        )?,
        p99_ms: parse_u64(
            DICOM_WEB_SERVICE_NAME,
            &format!("DICOM_WEB_{route}_P99_LATENCY_MS"),
            p99_default,
            NumericBounds::at_least(1),
        )?,
    })
}

fn is_wado_read(method: &HttpMethod, path: &str) -> bool {
    matches!(method, HttpMethod::Get | HttpMethod::Head)
        && (path == "/wado"
            || is_wado_study_or_series_retrieve_path(path)
            || (path.starts_with("/studies/")
                && path.contains("/series/")
                && path.contains("/instances/")))
}

fn is_wado_study_or_series_retrieve_path(path: &str) -> bool {
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    matches!(
        segments.as_slice(),
        ["studies", _] | ["studies", _, "series", _]
    )
}

fn is_qido_read(method: &HttpMethod, path: &str) -> bool {
    matches!(method, HttpMethod::Get | HttpMethod::Head)
        && (path == "/studies" || path.starts_with("/studies/"))
}

fn is_stow_write(method: &HttpMethod, path: &str) -> bool {
    if !matches!(method, HttpMethod::Post) {
        return false;
    }
    if path == "/studies" {
        return true;
    }
    let Some(uid) = path.strip_prefix("/studies/") else {
        return false;
    };
    !uid.contains('/')
}

fn is_delete_write(method: &HttpMethod, path: &str) -> bool {
    if !matches!(method, HttpMethod::Delete) {
        return false;
    }
    path.starts_with("/studies/") && path.len() > "/studies/".len()
}

fn redact_token(token: &str) -> String {
    let (prefix, candidate) = match token.split_once('=') {
        Some((left, right)) if !left.is_empty() => (Some(left), right),
        _ => (None, token),
    };
    if prefix.is_some_and(looks_like_sensitive_key) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_PII]"),
            None => "[REDACTED_PII]".to_string(),
        };
    }
    let trimmed = candidate.trim_matches(|ch: char| ",;()[]{}\"'".contains(ch));
    if looks_like_email(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_EMAIL]"),
            None => "[REDACTED_EMAIL]".to_string(),
        };
    }
    if looks_like_uid(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_UID]"),
            None => "[REDACTED_UID]".to_string(),
        };
    }
    if looks_like_path(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_PATH]"),
            None => "[REDACTED_PATH]".to_string(),
        };
    }
    if looks_like_host(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_HOST]"),
            None => "[REDACTED_HOST]".to_string(),
        };
    }
    token.to_string()
}

fn looks_like_uid(value: &str) -> bool {
    if value.len() < 7 {
        return false;
    }
    let mut part_count = 0usize;
    for part in value.split('.') {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        part_count = part_count.saturating_add(1);
    }
    part_count >= 3
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

fn looks_like_host(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    if host.is_empty() || port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn looks_like_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn looks_like_sensitive_key(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "patient"
            | "patient_id"
            | "patient_name"
            | "mrn"
            | "email"
            | "ssn"
            | "dob"
            | "birth_date"
            | "phone"
            | "person_name"
    )
}
