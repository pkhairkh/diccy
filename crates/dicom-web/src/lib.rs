#![deny(missing_docs)]

//! DICOMweb request parsing/routing and in-process service runtime execution.

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

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
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
}

impl Default for DicomWebServiceConfig {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            policy: WebPolicy::new(TlsPolicy::RequireTls, ThrottleDecision::Allow),
            auth: WebAuthConfig::deny_all(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    use dicom_audit::AuditValue;
    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    use dicom_auth::{AuthAction, AuthDecision, AuthDenyReason, AuthRequest, Authorizer};
    use dicom_auth::{AuthResource, AuthScope, AuthSubject};
    use dicom_core::ErrorKind;
    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    use dicom_core::Tag;
    #[cfg(feature = "qido")]
    use dicom_core::{Dataset, Element, Value, Vr};
    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    use dicom_storage::{IngestOutcome, Storage};
    #[cfg(feature = "qido")]
    use qido::{TAG_ACCESSION_NUMBER, TAG_MODALITY, TAG_PATIENT_ID, TAG_STUDY_DATE};
    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    use std::sync::{Arc, Mutex};

    fn limits() -> Limits {
        Limits::default()
    }

    fn policy() -> WebPolicy {
        WebPolicy::new(TlsPolicy::AllowInsecure, ThrottleDecision::Allow)
    }

    fn request(method: HttpMethod, path: &str, body: &[u8]) -> WebRequest {
        WebRequest {
            method,
            transport: TransportSecurity::Insecure,
            path: path.to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            body: body.to_vec(),
        }
    }

    #[test]
    fn default_config_is_secure() {
        // REQ-HTTP-303, REQ-AUTH-300: runtime defaults are secure by default.
        let config = DicomWebServiceConfig::default();
        assert_eq!(config.policy.tls, TlsPolicy::RequireTls);
        let decision = config
            .auth
            .authorizer
            .authorize(&dicom_auth::AuthRequest {
                scope: AuthScope::Dicomweb,
                action: dicom_auth::AuthAction::WebRequest,
                subject: AuthSubject::anonymous(),
                resource: AuthResource::none(),
            })
            .expect("decision");
        assert!(matches!(
            decision,
            dicom_auth::AuthDecision::Deny(AuthDenyReason::Policy)
        ));
    }

    #[test]
    fn parse_http_request_basic() {
        // REQ-HTTP-300: DICOMweb supports GET/HEAD/POST and must parse deterministically.
        let data = b"GET /studies?PatientID=123\nHost: example\n\n";
        let req = parse_http_request(data, &limits(), TransportSecurity::Insecure).expect("parse");
        assert_eq!(req.method, HttpMethod::Get);
        assert_eq!(req.path, "/studies");
        assert_eq!(req.query.len(), 1);
        assert_eq!(req.query[0].key, "PatientID");
    }

    #[test]
    fn parse_http_request_head_method() {
        // REQ-HTTP-300: HEAD requests are accepted for read-only DICOMweb routes.
        let data = b"HEAD /studies\nHost: example\n\n";
        let req = parse_http_request(data, &limits(), TransportSecurity::Insecure).expect("parse");
        assert_eq!(req.method, HttpMethod::Head);
        assert_eq!(req.path, "/studies");
    }

    #[test]
    fn parse_http_rejects_unsupported_method() {
        // REQ-HTTP-300: Unsupported HTTP methods must fail closed.
        let data = b"PUT /studies\n\n";
        let err =
            parse_http_request(data, &limits(), TransportSecurity::Insecure).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_http_rejects_long_query_keys() {
        // REQ-HTTP-301: URI and query parameter lengths are bounded by max_string_bytes.
        let mut limits = Limits::default();
        limits.set_max_string_bytes(4);
        let data = b"GET /studies?ABCDE=1\n\n";
        let err =
            parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_string_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn parse_http_rejects_excess_query_params() {
        // REQ-HTTP-301: Query parameter count is bounded by max_dataset_elements.
        let mut limits = Limits::default();
        limits.set_max_dataset_elements(1);
        let data = b"GET /studies?A=1&B=2\n\n";
        let err =
            parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_dataset_elements");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn get_with_body_rejected() {
        // REQ-HTTP-302: GET/HEAD requests must not include a body.
        let req = request(HttpMethod::Get, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn head_with_body_rejected() {
        // REQ-HTTP-302: GET/HEAD requests must not include a body.
        let req = request(HttpMethod::Head, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn tls_policy_requires_tls() {
        // REQ-HTTP-303: TLS policy must fail closed on insecure transports.
        let req = request(HttpMethod::Get, "/studies", &[]);
        let policy = WebPolicy::new(TlsPolicy::RequireTls, ThrottleDecision::Allow);
        let err = parse_dicomweb_request(req, &limits(), policy).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn throttle_rejection_returns_limit_exceeded() {
        // REQ-HTTP-304: Throttling decisions must return LimitExceeded on rejection.
        let req = request(HttpMethod::Get, "/studies", &[]);
        let policy = WebPolicy::new(
            TlsPolicy::AllowInsecure,
            ThrottleDecision::Reject {
                limit_name: "max_web_requests_inflight",
                observed: 2,
                allowed: 1,
            },
        );
        let err = parse_dicomweb_request(req, &limits(), policy).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded {
                limit_name,
                observed,
                allowed,
            } => {
                assert_eq!(*limit_name, "max_web_requests_inflight");
                assert_eq!(*observed, 2);
                assert_eq!(*allowed, 1);
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_studies_parses() {
        // REQ-WEB-300: Supported QIDO-RS endpoints must parse deterministically.
        let req = request(HttpMethod::Get, "/studies", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::QidoStudies { .. } => {}
            _ => panic!("expected QIDO studies"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_studies_head_parses() {
        // REQ-WEB-300: HEAD is accepted for QIDO study queries.
        let req = request(HttpMethod::Head, "/studies", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::QidoStudies { .. } => {}
            _ => panic!("expected QIDO studies"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_all_series_parses() {
        // REQ-WEB-300: global series QIDO endpoint parses deterministically.
        let req = request(HttpMethod::Get, "/series", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::QidoAllSeries { .. } => {}
            _ => panic!("expected QIDO all series"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_all_instances_parses() {
        // REQ-WEB-300: global instances QIDO endpoint parses deterministically.
        let req = request(HttpMethod::Get, "/instances", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::QidoAllInstances { .. } => {}
            _ => panic!("expected QIDO all instances"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_study_instances_parses() {
        // REQ-WEB-300: study-level instance QIDO endpoint parses deterministically.
        let req = request(HttpMethod::Get, "/studies/1.2.3/instances", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::QidoStudyInstances { study_uid, .. } => assert_eq!(study_uid, "1.2.3"),
            _ => panic!("expected QIDO study instances"),
        }
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_study_uid() {
        // REQ-QR-300: only supported UID keys may be used for matching.
        // REQ-QR-301: query results are deterministically ordered by UID.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        let mut dataset_other = Dataset::new();
        dataset_other
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
        dataset_other
            .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
        dataset_other.insert(
            Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
        );
        let datasets = vec![dataset, dataset_other];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies".to_string(),
            query: vec![QueryParam {
                key: "StudyInstanceUID".to_string(),
                value: "1.2.3".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_patient_and_modality() {
        // REQ-QR-300: QIDO query mapping supports PatientID and Modality keys.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        let mut dataset_other = Dataset::new();
        dataset_other
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset_other.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.9".to_string())).unwrap(),
        );
        dataset_other.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.9.1".to_string()),
            )
            .unwrap(),
        );
        dataset_other.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
        );
        dataset_other
            .insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("MR".to_string())).unwrap());
        let datasets = vec![dataset, dataset_other];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies/1.2.3/series".to_string(),
            query: vec![
                QueryParam {
                    key: "PatientID".to_string(),
                    value: "PATIENT_A".to_string(),
                },
                QueryParam {
                    key: "Modality".to_string(),
                    value: "CT".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_accession_and_study_date() {
        // REQ-QR-300: QIDO query mapping supports AccessionNumber and StudyDate keys.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_ACCESSION_NUMBER,
                Vr::Lo,
                Value::Str("ACC123".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20260211".to_string())).unwrap(),
        );

        let mut dataset_other = Dataset::new();
        dataset_other
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
        dataset_other
            .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
        dataset_other.insert(
            Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
        );
        dataset_other.insert(
            Element::new(
                TAG_ACCESSION_NUMBER,
                Vr::Lo,
                Value::Str("ACC999".to_string()),
            )
            .unwrap(),
        );
        dataset_other.insert(
            Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20260101".to_string())).unwrap(),
        );
        let datasets = vec![dataset, dataset_other];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies".to_string(),
            query: vec![
                QueryParam {
                    key: "AccessionNumber".to_string(),
                    value: "ACC123".to_string(),
                },
                QueryParam {
                    key: "StudyDate".to_string(),
                    value: "20260211".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_study_instances_path() {
        // REQ-WEB-300: /studies/{StudyUID}/instances maps to instance-level queries.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );

        let mut dataset_same_study = Dataset::new();
        dataset_same_study
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset_same_study.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.9".to_string())).unwrap(),
        );
        dataset_same_study.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.9.1".to_string()),
            )
            .unwrap(),
        );

        let mut dataset_other_study = Dataset::new();
        dataset_other_study
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
        dataset_other_study
            .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
        dataset_other_study.insert(
            Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
        );

        let datasets = vec![dataset_same_study, dataset_other_study, dataset];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies/1.2.3/instances".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].study_uid, "1.2.3");
        assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
        assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.3.4.5"));
        assert_eq!(matches[1].study_uid, "1.2.3");
        assert_eq!(matches[1].series_uid.as_deref(), Some("1.2.3.9"));
        assert_eq!(matches[1].instance_uid.as_deref(), Some("1.2.3.9.1"));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_all_series_path() {
        // REQ-WEB-300: /series maps to series-level queries.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());

        let mut dataset_other = Dataset::new();
        dataset_other
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
        dataset_other
            .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
        dataset_other.insert(
            Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
        );
        dataset_other
            .insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("MR".to_string())).unwrap());

        let datasets = vec![dataset_other, dataset];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/series".to_string(),
            query: vec![QueryParam {
                key: "Modality".to_string(),
                value: "CT".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
        assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
        assert_eq!(matches[0].instance_uid, None);
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_matches_all_instances_path() {
        // REQ-WEB-300: /instances maps to instance-level queries.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
        );

        let mut dataset_other = Dataset::new();
        dataset_other
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
        dataset_other
            .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
        dataset_other.insert(
            Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
        );
        dataset_other.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_B".to_string())).unwrap(),
        );

        let datasets = vec![dataset_other, dataset];

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/instances".to_string(),
            query: vec![QueryParam {
                key: "PatientID".to_string(),
                value: "PATIENT_A".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
        assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
        assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.3.4.5"));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_rejects_unsupported_param() {
        // REQ-QR-300: unsupported query keys must fail closed.
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies".to_string(),
            query: vec![QueryParam {
                key: "StudyDescription".to_string(),
                value: "HEAD".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let err = qido_query_matches(&parsed, &[], &limits()).expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::DecodeError { .. } | ErrorKind::InvalidTagValue { .. }
        ));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_query_rejects_path_conflict() {
        // REQ-QR-300: conflicting query keys must fail closed.
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies/1.2.3/series".to_string(),
            query: vec![QueryParam {
                key: "StudyInstanceUID".to_string(),
                value: "9.9".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let err = qido_query_matches(&parsed, &[], &limits()).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_limit_and_offset_are_applied_deterministically() {
        let mut dataset_a = Dataset::new();
        dataset_a
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset_a.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.1".to_string())).unwrap(),
        );
        dataset_a.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.1.1".to_string()),
            )
            .unwrap(),
        );

        let mut dataset_b = Dataset::new();
        dataset_b
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.4".to_string())).unwrap());
        dataset_b.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.4.1".to_string())).unwrap(),
        );
        dataset_b.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.4.1.1".to_string()),
            )
            .unwrap(),
        );

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies".to_string(),
            query: vec![
                QueryParam {
                    key: "offset".to_string(),
                    value: "1".to_string(),
                },
                QueryParam {
                    key: "limit".to_string(),
                    value: "1".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches =
            qido_query_matches(&parsed, &[dataset_b, dataset_a], &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.4");
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_accepts_normalized_keyword_and_tag_key_forms() {
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.1.1".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
        );

        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies".to_string(),
            query: vec![
                QueryParam {
                    key: "study_instance_uid".to_string(),
                    value: "1.2.3".to_string(),
                },
                QueryParam {
                    key: "(0010,0020)".to_string(),
                    value: "PATIENT_A".to_string(),
                },
                QueryParam {
                    key: "includefield".to_string(),
                    value: "all".to_string(),
                },
                QueryParam {
                    key: "fuzzy".to_string(),
                    value: "false".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        let matches = qido_query_matches(&parsed, &[dataset], &limits()).expect("matches");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[cfg(not(feature = "qido"))]
    #[test]
    fn qido_feature_disabled_fails_closed() {
        // REQ-WEB-300: QIDO-RS endpoints fail closed when feature is disabled.
        let req = request(HttpMethod::Get, "/studies", &[]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(not(feature = "wado"))]
    #[test]
    fn wado_feature_disabled_fails_closed() {
        // REQ-WEB-300: WADO-RS endpoints fail closed when feature is disabled.
        let req = request(
            HttpMethod::Get,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5",
            &[],
        );
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn invalid_uid_fails_closed() {
        // REQ-WEB-301: UID segments must be valid UIDs.
        let req = request(
            HttpMethod::Get,
            "/studies/1.2.x/series/1.2.4/instances/1.2.5",
            &[],
        );
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[cfg(feature = "stow")]
    #[test]
    fn stow_requires_content_type() {
        // REQ-WEB-302: STOW-RS requires a valid content-type.
        let mut req = request(HttpMethod::Post, "/studies", &[1, 2]);
        req.headers.clear();
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn body_limit_enforced() {
        // REQ-WEB-303: Request bodies are bounded by max_input_bytes.
        let mut limits = Limits::default();
        limits.set_max_input_bytes(1);
        let req = request(HttpMethod::Post, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits, policy()).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(*limit_name, "max_input_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_rejects_query_params() {
        // REQ-WEB-300: Unsupported query parameters must fail closed.
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies/1.2.3/series/1.2.4/instances/1.2.5".to_string(),
            query: vec![QueryParam {
                key: "unexpected".to_string(),
                value: "x".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_allows_supported_transfer_syntax_query_param() {
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/studies/1.2.3/series/1.2.4/instances/1.2.5".to_string(),
            query: vec![QueryParam {
                key: "transferSyntax".to_string(),
                value: "1.2.840.10008.1.2".to_string(),
            }],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::WadoInstance {
                transfer_syntax_uid,
                ..
            } => {
                assert_eq!(transfer_syntax_uid.as_deref(), Some("1.2.840.10008.1.2"));
            }
            _ => panic!("expected WADO instance"),
        }
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_uri_compatibility_parses() {
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/wado".to_string(),
            query: vec![
                QueryParam {
                    key: "requestType".to_string(),
                    value: "WADO".to_string(),
                },
                QueryParam {
                    key: "studyUID".to_string(),
                    value: "1.2.3".to_string(),
                },
                QueryParam {
                    key: "seriesUID".to_string(),
                    value: "1.2.4".to_string(),
                },
                QueryParam {
                    key: "objectUID".to_string(),
                    value: "1.2.5".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::WadoInstance {
                study_uid,
                series_uid,
                instance_uid,
                ..
            } => {
                assert_eq!(study_uid, "1.2.3");
                assert_eq!(series_uid, "1.2.4");
                assert_eq!(instance_uid, "1.2.5");
            }
            _ => panic!("expected WADO instance"),
        }
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_uri_rejects_unsupported_query_key() {
        let req = WebRequest {
            method: HttpMethod::Get,
            transport: TransportSecurity::Insecure,
            path: "/wado".to_string(),
            query: vec![
                QueryParam {
                    key: "requestType".to_string(),
                    value: "WADO".to_string(),
                },
                QueryParam {
                    key: "studyUID".to_string(),
                    value: "1.2.3".to_string(),
                },
                QueryParam {
                    key: "seriesUID".to_string(),
                    value: "1.2.4".to_string(),
                },
                QueryParam {
                    key: "objectUID".to_string(),
                    value: "1.2.5".to_string(),
                },
                QueryParam {
                    key: "foo".to_string(),
                    value: "bar".to_string(),
                },
            ],
            headers: Vec::new(),
            body: Vec::new(),
        };
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_rejects_post_method() {
        // REQ-WEB-300: WADO routes accept GET/HEAD only; POST must fail closed.
        let req = request(
            HttpMethod::Post,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5",
            &[1, 2, 3],
        );
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn study_uid_route_get_parses_as_wado_study_retrieve() {
        // REQ-WEB-300: /studies/{StudyUID} supports WADO retrieve on GET/HEAD.
        let req = request(HttpMethod::Get, "/studies/1.2.3", &[]);
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        assert!(matches!(parsed, DicomWebRequest::WadoStudyRetrieve { .. }));
    }

    #[cfg(not(feature = "wado"))]
    #[test]
    fn study_uid_route_rejects_get_when_wado_disabled() {
        let req = request(HttpMethod::Get, "/studies/1.2.3", &[]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn wado_head_parses() {
        // REQ-WEB-300: HEAD is accepted for WADO instance routes.
        let req = request(
            HttpMethod::Head,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5",
            &[],
        );
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::WadoInstance { .. } => {}
            _ => panic!("expected WADO instance"),
        }
    }

    #[cfg(feature = "wado")]
    #[test]
    fn study_and_series_retrieve_routes_parse_as_wado_requests() {
        let study = parse_dicomweb_request(
            request(HttpMethod::Get, "/studies/1.2.3", &[]),
            &limits(),
            policy(),
        )
        .expect("study retrieve");
        assert!(matches!(study, DicomWebRequest::WadoStudyRetrieve { .. }));

        let series = parse_dicomweb_request(
            request(HttpMethod::Get, "/studies/1.2.3/series/1.2.4", &[]),
            &limits(),
            policy(),
        )
        .expect("series retrieve");
        assert!(matches!(series, DicomWebRequest::WadoSeriesRetrieve { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn metadata_routes_parse_as_wado_requests() {
        let study = parse_dicomweb_request(
            request(HttpMethod::Get, "/studies/1.2.3/metadata", &[]),
            &limits(),
            policy(),
        )
        .expect("study metadata");
        assert!(matches!(study, DicomWebRequest::WadoStudyMetadata { .. }));

        let series = parse_dicomweb_request(
            request(HttpMethod::Get, "/studies/1.2.3/series/1.2.4/metadata", &[]),
            &limits(),
            policy(),
        )
        .expect("series metadata");
        assert!(matches!(series, DicomWebRequest::WadoSeriesMetadata { .. }));

        let instance = parse_dicomweb_request(
            request(
                HttpMethod::Get,
                "/studies/1.2.3/series/1.2.4/instances/1.2.5/metadata",
                &[],
            ),
            &limits(),
            policy(),
        )
        .expect("instance metadata");
        assert!(matches!(
            instance,
            DicomWebRequest::WadoInstanceMetadata { .. }
        ));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn rendered_and_bulkdata_routes_parse_with_allowlisted_query() {
        let mut rendered_req = request(
            HttpMethod::Get,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5/rendered",
            &[],
        );
        rendered_req.query = vec![
            QueryParam {
                key: "accept".to_string(),
                value: "image/png".to_string(),
            },
            QueryParam {
                key: "transferSyntax".to_string(),
                value: "1.2.840.10008.1.2.1".to_string(),
            },
        ];
        let rendered =
            parse_dicomweb_request(rendered_req, &limits(), policy()).expect("rendered parse");
        assert!(matches!(rendered, DicomWebRequest::WadoRendered { .. }));

        let mut bulk_req = request(
            HttpMethod::Get,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5/bulkdata",
            &[],
        );
        bulk_req.query = vec![QueryParam {
            key: "accept".to_string(),
            value: "application/octet-stream".to_string(),
        }];
        let bulkdata =
            parse_dicomweb_request(bulk_req, &limits(), policy()).expect("bulkdata parse");
        assert!(matches!(bulkdata, DicomWebRequest::WadoBulkData { .. }));
    }

    #[cfg(feature = "wado")]
    #[test]
    fn frame_route_parses_with_frame_number() {
        let req = request(
            HttpMethod::Get,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5/frames/1",
            &[],
        );
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::WadoInstance { frame_number, .. } => {
                assert_eq!(frame_number, Some(1));
            }
            _ => panic!("expected WADO instance"),
        }
    }

    #[cfg(feature = "stow")]
    #[test]
    fn stow_parses_multipart_content_type() {
        // REQ-WEB-302: multipart/related requires type=application/dicom and boundary.
        let mut req = request(HttpMethod::Post, "/studies", &[1, 2, 3]);
        req.headers.push(Header {
            name: "content-type".to_string(),
            value: "multipart/related; type=\"application/dicom\"; boundary=abc".to_string(),
        });
        let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
        match parsed {
            DicomWebRequest::Stow { content_type, .. } => match content_type {
                DicomWebContentType::MultipartRelated { boundary } => {
                    assert_eq!(boundary, "abc");
                }
                _ => panic!("expected multipart"),
            },
            _ => panic!("expected STOW"),
        }
    }

    #[cfg(feature = "stow")]
    #[test]
    fn stow_accepts_dicom_xml_and_json_content_types() {
        let mut xml_req = request(HttpMethod::Post, "/studies", b"<NativeDicomModel/>");
        xml_req.headers.push(Header {
            name: "content-type".to_string(),
            value: "application/dicom+xml".to_string(),
        });
        let xml_parsed = parse_dicomweb_request(xml_req, &limits(), policy()).expect("xml parse");
        match xml_parsed {
            DicomWebRequest::Stow { content_type, .. } => {
                assert!(matches!(
                    content_type,
                    DicomWebContentType::ApplicationDicomXml
                ));
            }
            _ => panic!("expected STOW"),
        }

        let mut json_req = request(HttpMethod::Post, "/studies", b"{}");
        json_req.headers.push(Header {
            name: "content-type".to_string(),
            value: "application/dicom+json".to_string(),
        });
        let json_parsed =
            parse_dicomweb_request(json_req, &limits(), policy()).expect("json parse");
        match json_parsed {
            DicomWebRequest::Stow { content_type, .. } => {
                assert!(matches!(
                    content_type,
                    DicomWebContentType::ApplicationDicomJson
                ));
            }
            _ => panic!("expected STOW"),
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    struct DenyRetrieveAllowOthers;

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    struct RequireHeaderSubject;

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    impl Authorizer for DenyRetrieveAllowOthers {
        fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
            if request.action == AuthAction::Retrieve {
                return Ok(AuthDecision::Deny(AuthDenyReason::Policy));
            }
            Ok(AuthDecision::Allow)
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    impl Authorizer for RequireHeaderSubject {
        fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
            if request.subject.principal == Some("alice")
                && request.subject.peer == Some("10.0.0.7")
            {
                return Ok(AuthDecision::Allow);
            }
            Ok(AuthDecision::Deny(AuthDenyReason::Unauthenticated))
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(b"UI");
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&bytes);
        buf
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&vr);
        let mut bytes = value.to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            _ => {
                buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            }
        }
        buf.extend_from_slice(&bytes);
        buf
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    fn sample_p10(study_uid: &str, series_uid: &str, instance_uid: &str) -> Vec<u8> {
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0016),
            *b"UI",
            b"1.2.840.10008.5.1.4.1.1.7",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0018),
            *b"UI",
            instance_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000D),
            *b"UI",
            study_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000E),
            *b"UI",
            series_uid.as_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0002),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0004),
            *b"CS",
            b"MONOCHROME2",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0010),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0011),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0100),
            *b"US",
            &16u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0101),
            *b"US",
            &12u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0102),
            *b"US",
            &11u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0103),
            *b"US",
            &0u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x7FE0, 0x0010),
            *b"OB",
            &[0u8],
        ));

        let mut bytes = vec![0u8; 128];
        bytes.extend_from_slice(b"DICM");
        bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), "1.2.840.10008.1.2.1"));
        bytes.extend_from_slice(&dataset);
        bytes
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    fn multipart_body(boundary: &str, payloads: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        for payload in payloads {
            out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            out.extend_from_slice(b"content-type: application/dicom\r\n\r\n");
            out.extend_from_slice(payload);
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        out
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    fn service(
        authorizer: Arc<dyn Authorizer + Send + Sync>,
        events: Arc<Mutex<Vec<AuditEvent>>>,
    ) -> DicomWebService {
        let config = DicomWebServiceConfig {
            limits: Limits::default(),
            policy: policy(),
            auth: WebAuthConfig {
                authorizer,
                audit: Some(Arc::new(move |event| {
                    let mut guard = events.lock().expect("audit lock");
                    guard.push(event);
                    Ok(())
                })),
            },
        };
        DicomWebService::new(config)
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_qido_allows_and_emits_audit() {
        // REQ-WEB-304, REQ-AUTH-300, REQ-AUDIT-350
        let mut storage = Storage::new(Limits::default());
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
            .expect("ingest");
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events.clone());

        let response = service
            .handle_http(
                b"GET /studies?StudyInstanceUID=1.2.3\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("service response");

        match response {
            DicomWebResponse::Qido { matches } => {
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0].study_uid, "1.2.3");
            }
            _ => panic!("expected QIDO response"),
        }

        let events = events.lock().expect("audit lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|field| {
            field.key == "action" && matches!(field.value, AuditValue::Plain(ref v) if v == "query")
        }));
        assert!(events[0].fields.iter().any(|field| {
            field.key == "decision"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "allow")
        }));
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_wado_denied_records_audit() {
        // REQ-WEB-304, REQ-AUTH-302, REQ-AUDIT-350
        let mut storage = Storage::new(Limits::default());
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
            .expect("ingest");
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(DenyRetrieveAllowOthers), events.clone());

        let err = service
            .handle_http(
                b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect_err("expected deny");
        assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));

        let events = events.lock().expect("audit lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|field| {
            field.key == "decision"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "deny")
        }));
        assert!(events[0].fields.iter().any(|field| {
            field.key == "deny_reason"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "policy")
        }));
        assert!(events[0].fields.iter().any(|field| {
            field.key == "protocol_path"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "dicomweb")
        }));
        assert!(events[0].fields.iter().any(|field| {
            field.key == "operation"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "wado.instance")
        }));
        assert!(events[0].fields.iter().any(|field| {
            field.key == "event_code"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "auth.deny")
        }));
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_stow_then_wado_round_trip() {
        // REQ-WEB-304, REQ-WEB-305
        let mut storage = Storage::new(Limits::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events);
        let body = sample_p10("1.2.3", "2.3.4", "3.4.5");

        let mut stow_http = b"POST /studies\r\ncontent-type: application/dicom\r\n\r\n".to_vec();
        stow_http.extend_from_slice(&body);
        let stow_response = service
            .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
            .expect("stow response");
        match stow_response {
            DicomWebResponse::Stow { outcomes } => {
                assert_eq!(outcomes.len(), 1);
                assert!(matches!(outcomes[0], IngestOutcome::Inserted { .. }));
            }
            _ => panic!("expected STOW response"),
        }

        let wado_response = service
            .handle_http(
                b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("wado response");
        match wado_response {
            DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, body),
            _ => panic!("expected WADO response"),
        }

        let err = service
            .handle_http(
                b"GET /studies/9.9.9/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect_err("expected not found");
        assert!(matches!(err.kind(), ErrorKind::NotFound { .. }));
        assert_eq!(err.code(), DICOM_WEB_NOT_FOUND_CODE);
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_wado_study_retrieve_returns_multipart() {
        let mut storage = Storage::new(Limits::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events);
        let first = sample_p10("1.2.3", "2.3.4", "3.4.5");
        let second = sample_p10("1.2.3", "2.3.4", "3.4.6");
        storage.ingest_bytes(first.clone()).expect("first ingest");
        storage.ingest_bytes(second.clone()).expect("second ingest");

        let response = service
            .handle_http(
                b"GET /studies/1.2.3\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("study retrieve");

        match response {
            DicomWebResponse::WadoMultipart { media_type, bytes } => {
                assert_eq!(
                    media_type,
                    "multipart/related; type=\"application/dicom\"; boundary=\"dicomweb-dataset\""
                );
                let body = String::from_utf8_lossy(&bytes);
                assert!(
                    body.contains("--dicomweb-dataset\r\ncontent-type: application/dicom\r\n\r\n")
                );
                assert!(bytes
                    .windows(first.len())
                    .any(|window| window == first.as_slice()));
                assert!(bytes
                    .windows(second.len())
                    .any(|window| window == second.as_slice()));
                assert!(body.contains("--dicomweb-dataset--\r\n"));
            }
            _ => panic!("expected multipart WADO response"),
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_stow_multipart_ingests_each_part() {
        // REQ-WEB-302, REQ-WEB-305
        let mut storage = Storage::new(Limits::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events);
        let first = sample_p10("1.2.3", "2.3.4", "3.4.5");
        let second = sample_p10("1.2.3", "2.3.4", "3.4.6");
        let boundary = "batch-1";

        let mut stow_http = format!(
            "POST /studies\r\ncontent-type: multipart/related; type=\"application/dicom\"; boundary={boundary}\r\n\r\n"
        )
        .into_bytes();
        stow_http.extend_from_slice(&multipart_body(boundary, &[first.clone(), second.clone()]));

        let response = service
            .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
            .expect("multipart stow");
        match response {
            DicomWebResponse::Stow { outcomes } => {
                assert_eq!(outcomes.len(), 2);
                assert!(matches!(outcomes[0], IngestOutcome::Inserted { .. }));
                assert!(matches!(outcomes[1], IngestOutcome::Inserted { .. }));
            }
            _ => panic!("expected STOW response"),
        }

        let first_wado = service
            .handle_http(
                b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("first wado");
        match first_wado {
            DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, first),
            _ => panic!("expected first WADO"),
        }

        let second_wado = service
            .handle_http(
                b"GET /studies/1.2.3/series/2.3.4/instances/3.4.6\r\nHost: example\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("second wado");
        match second_wado {
            DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, second),
            _ => panic!("expected second WADO"),
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_stow_rejects_study_uid_mismatch() {
        // REQ-WEB-305: /studies/{StudyUID} must enforce Study Instance UID consistency.
        let mut storage = Storage::new(Limits::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events);
        let body = sample_p10("9.9.9", "2.3.4", "3.4.5");
        let mut stow_http =
            b"POST /studies/1.2.3\r\ncontent-type: application/dicom\r\n\r\n".to_vec();
        stow_http.extend_from_slice(&body);

        let err = service
            .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
            .expect_err("expected study conflict");
        assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
        assert_eq!(storage.index().total_instances(), 0);
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn request_requires_write_detects_mutating_routes() {
        // REQ-WEB-305: STOW and DELETE mutate storage state.
        let qido = DicomWebRequest::QidoStudies { params: Vec::new() };
        let wado = DicomWebRequest::WadoInstance {
            study_uid: "1.2.3".to_string(),
            series_uid: "2.3.4".to_string(),
            instance_uid: "3.4.5".to_string(),
            transfer_syntax_uid: None,
            frame_number: None,
        };
        let stow = DicomWebRequest::Stow {
            study_uid: None,
            content_type: DicomWebContentType::ApplicationDicom,
            body: vec![0u8; 1],
        };
        let delete_instance = DicomWebRequest::DeleteInstance {
            study_uid: "1.2.3".to_string(),
            series_uid: "2.3.4".to_string(),
            instance_uid: "3.4.5".to_string(),
            hard_delete: false,
        };
        assert!(!request_requires_write(&qido));
        assert!(!request_requires_write(&wado));
        assert!(request_requires_write(&stow));
        assert!(request_requires_write(&delete_instance));
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn execute_routed_read_only_rejects_stow() {
        // REQ-WEB-305: read-only execution path fails closed on mutating requests.
        let storage = Storage::new(Limits::default());
        let service = DicomWebService::new(DicomWebServiceConfig::default());
        let request = DicomWebRequest::Stow {
            study_uid: None,
            content_type: DicomWebContentType::ApplicationDicom,
            body: sample_p10("1.2.3", "2.3.4", "3.4.5"),
        };
        let err = service
            .execute_routed_read_only(&request, &storage)
            .expect_err("expected read-only rejection");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn delete_routes_are_policy_gated_and_soft_delete_by_default() {
        let disabled = parse_dicomweb_request(
            request(HttpMethod::Delete, "/studies/1.2.3", &[]),
            &limits(),
            policy(),
        )
        .expect_err("delete disabled");
        assert!(matches!(disabled.kind(), ErrorKind::DecodeError { .. }));

        let enabled = parse_dicomweb_request(
            request(HttpMethod::Delete, "/studies/1.2.3", &[]),
            &limits(),
            policy().with_delete_enabled(true),
        )
        .expect("delete enabled");
        match enabled {
            DicomWebRequest::DeleteStudy { hard_delete, .. } => assert!(!hard_delete),
            _ => panic!("expected delete study"),
        }
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn write_routes_fail_closed_on_cross_tenant_scope() {
        let mut storage = Storage::new(Limits::default());
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(dicom_auth::AllowAll), events);

        let mut stow_http = b"POST /studies\r\ncontent-type: application/dicom\r\nx-tenant-id: tenant-a\r\nx-tenant-scope: tenant-b\r\n\r\n".to_vec();
        stow_http.extend_from_slice(&sample_p10("1.2.3", "2.3.4", "3.4.5"));

        let err = service
            .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
            .expect_err("cross-tenant write should fail");
        assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
        assert_eq!(storage.index().total_instances(), 0);
    }

    #[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
    #[test]
    fn service_uses_header_subject_for_authz() {
        // REQ-AUTH-300: DICOMweb auth context must preserve caller identity when provided.
        let mut storage = Storage::new(Limits::default());
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
            .expect("ingest");
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = service(Arc::new(RequireHeaderSubject), events);
        let response = service
            .handle_http(
                b"GET /studies?StudyInstanceUID=1.2.3\r\nx-auth-principal: alice\r\nx-forwarded-for: 10.0.0.7, 10.0.0.8\r\n\r\n",
                TransportSecurity::Insecure,
                &mut storage,
            )
            .expect("authorized request");
        match response {
            DicomWebResponse::Qido { matches } => assert_eq!(matches.len(), 1),
            _ => panic!("expected QIDO response"),
        }
    }
}
