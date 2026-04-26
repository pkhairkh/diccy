#![deny(missing_docs)]

//! Minimal packaged DICOMweb server runtime built on `dicom-web`.

use dicom_core::{Error, Limits};
use dicom_env_contract::{
    dicom_web_env_contract, parse_bool, parse_optional_string, parse_string_non_empty, parse_u64,
    parse_usize, validate_envelope_version, NumericBounds, DEFAULT_DICOM_ENVELOPE_VERSION,
    DICOM_ENVELOPE_VERSION, DICOM_WEB_ENV_PREFIX, SUPPORTED_DICOM_ENVELOPE_VERSIONS,
};
use dicom_storage::{IngestOutcome, Storage};
use dicom_web::{
    dicomweb_status_for_error, parse_http_request, request_requires_write, DicomWebResponse,
    DicomWebService, DicomWebServiceConfig, HttpMethod, ThrottleDecision, TransportSecurity,
    WebAuthConfig, WebPolicy,
};
use dicom_web_server::{
    classify_accept_queue_state, prepare_persistence_file_with_diagnostics,
    queue_backpressure_guidance, startup_policy_diagnostics, storage_recovery_diagnostic,
    worker_queue_contract, WorkerQueueContract,
};
use std::collections::VecDeque;
use std::env;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::{Duration, Instant};

const ROUTE_LATENCY_SAMPLE_WINDOW: usize = 256;
const DICOM_WEB_SERVICE_NAME: &str = "dicom-web-server";
const DEFAULT_WEB_QIDO_P95_LATENCY_MS: u64 = 250;
const DEFAULT_WEB_QIDO_P99_LATENCY_MS: u64 = 500;
const DEFAULT_WEB_WADO_P95_LATENCY_MS: u64 = 400;
const DEFAULT_WEB_WADO_P99_LATENCY_MS: u64 = 800;
const DEFAULT_WEB_STOW_P95_LATENCY_MS: u64 = 1200;
const DEFAULT_WEB_STOW_P99_LATENCY_MS: u64 = 2400;

#[derive(Debug, Clone, Copy)]
enum WebLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl WebLogLevel {
    fn parse(raw: &str) -> Option<Self> {
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

    fn should_log(&self, level: Self) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebAuthMode {
    AllowAll,
    DenyAll,
}

impl WebAuthMode {
    fn parse(raw: Option<&str>) -> std::io::Result<Self> {
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

    fn as_str(self) -> &'static str {
        match self {
            Self::AllowAll => "allow_all",
            Self::DenyAll => "deny_all",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DicomWebRouteClass {
    Qido,
    Wado,
    Stow,
    Delete,
    Other,
}

impl DicomWebRouteClass {
    fn as_metric_label(&self) -> &'static str {
        match self {
            Self::Qido => "qido",
            Self::Wado => "wado",
            Self::Stow => "stow",
            Self::Delete => "delete",
            Self::Other => "other",
        }
    }
}

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
                return Err(web_env_parse_error(
                    "DICOM_WEB_TRANSPORT_SECURITY",
                    raw,
                    "supported values are tls | insecure",
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

#[derive(Clone, Copy)]
struct RouteLatencyBudget {
    p95_ms: u64,
    p99_ms: u64,
}

#[derive(Clone, Copy)]
struct RoutePerformanceBudgets {
    qido: RouteLatencyBudget,
    wado: RouteLatencyBudget,
    stow: RouteLatencyBudget,
}

impl RoutePerformanceBudgets {
    fn from_env() -> std::io::Result<Self> {
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

    fn for_route(&self, route: DicomWebRouteClass) -> Option<RouteLatencyBudget> {
        match route {
            DicomWebRouteClass::Qido => Some(self.qido),
            DicomWebRouteClass::Wado => Some(self.wado),
            DicomWebRouteClass::Stow => Some(self.stow),
            DicomWebRouteClass::Delete => Some(self.stow),
            DicomWebRouteClass::Other => None,
        }
    }
}

#[derive(Debug)]
struct RouteLatencyWindow {
    samples: VecDeque<u128>,
}

impl RouteLatencyWindow {
    fn new() -> Self {
        Self {
            samples: VecDeque::new(),
        }
    }

    fn len(&self) -> usize {
        self.samples.len()
    }

    fn record_sample(&mut self, latency_ms: u128, sample_limit: usize) {
        while self.samples.len() >= sample_limit {
            let _ = self.samples.pop_front();
        }
        self.samples.push_back(latency_ms);
    }

    fn p95_p99_ms(&self) -> Option<(u128, u128)> {
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

#[derive(Debug)]
struct RoutePerformanceTracker {
    sample_limit: usize,
    qido: RouteLatencyWindow,
    wado: RouteLatencyWindow,
    stow: RouteLatencyWindow,
}

#[derive(Debug)]
struct RouteLatencyViolation {
    route: DicomWebRouteClass,
    sample_count: usize,
    sample_ms: u128,
    p95_ms: u128,
    p99_ms: u128,
    budget_p95_ms: u64,
    budget_p99_ms: u64,
}

impl RoutePerformanceTracker {
    fn new(sample_limit: usize) -> Self {
        Self {
            sample_limit,
            qido: RouteLatencyWindow::new(),
            wado: RouteLatencyWindow::new(),
            stow: RouteLatencyWindow::new(),
        }
    }

    fn record_and_evaluate(
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
    env::args().skip(1).any(|arg| arg == name)
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

fn classify_dicomweb_route(method: &HttpMethod, path: &str) -> DicomWebRouteClass {
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

fn content_length(head: &str) -> Option<usize> {
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

fn parse_env_u64(name: &str, default: u64) -> std::io::Result<u64> {
    parse_u64(
        DICOM_WEB_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

fn parse_log_level(name: &str) -> std::io::Result<WebLogLevel> {
    match parse_optional_string(DICOM_WEB_SERVICE_NAME, name)? {
        Some(raw) => WebLogLevel::parse(&raw).ok_or_else(|| {
            web_env_parse_error(name, raw.trim(), "must be off/error/warn/info/debug/trace")
        }),
        None => Ok(WebLogLevel::Info),
    }
}

fn web_env_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!("service={DICOM_WEB_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}"),
    )
}

fn parse_env_usize(name: &str, default: usize) -> std::io::Result<usize> {
    parse_usize(
        DICOM_WEB_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

fn parse_bool_env_with_default(name: &str, default: bool) -> std::io::Result<bool> {
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

fn json_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderProfile {
    Azure,
    AwsHealthImaging,
    Orthanc,
    Dcm4chee,
    Generic,
}

impl ProviderProfile {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "azure" => Some(Self::Azure),
            "aws_health_imaging" | "aws" | "ahi" => Some(Self::AwsHealthImaging),
            "orthanc" => Some(Self::Orthanc),
            "dcm4chee" => Some(Self::Dcm4chee),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Azure => "azure",
            Self::AwsHealthImaging => "aws_health_imaging",
            Self::Orthanc => "orthanc",
            Self::Dcm4chee => "dcm4chee",
            Self::Generic => "generic",
        }
    }

    fn capabilities(self) -> ProviderCapabilityMap {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderCapabilityMap {
    retrieve: bool,
    search: bool,
    store: bool,
    delete: bool,
    workitem: bool,
    metadata: bool,
    frame: bool,
    rendered: bool,
    bulkdata: bool,
    wado_uri: bool,
}

impl ProviderCapabilityMap {
    fn supports_web_route(self, method: &HttpMethod, path: &str) -> Option<&'static str> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderOperation {
    Search,
    Retrieve,
    Metadata,
    Frame,
    Rendered,
    Bulkdata,
    Store,
    Delete,
    WadoUri,
    Other,
}

fn classify_provider_operation(method: &HttpMethod, path: &str) -> ProviderOperation {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteropBlockReason {
    FeatureDisabled(&'static str),
    ProviderUnsupported {
        profile: ProviderProfile,
        operation: &'static str,
    },
}

#[derive(Debug, Clone, Copy)]
struct InteropRuntimePolicy {
    qido: bool,
    wado: bool,
    stow: bool,
    delete: bool,
    provider_profile: ProviderProfile,
}

impl InteropRuntimePolicy {
    fn from_env() -> std::io::Result<Self> {
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

fn blocked_interop_feature(
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

fn resolve_worker_queue_contract(worker_default: usize) -> std::io::Result<WorkerQueueContract> {
    let worker_count_raw = parse_env_usize("DICOM_WEB_WORKERS", worker_default)?;
    let queue_depth_raw = parse_env_usize(
        "DICOM_WEB_ACCEPT_QUEUE_DEPTH",
        worker_count_raw.saturating_mul(8),
    )?;
    worker_queue_contract(worker_count_raw, queue_depth_raw)
}

#[cfg(test)]
fn prepare_persistence_file(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<()> {
    let _ = prepare_persistence_file_with_diagnostics(path, max_bytes, max_rotated_files, label)?;
    Ok(())
}

fn status_for_error(error: &Error) -> (u16, &'static str) {
    dicomweb_status_for_error(error)
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

fn stow_json_body(outcomes: &[IngestOutcome]) -> String {
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

fn escape_json(value: &str) -> String {
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

fn redact_diagnostic_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
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

fn auth_config_from_mode(mode: WebAuthMode) -> WebAuthConfig {
    match mode {
        WebAuthMode::AllowAll => WebAuthConfig::allow_all(),
        WebAuthMode::DenyAll => WebAuthConfig::deny_all(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{ErrorKind, Tag};
    use dicom_web_server::path_with_suffix;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    fn with_env_var<F, R>(name: &str, value: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = std::env::var(name).ok();
        match value {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        let result = f();
        match previous {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        result
    }

    fn with_interop_env<F>(qido: Option<&str>, wado: Option<&str>, stow: Option<&str>, f: F)
    where
        F: FnOnce(),
    {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let prev_qido = std::env::var("DICOM_WEB_ENABLE_QIDO").ok();
        let prev_wado = std::env::var("DICOM_WEB_ENABLE_WADO").ok();
        let prev_stow = std::env::var("DICOM_WEB_ENABLE_STOW").ok();

        if let Some(value) = qido {
            std::env::set_var("DICOM_WEB_ENABLE_QIDO", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_QIDO");
        }
        if let Some(value) = wado {
            std::env::set_var("DICOM_WEB_ENABLE_WADO", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_WADO");
        }
        if let Some(value) = stow {
            std::env::set_var("DICOM_WEB_ENABLE_STOW", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_STOW");
        }

        f();

        if let Some(value) = prev_qido {
            std::env::set_var("DICOM_WEB_ENABLE_QIDO", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_QIDO");
        }
        if let Some(value) = prev_wado {
            std::env::set_var("DICOM_WEB_ENABLE_WADO", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_WADO");
        }
        if let Some(value) = prev_stow {
            std::env::set_var("DICOM_WEB_ENABLE_STOW", value);
        } else {
            std::env::remove_var("DICOM_WEB_ENABLE_STOW");
        }
    }

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.{ext}"))
    }

    fn cleanup_with_rotations(path: &Path, max_rotations: usize) {
        let _ = fs::remove_file(path);
        for index in 1..=max_rotations + 1 {
            let _ = fs::remove_file(path_with_suffix(path, index));
        }
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }

    fn with_worker_env<F>(workers: Option<&str>, queue_depth: Option<&str>, f: F)
    where
        F: FnOnce(),
    {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let prev_workers = std::env::var("DICOM_WEB_WORKERS").ok();
        let prev_queue = std::env::var("DICOM_WEB_ACCEPT_QUEUE_DEPTH").ok();

        if let Some(value) = workers {
            std::env::set_var("DICOM_WEB_WORKERS", value);
        } else {
            std::env::remove_var("DICOM_WEB_WORKERS");
        }
        if let Some(value) = queue_depth {
            std::env::set_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH", value);
        } else {
            std::env::remove_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH");
        }

        f();

        if let Some(value) = prev_workers {
            std::env::set_var("DICOM_WEB_WORKERS", value);
        } else {
            std::env::remove_var("DICOM_WEB_WORKERS");
        }
        if let Some(value) = prev_queue {
            std::env::set_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH", value);
        } else {
            std::env::remove_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH");
        }
    }

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

    #[test]
    fn worker_queue_env_uses_defaults_when_unset() {
        with_worker_env(None, None, || {
            let contract = resolve_worker_queue_contract(4).expect("default contract");
            assert_eq!(contract.workers, 4);
            assert_eq!(contract.queue_depth, 32);
        });
    }

    #[test]
    fn worker_queue_env_uses_explicit_values() {
        with_worker_env(Some("3"), Some("11"), || {
            let contract = resolve_worker_queue_contract(4).expect("explicit contract");
            assert_eq!(contract.workers, 3);
            assert_eq!(contract.queue_depth, 11);
        });
    }

    #[test]
    fn worker_queue_env_rejects_invalid_values() {
        with_worker_env(Some("bad"), Some("11"), || {
            let err = resolve_worker_queue_contract(4).expect_err("invalid worker value must fail");
            assert!(err.to_string().contains("DICOM_WEB_WORKERS"));
        });
        with_worker_env(Some("2"), Some("bad"), || {
            let err = resolve_worker_queue_contract(4).expect_err("invalid queue value must fail");
            assert!(err.to_string().contains("DICOM_WEB_ACCEPT_QUEUE_DEPTH"));
        });
    }

    #[test]
    fn preflight_creates_missing_parent_and_file() {
        let root = temp_file_path("web_preflight_create", "dir");
        let wal = root.join("storage.wal");
        let wal_str = wal.to_string_lossy().to_string();
        prepare_persistence_file(&wal_str, 1024, 2, "storage WAL").expect("preflight");
        assert!(wal.exists());
        cleanup_with_rotations(&wal, 2);
    }

    #[test]
    fn preflight_rejects_directory_path() {
        let dir = temp_file_path("web_preflight_dir", "wal");
        fs::create_dir_all(&dir).expect("mkdir");
        let err = prepare_persistence_file(&dir.to_string_lossy(), 1024, 2, "storage WAL")
            .expect_err("expected error");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn preflight_rotates_oversized_wal_and_bounds_backup_count() {
        let wal = temp_file_path("web_rotation", "wal");
        fs::write(&wal, vec![1u8; 64]).expect("write wal");
        fs::write(path_with_suffix(&wal, 1), vec![2u8; 8]).expect("write wal.1");
        let wal_str = wal.to_string_lossy().to_string();
        prepare_persistence_file(&wal_str, 16, 2, "storage WAL").expect("preflight");
        assert!(wal.exists());
        assert!(path_with_suffix(&wal, 1).exists());
        assert!(path_with_suffix(&wal, 2).exists());
        assert!(!path_with_suffix(&wal, 3).exists());
        cleanup_with_rotations(&wal, 3);
    }

    #[test]
    fn durable_wal_recovers_after_restart() {
        let wal = temp_file_path("web_restart", "wal");
        let wal_str = wal.to_string_lossy().to_string();
        prepare_persistence_file(&wal_str, 1_048_576, 2, "storage WAL").expect("preflight");

        let payload = sample_p10("1.2.840.1", "1.2.840.1.1", "1.2.840.1.1.1");
        let mut storage = Storage::open(Limits::default(), &wal).expect("open");
        let inserted_hash = match storage.ingest_bytes(payload.clone()).expect("ingest") {
            IngestOutcome::Inserted { hash } => hash,
            IngestOutcome::Duplicate { .. } => panic!("unexpected duplicate"),
        };
        drop(storage);

        let reopened = Storage::open(Limits::default(), &wal).expect("reopen");
        let datasets = reopened.datasets().expect("datasets");
        assert_eq!(datasets.len(), 1);
        assert_eq!(
            reopened.bytes_for_hash(&inserted_hash).map(|v| v.to_vec()),
            Some(payload)
        );
        cleanup_with_rotations(&wal, 2);
    }

    #[test]
    fn content_length_ignores_request_line_and_malformed_headers() {
        let head = "POST /studies HTTP/1.1\r\nHost example\r\ncontent-length: 15\r\n\r\n";
        assert_eq!(content_length(head), Some(15));
    }

    #[test]
    fn status_for_error_uses_structured_not_found_code() {
        let error = Error::from_kind(
            ErrorKind::NotFound {
                detail: "requested WADO instance not found".to_string(),
            },
            "not found",
        );
        assert_eq!(status_for_error(&error), (404, "Not Found"));
    }

    #[test]
    fn interop_policy_defaults_to_enabled() {
        with_interop_env(None, None, None, || {
            let policy = InteropRuntimePolicy::from_env().expect("interop policy");
            assert!(policy.qido);
            assert!(policy.wado);
            assert!(policy.stow);
        });
    }

    #[test]
    fn parse_env_bool_accepts_true_like_and_false_like_values() {
        with_interop_env(Some("1"), Some("yes"), Some("on"), || {
            let policy = InteropRuntimePolicy::from_env().expect("interop policy");
            assert!(policy.qido);
            assert!(policy.wado);
            assert!(policy.stow);
        });

        with_interop_env(Some("0"), Some("no"), Some("off"), || {
            let policy = InteropRuntimePolicy::from_env().expect("interop policy");
            assert!(!policy.qido);
            assert!(!policy.wado);
            assert!(!policy.stow);
        });
    }

    #[test]
    fn parse_env_bool_rejects_invalid_value() {
        with_interop_env(Some("maybe"), Some("yes"), Some("yes"), || {
            let err = InteropRuntimePolicy::from_env().expect_err("invalid bool must fail");
            assert!(err.to_string().contains("DICOM_WEB_ENABLE_QIDO"));
        });
    }

    #[test]
    fn parse_env_u64_uses_default_when_unset() {
        with_env_var("DICOM_WEB_TEST_STORAGE_BYTES", None, || {
            let value = parse_env_u64("DICOM_WEB_TEST_STORAGE_BYTES", 64 * 1024 * 1024).unwrap();
            assert_eq!(value, 64 * 1024 * 1024);
        });
    }

    #[test]
    fn parse_env_u64_rejects_invalid_value() {
        with_env_var("DICOM_WEB_TEST_STORAGE_BYTES", Some("invalid"), || {
            let err = parse_env_u64("DICOM_WEB_TEST_STORAGE_BYTES", 64 * 1024 * 1024)
                .expect_err("invalid value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("DICOM_WEB_TEST_STORAGE_BYTES"));
        });
    }

    #[test]
    fn parse_env_u64_accepts_positive_value() {
        with_env_var("DICOM_WEB_TEST_STORAGE_BYTES", Some("1048576"), || {
            let value = parse_env_u64("DICOM_WEB_TEST_STORAGE_BYTES", 64 * 1024 * 1024)
                .expect("positive value");
            assert_eq!(value, 1048576);
        });
    }

    #[test]
    fn parse_env_u64_rejects_out_of_range_value() {
        with_env_var("DICOM_WEB_TEST_STORAGE_BYTES", Some("0"), || {
            let err = parse_env_u64("DICOM_WEB_TEST_STORAGE_BYTES", 64 * 1024 * 1024)
                .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_env_u64_rejects_negative_value() {
        with_env_var("DICOM_WEB_TEST_STORAGE_BYTES", Some("-1"), || {
            let err = parse_env_u64("DICOM_WEB_TEST_STORAGE_BYTES", 64 * 1024 * 1024)
                .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("non-negative integer"));
        });
    }

    #[test]
    fn parse_env_usize_uses_default_when_unset() {
        with_env_var("DICOM_WEB_TEST_WORKERS", None, || {
            let value = parse_env_usize("DICOM_WEB_TEST_WORKERS", 8).unwrap();
            assert_eq!(value, 8);
        });
    }

    #[test]
    fn parse_env_usize_rejects_invalid_value() {
        with_env_var("DICOM_WEB_TEST_WORKERS", Some("bad"), || {
            let err =
                parse_env_usize("DICOM_WEB_TEST_WORKERS", 8).expect_err("invalid value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("DICOM_WEB_TEST_WORKERS"));
        });
    }

    #[test]
    fn parse_env_usize_accepts_positive_value() {
        with_env_var("DICOM_WEB_TEST_WORKERS", Some("16"), || {
            let value = parse_env_usize("DICOM_WEB_TEST_WORKERS", 8).expect("positive value");
            assert_eq!(value, 16);
        });
    }

    #[test]
    fn parse_env_usize_rejects_out_of_range_value() {
        with_env_var("DICOM_WEB_TEST_WORKERS", Some("0"), || {
            let err = parse_env_usize("DICOM_WEB_TEST_WORKERS", 8)
                .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_env_usize_rejects_negative_value() {
        with_env_var("DICOM_WEB_TEST_WORKERS", Some("-1"), || {
            let err =
                parse_env_usize("DICOM_WEB_TEST_WORKERS", 8).expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("non-negative integer"));
        });
    }

    #[test]
    fn parse_log_level_rejects_invalid_value() {
        with_env_var("DICOM_WEB_LOG_LEVEL", Some("invalid"), || {
            let err =
                parse_log_level("DICOM_WEB_LOG_LEVEL").expect_err("invalid log level must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err
                .to_string()
                .contains("service=dicom-web-server var=DICOM_WEB_LOG_LEVEL"));
        });
    }

    #[test]
    fn test_only_env_var_does_not_change_production_worker_defaults() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let prev_test_workers = std::env::var("DICOM_WEB_TEST_WORKERS").ok();
        let prev_workers = std::env::var("DICOM_WEB_WORKERS").ok();
        let prev_queue = std::env::var("DICOM_WEB_ACCEPT_QUEUE_DEPTH").ok();

        std::env::set_var("DICOM_WEB_TEST_WORKERS", "99");
        std::env::remove_var("DICOM_WEB_WORKERS");
        std::env::remove_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH");

        let contract = resolve_worker_queue_contract(4).expect("default worker contract");
        assert_eq!(contract.workers, 4);
        assert_eq!(contract.queue_depth, 32);

        match prev_test_workers {
            Some(value) => std::env::set_var("DICOM_WEB_TEST_WORKERS", value),
            None => std::env::remove_var("DICOM_WEB_TEST_WORKERS"),
        }
        match prev_workers {
            Some(value) => std::env::set_var("DICOM_WEB_WORKERS", value),
            None => std::env::remove_var("DICOM_WEB_WORKERS"),
        }
        match prev_queue {
            Some(value) => std::env::set_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH", value),
            None => std::env::remove_var("DICOM_WEB_ACCEPT_QUEUE_DEPTH"),
        }
    }

    #[test]
    fn web_auth_mode_parse_rejects_invalid_value() {
        let err = WebAuthMode::parse(Some("invalid")).expect_err("invalid auth mode must fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_WEB_AUTH_MODE"));
    }

    #[test]
    fn web_auth_mode_parse_default_is_deny_all() {
        assert_eq!(
            WebAuthMode::parse(None).expect("default parse"),
            WebAuthMode::DenyAll,
        );
    }

    #[test]
    fn blocked_interop_feature_blocks_only_when_disabled() {
        let policy_enabled = InteropRuntimePolicy {
            qido: true,
            wado: true,
            stow: true,
            delete: false,
            provider_profile: ProviderProfile::Generic,
        };
        let policy_qido_off = InteropRuntimePolicy {
            qido: false,
            wado: true,
            stow: true,
            delete: false,
            provider_profile: ProviderProfile::Generic,
        };
        let policy_wado_off = InteropRuntimePolicy {
            qido: true,
            wado: false,
            stow: true,
            delete: false,
            provider_profile: ProviderProfile::Generic,
        };
        let policy_stow_off = InteropRuntimePolicy {
            qido: true,
            wado: true,
            stow: false,
            delete: false,
            provider_profile: ProviderProfile::Generic,
        };

        assert_eq!(
            blocked_interop_feature(&HttpMethod::Get, "/studies", &policy_enabled),
            None
        );
        assert_eq!(
            blocked_interop_feature(&HttpMethod::Get, "/studies", &policy_qido_off),
            Some(InteropBlockReason::FeatureDisabled("qido"))
        );
        assert_eq!(
            blocked_interop_feature(
                &HttpMethod::Get,
                "/studies/1/series/2/instances/3",
                &policy_wado_off
            ),
            Some(InteropBlockReason::FeatureDisabled("wado"))
        );
        assert_eq!(
            blocked_interop_feature(&HttpMethod::Post, "/studies", &policy_stow_off),
            Some(InteropBlockReason::FeatureDisabled("stow"))
        );
        assert_eq!(
            blocked_interop_feature(&HttpMethod::Post, "/studies", &policy_enabled),
            None
        );
        assert_eq!(
            blocked_interop_feature(&HttpMethod::Post, "/studies/1/series", &policy_qido_off),
            None
        );
    }

    #[test]
    fn provider_profile_blocks_unsupported_dicomweb_operations() {
        let aws_policy = InteropRuntimePolicy {
            qido: true,
            wado: true,
            stow: true,
            delete: false,
            provider_profile: ProviderProfile::AwsHealthImaging,
        };
        assert_eq!(
            blocked_interop_feature(&HttpMethod::Get, "/wado", &aws_policy),
            Some(InteropBlockReason::ProviderUnsupported {
                profile: ProviderProfile::AwsHealthImaging,
                operation: "wado_uri",
            })
        );
        assert_eq!(
            blocked_interop_feature(
                &HttpMethod::Get,
                "/studies/1/series/2/instances/3/frames/1",
                &aws_policy,
            ),
            Some(InteropBlockReason::ProviderUnsupported {
                profile: ProviderProfile::AwsHealthImaging,
                operation: "frame",
            })
        );

        let dcm4chee_policy = InteropRuntimePolicy {
            qido: true,
            wado: true,
            stow: true,
            delete: true,
            provider_profile: ProviderProfile::Dcm4chee,
        };
        assert_eq!(
            blocked_interop_feature(
                &HttpMethod::Get,
                "/studies/1/series/2/instances/3/bulkdata",
                &dcm4chee_policy,
            ),
            None
        );
    }

    #[test]
    fn provider_profile_parse_rejects_invalid_value() {
        let err = with_env_var("DICOM_WEB_PROVIDER_PROFILE", Some("nope"), || {
            InteropRuntimePolicy::from_env().expect_err("invalid provider profile must fail")
        });
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_WEB_PROVIDER_PROFILE"));
    }

    #[test]
    fn classify_dicomweb_route_maps_known_paths_to_budgets() {
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Get, "/studies"),
            DicomWebRouteClass::Qido
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Get, "/studies/1"),
            DicomWebRouteClass::Wado
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Get, "/studies/1/series/2"),
            DicomWebRouteClass::Wado
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Head, "/studies/1/series/2/instances/3"),
            DicomWebRouteClass::Wado
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Get, "/wado"),
            DicomWebRouteClass::Wado
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Post, "/studies/1.2.3"),
            DicomWebRouteClass::Stow
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Get, "/healthz"),
            DicomWebRouteClass::Other
        );
        assert_eq!(
            classify_dicomweb_route(&HttpMethod::Post, "/studies/1/series"),
            DicomWebRouteClass::Other
        );
    }

    #[test]
    fn route_latency_window_calculates_percentiles_and_violations() {
        let mut tracker = RoutePerformanceTracker::new(4);
        let budget = RoutePerformanceBudgets {
            qido: RouteLatencyBudget {
                p95_ms: 11,
                p99_ms: 11,
            },
            wado: RouteLatencyBudget {
                p95_ms: 50,
                p99_ms: 60,
            },
            stow: RouteLatencyBudget {
                p95_ms: 100,
                p99_ms: 120,
            },
        };

        assert!(tracker
            .record_and_evaluate(DicomWebRouteClass::Qido, 10, &budget)
            .is_none());
        assert!(tracker
            .record_and_evaluate(DicomWebRouteClass::Qido, 12, &budget)
            .is_none());
        assert!(tracker
            .record_and_evaluate(DicomWebRouteClass::Qido, 20, &budget)
            .is_some());

        let budgets = RouteLatencyBudget {
            p95_ms: 5,
            p99_ms: 6,
        };
        let stow_budget = RoutePerformanceBudgets {
            qido: RouteLatencyBudget {
                p95_ms: 100,
                p99_ms: 200,
            },
            wado: RouteLatencyBudget {
                p95_ms: 100,
                p99_ms: 200,
            },
            stow: budgets,
        };

        let mut stow_tracker = RoutePerformanceTracker::new(3);
        let first = stow_tracker
            .record_and_evaluate(DicomWebRouteClass::Stow, 10, &stow_budget)
            .expect("violation should be emitted when budget is missed");
        assert_eq!(first.sample_count, 1);
        assert_eq!(first.sample_ms, 10);
        assert_eq!(first.p95_ms, 10);
        assert_eq!(first.p99_ms, 10);
    }

    #[test]
    fn route_latency_budget_defaults_are_loaded_from_env_when_set() {
        with_env_var("DICOM_WEB_QIDO_P95_LATENCY_MS", Some("99"), || {
            let budgets = RoutePerformanceBudgets::from_env().expect("route perf env");
            assert_eq!(budgets.qido.p95_ms, 99);
        });
        with_env_var("DICOM_WEB_STOW_P99_LATENCY_MS", Some("777"), || {
            let budgets = RoutePerformanceBudgets::from_env().expect("route perf env");
            assert_eq!(budgets.stow.p99_ms, 777);
        });
    }

    #[test]
    fn stow_json_body_serializes_multiple_outcomes() {
        let body = stow_json_body(&[
            IngestOutcome::Inserted {
                hash: "aaa".to_string(),
            },
            IngestOutcome::Duplicate {
                hash: "bbb".to_string(),
            },
        ]);
        assert_eq!(
            body,
            "{\"outcomes\":[{\"outcome\":\"inserted\",\"hash\":\"aaa\"},{\"outcome\":\"duplicate\",\"hash\":\"bbb\"}]}"
        );
    }

    #[test]
    fn diagnostics_redact_uid_path_and_host_tokens() {
        // REQ-HI-188, REQ-HI-195, REQ-HI-245
        let raw = "uid=1.2.840.10008 path=/var/data/case.dcm peer=node.internal:11112";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("1.2.840.10008"));
        assert!(!redacted.contains("/var/data/case.dcm"));
        assert!(!redacted.contains("node.internal:11112"));
        assert!(redacted.contains("[REDACTED_UID]"));
        assert!(redacted.contains("[REDACTED_PATH]"));
        assert!(redacted.contains("[REDACTED_HOST]"));
    }

    #[test]
    fn diagnostics_redact_phi_pii_keyed_and_email_tokens() {
        let raw = "patient_id=PX-0001 patient_name=Alice email=alice@example.org";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("PX-0001"));
        assert!(!redacted.contains("Alice"));
        assert!(!redacted.contains("alice@example.org"));
        assert!(redacted.contains("patient_id=[REDACTED_PII]"));
        assert!(redacted.contains("patient_name=[REDACTED_PII]"));
        assert!(redacted.contains("email=[REDACTED_PII]"));
    }
}
