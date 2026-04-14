#![deny(missing_docs)]

//! DICOMweb request parsing/routing and in-process service runtime execution.

use dicom_audit::AuditEvent;
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_audit::{AuditEventKind, AuditField, AuditValue};
use dicom_auth::{enforce_tenant_scope, AllowAll, AuthDenyReason, Authorizer, DenyAll};
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_auth::{AuthAction, AuthDecision, AuthRequest, AuthResource, AuthScope, AuthSubject};
#[cfg(feature = "qido")]
use dicom_core::Dataset;
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_core::{validate_uid_strict, Tag};
use dicom_core::{Error, ErrorKind, Limits, Result};
#[cfg(feature = "qido")]
use dicom_query::{query as run_query, Query, QueryKey, QueryLevel, QueryMatch};
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_storage::{IngestOutcome, Storage};

use std::fmt;
use std::sync::Arc;

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
#[cfg(any(feature = "qido", feature = "wado"))]
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
#[cfg(any(feature = "qido", feature = "wado"))]
const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
#[cfg(feature = "qido")]
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
#[cfg(feature = "qido")]
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
#[cfg(feature = "qido")]
const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
#[cfg(feature = "qido")]
const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);
#[cfg(any(feature = "wado", feature = "stow"))]
const TAG_TRANSFER_SYNTAX_UID: Tag = Tag(0x0002, 0x0010);
#[cfg(feature = "wado")]
const TRANSFER_SYNTAX_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
#[cfg(feature = "wado")]
const TRANSFER_SYNTAX_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

/// Stable error code for DICOMweb not-found responses.
pub const DICOM_WEB_NOT_FOUND_CODE: &str = "DVF.WEB.NOT_FOUND";

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

/// Audit callback for authorization decisions.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// Authorization and audit configuration for DICOMweb.
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
        matches: Vec<QueryMatch>,
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
        outcomes: Vec<IngestOutcome>,
    },
    /// Delete route outcome.
    Delete {
        /// True when soft-delete tombstone was applied.
        tombstoned: bool,
    },
}

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
        let subject = derive_auth_subject(&request);
        let routed = parse_dicomweb_request(request, &self.config.limits, self.config.policy)?;
        authorize_and_audit_web(&self.config.auth, &routed, &subject)?;
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

/// Route/method capability state for DICOMweb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DicomWebRouteState {
    /// Route/method is fully implemented and exposed.
    Implemented,
    /// Route/method is exposed but intentionally partial.
    Partial,
    /// Route/method is blocked (for example by missing feature support).
    Blocked,
    /// Route/method exists in capability registry but is not exposed at runtime.
    NotExposed,
}

/// A route/method capability entry for DICOMweb interoperability matrices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DicomWebRouteCapability {
    /// Route path pattern.
    pub path: &'static str,
    /// HTTP method.
    pub method: HttpMethod,
    /// Operation label.
    pub operation: &'static str,
    /// Required feature gate.
    pub required_feature: &'static str,
    /// Availability state for this route/method.
    pub state: DicomWebRouteState,
    /// Content-type contract when applicable.
    pub content_type: Option<&'static str>,
}

/// Build a deterministic DICOMweb route/method capability matrix.
pub fn dicomweb_route_capability_matrix() -> [DicomWebRouteCapability; 34] {
    let qido = route_state(cfg!(feature = "qido"));
    let wado = route_state(cfg!(feature = "wado"));
    let stow = route_state(cfg!(feature = "stow"));
    let delete_state = DicomWebRouteState::Blocked;
    [
        DicomWebRouteCapability {
            path: "/studies",
            method: HttpMethod::Get,
            operation: "QIDO studies",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies",
            method: HttpMethod::Head,
            operation: "QIDO studies",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies",
            method: HttpMethod::Post,
            operation: "STOW studies",
            required_feature: "stow",
            state: stow,
            content_type: Some(
                "application/dicom, application/dicom+xml, application/dicom+json, or multipart/related",
            ),
        },
        DicomWebRouteCapability {
            path: "/series",
            method: HttpMethod::Get,
            operation: "QIDO all series",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/series",
            method: HttpMethod::Head,
            operation: "QIDO all series",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/instances",
            method: HttpMethod::Get,
            operation: "QIDO all instances",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/instances",
            method: HttpMethod::Head,
            operation: "QIDO all instances",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}",
            method: HttpMethod::Post,
            operation: "STOW scoped by study UID",
            required_feature: "stow",
            state: stow,
            content_type: Some(
                "application/dicom, application/dicom+xml, application/dicom+json, or multipart/related",
            ),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}",
            method: HttpMethod::Get,
            operation: "WADO study retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("multipart/related; type=\"application/dicom\""),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}",
            method: HttpMethod::Head,
            operation: "WADO study retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("multipart/related; type=\"application/dicom\""),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}",
            method: HttpMethod::Get,
            operation: "WADO series retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("multipart/related; type=\"application/dicom\""),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}",
            method: HttpMethod::Head,
            operation: "WADO series retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("multipart/related; type=\"application/dicom\""),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series",
            method: HttpMethod::Get,
            operation: "QIDO series by study",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series",
            method: HttpMethod::Head,
            operation: "QIDO series by study",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/instances",
            method: HttpMethod::Get,
            operation: "QIDO instances by study",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/instances",
            method: HttpMethod::Head,
            operation: "QIDO instances by study",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances",
            method: HttpMethod::Get,
            operation: "QIDO instances by study/series",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances",
            method: HttpMethod::Head,
            operation: "QIDO instances by study/series",
            required_feature: "qido",
            state: qido,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}",
            method: HttpMethod::Get,
            operation: "WADO instance retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}",
            method: HttpMethod::Head,
            operation: "WADO instance retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom"),
        },
        DicomWebRouteCapability {
            path: "/wado",
            method: HttpMethod::Get,
            operation: "WADO-URI compatibility retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom"),
        },
        DicomWebRouteCapability {
            path: "/wado",
            method: HttpMethod::Head,
            operation: "WADO-URI compatibility retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/metadata",
            method: HttpMethod::Get,
            operation: "WADO study metadata",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom+json"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/metadata",
            method: HttpMethod::Get,
            operation: "WADO series metadata",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom+json"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/metadata",
            method: HttpMethod::Get,
            operation: "WADO instance metadata",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/dicom+json"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}",
            method: HttpMethod::Get,
            operation: "WADO frame retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/octet-stream"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered",
            method: HttpMethod::Get,
            operation: "WADO rendered instance retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("image/png or image/jpeg"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered",
            method: HttpMethod::Head,
            operation: "WADO rendered instance retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("image/png or image/jpeg"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}/rendered",
            method: HttpMethod::Get,
            operation: "WADO rendered frame retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("image/png or image/jpeg"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata",
            method: HttpMethod::Get,
            operation: "WADO bulkdata retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/octet-stream"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata",
            method: HttpMethod::Head,
            operation: "WADO bulkdata retrieve",
            required_feature: "wado",
            state: wado,
            content_type: Some("application/octet-stream"),
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}",
            method: HttpMethod::Delete,
            operation: "Delete study",
            required_feature: "delete",
            state: delete_state,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}",
            method: HttpMethod::Delete,
            operation: "Delete series",
            required_feature: "delete",
            state: delete_state,
            content_type: None,
        },
        DicomWebRouteCapability {
            path: "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}",
            method: HttpMethod::Delete,
            operation: "Delete instance",
            required_feature: "delete",
            state: delete_state,
            content_type: None,
        },
    ]
}

/// Deterministic HTTP status mapping contract row for DICOMweb diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpStatusContractRow {
    /// Stable error class identifier.
    pub class: &'static str,
    /// HTTP status code returned for this class.
    pub status: u16,
    /// HTTP status label.
    pub label: &'static str,
}

/// Return the deterministic HTTP status contract for DICOMweb runtime errors.
pub fn dicomweb_http_status_contract() -> [HttpStatusContractRow; 6] {
    [
        HttpStatusContractRow {
            class: "limit_exceeded",
            status: 413,
            label: "Payload Too Large",
        },
        HttpStatusContractRow {
            class: "auth_denied",
            status: 403,
            label: "Forbidden",
        },
        HttpStatusContractRow {
            class: "not_found",
            status: 404,
            label: "Not Found",
        },
        HttpStatusContractRow {
            class: "validation_or_decode",
            status: 400,
            label: "Bad Request",
        },
        HttpStatusContractRow {
            class: "integrity",
            status: 409,
            label: "Conflict",
        },
        HttpStatusContractRow {
            class: "io_or_internal",
            status: 500,
            label: "Internal Server Error",
        },
    ]
}

/// Return the pre-read hard cap used by HTTP framing guards.
pub fn http_preread_hard_cap_bytes(limits: &Limits) -> u64 {
    limits
        .max_input_bytes
        .saturating_add(limits.max_string_bytes)
}

/// Translate a structured DICOMweb error to deterministic HTTP status/label output.
pub fn dicomweb_status_for_error(error: &Error) -> (u16, &'static str) {
    match &error.kind {
        ErrorKind::LimitExceeded { .. } => (413, "Payload Too Large"),
        ErrorKind::DecodeError { stage, detail: _ } if stage == "dicom-auth" => (403, "Forbidden"),
        _ if error.code == DICOM_WEB_NOT_FOUND_CODE => (404, "Not Found"),
        ErrorKind::DecodeError { .. }
        | ErrorKind::MissingRequiredTag { .. }
        | ErrorKind::InvalidTagValue { .. }
        | ErrorKind::UnsupportedSopClass { .. }
        | ErrorKind::UnsupportedTransferSyntax { .. }
        | ErrorKind::InvalidGeometry { .. }
        | ErrorKind::InvalidPixelTransform { .. } => (400, "Bad Request"),
        ErrorKind::IntegrityError { .. } => (409, "Conflict"),
        ErrorKind::IoError { .. } | ErrorKind::InternalError { .. } => {
            (500, "Internal Server Error")
        }
    }
}

/// Deterministic STOW preflight result for compatibility checks.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StowPreflight {
    /// Strictly validated Study Instance UID extracted from payload metadata.
    pub study_uid: String,
}

/// Preflight STOW payload bytes for SOP/transfer-syntax compatibility and study context.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub fn stow_preflight_compatibility(bytes: &[u8], limits: &Limits) -> Result<StowPreflight> {
    let study_uid = dicom_storage::extract_study_uid(bytes, limits)?;
    Ok(StowPreflight { study_uid })
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

fn route_state(feature_enabled: bool) -> DicomWebRouteState {
    if feature_enabled {
        DicomWebRouteState::Implemented
    } else {
        DicomWebRouteState::Blocked
    }
}

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

/// Parse an HTTP request (minimal subset) into a `WebRequest`.
pub fn parse_http_request(
    input: &[u8],
    limits: &Limits,
    transport: TransportSecurity,
) -> Result<WebRequest> {
    let (head, body) = split_head_body(input)?;
    enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes)?;

    let head_str = std::str::from_utf8(head)
        .map_err(|_| decode_error("request headers are not valid UTF-8"))?;
    let mut lines = head_str.split('\n');
    let request_line = lines
        .next()
        .ok_or_else(|| decode_error("missing request line"))?
        .trim_end_matches('\r');
    if request_line.is_empty() {
        return Err(decode_error("empty request line"));
    }

    let mut parts = request_line.split_whitespace();
    let method_str = parts
        .next()
        .ok_or_else(|| decode_error("missing HTTP method"))?;
    let uri = parts
        .next()
        .ok_or_else(|| decode_error("missing request URI"))?;

    let method = match method_str {
        "GET" => HttpMethod::Get,
        "HEAD" => HttpMethod::Head,
        "POST" => HttpMethod::Post,
        "DELETE" => HttpMethod::Delete,
        _ => return Err(decode_error("unsupported HTTP method")),
    };

    let (path, query) = split_uri(uri, limits)?;

    let mut headers = Vec::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| decode_error("invalid header line"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        ensure_ascii_graphic(&name, "header name")?;
        ensure_ascii_printable(&value, "header value")?;
        enforce_limit(
            "max_string_bytes",
            name.len() as u64,
            limits.max_string_bytes,
        )?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        )?;
        headers.push(Header { name, value });
    }

    Ok(WebRequest {
        method,
        transport,
        path,
        query,
        headers,
        body: body.to_vec(),
    })
}

/// Route a `WebRequest` into a DICOMweb operation.
pub fn parse_dicomweb_request(
    request: WebRequest,
    limits: &Limits,
    policy: WebPolicy,
) -> Result<DicomWebRequest> {
    enforce_tls_policy(request.transport, policy.tls)?;
    enforce_throttle(policy.throttle)?;
    enforce_limit(
        "max_input_bytes",
        request.body.len() as u64,
        limits.max_input_bytes,
    )?;
    enforce_limit(
        "max_string_bytes",
        request.path.len() as u64,
        limits.max_string_bytes,
    )?;
    ensure_ascii_graphic(&request.path, "path")?;
    validate_query_params(&request.query, limits)?;
    if is_bodyless_method(request.method) && !request.body.is_empty() {
        return Err(decode_error(
            "GET/HEAD/DELETE request must not include a body",
        ));
    }

    #[cfg(feature = "wado")]
    if request.path == "/wado" {
        if !is_query_method(request.method) {
            return Err(decode_error("unsupported DICOMweb method for WADO-URI"));
        }
        let parsed = parse_wado_uri_query(&request.query)?;
        return Ok(DicomWebRequest::WadoInstance {
            study_uid: parsed.study_uid,
            series_uid: parsed.series_uid,
            instance_uid: parsed.instance_uid,
            transfer_syntax_uid: parsed.transfer_syntax_uid,
            frame_number: None,
        });
    }
    #[cfg(not(feature = "wado"))]
    if request.path == "/wado" {
        return Err(feature_error("wado"));
    }

    let segments = split_path_segments(&request.path)?;
    if segments.is_empty() {
        return Err(decode_error("missing DICOMweb path"));
    }

    match segments.as_slice() {
        ["studies"] => match request.method {
            HttpMethod::Get | HttpMethod::Head => {
                #[cfg(feature = "qido")]
                {
                    Ok(DicomWebRequest::QidoStudies {
                        params: request.query,
                    })
                }
                #[cfg(not(feature = "qido"))]
                {
                    Err(feature_error("qido"))
                }
            }
            HttpMethod::Post => {
                #[cfg(feature = "stow")]
                {
                    if !request.query.is_empty() {
                        return Err(decode_error("query parameters not supported for STOW"));
                    }
                    let content_type = parse_content_type(&request, limits)?;
                    Ok(DicomWebRequest::Stow {
                        study_uid: None,
                        content_type,
                        body: request.body,
                    })
                }
                #[cfg(not(feature = "stow"))]
                {
                    Err(feature_error("stow"))
                }
            }
            HttpMethod::Delete => Err(decode_error("unsupported DICOMweb method for /studies")),
        },
        ["series"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for QIDO"));
            }
            #[cfg(feature = "qido")]
            {
                Ok(DicomWebRequest::QidoAllSeries {
                    params: request.query,
                })
            }
            #[cfg(not(feature = "qido"))]
            {
                Err(feature_error("qido"))
            }
        }
        ["instances"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for QIDO"));
            }
            #[cfg(feature = "qido")]
            {
                Ok(DicomWebRequest::QidoAllInstances {
                    params: request.query,
                })
            }
            #[cfg(not(feature = "qido"))]
            {
                Err(feature_error("qido"))
            }
        }
        ["studies", _study_uid] => match request.method {
            HttpMethod::Post => {
                #[cfg(feature = "stow")]
                {
                    if !request.query.is_empty() {
                        return Err(decode_error("query parameters not supported for STOW"));
                    }
                    let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                    let content_type = parse_content_type(&request, limits)?;
                    Ok(DicomWebRequest::Stow {
                        study_uid: Some(study_uid),
                        content_type,
                        body: request.body,
                    })
                }
                #[cfg(not(feature = "stow"))]
                {
                    Err(feature_error("stow"))
                }
            }
            HttpMethod::Get | HttpMethod::Head => {
                #[cfg(feature = "wado")]
                {
                    let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                    let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                    Ok(DicomWebRequest::WadoStudyRetrieve {
                        study_uid,
                        transfer_syntax_uid,
                    })
                }
                #[cfg(not(feature = "wado"))]
                {
                    Err(feature_error("wado"))
                }
            }
            HttpMethod::Delete => {
                if !policy.delete_enabled {
                    return Err(decode_error("delete routes are not exposed"));
                }
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let hard_delete = parse_delete_mode(&request.query)?;
                Ok(DicomWebRequest::DeleteStudy {
                    study_uid,
                    hard_delete,
                })
            }
        },
        ["studies", _study_uid, "series"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for QIDO"));
            }
            #[cfg(feature = "qido")]
            {
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                Ok(DicomWebRequest::QidoSeries {
                    study_uid,
                    params: request.query,
                })
            }
            #[cfg(not(feature = "qido"))]
            {
                Err(feature_error("qido"))
            }
        }
        ["studies", _study_uid, "instances"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for QIDO"));
            }
            #[cfg(feature = "qido")]
            {
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                Ok(DicomWebRequest::QidoStudyInstances {
                    study_uid,
                    params: request.query,
                })
            }
            #[cfg(not(feature = "qido"))]
            {
                Err(feature_error("qido"))
            }
        }
        ["studies", _study_uid, "series", _series_uid] => match request.method {
            HttpMethod::Delete => {
                if !policy.delete_enabled {
                    return Err(decode_error("delete routes are not exposed"));
                }
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let hard_delete = parse_delete_mode(&request.query)?;
                Ok(DicomWebRequest::DeleteSeries {
                    study_uid,
                    series_uid,
                    hard_delete,
                })
            }
            HttpMethod::Get | HttpMethod::Head => {
                #[cfg(feature = "wado")]
                {
                    let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                    let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                    let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                    Ok(DicomWebRequest::WadoSeriesRetrieve {
                        study_uid,
                        series_uid,
                        transfer_syntax_uid,
                    })
                }
                #[cfg(not(feature = "wado"))]
                {
                    Err(feature_error("wado"))
                }
            }
            HttpMethod::Post => Err(decode_error("unsupported DICOMweb method for path")),
        },
        ["studies", _study_uid, "series", _series_uid, "instances"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for QIDO"));
            }
            #[cfg(feature = "qido")]
            {
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                Ok(DicomWebRequest::QidoInstances {
                    study_uid,
                    series_uid,
                    params: request.query,
                })
            }
            #[cfg(not(feature = "qido"))]
            {
                Err(feature_error("qido"))
            }
        }
        ["studies", _study_uid, "metadata"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for metadata"));
            }
            #[cfg(feature = "wado")]
            {
                let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                Ok(DicomWebRequest::WadoStudyMetadata {
                    study_uid,
                    transfer_syntax_uid,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "metadata"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for metadata"));
            }
            #[cfg(feature = "wado")]
            {
                let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                Ok(DicomWebRequest::WadoSeriesMetadata {
                    study_uid,
                    series_uid,
                    transfer_syntax_uid,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid, "metadata"] => {
            if !is_query_method(request.method) {
                return Err(decode_error("unsupported DICOMweb method for metadata"));
            }
            #[cfg(feature = "wado")]
            {
                let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                Ok(DicomWebRequest::WadoInstanceMetadata {
                    study_uid,
                    series_uid,
                    instance_uid,
                    transfer_syntax_uid,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid, "frames", _frame_number] =>
        {
            if !is_query_method(request.method) {
                return Err(decode_error(
                    "unsupported DICOMweb method for WADO frame retrieve",
                ));
            }
            #[cfg(feature = "wado")]
            {
                let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                let frame_number = parse_frame_number(_frame_number)?;
                Ok(DicomWebRequest::WadoInstance {
                    study_uid,
                    series_uid,
                    instance_uid,
                    transfer_syntax_uid,
                    frame_number: Some(frame_number),
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid, "frames", _frame_number, "rendered"] =>
        {
            if !is_query_method(request.method) {
                return Err(decode_error(
                    "unsupported DICOMweb method for rendered frame retrieve",
                ));
            }
            #[cfg(feature = "wado")]
            {
                let (transfer_syntax_uid, media_type) =
                    parse_wado_rendered_query(&request.query, &request.headers)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                let frame_number = parse_frame_number(_frame_number)?;
                Ok(DicomWebRequest::WadoRendered {
                    study_uid,
                    series_uid,
                    instance_uid,
                    frame_number: Some(frame_number),
                    transfer_syntax_uid,
                    media_type,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid, "rendered"] => {
            if !is_query_method(request.method) {
                return Err(decode_error(
                    "unsupported DICOMweb method for rendered retrieve",
                ));
            }
            #[cfg(feature = "wado")]
            {
                let (transfer_syntax_uid, media_type) =
                    parse_wado_rendered_query(&request.query, &request.headers)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                Ok(DicomWebRequest::WadoRendered {
                    study_uid,
                    series_uid,
                    instance_uid,
                    frame_number: None,
                    transfer_syntax_uid,
                    media_type,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid, "bulkdata"] => {
            if !is_query_method(request.method) {
                return Err(decode_error(
                    "unsupported DICOMweb method for bulkdata retrieve",
                ));
            }
            #[cfg(feature = "wado")]
            {
                let (transfer_syntax_uid, media_type) =
                    parse_wado_bulkdata_query(&request.query, &request.headers)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                Ok(DicomWebRequest::WadoBulkData {
                    study_uid,
                    series_uid,
                    instance_uid,
                    transfer_syntax_uid,
                    media_type,
                })
            }
            #[cfg(not(feature = "wado"))]
            {
                Err(feature_error("wado"))
            }
        }
        ["studies", _study_uid, "series", _series_uid, "instances", _instance_uid] => {
            match request.method {
                HttpMethod::Delete => {
                    if !policy.delete_enabled {
                        return Err(decode_error("delete routes are not exposed"));
                    }
                    let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                    let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                    let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                    let hard_delete = parse_delete_mode(&request.query)?;
                    Ok(DicomWebRequest::DeleteInstance {
                        study_uid,
                        series_uid,
                        instance_uid,
                        hard_delete,
                    })
                }
                HttpMethod::Get | HttpMethod::Head => {
                    #[cfg(feature = "wado")]
                    {
                        let transfer_syntax_uid = parse_wado_retrieve_query(&request.query)?;
                        let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                        let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                        let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                        Ok(DicomWebRequest::WadoInstance {
                            study_uid,
                            series_uid,
                            instance_uid,
                            transfer_syntax_uid,
                            frame_number: None,
                        })
                    }
                    #[cfg(not(feature = "wado"))]
                    {
                        Err(feature_error("wado"))
                    }
                }
                HttpMethod::Post => Err(decode_error("unsupported DICOMweb method for WADO")),
            }
        }
        _ => Err(decode_error("unsupported DICOMweb path")),
    }
}

/// Execute a QIDO-RS query against decoded datasets using `dicom-query`.
#[cfg(feature = "qido")]
pub fn qido_query_matches(
    request: &DicomWebRequest,
    datasets: &[Dataset],
    limits: &Limits,
) -> Result<Vec<QueryMatch>> {
    let (level, params, path_keys): (QueryLevel, &[QueryParam], Vec<(Tag, String)>) = match request
    {
        DicomWebRequest::QidoStudies { params } => (QueryLevel::Study, params, Vec::new()),
        DicomWebRequest::QidoAllSeries { params } => (QueryLevel::Series, params, Vec::new()),
        DicomWebRequest::QidoAllInstances { params } => (QueryLevel::Instance, params, Vec::new()),
        DicomWebRequest::QidoSeries {
            study_uid, params, ..
        } => (
            QueryLevel::Series,
            params,
            vec![(TAG_STUDY_UID, study_uid.clone())],
        ),
        DicomWebRequest::QidoStudyInstances {
            study_uid, params, ..
        } => (
            QueryLevel::Instance,
            params,
            vec![(TAG_STUDY_UID, study_uid.clone())],
        ),
        DicomWebRequest::QidoInstances {
            study_uid,
            series_uid,
            params,
            ..
        } => (
            QueryLevel::Instance,
            params,
            vec![
                (TAG_STUDY_UID, study_uid.clone()),
                (TAG_SERIES_UID, series_uid.clone()),
            ],
        ),
        _ => return Err(decode_error("non-QIDO request for QIDO query handler")),
    };

    let mut keys = Vec::new();
    let mut seen_study: Option<String> = None;
    let mut seen_series: Option<String> = None;
    let mut seen_instance: Option<String> = None;
    let mut seen_modality: Option<String> = None;
    let mut seen_patient_id: Option<String> = None;
    let mut seen_accession_number: Option<String> = None;
    let mut seen_study_date: Option<String> = None;

    let mut insert_key = |tag: Tag, value: &str| -> Result<()> {
        let slot = match tag {
            TAG_STUDY_UID => &mut seen_study,
            TAG_SERIES_UID => &mut seen_series,
            TAG_INSTANCE_UID => &mut seen_instance,
            TAG_MODALITY => &mut seen_modality,
            TAG_PATIENT_ID => &mut seen_patient_id,
            TAG_ACCESSION_NUMBER => &mut seen_accession_number,
            TAG_STUDY_DATE => &mut seen_study_date,
            _ => return Err(decode_error("unsupported query key")),
        };
        match slot {
            Some(existing) => {
                if existing != value
                    && matches!(tag, TAG_STUDY_UID | TAG_SERIES_UID | TAG_INSTANCE_UID)
                {
                    return Err(decode_error("query key conflicts with path UID"));
                }
                Ok(())
            }
            None => {
                *slot = Some(value.to_string());
                keys.push(QueryKey {
                    tag,
                    value: value.to_string(),
                });
                Ok(())
            }
        }
    };

    for (tag, value) in path_keys {
        insert_key(tag, &value)?;
    }

    let mut offset: usize = 0;
    let mut limit: Option<usize> = None;

    for param in params {
        match param.key.to_ascii_lowercase().as_str() {
            "offset" => {
                offset = parse_qido_pagination_value("offset", &param.value, limits)?;
                continue;
            }
            "limit" => {
                limit = Some(parse_qido_pagination_value("limit", &param.value, limits)?);
                continue;
            }
            "includefield" => {
                continue;
            }
            "fuzzy" => {
                let value = param.value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "0" | "1" | "false" | "true") {
                    return Err(decode_error("fuzzy must be one of 0,1,false,true"));
                }
                continue;
            }
            _ => {}
        }

        let tag = normalize_qido_param_key(&param.key)?;
        let supported = match level {
            QueryLevel::Study => matches!(
                tag,
                TAG_STUDY_UID | TAG_PATIENT_ID | TAG_ACCESSION_NUMBER | TAG_STUDY_DATE
            ),
            QueryLevel::Series => matches!(
                tag,
                TAG_STUDY_UID
                    | TAG_SERIES_UID
                    | TAG_PATIENT_ID
                    | TAG_MODALITY
                    | TAG_ACCESSION_NUMBER
                    | TAG_STUDY_DATE
            ),
            QueryLevel::Instance => matches!(
                tag,
                TAG_STUDY_UID
                    | TAG_SERIES_UID
                    | TAG_INSTANCE_UID
                    | TAG_PATIENT_ID
                    | TAG_MODALITY
                    | TAG_ACCESSION_NUMBER
                    | TAG_STUDY_DATE
            ),
        };
        if !supported {
            return Err(decode_error("unsupported query parameter for level"));
        }
        insert_key(tag, &param.value)?;
    }

    let query = Query { level, keys };
    let mut matches = run_query(datasets, &query, limits)?;
    if offset >= matches.len() {
        return Ok(Vec::new());
    }
    if offset > 0 {
        matches.drain(0..offset);
    }
    if let Some(limit) = limit {
        matches.truncate(limit);
    }
    Ok(matches)
}

#[cfg(feature = "qido")]
fn parse_qido_pagination_value(key: &'static str, value: &str, limits: &Limits) -> Result<usize> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| decode_error(format!("{key} must be a non-negative integer")))?;
    enforce_limit(
        "max_dataset_elements",
        parsed as u64,
        limits.max_dataset_elements,
    )?;
    Ok(parsed)
}

#[cfg(feature = "qido")]
fn normalize_qido_param_key(key: &str) -> Result<Tag> {
    let normalized = key
        .trim()
        .chars()
        .filter(|ch| *ch != '_' && !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let by_keyword = match normalized.as_str() {
        "studyinstanceuid" => Some(TAG_STUDY_UID),
        "seriesinstanceuid" => Some(TAG_SERIES_UID),
        "sopinstanceuid" => Some(TAG_INSTANCE_UID),
        "patientid" => Some(TAG_PATIENT_ID),
        "modality" => Some(TAG_MODALITY),
        "accessionnumber" => Some(TAG_ACCESSION_NUMBER),
        "studydate" => Some(TAG_STUDY_DATE),
        _ => None,
    };
    if let Some(tag) = by_keyword {
        return Ok(tag);
    }

    let parsed = Tag::parse_str(key)?;
    match parsed {
        TAG_STUDY_UID | TAG_SERIES_UID | TAG_INSTANCE_UID | TAG_PATIENT_ID | TAG_MODALITY
        | TAG_ACCESSION_NUMBER | TAG_STUDY_DATE => Ok(parsed),
        _ => Err(decode_error("unsupported query parameter")),
    }
}

#[cfg(feature = "wado")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct WadoUriQuery {
    study_uid: String,
    series_uid: String,
    instance_uid: String,
    transfer_syntax_uid: Option<String>,
}

#[cfg(feature = "wado")]
fn parse_wado_uri_query(params: &[QueryParam]) -> Result<WadoUriQuery> {
    let mut request_type: Option<String> = None;
    let mut study_uid: Option<String> = None;
    let mut series_uid: Option<String> = None;
    let mut object_uid: Option<String> = None;
    let mut transfer_syntax_uid: Option<String> = None;

    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        let value = param.value.trim();
        match key.as_str() {
            "requesttype" => request_type = Some(value.to_string()),
            "studyuid" => study_uid = Some(validate_uid(TAG_STUDY_UID, value)?),
            "seriesuid" => series_uid = Some(validate_uid(TAG_SERIES_UID, value)?),
            "objectuid" => object_uid = Some(validate_uid(TAG_INSTANCE_UID, value)?),
            "transfersyntax" => {
                transfer_syntax_uid = Some(parse_transfer_syntax_uid(value)?);
            }
            "contenttype" => {
                if !value.eq_ignore_ascii_case("application/dicom") {
                    return Err(decode_error(
                        "WADO-URI contentType must be application/dicom",
                    ));
                }
            }
            _ => return Err(decode_error("unsupported WADO-URI parameter")),
        }
    }

    match request_type {
        Some(value) if value.eq_ignore_ascii_case("WADO") => {}
        Some(_) => return Err(decode_error("WADO-URI requestType must be WADO")),
        None => return Err(decode_error("WADO-URI requestType is required")),
    }

    Ok(WadoUriQuery {
        study_uid: study_uid.ok_or_else(|| decode_error("WADO-URI studyUID is required"))?,
        series_uid: series_uid.ok_or_else(|| decode_error("WADO-URI seriesUID is required"))?,
        instance_uid: object_uid.ok_or_else(|| decode_error("WADO-URI objectUID is required"))?,
        transfer_syntax_uid,
    })
}

#[cfg(feature = "wado")]
fn parse_wado_retrieve_query(params: &[QueryParam]) -> Result<Option<String>> {
    let mut transfer_syntax_uid = None;
    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        if key != "transfersyntax" {
            return Err(decode_error("unsupported query parameters for WADO"));
        }
        transfer_syntax_uid = Some(parse_transfer_syntax_uid(param.value.trim())?);
    }
    Ok(transfer_syntax_uid)
}

#[cfg(feature = "wado")]
fn parse_wado_rendered_query(
    params: &[QueryParam],
    headers: &[Header],
) -> Result<(Option<String>, String)> {
    parse_wado_media_query(
        params,
        headers,
        &["image/png", "image/jpeg", "application/octet-stream"],
        "image/png",
        "rendered",
    )
}

#[cfg(feature = "wado")]
fn parse_wado_bulkdata_query(
    params: &[QueryParam],
    headers: &[Header],
) -> Result<(Option<String>, String)> {
    parse_wado_media_query(
        params,
        headers,
        &["application/octet-stream", "application/dicom"],
        "application/octet-stream",
        "bulkdata",
    )
}

#[cfg(feature = "wado")]
fn parse_wado_media_query(
    params: &[QueryParam],
    headers: &[Header],
    allowed_media_types: &[&str],
    default_media_type: &str,
    route_label: &str,
) -> Result<(Option<String>, String)> {
    let mut transfer_syntax_uid = None;
    let mut media_type: Option<String> = None;
    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        match key.as_str() {
            "transfersyntax" => {
                transfer_syntax_uid = Some(parse_transfer_syntax_uid(param.value.trim())?);
            }
            "accept" => {
                media_type = Some(parse_wado_media_type(
                    param.value.trim(),
                    allowed_media_types,
                    route_label,
                )?);
            }
            _ => {
                return Err(decode_error(format!(
                    "unsupported query parameters for WADO {route_label}"
                )));
            }
        }
    }
    if media_type.is_none() {
        if let Some(value) = header_value(headers, "accept") {
            let candidate = value.split(',').next().map(str::trim).unwrap_or_default();
            if !candidate.is_empty() {
                media_type = Some(parse_wado_media_type(
                    candidate,
                    allowed_media_types,
                    route_label,
                )?);
            }
        }
    }
    Ok((
        transfer_syntax_uid,
        media_type.unwrap_or_else(|| default_media_type.to_string()),
    ))
}

#[cfg(feature = "wado")]
fn parse_wado_media_type(value: &str, allowed: &[&str], route_label: &str) -> Result<String> {
    ensure_ascii_printable(value, "accept media type")?;
    let normalized = value.to_ascii_lowercase();
    if !allowed.iter().any(|candidate| *candidate == normalized) {
        return Err(decode_error(format!(
            "unsupported media type for {route_label} retrieve"
        )));
    }
    Ok(normalized)
}

fn parse_delete_mode(params: &[QueryParam]) -> Result<bool> {
    let mut hard_delete = false;
    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        if key != "deletemode" {
            return Err(decode_error(
                "unsupported query parameters for delete routes",
            ));
        }
        let mode = param.value.trim().to_ascii_lowercase();
        match mode.as_str() {
            "soft" => hard_delete = false,
            "hard" => hard_delete = true,
            _ => return Err(decode_error("deletemode must be soft or hard")),
        }
    }
    Ok(hard_delete)
}

#[cfg(feature = "wado")]
fn parse_frame_number(value: &str) -> Result<u32> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| decode_error("frame number must be a positive integer"))?;
    if parsed == 0 {
        return Err(decode_error("frame number must be >= 1"));
    }
    Ok(parsed)
}

#[cfg(feature = "wado")]
fn parse_transfer_syntax_uid(value: &str) -> Result<String> {
    validate_uid_strict(TAG_TRANSFER_SYNTAX_UID, value)?;
    if value != TRANSFER_SYNTAX_IMPLICIT_VR_LE && value != TRANSFER_SYNTAX_EXPLICIT_VR_LE {
        return Err(unsupported_transfer_syntax(value));
    }
    Ok(value.to_string())
}

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
        let payloads = stow_payloads(content_type, body, limits)?;
        let preflights: Vec<StowPreflight> = payloads
            .iter()
            .map(|payload| stow_preflight_compatibility(payload, limits))
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
                validate_frame_number(storage, study_uid, series_uid, instance_uid, *frame_number)?;
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
            let bytes = wado_dataset_retrieve_multipart_bytes(storage, Some(study_uid), None)?;
            Ok(DicomWebResponse::WadoMultipart {
                media_type: WADO_MULTIPART_CONTENT_TYPE.to_string(),
                bytes,
            })
        }
        DicomWebRequest::WadoSeriesRetrieve {
            study_uid,
            series_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes =
                wado_dataset_retrieve_multipart_bytes(storage, Some(study_uid), Some(series_uid))?;
            Ok(DicomWebResponse::WadoMultipart {
                media_type: WADO_MULTIPART_CONTENT_TYPE.to_string(),
                bytes,
            })
        }
        DicomWebRequest::WadoStudyMetadata {
            study_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = metadata_response_bytes(storage, Some(study_uid), None, None)?;
            Ok(DicomWebResponse::WadoMetadata { bytes })
        }
        DicomWebRequest::WadoSeriesMetadata {
            study_uid,
            series_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = metadata_response_bytes(storage, Some(study_uid), Some(series_uid), None)?;
            Ok(DicomWebResponse::WadoMetadata { bytes })
        }
        DicomWebRequest::WadoInstanceMetadata {
            study_uid,
            series_uid,
            instance_uid,
            transfer_syntax_uid,
        } => {
            let _ = transfer_syntax_uid;
            let bytes = metadata_response_bytes(
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
                validate_frame_number(storage, study_uid, series_uid, instance_uid, *frame_number)?;
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

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct WebAuthSubject {
    principal: Option<String>,
    peer: Option<String>,
    tenant_id: Option<String>,
    tenant_scope: Option<String>,
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
impl WebAuthSubject {
    fn as_auth_subject(&self) -> AuthSubject<'_> {
        AuthSubject {
            principal: self.principal.as_deref(),
            peer: self.peer.as_deref(),
        }
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn derive_auth_subject(request: &WebRequest) -> WebAuthSubject {
    let principal = header_value(&request.headers, "x-auth-principal")
        .or_else(|| header_value(&request.headers, "x-forwarded-user"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    let peer = header_value(&request.headers, "x-peer-id")
        .or_else(|| header_value(&request.headers, "x-real-ip"))
        .or_else(|| {
            header_value(&request.headers, "x-forwarded-for")
                .and_then(|value| value.split(',').next())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
        .map(|value| value.to_string());
    let tenant_id = header_value(&request.headers, "x-tenant-id")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let tenant_scope = header_value(&request.headers, "x-tenant-scope")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());

    WebAuthSubject {
        principal,
        peer,
        tenant_id,
        tenant_scope,
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn authorize_and_audit_web(
    auth: &WebAuthConfig,
    request: &DicomWebRequest,
    subject: &WebAuthSubject,
) -> Result<()> {
    if request_requires_write(request) {
        enforce_tenant_scope(
            subject.tenant_scope.as_deref(),
            subject.tenant_id.as_deref(),
        )?;
    }
    let auth_request = AuthRequest {
        scope: AuthScope::Dicomweb,
        action: auth_action_for_request(request),
        subject: subject.as_auth_subject(),
        resource: auth_resource_for_request(request),
    };
    let decision = auth.authorizer.authorize(&auth_request)?;
    record_auth_audit(
        &auth.audit,
        auth_request.action,
        auth_request.resource,
        decision,
        subject.tenant_id.as_deref(),
        dicomweb_operation_label(request),
    )?;
    decision.enforce()
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_action_for_request(request: &DicomWebRequest) -> AuthAction {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. }
        | DicomWebRequest::QidoAllSeries { .. }
        | DicomWebRequest::QidoAllInstances { .. }
        | DicomWebRequest::QidoSeries { .. }
        | DicomWebRequest::QidoStudyInstances { .. }
        | DicomWebRequest::QidoInstances { .. } => AuthAction::Query,
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance { .. }
        | DicomWebRequest::WadoStudyRetrieve { .. }
        | DicomWebRequest::WadoSeriesRetrieve { .. }
        | DicomWebRequest::WadoStudyMetadata { .. }
        | DicomWebRequest::WadoSeriesMetadata { .. }
        | DicomWebRequest::WadoInstanceMetadata { .. }
        | DicomWebRequest::WadoRendered { .. }
        | DicomWebRequest::WadoBulkData { .. } => AuthAction::Retrieve,
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { .. } => AuthAction::Store,
        DicomWebRequest::DeleteStudy { .. }
        | DicomWebRequest::DeleteSeries { .. }
        | DicomWebRequest::DeleteInstance { .. } => AuthAction::Delete,
        #[allow(unreachable_patterns)]
        _ => AuthAction::WebRequest,
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_resource_for_request<'a>(request: &'a DicomWebRequest) -> AuthResource<'a> {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllSeries { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllInstances { .. } => AuthResource {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoSeries { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudyInstances { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoInstances {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoInstanceMetadata {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoRendered {
            study_uid,
            series_uid,
            instance_uid,
            ..
        }
        | DicomWebRequest::WadoBulkData {
            study_uid,
            series_uid,
            instance_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: Some(instance_uid.as_str()),
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve { study_uid, .. }
        | DicomWebRequest::WadoStudyMetadata { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
        },
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve {
            study_uid,
            series_uid,
            ..
        }
        | DicomWebRequest::WadoSeriesMetadata {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
        },
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { study_uid, .. } => AuthResource {
            study_uid: study_uid.as_deref(),
            series_uid: None,
            instance_uid: None,
        },
        DicomWebRequest::DeleteStudy { study_uid, .. } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: None,
            instance_uid: None,
        },
        DicomWebRequest::DeleteSeries {
            study_uid,
            series_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: None,
        },
        DicomWebRequest::DeleteInstance {
            study_uid,
            series_uid,
            instance_uid,
            ..
        } => AuthResource {
            study_uid: Some(study_uid.as_str()),
            series_uid: Some(series_uid.as_str()),
            instance_uid: Some(instance_uid.as_str()),
        },
        #[allow(unreachable_patterns)]
        _ => AuthResource::none(),
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn record_auth_audit(
    audit: &Option<AuditCallback>,
    action: AuthAction,
    resource: AuthResource<'_>,
    decision: AuthDecision,
    tenant_id: Option<&str>,
    operation: &'static str,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };

    let mut fields = vec![
        AuditField {
            key: "scope",
            value: AuditValue::Plain("dicomweb".to_string()),
        },
        AuditField {
            key: "protocol_path",
            value: AuditValue::Plain("dicomweb".to_string()),
        },
        AuditField {
            key: "action",
            value: AuditValue::Plain(auth_action_label(action).to_string()),
        },
        AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        },
        AuditField {
            key: "decision",
            value: AuditValue::Plain(auth_decision_label(decision).to_string()),
        },
        AuditField {
            key: "event_code",
            value: AuditValue::Plain(if decision.is_allowed() {
                "auth.allow".to_string()
            } else {
                "auth.deny".to_string()
            }),
        },
    ];
    if let AuthDecision::Deny(reason) = decision {
        fields.push(AuditField {
            key: "deny_reason",
            value: AuditValue::Plain(auth_deny_reason_label(reason).to_string()),
        });
    }
    if let Some(uid) = resource.study_uid {
        fields.push(AuditField {
            key: "study_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(uid) = resource.series_uid {
        fields.push(AuditField {
            key: "series_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(uid) = resource.instance_uid {
        fields.push(AuditField {
            key: "instance_uid",
            value: AuditValue::Sensitive(uid.to_string()),
        });
    }
    if let Some(tenant_id) = tenant_id {
        fields.push(AuditField {
            key: "tenant",
            value: AuditValue::Plain(tenant_id.to_string()),
        });
    }

    callback(AuditEvent {
        kind: AuditEventKind::AuthzDecision,
        fields,
    })
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn dicomweb_operation_label(request: &DicomWebRequest) -> &'static str {
    match request {
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudies { .. } => "qido.studies",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllSeries { .. } => "qido.all_series",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoAllInstances { .. } => "qido.all_instances",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoSeries { .. } => "qido.series",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoStudyInstances { .. } => "qido.study_instances",
        #[cfg(feature = "qido")]
        DicomWebRequest::QidoInstances { .. } => "qido.instances",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            frame_number: Some(_),
            ..
        } => "wado.frame",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.instance.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstance { .. } => "wado.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.study.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyRetrieve { .. } => "wado.study",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve {
            transfer_syntax_uid: Some(_),
            ..
        } => "wado.series.transfer_syntax",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesRetrieve { .. } => "wado.series",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoStudyMetadata { .. } => "wado.metadata.study",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoSeriesMetadata { .. } => "wado.metadata.series",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoInstanceMetadata { .. } => "wado.metadata.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoRendered {
            frame_number: Some(_),
            ..
        } => "wado.rendered.frame",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoRendered { .. } => "wado.rendered.instance",
        #[cfg(feature = "wado")]
        DicomWebRequest::WadoBulkData { .. } => "wado.bulkdata",
        #[cfg(feature = "stow")]
        DicomWebRequest::Stow { .. } => "stow.studies",
        DicomWebRequest::DeleteStudy { .. } => "delete.study",
        DicomWebRequest::DeleteSeries { .. } => "delete.series",
        DicomWebRequest::DeleteInstance { .. } => "delete.instance",
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_action_label(action: AuthAction) -> &'static str {
    match action {
        AuthAction::Associate => "associate",
        AuthAction::Command => "command",
        AuthAction::Query => "query",
        AuthAction::Retrieve => "retrieve",
        AuthAction::Store => "store",
        AuthAction::Delete => "delete",
        AuthAction::Echo => "echo",
        AuthAction::WebRequest => "web_request",
        AuthAction::StorageCommitment => "storage_commitment",
        AuthAction::Ups => "ups",
        AuthAction::Ian => "ian",
        AuthAction::ViewerMeasurementWrite => "viewer_measurement_write",
        AuthAction::ViewerSegmentationWrite => "viewer_segmentation_write",
        AuthAction::ViewerOverlayWrite => "viewer_overlay_write",
        AuthAction::ViewerAnnotationWrite => "viewer_annotation_write",
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_decision_label(decision: AuthDecision) -> &'static str {
    match decision {
        AuthDecision::Allow => "allow",
        AuthDecision::Deny(_) => "deny",
    }
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn auth_deny_reason_label(reason: AuthDenyReason) -> &'static str {
    match reason {
        AuthDenyReason::Unauthenticated => "unauthenticated",
        AuthDenyReason::Unauthorized => "unauthorized",
        AuthDenyReason::Policy => "policy",
    }
}

fn split_head_body(input: &[u8]) -> Result<(&[u8], &[u8])> {
    if let Some(pos) = input.windows(4).position(|w| w == b"\r\n\r\n") {
        let head = &input[..pos];
        let body = &input[pos + 4..];
        return Ok((head, body));
    }
    if let Some(pos) = input.windows(2).position(|w| w == b"\n\n") {
        let head = &input[..pos];
        let body = &input[pos + 2..];
        return Ok((head, body));
    }
    Err(decode_error("missing HTTP header terminator"))
}

fn split_uri(uri: &str, limits: &Limits) -> Result<(String, Vec<QueryParam>)> {
    ensure_ascii_graphic(uri, "URI")?;
    enforce_limit(
        "max_string_bytes",
        uri.len() as u64,
        limits.max_string_bytes,
    )?;
    if !uri.starts_with('/') {
        return Err(decode_error("URI must start with '/'"));
    }
    let (path, query) = match uri.split_once('?') {
        Some((path, query)) => (path, query),
        None => (uri, ""),
    };
    let params = if query.is_empty() {
        Vec::new()
    } else {
        parse_query(query, limits)?
    };
    Ok((path.to_string(), params))
}

fn parse_query(query: &str, limits: &Limits) -> Result<Vec<QueryParam>> {
    let mut params = Vec::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            return Err(decode_error("empty query parameter"));
        }
        let (key, value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        if key.is_empty() {
            return Err(decode_error("query parameter key is empty"));
        }
        ensure_ascii_graphic(key, "query key")?;
        ensure_ascii_graphic(value, "query value")?;
        enforce_limit(
            "max_string_bytes",
            key.len() as u64,
            limits.max_string_bytes,
        )?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        )?;
        params.push(QueryParam {
            key: key.to_string(),
            value: value.to_string(),
        });
        if params.len() as u64 > limits.max_dataset_elements {
            return Err(limit_exceeded(
                "max_dataset_elements",
                params.len() as u64,
                limits.max_dataset_elements,
            ));
        }
    }
    Ok(params)
}

fn split_path_segments(path: &str) -> Result<Vec<&str>> {
    if !path.starts_with('/') {
        return Err(decode_error("path must start with '/'"));
    }
    Ok(path.split('/').filter(|s| !s.is_empty()).collect())
}

fn validate_query_params(params: &[QueryParam], limits: &Limits) -> Result<()> {
    if params.len() as u64 > limits.max_dataset_elements {
        return Err(limit_exceeded(
            "max_dataset_elements",
            params.len() as u64,
            limits.max_dataset_elements,
        ));
    }
    for param in params {
        ensure_ascii_graphic(&param.key, "query key")?;
        ensure_ascii_graphic(&param.value, "query value")?;
        enforce_limit(
            "max_string_bytes",
            param.key.len() as u64,
            limits.max_string_bytes,
        )?;
        enforce_limit(
            "max_string_bytes",
            param.value.len() as u64,
            limits.max_string_bytes,
        )?;
    }
    Ok(())
}

#[cfg(feature = "stow")]
fn parse_content_type(request: &WebRequest, limits: &Limits) -> Result<DicomWebContentType> {
    let value = header_value(&request.headers, "content-type")
        .ok_or_else(|| decode_error("missing content-type header"))?;
    enforce_limit(
        "max_string_bytes",
        value.len() as u64,
        limits.max_string_bytes,
    )?;
    ensure_ascii_printable(value, "content-type")?;

    let mut parts = value.split(';');
    let base = parts
        .next()
        .ok_or_else(|| decode_error("invalid content-type"))?
        .trim()
        .to_ascii_lowercase();

    if base == "application/dicom" {
        return Ok(DicomWebContentType::ApplicationDicom);
    }
    if base == "application/dicom+xml" {
        return Ok(DicomWebContentType::ApplicationDicomXml);
    }
    if base == "application/dicom+json" {
        return Ok(DicomWebContentType::ApplicationDicomJson);
    }
    if base != "multipart/related" {
        return Err(decode_error("unsupported content-type"));
    }

    let mut boundary = None;
    let mut related_type = None;
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| decode_error("invalid content-type parameter"))?;
        let key = key.trim().to_ascii_lowercase();
        let mut value = value.trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = &value[1..value.len() - 1];
        }
        ensure_ascii_printable(value, "content-type parameter")?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        )?;
        match key.as_str() {
            "boundary" => boundary = Some(value.to_string()),
            "type" => related_type = Some(value.to_string()),
            _ => {}
        }
    }

    let related_type = related_type
        .ok_or_else(|| decode_error("multipart/related missing type"))?
        .to_ascii_lowercase();
    if !matches!(
        related_type.as_str(),
        "application/dicom" | "application/dicom+xml" | "application/dicom+json"
    ) {
        return Err(decode_error(
            "multipart/related type must be application/dicom, application/dicom+xml, or application/dicom+json",
        ));
    }
    let boundary = boundary.ok_or_else(|| decode_error("multipart/related missing boundary"))?;
    if boundary.is_empty() {
        return Err(decode_error("multipart/related boundary is empty"));
    }

    Ok(DicomWebContentType::MultipartRelated { boundary })
}

#[cfg(feature = "stow")]
fn stow_payloads(
    content_type: &DicomWebContentType,
    body: &[u8],
    limits: &Limits,
) -> Result<Vec<Vec<u8>>> {
    match content_type {
        DicomWebContentType::ApplicationDicom => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes)?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::ApplicationDicomXml => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes)?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::ApplicationDicomJson => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes)?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::MultipartRelated { boundary } => {
            parse_stow_multipart_related(body, boundary, limits)
        }
    }
}

#[cfg(feature = "stow")]
fn parse_stow_multipart_related(
    body: &[u8],
    boundary: &str,
    limits: &Limits,
) -> Result<Vec<Vec<u8>>> {
    let delimiter = format!("--{boundary}").into_bytes();
    let mut cursor = skip_optional_line_ending(body, 0);
    if !body[cursor..].starts_with(&delimiter) {
        return Err(decode_error("multipart body missing opening boundary"));
    }
    cursor += delimiter.len();
    cursor = consume_required_line_ending(body, cursor, "multipart boundary line")?;

    let mut parts = Vec::new();
    loop {
        let (head_end, body_start) = part_header_offsets(body, cursor)?;
        validate_multipart_part_headers(&body[cursor..head_end], limits)?;

        let (part_end, boundary_marker_start) =
            find_next_multipart_boundary(body, body_start, &delimiter)
                .ok_or_else(|| decode_error("multipart body missing closing boundary"))?;
        enforce_limit(
            "max_input_bytes",
            (part_end - body_start) as u64,
            limits.max_input_bytes,
        )?;
        if part_end == body_start {
            return Err(decode_error("multipart part payload is empty"));
        }
        parts.push(body[body_start..part_end].to_vec());
        if parts.len() as u64 > limits.max_dataset_elements {
            return Err(limit_exceeded(
                "max_dataset_elements",
                parts.len() as u64,
                limits.max_dataset_elements,
            ));
        }

        cursor = boundary_marker_start + delimiter.len();
        if body[cursor..].starts_with(b"--") {
            cursor += 2;
            let cursor = skip_optional_line_ending(body, cursor);
            if cursor != body.len() {
                return Err(decode_error(
                    "unexpected bytes after multipart closing boundary",
                ));
            }
            break;
        }
        cursor = consume_required_line_ending(body, cursor, "multipart boundary line")?;
    }

    if parts.is_empty() {
        return Err(decode_error("multipart body does not contain DICOM parts"));
    }
    Ok(parts)
}

#[cfg(feature = "stow")]
fn validate_multipart_part_headers(headers: &[u8], limits: &Limits) -> Result<()> {
    let head_str = std::str::from_utf8(headers)
        .map_err(|_| decode_error("multipart part headers are not valid UTF-8"))?;
    let mut content_type_seen = false;
    for line in head_str.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| decode_error("invalid multipart part header line"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        )?;
        if name == "content-type" {
            content_type_seen = true;
            let base = value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if !matches!(
                base.as_str(),
                "application/dicom" | "application/dicom+xml" | "application/dicom+json"
            ) {
                return Err(decode_error(
                    "multipart part content-type must be application/dicom, application/dicom+xml, or application/dicom+json",
                ));
            }
        }
    }
    if !content_type_seen {
        return Err(decode_error("multipart part missing content-type header"));
    }
    Ok(())
}

#[cfg(feature = "stow")]
fn part_header_offsets(buffer: &[u8], start: usize) -> Result<(usize, usize)> {
    if let Some(pos) = find_subslice(buffer, start, b"\r\n\r\n") {
        return Ok((pos, pos + 4));
    }
    if let Some(pos) = find_subslice(buffer, start, b"\n\n") {
        return Ok((pos, pos + 2));
    }
    Err(decode_error("multipart part missing header terminator"))
}

#[cfg(feature = "stow")]
fn find_next_multipart_boundary(
    buffer: &[u8],
    start: usize,
    delimiter: &[u8],
) -> Option<(usize, usize)> {
    let mut crlf_pattern = Vec::with_capacity(delimiter.len() + 2);
    crlf_pattern.extend_from_slice(b"\r\n");
    crlf_pattern.extend_from_slice(delimiter);
    let crlf = find_subslice(buffer, start, &crlf_pattern).map(|pos| (pos, pos + 2));

    let mut lf_pattern = Vec::with_capacity(delimiter.len() + 1);
    lf_pattern.extend_from_slice(b"\n");
    lf_pattern.extend_from_slice(delimiter);
    let lf = find_subslice(buffer, start, &lf_pattern).map(|pos| (pos, pos + 1));

    match (crlf, lf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(feature = "stow")]
fn find_subslice(buffer: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || start >= buffer.len() || needle.len() > buffer.len() - start {
        return None;
    }
    buffer[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

#[cfg(feature = "stow")]
fn skip_optional_line_ending(buffer: &[u8], start: usize) -> usize {
    if buffer.get(start..start + 2) == Some(b"\r\n") {
        return start + 2;
    }
    if buffer.get(start) == Some(&b'\n') {
        return start + 1;
    }
    start
}

#[cfg(feature = "stow")]
fn consume_required_line_ending(buffer: &[u8], start: usize, context: &str) -> Result<usize> {
    if buffer.get(start..start + 2) == Some(b"\r\n") {
        return Ok(start + 2);
    }
    if buffer.get(start) == Some(&b'\n') {
        return Ok(start + 1);
    }
    Err(decode_error(format!(
        "expected line terminator after {context}"
    )))
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    let name = name.to_ascii_lowercase();
    headers
        .iter()
        .find(|header| header.name == name)
        .map(|header| header.value.as_str())
}

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
fn validate_uid(tag: Tag, value: &str) -> Result<String> {
    validate_uid_strict(tag, value)?;
    Ok(value.to_string())
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

#[cfg(feature = "wado")]
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
#[cfg(feature = "wado")]
const WADO_MULTIPART_BOUNDARY: &str = "dicomweb-dataset";
#[cfg(feature = "wado")]
const WADO_MULTIPART_CONTENT_TYPE: &str =
    "multipart/related; type=\"application/dicom\"; boundary=\"dicomweb-dataset\"";

#[cfg(feature = "wado")]
fn validate_frame_number(
    storage: &Storage,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
    frame_number: u32,
) -> Result<()> {
    let max_frames = instance_number_of_frames(storage, study_uid, series_uid, instance_uid)?;
    if frame_number > max_frames {
        return Err(decode_error("frame number exceeds NumberOfFrames"));
    }
    Ok(())
}

#[cfg(feature = "wado")]
fn instance_number_of_frames(
    storage: &Storage,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
) -> Result<u32> {
    let datasets = storage.datasets()?;
    let dataset = datasets.into_iter().find(|dataset| {
        dataset.get_uid(TAG_STUDY_UID) == Some(study_uid)
            && dataset.get_uid(TAG_SERIES_UID) == Some(series_uid)
            && dataset.get_uid(TAG_INSTANCE_UID) == Some(instance_uid)
    });
    let Some(dataset) = dataset else {
        return Err(not_found_error("requested WADO instance not found"));
    };
    if let Some(value) = dataset.get_i32(TAG_NUMBER_OF_FRAMES) {
        if value <= 0 {
            return Err(decode_error("NumberOfFrames must be >= 1"));
        }
        return Ok(value as u32);
    }
    if let Some(value) = dataset.get_str(TAG_NUMBER_OF_FRAMES) {
        let parsed = value
            .trim()
            .parse::<u32>()
            .map_err(|_| decode_error("invalid NumberOfFrames"))?;
        if parsed == 0 {
            return Err(decode_error("NumberOfFrames must be >= 1"));
        }
        return Ok(parsed);
    }
    Ok(1)
}

#[cfg(feature = "wado")]
fn wado_dataset_retrieve_multipart_bytes(
    storage: &Storage,
    study_uid: Option<&str>,
    series_uid: Option<&str>,
) -> Result<Vec<u8>> {
    let datasets = storage.datasets()?;
    let mut out = Vec::new();
    let mut parts = 0usize;
    for dataset in datasets {
        let Some(current_study) = dataset.get_uid(TAG_STUDY_UID) else {
            continue;
        };
        let Some(current_series) = dataset.get_uid(TAG_SERIES_UID) else {
            continue;
        };
        let Some(current_instance) = dataset.get_uid(TAG_INSTANCE_UID) else {
            continue;
        };
        if let Some(expected_study) = study_uid {
            if current_study != expected_study {
                continue;
            }
        }
        if let Some(expected_series) = series_uid {
            if current_series != expected_series {
                continue;
            }
        }
        let Some(bytes) = storage.instance_bytes(current_study, current_series, current_instance)
        else {
            continue;
        };
        out.extend_from_slice(format!("--{WADO_MULTIPART_BOUNDARY}\r\n").as_bytes());
        out.extend_from_slice(b"content-type: application/dicom\r\n\r\n");
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
        parts += 1;
    }
    if parts == 0 {
        return Err(not_found_error("requested WADO retrieve not found"));
    }
    out.extend_from_slice(format!("--{WADO_MULTIPART_BOUNDARY}--\r\n").as_bytes());
    Ok(out)
}

#[cfg(feature = "wado")]
fn metadata_response_bytes(
    storage: &Storage,
    study_uid: Option<&str>,
    series_uid: Option<&str>,
    instance_uid: Option<&str>,
) -> Result<Vec<u8>> {
    let datasets = storage.datasets()?;
    let mut matched = Vec::new();
    for dataset in datasets {
        let Some(current_study) = dataset.get_uid(TAG_STUDY_UID) else {
            continue;
        };
        let Some(current_series) = dataset.get_uid(TAG_SERIES_UID) else {
            continue;
        };
        let Some(current_instance) = dataset.get_uid(TAG_INSTANCE_UID) else {
            continue;
        };
        if let Some(expected_study) = study_uid {
            if current_study != expected_study {
                continue;
            }
        }
        if let Some(expected_series) = series_uid {
            if current_series != expected_series {
                continue;
            }
        }
        if let Some(expected_instance) = instance_uid {
            if current_instance != expected_instance {
                continue;
            }
        }
        matched.push(dataset);
    }
    if matched.is_empty() {
        return Err(not_found_error("requested WADO metadata not found"));
    }
    Ok(render_metadata_json(&matched).into_bytes())
}

#[cfg(feature = "wado")]
fn render_metadata_json(datasets: &[dicom_core::Dataset]) -> String {
    let mut out = String::from("[");
    for (index, dataset) in datasets.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut elements = dataset.elements().iter().collect::<Vec<_>>();
        elements.sort_by_key(|element| element.tag.as_u32());
        for (element_index, element) in elements.iter().enumerate() {
            if element_index > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(&format!("{:04X}{:04X}", element.tag.0, element.tag.1));
            out.push_str("\":{");
            out.push_str("\"vr\":\"");
            let vr_text = String::from_utf8_lossy(&element.vr.as_bytes()).to_string();
            out.push_str(&escape_json_string(&vr_text));
            out.push('"');
            match &element.value {
                dicom_core::Value::Empty => {}
                dicom_core::Value::Str(value) | dicom_core::Value::Uid(value) => {
                    out.push_str(",\"Value\":[\"");
                    out.push_str(&escape_json_string(value));
                    out.push_str("\"]");
                }
                dicom_core::Value::I32(value) => {
                    out.push_str(&format!(",\"Value\":[{value}]"));
                }
                dicom_core::Value::F64(value) => {
                    out.push_str(&format!(",\"Value\":[{}]", value));
                }
                dicom_core::Value::Bytes(value) => {
                    let hex = value
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<String>();
                    out.push_str(",\"InlineBinary\":\"");
                    out.push_str(&hex);
                    out.push('"');
                }
                dicom_core::Value::Sequence(sequence) => {
                    out.push_str(",\"Value\":[");
                    let nested = render_metadata_json(sequence);
                    out.push_str(nested.trim_start_matches('[').trim_end_matches(']'));
                    out.push(']');
                }
            }
            out.push('}');
        }
        out.push('}');
    }
    out.push(']');
    out
}

#[cfg(feature = "wado")]
fn escape_json_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

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
    Error::new(
        DICOM_WEB_NOT_FOUND_CODE,
        ErrorKind::DecodeError {
            stage: "dicom-web".to_string(),
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
fn feature_error(feature: &str) -> Box<Error> {
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

fn is_query_method(method: HttpMethod) -> bool {
    matches!(method, HttpMethod::Get | HttpMethod::Head)
}

fn is_bodyless_method(method: HttpMethod) -> bool {
    matches!(
        method,
        HttpMethod::Get | HttpMethod::Head | HttpMethod::Delete
    )
}

fn enforce_tls_policy(transport: TransportSecurity, policy: TlsPolicy) -> Result<()> {
    if policy == TlsPolicy::RequireTls && transport != TransportSecurity::Tls {
        return Err(decode_error("TLS required for DICOMweb request"));
    }
    Ok(())
}

fn enforce_throttle(decision: ThrottleDecision) -> Result<()> {
    match decision {
        ThrottleDecision::Allow => Ok(()),
        ThrottleDecision::Reject {
            limit_name,
            observed,
            allowed,
        } => Err(limit_exceeded(limit_name, observed, allowed)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_http_rejects_long_query_keys() {
        // REQ-HTTP-301: URI and query parameter lengths are bounded by max_string_bytes.
        let limits = Limits {
            max_string_bytes: 4,
            ..Limits::default()
        };
        let data = b"GET /studies?ABCDE=1\n\n";
        let err =
            parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_string_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn parse_http_rejects_excess_query_params() {
        // REQ-HTTP-301: Query parameter count is bounded by max_dataset_elements.
        let limits = Limits {
            max_dataset_elements: 1,
            ..Limits::default()
        };
        let data = b"GET /studies?A=1&B=2\n\n";
        let err =
            parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_dataset_elements");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn get_with_body_rejected() {
        // REQ-HTTP-302: GET/HEAD requests must not include a body.
        let req = request(HttpMethod::Get, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn head_with_body_rejected() {
        // REQ-HTTP-302: GET/HEAD requests must not include a body.
        let req = request(HttpMethod::Head, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn tls_policy_requires_tls() {
        // REQ-HTTP-303: TLS policy must fail closed on insecure transports.
        let req = request(HttpMethod::Get, "/studies", &[]);
        let policy = WebPolicy::new(TlsPolicy::RequireTls, ThrottleDecision::Allow);
        let err = parse_dicomweb_request(req, &limits(), policy).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        match err.kind {
            ErrorKind::LimitExceeded {
                limit_name,
                observed,
                allowed,
            } => {
                assert_eq!(limit_name, "max_web_requests_inflight");
                assert_eq!(observed, 2);
                assert_eq!(allowed, 1);
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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });
        let mut dataset_other = Dataset::new();
        dataset_other.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1.1".to_string()),
        });
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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_A".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_MODALITY,
            vr: Vr::Cs,
            value: Value::Str("CT".to_string()),
        });
        let mut dataset_other = Dataset::new();
        dataset_other.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.9".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.9.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_A".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_MODALITY,
            vr: Vr::Cs,
            value: Value::Str("MR".to_string()),
        });
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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Lo,
            value: Value::Str("ACC123".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_STUDY_DATE,
            vr: Vr::Da,
            value: Value::Str("20260211".to_string()),
        });

        let mut dataset_other = Dataset::new();
        dataset_other.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Lo,
            value: Value::Str("ACC999".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_STUDY_DATE,
            vr: Vr::Da,
            value: Value::Str("20260101".to_string()),
        });
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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });

        let mut dataset_same_study = Dataset::new();
        dataset_same_study.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset_same_study.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.9".to_string()),
        });
        dataset_same_study.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.9.1".to_string()),
        });

        let mut dataset_other_study = Dataset::new();
        dataset_other_study.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        dataset_other_study.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1".to_string()),
        });
        dataset_other_study.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1.1".to_string()),
        });

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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_MODALITY,
            vr: Vr::Cs,
            value: Value::Str("CT".to_string()),
        });

        let mut dataset_other = Dataset::new();
        dataset_other.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_MODALITY,
            vr: Vr::Cs,
            value: Value::Str("MR".to_string()),
        });

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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4.5".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_A".to_string()),
        });

        let mut dataset_other = Dataset::new();
        dataset_other.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9.1.1".to_string()),
        });
        dataset_other.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_B".to_string()),
        });

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
            err.kind,
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "qido")]
    #[test]
    fn qido_limit_and_offset_are_applied_deterministically() {
        let mut dataset_a = Dataset::new();
        dataset_a.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset_a.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.1".to_string()),
        });
        dataset_a.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.1.1".to_string()),
        });

        let mut dataset_b = Dataset::new();
        dataset_b.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.4".to_string()),
        });
        dataset_b.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.4.1".to_string()),
        });
        dataset_b.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.4.1.1".to_string()),
        });

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
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.1.1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_A".to_string()),
        });

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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[cfg(feature = "stow")]
    #[test]
    fn stow_requires_content_type() {
        // REQ-WEB-302: STOW-RS requires a valid content-type.
        let mut req = request(HttpMethod::Post, "/studies", &[1, 2]);
        req.headers.clear();
        let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn body_limit_enforced() {
        // REQ-WEB-303: Request bodies are bounded by max_input_bytes.
        let limits = Limits {
            max_input_bytes: 1,
            ..Limits::default()
        };
        let req = request(HttpMethod::Post, "/studies", &[1, 2, 3]);
        let err = parse_dicomweb_request(req, &limits, policy()).expect_err("error");
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_input_bytes");
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(
            err.kind,
            ErrorKind::DecodeError { ref stage, .. } if stage == "dicom-auth"
        ));

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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
        assert_eq!(err.code, DICOM_WEB_NOT_FOUND_CODE);
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
        assert!(matches!(err.kind, ErrorKind::IntegrityError { .. }));
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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
        assert!(matches!(disabled.kind, ErrorKind::DecodeError { .. }));

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
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
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
