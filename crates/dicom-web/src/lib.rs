#![deny(missing_docs)]

//! DICOMweb request parsing/routing and in-process service runtime execution.

pub mod cors;
mod auth_middleware;
mod qido;
mod router;
mod stow;
mod wado;

#[cfg(feature = "qido")]
pub use qido::*;
pub use router::*;
#[cfg(feature = "stow")]
pub use stow::*;

use dicom_audit::AuditEvent;
use dicom_auth::{AllowAll, AuthDenyReason};
use dicom_auth::{Authorizer, DenyAll};
use dicom_core::{validate_uid_strict, Tag};
use dicom_core::{Error, ErrorKind, Limits, Result};
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_storage::Storage;

use std::fmt;
use std::sync::Arc;

// --- Shared tag constants ---

/// DICOM tag constant.
pub const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
/// DICOM tag constant.
pub const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
/// DICOM tag constant.
pub const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
#[cfg(any(feature = "wado", feature = "stow"))]
const TAG_TRANSFER_SYNTAX_UID: Tag = Tag(0x0002, 0x0010);

// --- Shared constants ---

/// Stable error code for DICOMweb not-found responses.
pub const DICOM_WEB_NOT_FOUND_CODE: &str = "DVF.WEB.NOT_FOUND";

// --- Core types ---

/// Supported HTTP methods for DICOMweb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// HTTP GET.
    Get,
    /// HTTP HEAD.
    Head,
    /// HTTP POST.
    Post,
    /// HTTP DELETE.
    Delete,
}

/// Transport security status for a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportSecurity {
    /// Request arrived over TLS (or equivalent secure transport).
    Tls,
    /// Request arrived over an insecure transport.
    Insecure,
}

/// TLS enforcement policy for DICOMweb requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsPolicy {
    /// Require TLS; insecure transports must fail closed.
    RequireTls,
    /// Allow insecure transports (explicit opt-in).
    AllowInsecure,
}

/// Throttling decision for a DICOMweb request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleDecision {
    /// Allow the request to proceed.
    Allow,
    /// Reject the request with limit metadata.
    Reject {
        /// Limit name for structured errors.
        limit_name: &'static str,
        /// Observed value at the time of rejection.
        observed: u64,
        /// Allowed value for the limit.
        allowed: u64,
    },
}

/// Policy bundle for DICOMweb request validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebPolicy {
    /// TLS enforcement policy.
    pub tls: TlsPolicy,
    /// Throttling decision for this request.
    pub throttle: ThrottleDecision,
    /// Whether DICOMweb delete routes are enabled.
    pub delete_enabled: bool,
}

impl WebPolicy {
    /// Construct a policy with explicit TLS and throttling decisions.
    pub fn new(tls: TlsPolicy, throttle: ThrottleDecision) -> Self {
        Self {
            tls,
            throttle,
            delete_enabled: false,
        }
    }

    /// Return a policy with delete routes explicitly enabled/disabled.
    pub fn with_delete_enabled(mut self, enabled: bool) -> Self {
        self.delete_enabled = enabled;
        self
    }
}

/// Audit callback for DICOMweb operations.
///
/// # S13-T7 — Read-Only Trait Object
///
/// `Fn(AuditEvent) -> Result<()>` takes `&self`, so `Arc<AuditCallback>`
/// is safe for concurrent read-only invocation.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// Authorization and audit configuration for DICOMweb.
///
/// # S13-T7 — Read-Only `Arc<dyn Authorizer>`
///
/// The `Authorizer` trait's `authorize` method takes `&self`, so
/// `Arc<dyn Authorizer + Send + Sync>` is safe to share without
/// additional synchronization.
#[derive(Clone)]
pub struct WebAuthConfig {
    /// Authorization policy.
    pub authorizer: Arc<dyn Authorizer + Send + Sync>,
    /// Optional audit callback.
    pub audit: Option<AuditCallback>,
}

impl WebAuthConfig {
    /// Build a config that allows all requests.
    pub fn allow_all() -> Self {
        Self {
            authorizer: Arc::new(AllowAll),
            audit: None,
        }
    }

    /// Build a config that denies all requests (fail closed).
    pub fn deny_all() -> Self {
        Self {
            authorizer: Arc::new(DenyAll::new(AuthDenyReason::Policy)),
            audit: None,
        }
    }
}

impl Default for WebAuthConfig {
    /// Default is deny-all (fail closed) per S13-T5.
    fn default() -> Self {
        Self::deny_all()
    }
}

impl fmt::Debug for WebAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebAuthConfig")
            .field("authorizer", &"Authorizer")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

/// Runtime configuration for DICOMweb service execution.
#[derive(Debug, Clone)]
pub struct DicomWebServiceConfig {
    /// Input limits used by HTTP parsing and operation handlers.
    pub limits: Limits,
    /// TLS/throttling policy applied before request routing.
    pub policy: WebPolicy,
    /// Authorization and audit policy.
    pub auth: WebAuthConfig,
    /// CORS configuration for cross-origin requests.
    pub cors: cors::CorsConfig,
}

impl Default for DicomWebServiceConfig {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            policy: WebPolicy::new(TlsPolicy::RequireTls, ThrottleDecision::Allow),
            auth: WebAuthConfig::deny_all(),
            cors: cors::default_dicomweb_cors(),
        }
    }
}

/// Runtime response for routed DICOMweb operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DicomWebResponse {
    /// QIDO query result matches.
    #[cfg(feature = "qido")]
    Qido {
        /// Deterministic query matches.
        matches: Vec<dicom_query::QueryMatch>,
    },
    /// WADO instance retrieve bytes.
    #[cfg(feature = "wado")]
    WadoInstance {
        /// Retrieved DICOM P10 bytes.
        bytes: Vec<u8>,
    },
    /// WADO multi-instance retrieve payload.
    #[cfg(feature = "wado")]
    WadoMultipart {
        /// Negotiated multipart media type including boundary.
        media_type: String,
        /// Multipart payload bytes.
        bytes: Vec<u8>,
    },
    /// WADO metadata payload (DICOM JSON-like deterministic rendering).
    #[cfg(feature = "wado")]
    WadoMetadata {
        /// Retrieved metadata payload bytes.
        bytes: Vec<u8>,
    },
    /// WADO rendered retrieve payload.
    #[cfg(feature = "wado")]
    WadoRendered {
        /// Negotiated response media type.
        media_type: String,
        /// Retrieved rendered payload bytes.
        bytes: Vec<u8>,
    },
    /// WADO bulkdata retrieve payload.
    #[cfg(feature = "wado")]
    WadoBulkData {
        /// Negotiated response media type.
        media_type: String,
        /// Retrieved bulkdata payload bytes.
        bytes: Vec<u8>,
    },
    /// STOW ingestion outcome.
    #[cfg(feature = "stow")]
    Stow {
        /// Deterministic storage ingest outcomes in request part order.
        outcomes: Vec<dicom_storage::IngestOutcome>,
    },
    /// Delete route outcome.
    Delete {
        /// True when soft-delete tombstone was applied.
        tombstoned: bool,
    },
}

/// Query parameter in a DICOMweb request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParam {
    /// Parameter key.
    pub key: String,
    /// Parameter value (may be empty).
    pub value: String,
}

/// Header name/value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Header name (lowercase).
    pub name: String,
    /// Header value.
    pub value: String,
}

/// Parsed HTTP request for DICOMweb routing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebRequest {
    /// HTTP method.
    pub method: HttpMethod,
    /// Transport security status.
    pub transport: TransportSecurity,
    /// Request path (no query string).
    pub path: String,
    /// Query parameters in order.
    pub query: Vec<QueryParam>,
    /// Request headers.
    pub headers: Vec<Header>,
    /// Request body bytes.
    pub body: Vec<u8>,
}

/// Supported STOW-RS content types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DicomWebContentType {
    /// `application/dicom` payload.
    ApplicationDicom,
    /// `application/dicom+xml` payload.
    ApplicationDicomXml,
    /// `application/dicom+json` payload.
    ApplicationDicomJson,
    /// `multipart/related` with a boundary delimiter.
    MultipartRelated {
        /// MIME boundary string.
        boundary: String,
    },
}

/// Supported DICOMweb requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DicomWebRequest {
    /// QIDO-RS query for studies.
    #[cfg(feature = "qido")]
    QidoStudies {
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// QIDO-RS query for series.
    #[cfg(feature = "qido")]
    QidoAllSeries {
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// QIDO-RS query for instances.
    #[cfg(feature = "qido")]
    QidoAllInstances {
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// QIDO-RS query for series in a study.
    #[cfg(feature = "qido")]
    QidoSeries {
        /// Study Instance UID.
        study_uid: String,
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// QIDO-RS query for instances in a study.
    #[cfg(feature = "qido")]
    QidoStudyInstances {
        /// Study Instance UID.
        study_uid: String,
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// QIDO-RS query for instances in a series.
    #[cfg(feature = "qido")]
    QidoInstances {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// Query parameters.
        params: Vec<QueryParam>,
    },
    /// WADO-RS retrieve of a single instance.
    #[cfg(feature = "wado")]
    WadoInstance {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// SOP Instance UID.
        instance_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
        /// Optional frame number for frame-level retrieve compatibility.
        frame_number: Option<u32>,
    },
    /// WADO-RS retrieve all instances for a study.
    #[cfg(feature = "wado")]
    WadoStudyRetrieve {
        /// Study Instance UID.
        study_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
    },
    /// WADO-RS retrieve all instances for a series.
    #[cfg(feature = "wado")]
    WadoSeriesRetrieve {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
    },
    /// WADO-RS retrieve study metadata.
    #[cfg(feature = "wado")]
    WadoStudyMetadata {
        /// Study Instance UID.
        study_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
    },
    /// WADO-RS retrieve series metadata.
    #[cfg(feature = "wado")]
    WadoSeriesMetadata {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
    },
    /// WADO-RS retrieve instance metadata.
    #[cfg(feature = "wado")]
    WadoInstanceMetadata {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// SOP Instance UID.
        instance_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
    },
    /// WADO-RS retrieve rendered payload.
    #[cfg(feature = "wado")]
    WadoRendered {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// SOP Instance UID.
        instance_uid: String,
        /// Optional frame number for frame-level rendered retrieve.
        frame_number: Option<u32>,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
        /// Negotiated rendered media type.
        media_type: String,
    },
    /// WADO-RS retrieve bulkdata payload.
    #[cfg(feature = "wado")]
    WadoBulkData {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// SOP Instance UID.
        instance_uid: String,
        /// Optional transfer syntax request.
        transfer_syntax_uid: Option<String>,
        /// Negotiated bulkdata media type.
        media_type: String,
    },
    /// STOW-RS store request.
    #[cfg(feature = "stow")]
    Stow {
        /// Optional Study Instance UID in the path.
        study_uid: Option<String>,
        /// Content type descriptor.
        content_type: DicomWebContentType,
        /// Request body bytes.
        body: Vec<u8>,
    },
    /// Soft/hard delete study entity (feature-gated by runtime policy).
    DeleteStudy {
        /// Study Instance UID.
        study_uid: String,
        /// True when hard-delete mode is requested.
        hard_delete: bool,
    },
    /// Soft/hard delete series entity (feature-gated by runtime policy).
    DeleteSeries {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// True when hard-delete mode is requested.
        hard_delete: bool,
    },
    /// Soft/hard delete instance entity (feature-gated by runtime policy).
    DeleteInstance {
        /// Study Instance UID.
        study_uid: String,
        /// Series Instance UID.
        series_uid: String,
        /// SOP Instance UID.
        instance_uid: String,
        /// True when hard-delete mode is requested.
        hard_delete: bool,
    },
}

// --- Service interface types ---

/// Service interfaces visible to operator-facing interoperability capability views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceInterface {
    /// DIMSE Query/Retrieve and storage services.
    Dimse,
    /// DICOMweb QIDO/WADO/STOW services.
    Dicomweb,
    /// Modality Worklist services.
    Mwl,
    /// Modality Performed Procedure Step services.
    Mpps,
}

/// Operator-visible service availability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAvailability {
    /// Service is build-supported and runtime-enabled.
    Active,
    /// Service is build-supported but disabled by runtime policy.
    DisabledByPolicy,
    /// Service is not available in the active build/runtime.
    UnsupportedBuild,
}

/// A single service-interface capability row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceInterfaceCapability {
    /// Service interface identifier.
    pub interface: ServiceInterface,
    /// Availability state for this interface.
    pub availability: ServiceAvailability,
    /// Deterministic reason string for the current state.
    pub reason: &'static str,
}

/// Runtime toggles applied to service interfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceInterfacePolicy {
    /// Runtime toggle for DIMSE interface visibility.
    pub dimse_enabled: bool,
    /// Runtime toggle for DICOMweb interface visibility.
    pub dicomweb_enabled: bool,
    /// Runtime toggle for MWL interface visibility.
    pub mwl_enabled: bool,
    /// Runtime toggle for MPPS interface visibility.
    pub mpps_enabled: bool,
}

impl Default for ServiceInterfacePolicy {
    fn default() -> Self {
        Self {
            dimse_enabled: true,
            dicomweb_enabled: true,
            mwl_enabled: true,
            mpps_enabled: true,
        }
    }
}

/// Build a deterministic service-interface capability matrix for UI/operator display.
pub fn service_interface_capability_matrix(
    policy: ServiceInterfacePolicy,
) -> [ServiceInterfaceCapability; 4] {
    [
        service_capability(
            ServiceInterface::Dimse,
            false,
            policy.dimse_enabled,
            "DIMSE is not hosted by dicom-web runtime",
        ),
        service_capability(
            ServiceInterface::Dicomweb,
            cfg!(any(feature = "qido", feature = "wado", feature = "stow")),
            policy.dicomweb_enabled,
            "DICOMweb routes are not enabled in this build",
        ),
        service_capability(
            ServiceInterface::Mwl,
            false,
            policy.mwl_enabled,
            "MWL is not hosted by dicom-web runtime",
        ),
        service_capability(
            ServiceInterface::Mpps,
            false,
            policy.mpps_enabled,
            "MPPS is not hosted by dicom-web runtime",
        ),
    ]
}

fn service_capability(
    interface: ServiceInterface,
    build_supported: bool,
    runtime_enabled: bool,
    unsupported_reason: &'static str,
) -> ServiceInterfaceCapability {
    if !build_supported {
        return ServiceInterfaceCapability {
            interface,
            availability: ServiceAvailability::UnsupportedBuild,
            reason: unsupported_reason,
        };
    }
    if !runtime_enabled {
        return ServiceInterfaceCapability {
            interface,
            availability: ServiceAvailability::DisabledByPolicy,
            reason: "disabled by runtime policy",
        };
    }
    ServiceInterfaceCapability {
        interface,
        availability: ServiceAvailability::Active,
        reason: "active",
    }
}

// --- Context tuple types ---

/// Study/Series/Instance context tuple used for integrity checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextTuple {
    /// Study Instance UID.
    pub study_uid: Option<String>,
    /// Series Instance UID.
    pub series_uid: Option<String>,
    /// SOP Instance UID.
    pub instance_uid: Option<String>,
    /// Optional frame index for multi-frame references.
    pub frame_index: Option<u32>,
}

impl ContextTuple {
    /// Build an empty context tuple with no bound identifiers.
    pub fn empty() -> Self {
        Self {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            frame_index: None,
        }
    }
}

/// Validate that an observed context tuple matches an expected tuple.
///
/// Any field set in `expected` must match exactly in `observed`.
pub fn validate_context_tuple(expected: &ContextTuple, observed: &ContextTuple) -> Result<()> {
    validate_context_field(
        "study_uid",
        expected.study_uid.as_deref(),
        observed.study_uid.as_deref(),
    )?;
    validate_context_field(
        "series_uid",
        expected.series_uid.as_deref(),
        observed.series_uid.as_deref(),
    )?;
    validate_context_field(
        "instance_uid",
        expected.instance_uid.as_deref(),
        observed.instance_uid.as_deref(),
    )?;

    if let Some(expected_frame) = expected.frame_index {
        let observed_frame = observed
            .frame_index
            .ok_or_else(|| integrity_error("context tuple mismatch: frame_index missing"))?;
        if observed_frame != expected_frame {
            return Err(integrity_error(format!(
                "context tuple mismatch: frame_index expected {expected_frame}, observed {observed_frame}"
            )));
        }
    }
    Ok(())
}

/// Decision for a requested clinical context switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextSwitchDecision {
    /// Switch is allowed.
    Allow,
    /// Switch requires explicit operator confirmation.
    RequireConfirmation,
    /// Switch is blocked by mixed-patient safety policy.
    BlockMixedPatient,
}

/// Evaluate mixed-patient and unsaved-artifact safety constraints for context switches.
pub fn evaluate_context_switch(
    active_patient_id: Option<&str>,
    next_patient_id: Option<&str>,
    has_unsaved_artifacts: bool,
    operator_confirmed: bool,
) -> ContextSwitchDecision {
    if let (Some(active), Some(next)) = (active_patient_id, next_patient_id) {
        if active != next {
            return ContextSwitchDecision::BlockMixedPatient;
        }
    }
    if has_unsaved_artifacts && !operator_confirmed {
        return ContextSwitchDecision::RequireConfirmation;
    }
    ContextSwitchDecision::Allow
}

/// Cache freshness status for clinical context reload operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheFreshnessStatus {
    /// Cache reflects latest source revision.
    Fresh,
    /// Cache is stale relative to source revision.
    Stale,
    /// Freshness cannot be determined.
    Unknown,
}

/// Evaluate source/cache freshness from monotonic revision values.
pub fn evaluate_cache_freshness(
    source_revision_epoch_secs: Option<u64>,
    cache_revision_epoch_secs: Option<u64>,
) -> CacheFreshnessStatus {
    match (source_revision_epoch_secs, cache_revision_epoch_secs) {
        (Some(source), Some(cache)) if cache >= source => CacheFreshnessStatus::Fresh,
        (Some(_), Some(_)) => CacheFreshnessStatus::Stale,
        _ => CacheFreshnessStatus::Unknown,
    }
}

/// Timestamp rendering model with explicit timezone context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowTimestampDisplay {
    /// UTC timestamp string.
    pub timestamp_utc: String,
    /// Timezone offset in minutes.
    pub timezone_offset_minutes: i32,
    /// Operator-visible string with explicit offset.
    pub rendered: String,
}

/// Build workflow timestamp text with explicit timezone offset context.
pub fn render_workflow_timestamp(
    timestamp_utc: &str,
    timezone_offset_minutes: i32,
) -> Result<WorkflowTimestampDisplay> {
    if timestamp_utc.trim().is_empty() {
        return Err(decode_error("timestamp must not be empty"));
    }
    if timezone_offset_minutes.abs() > 24 * 60 {
        return Err(decode_error("timezone offset out of bounds"));
    }
    let sign = if timezone_offset_minutes < 0 {
        '-'
    } else {
        '+'
    };
    let minutes = timezone_offset_minutes.unsigned_abs();
    let hours = minutes / 60;
    let remainder = minutes % 60;
    let rendered = format!("{timestamp_utc} (UTC{sign}{hours:02}:{remainder:02})");
    Ok(WorkflowTimestampDisplay {
        timestamp_utc: timestamp_utc.to_string(),
        timezone_offset_minutes,
        rendered,
    })
}

/// Deterministic STOW preflight result for compatibility checks.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StowPreflight {
    /// Strictly validated Study Instance UID extracted from payload metadata.
    pub study_uid: String,
}

// --- Service struct ---

/// Runtime DICOMweb service integrating parser/routing with storage/query/auth/audit.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[derive(Debug, Clone)]
pub struct DicomWebService {
    config: DicomWebServiceConfig,
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
impl DicomWebService {
    /// Create a service instance with the provided configuration.
    pub fn new(config: DicomWebServiceConfig) -> Self {
        Self { config }
    }

    /// Return service configuration.
    pub fn config(&self) -> &DicomWebServiceConfig {
        &self.config
    }

    /// Parse, route, authorize, and execute a DICOMweb HTTP request.
    pub fn handle_http(
        &self,
        input: &[u8],
        transport: TransportSecurity,
        storage: &mut Storage,
    ) -> Result<DicomWebResponse> {
        let web_request = parse_http_request(input, &self.config.limits, transport)?;
        self.handle_request(web_request, storage)
    }

    /// Route, authorize, and execute a parsed web request.
    pub fn handle_request(
        &self,
        request: WebRequest,
        storage: &mut Storage,
    ) -> Result<DicomWebResponse> {
        let routed = self.route_request(request)?;
        self.execute_routed(&routed, storage)
    }

    /// Parse, route, and authorize a parsed web request without executing storage operations.
    pub fn route_request(&self, request: WebRequest) -> Result<DicomWebRequest> {
        let subject = auth_middleware::derive_auth_subject(&request);
        let routed = parse_dicomweb_request(request, &self.config.limits, self.config.policy)?;
        auth_middleware::authorize_and_audit_web(&self.config.auth, &routed, &subject)?;
        Ok(routed)
    }

    /// Execute a previously routed request against storage.
    pub fn execute_routed(
        &self,
        request: &DicomWebRequest,
        storage: &mut Storage,
    ) -> Result<DicomWebResponse> {
        execute_routed_request(request, storage, &self.config.limits)
    }

    /// Execute a previously routed read-only request against storage.
    ///
    /// Requests that require storage mutation (for example STOW-RS) fail closed.
    pub fn execute_routed_read_only(
        &self,
        request: &DicomWebRequest,
        storage: &Storage,
    ) -> Result<DicomWebResponse> {
        execute_routed_read_only_request(request, storage, &self.config.limits)
    }
}

/// Return true when executing this routed request requires mutable storage access.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub fn request_requires_write(request: &DicomWebRequest) -> bool {
    #[cfg(feature = "stow")]
    if matches!(request, DicomWebRequest::Stow { .. }) {
        return true;
    }
    if matches!(
        request,
        DicomWebRequest::DeleteStudy { .. }
            | DicomWebRequest::DeleteSeries { .. }
            | DicomWebRequest::DeleteInstance { .. }
    ) {
        return true;
    }
    let _ = request;
    false
}

// --- Execution dispatch ---

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn execute_routed_request(
    request: &DicomWebRequest,
    storage: &mut Storage,
    limits: &Limits,
) -> Result<DicomWebResponse> {
    if !request_requires_write(request) {
        return execute_routed_read_only_request(request, storage, limits);
    }

    #[cfg(feature = "stow")]
    if let DicomWebRequest::Stow {
        study_uid,
        content_type,
        body,
    } = request
    {
        let payloads = stow::stow_payloads(content_type, body, limits)?;
        let preflights: Vec<StowPreflight> = payloads
            .iter()
            .map(|payload| stow::stow_preflight_compatibility(payload, limits))
            .collect::<Result<_>>()?;

        let mut outcomes = Vec::with_capacity(payloads.len());
        if let Some(expected_study_uid) = study_uid.as_deref() {
            for preflight in &preflights {
                validate_context_tuple(
                    &ContextTuple {
                        study_uid: Some(expected_study_uid.to_string()),
                        series_uid: None,
                        instance_uid: None,
                        frame_index: None,
                    },
                    &ContextTuple {
                        study_uid: Some(preflight.study_uid.clone()),
                        series_uid: None,
                        instance_uid: None,
                        frame_index: None,
                    },
                )?;
            }
        }

        for payload in payloads {
            outcomes.push(storage.ingest_bytes(payload)?);
        }
        return Ok(DicomWebResponse::Stow { outcomes });
    }

    if let DicomWebRequest::DeleteStudy {
        study_uid,
        hard_delete,
    } = request
    {
        if *hard_delete {
            return Err(decode_error("hard delete is not enabled"));
        }
        storage.soft_delete_study(study_uid);
        return Ok(DicomWebResponse::Delete { tombstoned: true });
    }

    if let DicomWebRequest::DeleteSeries {
        study_uid,
        series_uid,
        hard_delete,
    } = request
    {
        if *hard_delete {
            return Err(decode_error("hard delete is not enabled"));
        }
        storage.soft_delete_series(study_uid, series_uid);
        return Ok(DicomWebResponse::Delete { tombstoned: true });
    }

    if let DicomWebRequest::DeleteInstance {
        study_uid,
        series_uid,
        instance_uid,
        hard_delete,
    } = request
    {
        if *hard_delete {
            return Err(decode_error("hard delete is not enabled"));
        }
        storage.soft_delete_instance(study_uid, series_uid, instance_uid);
        return Ok(DicomWebResponse::Delete { tombstoned: true });
    }

    Err(decode_error("unsupported DICOMweb operation"))
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn execute_routed_read_only_request(
    request: &DicomWebRequest,
    storage: &Storage,
    limits: &Limits,
) -> Result<DicomWebResponse> {
    #[cfg(feature = "qido")]
    if matches!(
        request,
        DicomWebRequest::QidoStudies { .. }
            | DicomWebRequest::QidoAllSeries { .. }
            | DicomWebRequest::QidoAllInstances { .. }
            | DicomWebRequest::QidoSeries { .. }
            | DicomWebRequest::QidoStudyInstances { .. }
            | DicomWebRequest::QidoInstances { .. }
    ) {
        let datasets = storage.datasets()?;
        let matches = qido_query_matches(request, &datasets, limits)?;
        return Ok(DicomWebResponse::Qido { matches });
    }

    match request {
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            study_uid,
            series_uid,
            instance_uid,
            frame_number,
            ..
        } => {
            if let Some(frame_number) = frame_number {
                wado::validate_frame_number(
                    storage,
                    study_uid,
                    series_uid,
                    instance_uid,
                    *frame_number,
                )?;
            }
            let bytes = storage
                .instance_bytes(study_uid, series_uid, instance_uid)
                .ok_or_else(|| not_found_error("requested WADO instance not found"))?;
            Ok(DicomWebResponse::WadoInstance {
                bytes: bytes.to_vec(),
            })
        }
        DicomWebRequest::WadoStudyRetrieve {
            study_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes =
                wado::wado_dataset_retrieve_multipart_bytes(storage, Some(study_uid), None)?;
            Ok(DicomWebResponse::WadoMultipart {
                media_type: wado::WADO_MULTIPART_CONTENT_TYPE.to_string(),
                bytes,
            })
        }
        DicomWebRequest::WadoSeriesRetrieve {
            study_uid,
            series_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = wado::wado_dataset_retrieve_multipart_bytes(
                storage,
                Some(study_uid),
                Some(series_uid),
            )?;
            Ok(DicomWebResponse::WadoMultipart {
                media_type: wado::WADO_MULTIPART_CONTENT_TYPE.to_string(),
                bytes,
            })
        }
        DicomWebRequest::WadoStudyMetadata {
            study_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = wado::metadata_response_bytes(storage, Some(study_uid), None, None)?;
            Ok(DicomWebResponse::WadoMetadata { bytes })
        }
        DicomWebRequest::WadoSeriesMetadata {
            study_uid,
            series_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes =
                wado::metadata_response_bytes(storage, Some(study_uid), Some(series_uid), None)?;
            Ok(DicomWebResponse::WadoMetadata { bytes })
        }
        DicomWebRequest::WadoInstanceMetadata {
            study_uid,
            series_uid,
            instance_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = wado::metadata_response_bytes(
                storage,
                Some(study_uid),
                Some(series_uid),
                Some(instance_uid),
            )?;
            Ok(DicomWebResponse::WadoMetadata { bytes })
        }
        DicomWebRequest::WadoRendered {
            study_uid,
            series_uid,
            instance_uid,
            frame_number,
            transfer_syntax_uid,
            media_type,
        } => {
            let _ = transfer_syntax_uid;
            if let Some(frame_number) = frame_number {
                wado::validate_frame_number(
                    storage,
                    study_uid,
                    series_uid,
                    instance_uid,
                    *frame_number,
                )?;
            }
            let bytes = storage
                .instance_bytes(study_uid, series_uid, instance_uid)
                .ok_or_else(|| not_found_error("requested WADO rendered instance not found"))?;
            Ok(DicomWebResponse::WadoRendered {
                media_type: media_type.clone(),
                bytes: bytes.to_vec(),
            })
        }
        DicomWebRequest::WadoBulkData {
            study_uid,
            series_uid,
            instance_uid,
            transfer_syntax_uid,
            media_type,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = storage
                .instance_bytes(study_uid, series_uid, instance_uid)
                .ok_or_else(|| not_found_error("requested WADO bulkdata instance not found"))?;
            Ok(DicomWebResponse::WadoBulkData {
                media_type: media_type.clone(),
                bytes: bytes.to_vec(),
            })
        }
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { .. } => Err(decode_error("STOW requires mutable storage access")),
        #[allow(unreachable_patterns)]
        _ => Err(decode_error("unsupported DICOMweb operation")),
    }
}

// --- Shared error helpers ---

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-web".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn integrity_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: detail.into(),
        },
        "integrity error",
    )
    .into()
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn not_found_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::NotFound {
            detail: detail.into(),
        },
        "not found",
    )
    .into()
}

#[cfg(feature = "wado")]
fn unsupported_transfer_syntax(transfer_syntax_uid: &str) -> Box<Error> {
    Error::from_kind(
        ErrorKind::UnsupportedTransferSyntax {
            transfer_syntax_uid: transfer_syntax_uid.to_string(),
        },
        "unsupported transfer syntax",
    )
    .into()
}

#[cfg(any(not(feature = "qido"), not(feature = "wado"), not(feature = "stow")))]
pub(crate) fn feature_error(feature: &str) -> Box<Error> {
    decode_error(format!("dicom-web feature '{feature}' is not enabled"))
}

fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "limit exceeded",
    )
    .into()
}

fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(limit_name, observed, allowed));
    }
    Ok(())
}

fn ensure_ascii_graphic(value: &str, label: &str) -> Result<()> {
    if value.bytes().any(|b| !(0x21..=0x7e).contains(&b)) {
        return Err(decode_error(format!(
            "{label} contains non-ASCII characters"
        )));
    }
    Ok(())
}

fn ensure_ascii_printable(value: &str, label: &str) -> Result<()> {
    if value.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
        return Err(decode_error(format!(
            "{label} contains non-ASCII characters"
        )));
    }
    Ok(())
}

fn validate_context_field(
    label: &str,
    expected: Option<&str>,
    observed: Option<&str>,
) -> Result<()> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let observed = observed
        .ok_or_else(|| integrity_error(format!("context tuple mismatch: {label} missing")))?;
    if observed != expected {
        return Err(integrity_error(format!(
            "context tuple mismatch: {label} expected {expected}, observed {observed}"
        )));
    }
    Ok(())
}

fn validate_uid(tag: Tag, value: &str) -> Result<String> {
    validate_uid_strict(tag, value)?;
    Ok(value.to_string())
}
