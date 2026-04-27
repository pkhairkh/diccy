#![deny(missing_docs)]

//! Minimal packaged DICOMweb server runtime built on `dicom-web`.

use dicom_env_contract::{
    dicom_web_env_contract, parse_optional_string, parse_string_non_empty, parse_u64, parse_usize,
    validate_envelope_version, NumericBounds, DEFAULT_DICOM_ENVELOPE_VERSION,
    DICOM_ENVELOPE_VERSION, DICOM_WEB_ENV_PREFIX, SUPPORTED_DICOM_ENVELOPE_VERSIONS,
};
use dicom_web::{
    parse_http_request, request_requires_write, DicomWebResponse, DicomWebService,
    DicomWebServiceConfig, HttpMethod, ThrottleDecision, TransportSecurity, WebAuthConfig,
    WebPolicy,
};
use dicom_web_server::{
    blocked_interop_feature, classify_dicomweb_route, content_length, escape_json,
    json_escape, parse_bool_env_with_default, parse_log_level,
    prepare_persistence_file_with_diagnostics, queue_backpressure_guidance,
    classify_accept_queue_state, redact_diagnostic_message, resolve_worker_queue_contract,
    startup_policy_diagnostics, storage_recovery_diagnostic, status_for_error, stow_json_body,
    InteropBlockReason, InteropRuntimePolicy, IoErrorKind, Limits,
    RoutePerformanceBudgets, RoutePerformanceTracker, Storage,
    WebAuthMode, WebLogLevel,
};
use std::io::{Error as IoError, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const ROUTE_LATENCY_SAMPLE_WINDOW: usize = 256;
const DICOM_WEB_SERVICE_NAME: &str = "dicom-web-server";

#[derive(Clone)]
struct DicomWebRuntimeConfig {
    bind: String,
    tls_policy_raw: Option<String>,
    auth_mode_raw: Option<String>,
    auth_mode: WebAuthMode,
    transport_security_raw: Option<String>,
    interop_policy: InteropRuntimePolicy,
    route_performance_budgets: RoutePerformanceBudgets,
    storage_wal_path: String,
    wal_max_bytes: u64,
    wal_max_rotated_files: usize,
    worker_count: usize,
    accept_queue_depth: usize,
}

impl DicomWebRuntimeConfig {
    fn from_env() -> std::io::Result<Self> {
        // Defaults and bounds in this parser are the runtime source for docs/09 and docs/14.
        let bind =
            parse_string_non_empty(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_BIND", "127.0.0.1:8080")?;
        let tls_policy_raw = parse_optional_string(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_TLS_POLICY")?;
        let auth_mode_raw = parse_optional_string(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_AUTH_MODE")?;
        let auth_mode = WebAuthMode::parse(auth_mode_raw.as_deref())?;
        let transport_security_raw =
            parse_optional_string(DICOM_WEB_SERVICE_NAME, "DICOM_WEB_TRANSPORT_SECURITY")?;
        if let Some(raw) = transport_security_raw.as_deref() {
            if raw != "tls" && raw != "insecure" {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    format!("service={DICOM_WEB_SERVICE_NAME} var=DICOM_WEB_TRANSPORT_SECURITY parsed_value={raw} detail=supported values are tls | insecure"),
                ));
            }
        }
        let interop_policy = InteropRuntimePolicy::from_env()?;
        let route_performance_budgets = RoutePerformanceBudgets::from_env()?;
        let storage_wal_path = parse_string_non_empty(
            DICOM_WEB_SERVICE_NAME,
            "DICOM_WEB_STORAGE_WAL",
            "./state/dicom-web/storage.wal",
        )?;
        let wal_max_bytes = parse_u64(
            DICOM_WEB_SERVICE_NAME,
            "DICOM_WEB_STORAGE_WAL_MAX_BYTES",
            128 * 1024 * 1024,
            NumericBounds::at_least(1),
        )?;
        let wal_max_rotated_files = parse_usize(
            DICOM_WEB_SERVICE_NAME,
            "DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES",
            3,
            NumericBounds::at_least(1),
        )?;
        let worker_queue = resolve_worker_queue_contract(
            thread::available_parallelism()
                .map(|parallelism| parallelism.get())
                .unwrap_or(4),
        )?;

        Ok(Self {
            bind,
            tls_policy_raw,
            auth_mode_raw,
            auth_mode,
            transport_security_raw,
            interop_policy,
            route_performance_budgets,
            storage_wal_path,
            wal_max_bytes,
            wal_max_rotated_files,
            worker_count: worker_queue.workers,
            accept_queue_depth: worker_queue.queue_depth,
        })
    }
}

#[derive(Clone)]
struct WebObservability {
    service: &'static str,
    log_level: WebLogLevel,
    telemetry_enabled: bool,
    telemetry_safe_subset: bool,
}

impl WebObservability {
    fn from_env(prefix: &str, service: &'static str) -> std::io::Result<Self> {
        let log_level = parse_log_level(&format!("{prefix}LOG_LEVEL"))?;
        let telemetry_enabled =
            parse_bool_env_with_default(&format!("{prefix}TELEMETRY_ENABLED"), false)?;

        let telemetry_safe_subset =
            parse_bool_env_with_default(&format!("{prefix}TELEMETRY_SAFE_SUBSET"), true)?;

        Ok(Self {
            service,
            log_level,
            telemetry_enabled,
            telemetry_safe_subset,
        })
    }

    fn log(&self, level: WebLogLevel, message: &str) {
        if self.log_level.should_log(level) {
            eprintln!("[{level:?}] {}: {message}", self.service);
        }
    }

    fn emit_telemetry(&self, event: &str, fields: &[(&str, &str)]) {
        if !self.telemetry_enabled {
            return;
        }
        let rendered_fields: String = fields
            .iter()
            .map(|(key, value)| {
                let sanitized = redact_diagnostic_message(value);
                format!("{key}={}", json_escape(&sanitized))
            })
            .collect::<Vec<_>>()
            .join(" ");
        eprintln!(
            "telemetry safe_subset={} service={} event={} {}",
            self.telemetry_safe_subset, self.service, event, rendered_fields,
        );
    }
}

/// Start the DICOMweb server loop.
fn main() -> std::io::Result<()> {
    let contract = dicom_web_env_contract(cfg!(test));
    let envelope_version = validate_envelope_version(
        DICOM_WEB_SERVICE_NAME,
        DICOM_ENVELOPE_VERSION,
        SUPPORTED_DICOM_ENVELOPE_VERSIONS,
        DEFAULT_DICOM_ENVELOPE_VERSION,
    )?;

    if has_flag("--print-env-contract") {
        println!("{}", contract.snapshot_json(&envelope_version));
        return Ok(());
    }

    contract.validate()?;

    let config = DicomWebRuntimeConfig::from_env()?;
    let observability = Arc::new(WebObservability::from_env(
        DICOM_WEB_ENV_PREFIX,
        "dicom-web-server",
    )?);
    let bind = config.bind;
    let listener = TcpListener::bind(&bind)?;

    let limits = Limits::default();
    let policy_diagnostics = startup_policy_diagnostics(
        config.tls_policy_raw.as_deref(),
        config.auth_mode_raw.as_deref(),
        config.transport_security_raw.as_deref(),
    );
    let tls_policy = policy_diagnostics.tls_policy;
    let auth_mode = config.auth_mode;
    let auth_mode_label = auth_mode.as_str();
    let auth = auth_config_from_mode(auth_mode);
    let transport_security = policy_diagnostics.transport_security;
    let interop_policy = config.interop_policy;
    let route_performance_budgets = config.route_performance_budgets;
    let storage_wal_path = config.storage_wal_path;
    let wal_max_bytes = config.wal_max_bytes;
    let wal_max_rotated = config.wal_max_rotated_files;
    let wal_preflight = prepare_persistence_file_with_diagnostics(
        &storage_wal_path,
        wal_max_bytes,
        wal_max_rotated,
        "storage WAL",
    )?;
    let service = Arc::new(DicomWebService::new(DicomWebServiceConfig {
        limits: limits.clone(),
        policy: WebPolicy::new(tls_policy, ThrottleDecision::Allow)
            .with_delete_enabled(interop_policy.delete),
        auth,
    }));
    let opened_storage = Storage::open(limits, &storage_wal_path)
        .map_err(|err| IoError::other(format!("failed to open durable storage WAL: {err}")))?;
    let recovery = storage_recovery_diagnostic(&opened_storage)?;
    let storage = Arc::new(RwLock::new(opened_storage));

    observability.log(
        WebLogLevel::Info,
        &format!("dicom-web-server listening on {bind}"),
    );
    observability.log(
        WebLogLevel::Info,
        &format!(
        "dicom-web-server policy: tls={tls_policy:?}, transport={transport_security:?}, auth={auth_mode_label}, tls_defaulted={}, auth_defaulted={}, transport_defaulted={}, fail_closed={}, wal={storage_wal_path}, wal_max_bytes={wal_max_bytes}, wal_max_rotated={wal_max_rotated}, wal_parent_created={}, wal_rotated={}, wal_destructive_rollover={}, wal_ready={}, recovery_durable={}, recovery_records={}",
        policy_diagnostics.tls_defaulted,
        policy_diagnostics.auth_defaulted,
        policy_diagnostics.transport_defaulted,
        policy_diagnostics.fail_closed(),
        wal_preflight.parent_created,
        wal_preflight.rotated,
        wal_preflight.destructive_rollover,
        wal_preflight.file_ready,
        recovery.durable,
        recovery.recovered_records,
    ),
    );
    let provider_capabilities = interop_policy.provider_profile.capabilities();
    observability.log(
        WebLogLevel::Debug,
        &format!(
            "dicom-web-server interop policy: profile={}, qido={}, wado={}, stow={}, delete={}, search={}, retrieve={}, store={}, provider_delete={}, workitem={}",
            interop_policy.provider_profile.label(),
            interop_policy.qido,
            interop_policy.wado,
            interop_policy.stow,
            interop_policy.delete,
            provider_capabilities.search,
            provider_capabilities.retrieve,
            provider_capabilities.store,
            provider_capabilities.delete,
            provider_capabilities.workitem,
        ),
    );
    observability.log(
        WebLogLevel::Info,
        &format!(
            "dicom-web-server route latency budgets (ms): qido p95={}/p99={}, wado p95={}/p99={}, stow p95={}/p99={}",
            route_performance_budgets.qido.p95_ms,
            route_performance_budgets.qido.p99_ms,
            route_performance_budgets.wado.p95_ms,
            route_performance_budgets.wado.p99_ms,
            route_performance_budgets.stow.p95_ms,
            route_performance_budgets.stow.p99_ms,
        ),
    );

    let worker_count = config.worker_count;
    let queue_depth = config.accept_queue_depth;
    observability.emit_telemetry(
        "service_start",
        &[
            ("bind", &bind),
            ("transport", &format!("{transport_security:?}")),
            ("tls", &format!("{tls_policy:?}")),
            ("auth_mode", auth_mode_label),
            ("workers", &worker_count.to_string()),
            ("queue_depth", &queue_depth.to_string()),
            ("wal", &storage_wal_path),
        ],
    );
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(queue_depth);
    let receiver = Arc::new(Mutex::new(receiver));
    let route_performance_tracker = Arc::new(Mutex::new(RoutePerformanceTracker::new(
        ROUTE_LATENCY_SAMPLE_WINDOW,
    )));

    for _ in 0..worker_count {
        let receiver = Arc::clone(&receiver);
        let service = Arc::clone(&service);
        let storage = Arc::clone(&storage);
        let interop_policy = interop_policy;
        let observability = Arc::clone(&observability);
        let route_performance_tracker = Arc::clone(&route_performance_tracker);
        let route_performance_budgets = route_performance_budgets;
        thread::spawn(move || loop {
            let mut stream = {
                let receiver = match receiver.lock() {
                    Ok(guard) => guard,
                    Err(_) => break,
                };
                match receiver.recv() {
                    Ok(stream) => stream,
                    Err(_) => break,
                }
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
            let _ = handle_connection(
                &mut stream,
                &service,
                &storage,
                &route_performance_tracker,
                route_performance_budgets,
                &observability,
                interop_policy,
                transport_security,
            );
        });
    }

    for incoming in listener.incoming() {
        match incoming {
            Ok(stream) => match sender.try_send(stream) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(stream)) => {
                    let state = classify_accept_queue_state(queue_depth, queue_depth);
                    observability.log(
                        WebLogLevel::Warn,
                        &format!(
                        "dicom-web-server accept queue: state={state:?}, queued={queue_depth}, capacity={queue_depth}, guidance={}",
                            queue_backpressure_guidance(state)
                        ),
                    );
                    observability.emit_telemetry(
                        "accept_queue_full",
                        &[
                            ("state", &format!("{state:?}")),
                            ("queue_depth", &queue_depth.to_string()),
                        ],
                    );
                    if sender.send(stream).is_err() {
                        break;
                    }
                }
                Err(mpsc::TrySendError::Disconnected(_)) => break,
            },
            Err(err) => {
                observability.log(
                    WebLogLevel::Warn,
                    &format!("dicom-web-server accept error: {err}"),
                );
                observability.emit_telemetry("accept_error", &[("error", &err.to_string())]);
            }
        }
    }
    Ok(())
}

fn has_flag(name: &str) -> bool {
    std::env::args().skip(1).any(|arg| arg == name)
}

fn handle_connection(
    stream: &mut TcpStream,
    service: &DicomWebService,
    storage: &Arc<RwLock<Storage>>,
    route_performance_tracker: &Arc<Mutex<RoutePerformanceTracker>>,
    route_performance_budgets: RoutePerformanceBudgets,
    observability: &WebObservability,
    interop_policy: InteropRuntimePolicy,
    transport_security: TransportSecurity,
) -> std::io::Result<()> {
    let request_bytes = read_http_request(stream, &service.config().limits).inspect_err(|err| {
        let response = http_error_response(
            400,
            "Bad Request",
            "request_read_failed",
            &err.to_string(),
            false,
        );
        let _ = stream.write_all(&response);
    })?;

    let request =
        match parse_http_request(&request_bytes, &service.config().limits, transport_security) {
            Ok(request) => request,
            Err(err) => {
                let response =
                    http_error_response(400, "Bad Request", err.code(), &err.message(), false);
                stream.write_all(&response)?;
                return Ok(());
            }
        };
    let route_class = classify_dicomweb_route(&request.method, &request.path);
    let route_timer = Instant::now();

    if matches!(request.method, HttpMethod::Get | HttpMethod::Head) && request.path == "/healthz" {
        let head_only = request.method == HttpMethod::Head;
        let response = http_response_bytes(
            200,
            "OK",
            "application/json",
            br#"{"status":"ok"}"#,
            head_only,
        );
        stream.write_all(&response)?;
        return Ok(());
    }
    if matches!(request.method, HttpMethod::Get | HttpMethod::Head) && request.path == "/readyz" {
        let head_only = request.method == HttpMethod::Head;
        let response = http_response_bytes(
            200,
            "OK",
            "application/json",
            br#"{"status":"ready"}"#,
            head_only,
        );
        stream.write_all(&response)?;
        return Ok(());
    }

    if let Some(block_reason) =
        blocked_interop_feature(&request.method, &request.path, &interop_policy)
    {
        let (code, detail) = match block_reason {
            InteropBlockReason::FeatureDisabled(feature) => (
                "interoperability_feature_disabled",
                format!("web interoperability feature '{feature}' is disabled"),
            ),
            InteropBlockReason::ProviderUnsupported { profile, operation } => (
                "provider_profile_unsupported_operation",
                format!(
                    "provider profile '{}' does not support '{}' operations",
                    profile.label(),
                    operation,
                ),
            ),
        };
        let response = http_error_response(403, "Forbidden", code, &detail, false);
        stream.write_all(&response)?;
        return Ok(());
    }
    let head_only = request.method == HttpMethod::Head;
    let routed = match service.route_request(request) {
        Ok(request) => request,
        Err(err) => {
            let (status, label) = status_for_error(&err);
            let response = http_error_response(status, label, err.code(), &err.message(), false);
            stream.write_all(&response)?;
            return Ok(());
        }
    };

    let result = if request_requires_write(&routed) {
        let mut guard = storage
            .write()
            .map_err(|_| IoError::other("storage write lock poisoned"))?;
        service.execute_routed(&routed, &mut guard)
    } else {
        let guard = storage
            .read()
            .map_err(|_| IoError::other("storage read lock poisoned"))?;
        service.execute_routed_read_only(&routed, &guard)
    };

    let response = match result {
        Ok(response) => http_success_response(response, head_only),
        Err(err) => {
            let (status, label) = status_for_error(&err);
            http_error_response(status, label, err.code(), &err.message(), head_only)
        }
    };
    if let Ok(mut tracker) = route_performance_tracker.lock() {
        if let Some(violation) = tracker.record_and_evaluate(
            route_class,
            route_timer.elapsed().as_millis(),
            &route_performance_budgets,
        ) {
            observability.log(
                WebLogLevel::Warn,
                &format!(
                    "dicom-web-server route latency budget violation: route={route}, sample_ms={sample_ms}, p95_ms={p95_ms}, p99_ms={p99_ms}, budget_p95_ms={budget_p95_ms}, budget_p99_ms={budget_p99_ms}, samples={samples}",
                    route = violation.route.as_metric_label(),
                    sample_ms = violation.sample_ms,
                    p95_ms = violation.p95_ms,
                    p99_ms = violation.p99_ms,
                    budget_p95_ms = violation.budget_p95_ms,
                    budget_p99_ms = violation.budget_p99_ms,
                    samples = violation.sample_count,
                ),
            );
            observability.emit_telemetry(
                "route_latency_budget_violation",
                &[
                    ("route", violation.route.as_metric_label()),
                    ("sample_ms", &violation.sample_ms.to_string()),
                    ("p95_ms", &violation.p95_ms.to_string()),
                    ("p99_ms", &violation.p99_ms.to_string()),
                    ("budget_p95_ms", &violation.budget_p95_ms.to_string()),
                    ("budget_p99_ms", &violation.budget_p99_ms.to_string()),
                    ("samples", &violation.sample_count.to_string()),
                ],
            );
        }
    }
    stream.write_all(&response)?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream, limits: &Limits) -> std::io::Result<Vec<u8>> {
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

fn header_offsets(buffer: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, pos + 4));
    }
    buffer
        .windows(2)
        .position(|w| w == b"\n\n")
        .map(|pos| (pos, pos + 2))
}

fn http_success_response(response: DicomWebResponse, head_only: bool) -> Vec<u8> {
    match response {
        DicomWebResponse::Qido { matches } => {
            let body = qido_json_body(&matches).into_bytes();
            http_response_bytes(200, "OK", "application/dicom+json", &body, head_only)
        }
        DicomWebResponse::WadoInstance { bytes } => {
            http_response_bytes(200, "OK", "application/dicom", &bytes, head_only)
        }
        DicomWebResponse::WadoMultipart { media_type, bytes } => {
            http_response_bytes(200, "OK", &media_type, &bytes, head_only)
        }
        DicomWebResponse::WadoMetadata { bytes } => {
            http_response_bytes(200, "OK", "application/dicom+json", &bytes, head_only)
        }
        DicomWebResponse::WadoRendered { media_type, bytes } => {
            http_response_bytes(200, "OK", &media_type, &bytes, head_only)
        }
        DicomWebResponse::WadoBulkData { media_type, bytes } => {
            http_response_bytes(200, "OK", &media_type, &bytes, head_only)
        }
        DicomWebResponse::Stow { outcomes } => {
            let body = stow_json_body(&outcomes).into_bytes();
            http_response_bytes(200, "OK", "application/json", &body, head_only)
        }
        DicomWebResponse::Delete { tombstoned } => {
            let body = format!(
                "{{\"tombstoned\":{}}}",
                if tombstoned { "true" } else { "false" }
            )
            .into_bytes();
            http_response_bytes(200, "OK", "application/json", &body, head_only)
        }
    }
}

fn http_error_response(
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

fn http_response_bytes(
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

fn qido_json_body(matches: &[dicom_query::QueryMatch]) -> String {
    let mut out = String::from("[");
    for (index, item) in matches.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        out.push_str("\"StudyInstanceUID\":\"");
        out.push_str(&escape_json(&item.study_uid));
        out.push('"');
        if let Some(series_uid) = &item.series_uid {
            out.push_str(",\"SeriesInstanceUID\":\"");
            out.push_str(&escape_json(series_uid));
            out.push('"');
        }
        if let Some(instance_uid) = &item.instance_uid {
            out.push_str(",\"SOPInstanceUID\":\"");
            out.push_str(&escape_json(instance_uid));
            out.push('"');
        }
        out.push('}');
    }
    out.push(']');
    out
}

fn auth_config_from_mode(mode: WebAuthMode) -> WebAuthConfig {
    match mode {
        WebAuthMode::AllowAll => WebAuthConfig::allow_all(),
        WebAuthMode::DenyAll => WebAuthConfig::deny_all(),
    }
}
