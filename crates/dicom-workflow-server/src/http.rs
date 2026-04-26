//! HTTP request parsing, response rendering, and authentication.

use super::*;

pub fn render_optional_str(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

pub fn build_worklist_dataset(params: &BTreeMap<String, String>) -> Result<Dataset, Box<Error>> {
    let scheduled_step_id = required_param(params, "scheduled_step_id")?;
    let modality = required_param(params, "modality")?;
    let start_date = required_param(params, "start_date")?;
    let start_time = required_param(params, "start_time")?;

    let mut item = Dataset::new();
    item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(scheduled_step_id.to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(modality.to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_SPS_START_DATE, Vr::Da, Value::Str(start_date.to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_SPS_START_TIME, Vr::Tm, Value::Str(start_time.to_string()),
    ).unwrap());

    if let Some(value) = params.get("requested_procedure_id") {
        item.insert(Element::new(TAG_REQUESTED_PROCEDURE_ID, Vr::Sh, Value::Str(value.to_string()),
        ).unwrap());
    }
    if let Some(value) = params.get("scheduled_station_ae_title") {
        item.insert(Element::new(TAG_SCHEDULED_STATION_AE_TITLE, Vr::Ae, Value::Str(value.to_string()),
        ).unwrap());
    }

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![item]),
    ).unwrap());

    if let Some(value) = lookup_identifier_from_map(
        params,
        &["patient_id", "PatientID", "PatientId", "patientId"],
    ) {
        dataset.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str(value),
        ).unwrap());
    }
    if let Some(value) = lookup_identifier_from_map(
        params,
        &[
            "accession_number",
            "AccessionNumber",
            "accessionNo",
            "accession_number_raw",
        ],
    ) {
        dataset.insert(Element::new(TAG_ACCESSION_NUMBER, Vr::Sh, Value::Str(value),
        ).unwrap());
    }

    Ok(dataset)
}

pub fn build_mpps_dataset(params: &BTreeMap<String, String>) -> Result<Dataset, Box<Error>> {
    let sop_instance_uid = required_identifier_from_map(
        params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
    )?;
    let status = required_param(params, "status")?;
    let performed_step_id = required_param(params, "performed_step_id")?;
    let start_date = required_param(params, "start_date")?;
    let start_time = required_param(params, "start_time")?;

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(sop_instance_uid.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_STATUS, Vr::Cs, Value::Str(status.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PERFORMED_STEP_ID, Vr::Sh, Value::Str(performed_step_id.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_START_DATE, Vr::Da, Value::Str(start_date.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_START_TIME, Vr::Tm, Value::Str(start_time.to_string()),
    ).unwrap());

    if let Some(value) = params.get("end_date") {
        dataset.insert(Element::new(TAG_END_DATE, Vr::Da, Value::Str(value.to_string()),
        ).unwrap());
    }
    if let Some(value) = params.get("end_time") {
        dataset.insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str(value.to_string()),
        ).unwrap());
    }

    Ok(dataset)
}

pub fn required_param<'a>(
    params: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, Box<Error>> {
    let value = params
        .get(key)
        .ok_or_else(|| decode_error(format!("missing required parameter: {key}")))?;
    if value.is_empty() {
        return Err(decode_error(format!("empty required parameter: {key}")));
    }
    Ok(value)
}

pub fn parse_form_map(body: &[u8], limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    let text =
        std::str::from_utf8(body).map_err(|_| decode_error("form body is not valid UTF-8"))?;
    parse_pairs(text, limits)
}

pub fn parse_query_map(query: &str, limits: &Limits) -> Result<BTreeMap<String, String>, String> {
    parse_pairs(query, limits).map_err(|err| err.to_string())
}

pub fn parse_pairs(text: &str, limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    let mut out = BTreeMap::new();
    let max_pairs = limits
        .max_dataset_elements()
        .min(DEFAULT_WORKFLOW_MAX_PAIR_COUNT as u64);
    let mut pair_count = 0usize;
    if text.is_empty() {
        return Ok(out);
    }
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        pair_count = pair_count.saturating_add(1);
        if pair_count as u64 > max_pairs {
            return Err(limit_exceeded(
                "max_workflow_pair_count",
                pair_count as u64,
                max_pairs,
            ));
        }
        let (raw_key, raw_value) = pair
            .split_once('=')
            .ok_or_else(|| decode_error("invalid key-value pair"))?;
        let key = percent_decode(raw_key).ok_or_else(|| decode_error("invalid key encoding"))?;
        let value =
            percent_decode(raw_value).ok_or_else(|| decode_error("invalid value encoding"))?;
        if key.is_empty() {
            return Err(decode_error("empty key is not allowed"));
        }
        enforce_ascii_and_limits(&key, limits)?;
        enforce_ascii_and_limits(&value, limits)?;
        out.insert(key, value);
    }
    Ok(out)
}

pub fn enforce_ascii_and_limits(value: &str, limits: &Limits) -> Result<(), Box<Error>> {
    if !value.is_ascii() {
        return Err(decode_error("request values must be ASCII"));
    }
    if value.len() as u64 > limits.max_string_bytes() {
        return Err(limit_exceeded(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
        ));
    }
    Ok(())
}

pub fn percent_decode(input: &str) -> Option<String> {
    let mut out = String::with_capacity(input.len());
    let mut bytes = input.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => out.push(' '),
            b'%' => {
                let hi = bytes.next()?;
                let lo = bytes.next()?;
                let decoded = (from_hex(hi)? << 4) | from_hex(lo)?;
                out.push(decoded as char);
            }
            _ => out.push(byte as char),
        }
    }
    Some(out)
}

pub fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub fn parse_http_request(bytes: &[u8], limits: &Limits) -> Result<HttpRequest, String> {
    let (head_end, body_start) =
        header_offsets(bytes).ok_or_else(|| "missing header terminator".to_string())?;
    let head =
        std::str::from_utf8(&bytes[..head_end]).map_err(|_| "headers are not UTF-8".to_string())?;
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let target = request_parts
        .next()
        .ok_or_else(|| "missing target".to_string())?;

    if method != "GET" && method != "HEAD" && method != "POST" {
        return Err("unsupported method".to_string());
    }

    if method != "POST" && bytes.len() > body_start {
        return Err("GET/HEAD requests must not include a body".to_string());
    }

    let mut headers = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "invalid header format".to_string())?;
        let key = name.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if key.len() as u64 > limits.max_string_bytes()
            || value.len() as u64 > limits.max_string_bytes()
        {
            return Err("header length limit exceeded".to_string());
        }
        headers.insert(key, value);
    }

    let body = bytes[body_start..].to_vec();

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (
            path.to_string(),
            parse_query_map(query, limits).map_err(|err| err.to_string())?,
        ),
        None => (target.to_string(), BTreeMap::new()),
    };

    if path.len() as u64 > limits.max_string_bytes() {
        return Err("path length limit exceeded".to_string());
    }

    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

pub fn requested_admin_api_version(request: &HttpRequest) -> Option<String> {
    request
        .headers
        .get("x-workflow-admin-api-version")
        .or_else(|| request.query.get("api_version"))
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
}

pub fn enforce_admin_api_version(request: &HttpRequest) -> Result<(), Box<Error>> {
    let Some(version) = requested_admin_api_version(request) else {
        return Ok(());
    };
    if version == WORKFLOW_ADMIN_API_VERSION || version == "1" {
        return Ok(());
    }
    Err(decode_error(format!(
        "unsupported admin api_version: {version} (supported: v1, 1)"
    )))
}

pub fn request_id_from_headers(headers: &BTreeMap<String, String>) -> String {
    let Some(raw) = headers.get("x-request-id") else {
        return fallback_request_id();
    };
    let value = raw.trim();
    if value.is_empty() {
        fallback_request_id()
    } else {
        value.to_string()
    }
}

pub fn fallback_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let counter = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("request-{}-{}", now.as_nanos(), counter)
}

pub fn read_http_request(stream: &mut TcpStream, limits: &Limits) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 8192];
    let mut total_len: Option<usize> = None;
    let hard_cap = limits
        .max_input_bytes()
        .saturating_add(limits.max_string_bytes()) as usize;

    loop {
        let n = stream.read(&mut temp)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..n]);

        if buffer.len() > hard_cap {
            return Err(IoError::new(IoErrorKind::InvalidData, "request too large"));
        }

        if total_len.is_none() {
            if let Some((head_end, body_start)) = header_offsets(&buffer) {
                let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| {
                    IoError::new(IoErrorKind::InvalidData, "headers are not valid UTF-8")
                })?;
                let body_len = content_length(head).unwrap_or(0);
                total_len = Some(body_start.saturating_add(body_len));
            }
        }

        if let Some(total) = total_len {
            if buffer.len() >= total {
                buffer.truncate(total);
                return Ok(buffer);
            }
        }
    }

    if total_len.is_some() {
        return Err(IoError::new(
            IoErrorKind::UnexpectedEof,
            "incomplete HTTP request body",
        ));
    }
    Err(IoError::new(
        IoErrorKind::InvalidData,
        "missing HTTP header terminator",
    ))
}

pub fn header_offsets(buffer: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, pos + 4));
    }
    buffer
        .windows(2)
        .position(|w| w == b"\n\n")
        .map(|pos| (pos, pos + 2))
}

pub fn content_length(head: &str) -> Option<usize> {
    for line in head.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
}

pub fn authorize(
    request: &HttpRequest,
    mode: &WorkflowAuthMode,
    transport_security: &str,
    auth_token: Option<&str>,
) -> bool {
    if transport_security == "insecure" {
        return false;
    }
    match mode {
        WorkflowAuthMode::AllowAll => true,
        WorkflowAuthMode::DenyAll => false,
        WorkflowAuthMode::Token => request
            .headers
            .get("x-rdvf-token")
            .is_some_and(|value| Some(value.as_str()) == auth_token),
    }
}

pub fn auth_mode_from_env(
    auth_token_raw: Option<&str>,
    auth_mode_raw: Option<&str>,
) -> std::io::Result<WorkflowAuthMode> {
    let parsed = WorkflowAuthMode::parse(auth_mode_raw)?;
    if matches!(parsed, WorkflowAuthMode::Token)
        && !auth_token_raw.is_some_and(|token| !token.trim().is_empty())
    {
        return Err(environment_parse_error(
            "DICOM_WORKFLOW_AUTH_MODE",
            auth_mode_raw.unwrap_or("token"),
            "DICOM_WORKFLOW_AUTH_TOKEN must be set when auth mode is token",
        ));
    }
    Ok(parsed)
}

pub fn auth_mode_label(mode: &WorkflowAuthMode) -> &'static str {
    match mode {
        WorkflowAuthMode::AllowAll => "allow_all",
        WorkflowAuthMode::DenyAll => "deny_all",
        WorkflowAuthMode::Token => "token",
    }
}

pub fn validate_tls_secret_path(
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> std::io::Result<()> {
    match (cert_path, key_path) {
        (None, None) => Err(IoError::other(
            "transport requires TLS but DICOM_WORKFLOW_TLS_CERT_PATH and DICOM_WORKFLOW_TLS_KEY_PATH are both unset",
        )),
        (Some(_), None) | (None, Some(_)) => Err(IoError::other(
            "transport requires TLS but DICOM_WORKFLOW_TLS_CERT_PATH and DICOM_WORKFLOW_TLS_KEY_PATH must both be set",
        )),
        (Some(cert_path), Some(key_path)) => {
            if cert_path.trim().is_empty() || key_path.trim().is_empty() {
                return Err(IoError::other(
                    "transport requires TLS but TLS material path values must be non-empty",
                ));
            }
            let cert_meta = fs::metadata(cert_path)?;
            if !cert_meta.is_file() {
                return Err(IoError::other(format!(
                    "TLS cert path is not a file: {cert_path}"
                )));
            }
            let key_meta = fs::metadata(key_path)?;
            if !key_meta.is_file() {
                return Err(IoError::other(format!(
                    "TLS key path is not a file: {key_path}"
                )));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
pub fn prepare_persistence_file(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<()> {
    let _ = prepare_persistence_file_with_diagnostics(path, max_bytes, max_rotated_files, label)?;
    Ok(())
}

pub fn transport_security_from_env(raw_security: Option<&str>) -> &'static str {
    match raw_security {
        Some("tls") => "tls",
        Some("insecure") | None => "insecure",
        Some(_) => "insecure",
    }
}

pub enum WorkflowResponse {
    Json(u16, String),
}

#[derive(Clone, Copy)]
pub struct WorkflowRouteErrorInvariant {
    pub route_group: &'static str,
    pub error_code: &'static str,
    pub http_status: u16,
    pub http_label: &'static str,
}

pub const WORKFLOW_ROUTE_ERROR_INVARIANTS: &[WorkflowRouteErrorInvariant] = &[
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WEB.NOT_FOUND",
        http_status: 404,
        http_label: "Not Found",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.INTEGRITY.ERROR",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "http",
        error_code: "DVF.WORKFLOW.HTTP.DECODE_ERROR",
        http_status: 400,
        http_label: "Bad Request",
    },
];

pub fn workflow_route_error_invariant(error_code: &str) -> Option<WorkflowRouteErrorInvariant> {
    WORKFLOW_ROUTE_ERROR_INVARIANTS
        .iter()
        .find(|invariant| invariant.error_code == error_code)
        .copied()
}

pub fn status_for_error(error: &Error) -> (u16, &'static str) {
    if let Some(invariant) = workflow_route_error_invariant(error.code()) {
        let _route_group = invariant.route_group;
        return (invariant.http_status, invariant.http_label);
    }
    match &error.kind() {
        ErrorKind::LimitExceeded { limit_name, .. }
            if (*limit_name).contains("rate") || *limit_name == "workflow_rate_limit" =>
        {
            (429, "Too Many Requests")
        }
        ErrorKind::LimitExceeded { .. } => (413, "Payload Too Large"),
        ErrorKind::NotFound { .. } => (404, "Not Found"),
        ErrorKind::DecodeError { stage, .. } if stage.contains("auth") => (403, "Forbidden"),
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

pub fn http_success_response(response: WorkflowResponse, head_only: bool) -> Vec<u8> {
    match response {
        WorkflowResponse::Json(status, body) => {
            let status_label = if status == 200 { "OK" } else { "Response" };
            http_response_bytes(
                status,
                status_label,
                "application/json",
                body.as_bytes(),
                head_only,
            )
        }
    }
}

pub fn http_error_response(
    status: u16,
    status_label: &str,
    code: &str,
    message: &str,
    head_only: bool,
) -> Vec<u8> {
    let message = redact_diagnostic_message(message);
    let body = format!(
        "{{\"error\":\"{}\",\"message\":\"{}\"}}",
        escape_json(code),
        escape_json(&message),
    )
    .into_bytes();
    http_response_bytes(status, status_label, "application/json", &body, head_only)
}

pub fn http_response_bytes(
    status: u16,
    status_label: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(format!("HTTP/1.1 {status} {status_label}\r\n").as_bytes());
    response.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    response.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    response.extend_from_slice(b"Connection: close\r\n\r\n");
    if !head_only {
        response.extend_from_slice(body);
    }
    response
}

pub fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
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

pub fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

pub fn sr_not_found_error() -> Box<Error> {
    Error::from_kind(
        ErrorKind::NotFound {
            detail: "requested SR document not found".to_string(),
        },
        "requested SR document not found",
    )
    .into()
}

pub fn io_error(context: impl Into<String>, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: format!("{}: {}", context.into(), detail.into()),
        },
        "i/o error",
    )
    .into()
}

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

pub fn redact_diagnostic_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn redact_token(token: &str) -> String {
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

pub fn looks_like_uid(value: &str) -> bool {
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

pub fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

pub fn looks_like_host(value: &str) -> bool {
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

pub fn looks_like_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

pub fn looks_like_sensitive_key(value: &str) -> bool {
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

