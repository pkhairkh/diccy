//! HTTP routing and content negotiation types and functions.

use std::collections::BTreeMap;

use dicom_core::{Error, ErrorKind, Limits, Result};

use super::{
    decode_error, enforce_limit, ensure_ascii_graphic, ensure_ascii_printable, limit_exceeded,
    DicomWebRequest, Header, HttpMethod, QueryParam, ThrottleDecision, TlsPolicy,
    TransportSecurity, WebPolicy, WebRequest,
};

#[cfg(any(not(feature = "qido"), not(feature = "wado"), not(feature = "stow")))]
use super::feature_error;

use super::validate_uid;

use super::TAG_INSTANCE_UID;
use super::TAG_SERIES_UID;
use super::TAG_STUDY_UID;

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

/// Named DICOMweb route identifiers, one per distinct operation/method pair.
///
/// Replaces the previous fixed-size `[DicomWebRouteCapability; 34]` array with a
/// type-safe enum that makes the route set explicit and extensible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DicomWebRoute {
    // QIDO-RS
    /// QIDO-RS: search for studies (GET).
    QidoStudiesGet,
    /// QIDO-RS: search for studies (HEAD).
    QidoStudiesHead,
    /// STOW-RS: store instances (POST /studies).
    StowStudiesPost,
    /// QIDO-RS: search all series (GET).
    QidoAllSeriesGet,
    /// QIDO-RS: search all series (HEAD).
    QidoAllSeriesHead,
    /// QIDO-RS: search all instances (GET).
    QidoAllInstancesGet,
    /// QIDO-RS: search all instances (HEAD).
    QidoAllInstancesHead,
    /// STOW-RS: store instances scoped by study (POST /studies/{StudyUID}).
    StowStudyScopedPost,
    // WADO-RS
    /// WADO-RS: retrieve all instances in a study (GET).
    WadoStudyRetrieveGet,
    /// WADO-RS: retrieve all instances in a study (HEAD).
    WadoStudyRetrieveHead,
    /// WADO-RS: retrieve all instances in a series (GET).
    WadoSeriesRetrieveGet,
    /// WADO-RS: retrieve all instances in a series (HEAD).
    WadoSeriesRetrieveHead,
    /// WADO-RS: retrieve a single instance (GET).
    WadoInstanceRetrieveGet,
    /// WADO-RS: retrieve a single instance (HEAD).
    WadoInstanceRetrieveHead,
    /// WADO-URI: legacy retrieve (GET).
    WadoUriGet,
    /// WADO-URI: legacy retrieve (HEAD).
    WadoUriHead,
    /// WADO-RS: retrieve study-level metadata (GET).
    WadoStudyMetadataGet,
    /// WADO-RS: retrieve series-level metadata (GET).
    WadoSeriesMetadataGet,
    /// WADO-RS: retrieve instance-level metadata (GET).
    WadoInstanceMetadataGet,
    /// WADO-RS: retrieve a single frame (GET).
    WadoFrameRetrieveGet,
    /// WADO-RS: retrieve rendered instance (GET).
    WadoRenderedInstanceGet,
    /// WADO-RS: retrieve rendered instance (HEAD).
    WadoRenderedInstanceHead,
    /// WADO-RS: retrieve rendered frame (GET).
    WadoRenderedFrameGet,
    /// WADO-RS: retrieve bulkdata (GET).
    WadoBulkDataGet,
    /// WADO-RS: retrieve bulkdata (HEAD).
    WadoBulkDataHead,
    // QIDO-RS (scoped)
    /// QIDO-RS: search series within a study (GET).
    QidoSeriesByStudyGet,
    /// QIDO-RS: search series within a study (HEAD).
    QidoSeriesByStudyHead,
    /// QIDO-RS: search instances within a study (GET).
    QidoStudyInstancesGet,
    /// QIDO-RS: search instances within a study (HEAD).
    QidoStudyInstancesHead,
    /// QIDO-RS: search instances within a series (GET).
    QidoInstancesByStudySeriesGet,
    /// QIDO-RS: search instances within a series (HEAD).
    QidoInstancesByStudySeriesHead,
    // DELETE
    /// Delete an entire study.
    DeleteStudy,
    /// Delete a series within a study.
    DeleteSeries,
    /// Delete a single instance.
    DeleteInstance,
}

impl DicomWebRoute {
    /// Path pattern for this route.
    pub fn path(self) -> &'static str {
        match self {
            Self::QidoStudiesGet | Self::QidoStudiesHead | Self::StowStudiesPost => "/studies",
            Self::QidoAllSeriesGet | Self::QidoAllSeriesHead => "/series",
            Self::QidoAllInstancesGet | Self::QidoAllInstancesHead => "/instances",
            Self::StowStudyScopedPost => "/studies/{StudyUID}",
            Self::WadoStudyRetrieveGet | Self::WadoStudyRetrieveHead => "/studies/{StudyUID}",
            Self::WadoSeriesRetrieveGet | Self::WadoSeriesRetrieveHead => "/studies/{StudyUID}/series/{SeriesUID}",
            Self::WadoInstanceRetrieveGet | Self::WadoInstanceRetrieveHead => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}",
            Self::WadoUriGet | Self::WadoUriHead => "/wado",
            Self::WadoStudyMetadataGet => "/studies/{StudyUID}/metadata",
            Self::WadoSeriesMetadataGet => "/studies/{StudyUID}/series/{SeriesUID}/metadata",
            Self::WadoInstanceMetadataGet => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/metadata",
            Self::WadoFrameRetrieveGet => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}",
            Self::WadoRenderedInstanceGet | Self::WadoRenderedInstanceHead => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered",
            Self::WadoRenderedFrameGet => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}/rendered",
            Self::WadoBulkDataGet | Self::WadoBulkDataHead => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata",
            Self::QidoSeriesByStudyGet | Self::QidoSeriesByStudyHead => "/studies/{StudyUID}/series",
            Self::QidoStudyInstancesGet | Self::QidoStudyInstancesHead => "/studies/{StudyUID}/instances",
            Self::QidoInstancesByStudySeriesGet | Self::QidoInstancesByStudySeriesHead => "/studies/{StudyUID}/series/{SeriesUID}/instances",
            Self::DeleteStudy => "/studies/{StudyUID}",
            Self::DeleteSeries => "/studies/{StudyUID}/series/{SeriesUID}",
            Self::DeleteInstance => "/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}",
        }
    }

    /// HTTP method for this route.
    pub fn method(self) -> HttpMethod {
        match self {
            Self::QidoStudiesGet
            | Self::QidoAllSeriesGet
            | Self::QidoAllInstancesGet
            | Self::WadoStudyRetrieveGet
            | Self::WadoSeriesRetrieveGet
            | Self::WadoInstanceRetrieveGet
            | Self::WadoUriGet
            | Self::WadoStudyMetadataGet
            | Self::WadoSeriesMetadataGet
            | Self::WadoInstanceMetadataGet
            | Self::WadoFrameRetrieveGet
            | Self::WadoRenderedInstanceGet
            | Self::WadoRenderedFrameGet
            | Self::WadoBulkDataGet
            | Self::QidoSeriesByStudyGet
            | Self::QidoStudyInstancesGet
            | Self::QidoInstancesByStudySeriesGet => HttpMethod::Get,
            Self::QidoStudiesHead
            | Self::QidoAllSeriesHead
            | Self::QidoAllInstancesHead
            | Self::WadoStudyRetrieveHead
            | Self::WadoSeriesRetrieveHead
            | Self::WadoInstanceRetrieveHead
            | Self::WadoUriHead
            | Self::WadoRenderedInstanceHead
            | Self::WadoBulkDataHead
            | Self::QidoSeriesByStudyHead
            | Self::QidoStudyInstancesHead
            | Self::QidoInstancesByStudySeriesHead => HttpMethod::Head,
            Self::StowStudiesPost | Self::StowStudyScopedPost => HttpMethod::Post,
            Self::DeleteStudy | Self::DeleteSeries | Self::DeleteInstance => HttpMethod::Delete,
        }
    }

    /// Human-readable operation label.
    pub fn operation(self) -> &'static str {
        match self {
            Self::QidoStudiesGet | Self::QidoStudiesHead => "QIDO studies",
            Self::StowStudiesPost => "STOW studies",
            Self::QidoAllSeriesGet | Self::QidoAllSeriesHead => "QIDO all series",
            Self::QidoAllInstancesGet | Self::QidoAllInstancesHead => "QIDO all instances",
            Self::StowStudyScopedPost => "STOW scoped by study UID",
            Self::WadoStudyRetrieveGet | Self::WadoStudyRetrieveHead => "WADO study retrieve",
            Self::WadoSeriesRetrieveGet | Self::WadoSeriesRetrieveHead => "WADO series retrieve",
            Self::WadoInstanceRetrieveGet | Self::WadoInstanceRetrieveHead => {
                "WADO instance retrieve"
            }
            Self::WadoUriGet | Self::WadoUriHead => "WADO-URI compatibility retrieve",
            Self::WadoStudyMetadataGet => "WADO study metadata",
            Self::WadoSeriesMetadataGet => "WADO series metadata",
            Self::WadoInstanceMetadataGet => "WADO instance metadata",
            Self::WadoFrameRetrieveGet => "WADO frame retrieve",
            Self::WadoRenderedInstanceGet | Self::WadoRenderedInstanceHead => {
                "WADO rendered instance retrieve"
            }
            Self::WadoRenderedFrameGet => "WADO rendered frame retrieve",
            Self::WadoBulkDataGet | Self::WadoBulkDataHead => "WADO bulkdata retrieve",
            Self::QidoSeriesByStudyGet | Self::QidoSeriesByStudyHead => "QIDO series by study",
            Self::QidoStudyInstancesGet | Self::QidoStudyInstancesHead => "QIDO instances by study",
            Self::QidoInstancesByStudySeriesGet | Self::QidoInstancesByStudySeriesHead => {
                "QIDO instances by study/series"
            }
            Self::DeleteStudy => "Delete study",
            Self::DeleteSeries => "Delete series",
            Self::DeleteInstance => "Delete instance",
        }
    }

    /// Required feature gate.
    pub fn required_feature(self) -> &'static str {
        match self {
            Self::QidoStudiesGet
            | Self::QidoStudiesHead
            | Self::QidoAllSeriesGet
            | Self::QidoAllSeriesHead
            | Self::QidoAllInstancesGet
            | Self::QidoAllInstancesHead
            | Self::QidoSeriesByStudyGet
            | Self::QidoSeriesByStudyHead
            | Self::QidoStudyInstancesGet
            | Self::QidoStudyInstancesHead
            | Self::QidoInstancesByStudySeriesGet
            | Self::QidoInstancesByStudySeriesHead => "qido",
            Self::StowStudiesPost | Self::StowStudyScopedPost => "stow",
            Self::WadoStudyRetrieveGet
            | Self::WadoStudyRetrieveHead
            | Self::WadoSeriesRetrieveGet
            | Self::WadoSeriesRetrieveHead
            | Self::WadoInstanceRetrieveGet
            | Self::WadoInstanceRetrieveHead
            | Self::WadoUriGet
            | Self::WadoUriHead
            | Self::WadoStudyMetadataGet
            | Self::WadoSeriesMetadataGet
            | Self::WadoInstanceMetadataGet
            | Self::WadoFrameRetrieveGet
            | Self::WadoRenderedInstanceGet
            | Self::WadoRenderedInstanceHead
            | Self::WadoRenderedFrameGet
            | Self::WadoBulkDataGet
            | Self::WadoBulkDataHead => "wado",
            Self::DeleteStudy | Self::DeleteSeries | Self::DeleteInstance => "delete",
        }
    }

    /// Content-type contract, if applicable.
    pub fn content_type(self) -> Option<&'static str> {
        match self {
            Self::StowStudiesPost | Self::StowStudyScopedPost => Some(
                "application/dicom, application/dicom+xml, application/dicom+json, or multipart/related",
            ),
            Self::WadoStudyRetrieveGet | Self::WadoStudyRetrieveHead
            | Self::WadoSeriesRetrieveGet | Self::WadoSeriesRetrieveHead => {
                Some("multipart/related; type=\"application/dicom\"")
            }
            Self::WadoInstanceRetrieveGet | Self::WadoInstanceRetrieveHead
            | Self::WadoUriGet | Self::WadoUriHead => Some("application/dicom"),
            Self::WadoStudyMetadataGet
            | Self::WadoSeriesMetadataGet
            | Self::WadoInstanceMetadataGet => Some("application/dicom+json"),
            Self::WadoFrameRetrieveGet => Some("application/octet-stream"),
            Self::WadoRenderedInstanceGet | Self::WadoRenderedInstanceHead
            | Self::WadoRenderedFrameGet => Some("image/png or image/jpeg"),
            Self::WadoBulkDataGet | Self::WadoBulkDataHead => Some("application/octet-stream"),
            _ => None,
        }
    }

    /// Iterate over all known route variants in definition order.
    pub fn all() -> impl Iterator<Item = Self> {
        use DicomWebRoute::*;
        static ROUTES: &[DicomWebRoute] = &[
            QidoStudiesGet,
            QidoStudiesHead,
            StowStudiesPost,
            QidoAllSeriesGet,
            QidoAllSeriesHead,
            QidoAllInstancesGet,
            QidoAllInstancesHead,
            StowStudyScopedPost,
            WadoStudyRetrieveGet,
            WadoStudyRetrieveHead,
            WadoSeriesRetrieveGet,
            WadoSeriesRetrieveHead,
            WadoInstanceRetrieveGet,
            WadoInstanceRetrieveHead,
            WadoUriGet,
            WadoUriHead,
            WadoStudyMetadataGet,
            WadoSeriesMetadataGet,
            WadoInstanceMetadataGet,
            WadoFrameRetrieveGet,
            WadoRenderedInstanceGet,
            WadoRenderedInstanceHead,
            WadoRenderedFrameGet,
            WadoBulkDataGet,
            WadoBulkDataHead,
            QidoSeriesByStudyGet,
            QidoSeriesByStudyHead,
            QidoStudyInstancesGet,
            QidoStudyInstancesHead,
            QidoInstancesByStudySeriesGet,
            QidoInstancesByStudySeriesHead,
            DeleteStudy,
            DeleteSeries,
            DeleteInstance,
        ];
        ROUTES.iter().copied()
    }
}

/// A route/method capability entry for DICOMweb interoperability matrices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DicomWebRouteCapability {
    /// Named route identifier.
    pub route: DicomWebRoute,
    /// Availability state for this route/method.
    pub state: DicomWebRouteState,
}

/// Route capability registry: maps named routes to their availability state.
pub type DicomWebRouteCapabilityMatrix = BTreeMap<DicomWebRoute, DicomWebRouteState>;

/// Check whether a specific route is enabled.
pub fn is_route_enabled(matrix: &DicomWebRouteCapabilityMatrix, route: DicomWebRoute) -> bool {
    matches!(
        matrix.get(&route),
        Some(DicomWebRouteState::Implemented | DicomWebRouteState::Partial)
    )
}

/// Build a deterministic DICOMweb route/method capability matrix.
pub fn dicomweb_route_capability_matrix() -> DicomWebRouteCapabilityMatrix {
    let qido = route_state(cfg!(feature = "qido"));
    let wado = route_state(cfg!(feature = "wado"));
    let stow = route_state(cfg!(feature = "stow"));
    let delete_state = DicomWebRouteState::Blocked;

    let mut matrix = BTreeMap::new();
    for route in DicomWebRoute::all() {
        let state = match route.required_feature() {
            "qido" => qido,
            "wado" => wado,
            "stow" => stow,
            "delete" => delete_state,
            _ => DicomWebRouteState::NotExposed,
        };
        matrix.insert(route, state);
    }
    matrix
}

fn route_state(feature_enabled: bool) -> DicomWebRouteState {
    if feature_enabled {
        DicomWebRouteState::Implemented
    } else {
        DicomWebRouteState::Blocked
    }
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
        .max_input_bytes()
        .saturating_add(limits.max_string_bytes())
}

/// Translate a structured DICOMweb error to deterministic HTTP status/label output.
pub fn dicomweb_status_for_error(error: &Error) -> (u16, &'static str) {
    match error.kind() {
        ErrorKind::LimitExceeded { .. } => (413, "Payload Too Large"),
        ErrorKind::NotFound { .. } => (404, "Not Found"),
        ErrorKind::DecodeError { stage, detail: _ } if stage == "dicom-auth" => (403, "Forbidden"),
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
        ErrorKind::AuthorizationDenied { .. }
        | ErrorKind::PolicyViolation { .. }
        | ErrorKind::SessionError { .. } => (403, "Forbidden"),
        ErrorKind::CollaborationError { .. } => (500, "Internal Server Error"),
    }
}

/// Parse an HTTP request (minimal subset) into a `WebRequest`.
pub fn parse_http_request(
    input: &[u8],
    limits: &Limits,
    transport: TransportSecurity,
) -> Result<WebRequest> {
    let (head, body) = split_head_body(input)?;
    enforce_limit(
        "max_input_bytes",
        body.len() as u64,
        limits.max_input_bytes(),
    )?;

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
            limits.max_string_bytes(),
        )?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
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
        limits.max_input_bytes(),
    )?;
    enforce_limit(
        "max_string_bytes",
        request.path.len() as u64,
        limits.max_string_bytes(),
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
        let parsed = super::wado::parse_wado_uri_query(&request.query)?;
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
                    let content_type = super::stow::parse_content_type(&request, limits)?;
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
                    let content_type = super::stow::parse_content_type(&request, limits)?;
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
                    let transfer_syntax_uid =
                        super::wado::parse_wado_retrieve_query(&request.query)?;
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
                    let transfer_syntax_uid =
                        super::wado::parse_wado_retrieve_query(&request.query)?;
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
                let transfer_syntax_uid = super::wado::parse_wado_retrieve_query(&request.query)?;
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
                let transfer_syntax_uid = super::wado::parse_wado_retrieve_query(&request.query)?;
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
                let transfer_syntax_uid = super::wado::parse_wado_retrieve_query(&request.query)?;
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
                let transfer_syntax_uid = super::wado::parse_wado_retrieve_query(&request.query)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                let frame_number = super::wado::parse_frame_number(_frame_number)?;
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
                    super::wado::parse_wado_rendered_query(&request.query, &request.headers)?;
                let study_uid = validate_uid(TAG_STUDY_UID, _study_uid)?;
                let series_uid = validate_uid(TAG_SERIES_UID, _series_uid)?;
                let instance_uid = validate_uid(TAG_INSTANCE_UID, _instance_uid)?;
                let frame_number = super::wado::parse_frame_number(_frame_number)?;
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
                    super::wado::parse_wado_rendered_query(&request.query, &request.headers)?;
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
                    super::wado::parse_wado_bulkdata_query(&request.query, &request.headers)?;
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
                        let transfer_syntax_uid =
                            super::wado::parse_wado_retrieve_query(&request.query)?;
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

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub(crate) fn header_value<'a>(headers: &'a [Header], name: &str) -> Option<&'a str> {
    let name = name.to_ascii_lowercase();
    headers
        .iter()
        .find(|header| header.name == name)
        .map(|header| header.value.as_str())
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
        limits.max_string_bytes(),
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
            limits.max_string_bytes(),
        )?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
        )?;
        params.push(QueryParam {
            key: key.to_string(),
            value: value.to_string(),
        });
        if params.len() as u64 > limits.max_dataset_elements() {
            return Err(limit_exceeded(
                "max_dataset_elements",
                params.len() as u64,
                limits.max_dataset_elements(),
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
    if params.len() as u64 > limits.max_dataset_elements() {
        return Err(limit_exceeded(
            "max_dataset_elements",
            params.len() as u64,
            limits.max_dataset_elements(),
        ));
    }
    for param in params {
        ensure_ascii_graphic(&param.key, "query key")?;
        ensure_ascii_graphic(&param.value, "query value")?;
        enforce_limit(
            "max_string_bytes",
            param.key.len() as u64,
            limits.max_string_bytes(),
        )?;
        enforce_limit(
            "max_string_bytes",
            param.value.len() as u64,
            limits.max_string_bytes(),
        )?;
    }
    Ok(())
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

pub(crate) fn is_query_method(method: HttpMethod) -> bool {
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
