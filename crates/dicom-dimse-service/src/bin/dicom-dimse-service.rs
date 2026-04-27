#![deny(missing_docs)]

//! Deterministic standalone DIMSE listener runtime.

use dicom_core::Limits;
use dicom_dimse::DimseLimits;
use dicom_dimse_service::{
    DimseAuthConfig, DimseOperationResourceLimits, DimseOperationSizeConfig, DimseRoleConfig,
    DimseServer, DimseServerConfig, DimseTlsMaterialConfig, StorageBackedDimseService,
};
use dicom_dimse_service::{TlsPolicy, TransportSecurity};
use dicom_env_contract::{
    dicom_dimse_env_contract, parse_optional_string, parse_u64, parse_usize,
    validate_envelope_version, NumericBounds, DEFAULT_DICOM_ENVELOPE_VERSION,
    DICOM_DIMSE_ENV_PREFIX, DICOM_ENVELOPE_VERSION, SUPPORTED_DICOM_ENVELOPE_VERSIONS,
};
use dicom_net::NetworkLimits;
use std::env;
use std::fs::OpenOptions;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Write};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

const DIMSE_SERVICE_NAME: &str = "dicom-dimse-service";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DimseAuthMode {
    AllowAll,
    DenyAll,
}

impl DimseAuthMode {
    fn parse(raw: Option<&str>) -> std::io::Result<Self> {
        match raw.unwrap_or("deny_all") {
            "allow_all" => Ok(Self::AllowAll),
            "deny_all" => Ok(Self::DenyAll),
            value => Err(dimse_env_parse_error(
                "DICOM_DIMSE_AUTH_MODE",
                value,
                "must be allow_all | deny_all",
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

#[derive(Debug, Clone, Copy)]
enum DimseLogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl DimseLogLevel {
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

impl From<DimseLogLevel> for u8 {
    fn from(value: DimseLogLevel) -> Self {
        match value {
            DimseLogLevel::Off => 0,
            DimseLogLevel::Error => 1,
            DimseLogLevel::Warn => 2,
            DimseLogLevel::Info => 3,
            DimseLogLevel::Debug => 4,
            DimseLogLevel::Trace => 5,
        }
    }
}

struct DimseObservability {
    service: &'static str,
    log_level: DimseLogLevel,
    telemetry_enabled: bool,
    telemetry_safe_subset: bool,
}

impl DimseObservability {
    fn from_env(prefix: &str, service: &'static str) -> std::io::Result<Self> {
        let mut log_level = DimseLogLevel::Info;
        let log_level_key = format!("{prefix}LOG_LEVEL");
        if let Some(raw) = parse_optional_string(DIMSE_SERVICE_NAME, &log_level_key)? {
            log_level = DimseLogLevel::parse(&raw).ok_or_else(|| {
                dimse_env_parse_error(
                    &log_level_key,
                    raw.trim(),
                    "must be off/error/warn/info/debug/trace",
                )
            })?;
        }

        let telemetry_enabled = parse_bool(
            DIMSE_SERVICE_NAME,
            &format!("{prefix}TELEMETRY_ENABLED"),
            false,
        )?;
        let telemetry_safe_subset = parse_bool(
            DIMSE_SERVICE_NAME,
            &format!("{prefix}TELEMETRY_SAFE_SUBSET"),
            true,
        )?;

        Ok(Self {
            service,
            log_level,
            telemetry_enabled,
            telemetry_safe_subset,
        })
    }

    fn log(&self, level: DimseLogLevel, message: &str) {
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

#[derive(Clone)]
struct DimseRuntimeConfig {
    bind: String,
    limits: Limits,
    network_limits: NetworkLimits,
    dimse_limits: DimseLimits,
    read_timeout: Duration,
    write_timeout: Duration,
    max_in_flight_associations: usize,
    max_connections: usize,
    max_in_flight_operations: usize,
    max_query_response_count: usize,
    tls_policy: TlsPolicy,
    transport_security: TransportSecurity,
    tls_material: Option<DimseTlsMaterialConfig>,
    roles: DimseRoleConfig,
    operation_sizes: DimseOperationSizeConfig,
    auth: DimseAuthConfig,
    auth_mode: DimseAuthMode,
    wal_path: String,
    wal_max_bytes: u64,
    wal_max_rotated_files: usize,
    called_ae: Option<String>,
    health_bind: Option<String>,
    allowed_hosts: Vec<std::net::IpAddr>,
}

impl DimseRuntimeConfig {
    fn from_env() -> std::io::Result<Self> {
        let bind = parse_bind_arg("--bind", "DICOM_DIMSE_BIND")
            .unwrap_or_else(|| "127.0.0.1:11112".to_string());
        let limits = parse_limits()?;
        let network_limits = parse_network_limits()?;
        let dimse_limits = parse_dimse_limits()?;
        let read_timeout = parse_duration_secs("DICOM_DIMSE_READ_TIMEOUT_SECS", 30)?;
        let write_timeout = parse_duration_secs("DICOM_DIMSE_WRITE_TIMEOUT_SECS", 30)?;
        let max_in_flight_associations = parse_usize(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS",
            64,
            NumericBounds::at_least(1),
        )?;
        let max_connections = parse_usize(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_CONNECTIONS",
            max_in_flight_associations,
            NumericBounds::at_least(1),
        )?;
        let max_in_flight_operations = parse_usize(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS",
            64,
            NumericBounds::at_least(1),
        )?;
        let max_query_response_count = parse_usize(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT",
            4_096,
            NumericBounds::at_least(1),
        )?;
        let tls_policy_raw = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_TLS_POLICY")?;
        let transport_security_raw =
            parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_TRANSPORT_SECURITY")?;
        let tls_policy = parse_tls_policy(tls_policy_raw.as_deref())?;
        let transport_security = parse_transport_security(transport_security_raw.as_deref())?;
        let tls_cert_path = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_TLS_CERT_PATH")?;
        let tls_key_path = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_TLS_KEY_PATH")?;
        let tls_ca_bundle =
            parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_TLS_CA_BUNDLE_PATH")?;
        let tls_cert_rotation_interval_secs = parse_optional_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS",
            NumericBounds::at_least(0),
        )?;
        let tls_material = parse_tls_material(
            transport_security,
            tls_cert_path.as_deref(),
            tls_key_path.as_deref(),
            tls_ca_bundle.as_deref(),
            tls_cert_rotation_interval_secs,
        )?;
        let roles = parse_role_config()?;
        let auth_mode_raw = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_AUTH_MODE")?;
        let auth_mode = DimseAuthMode::parse(auth_mode_raw.as_deref())?;
        let auth = parse_auth_config(auth_mode);
        let operation_sizes = parse_operation_size_limits(&limits)?;
        let wal_path = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_STORAGE_WAL")?
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "./state/dicom-dimse/storage.wal".to_string());
        let wal_max_bytes = parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_STORAGE_WAL_MAX_BYTES",
            128 * 1024 * 1024,
            NumericBounds::at_least(1),
        )?;
        let wal_max_rotated_files = parse_usize(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES",
            3,
            NumericBounds::at_least(1),
        )?;
        let called_ae = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_CALLED_AE")?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let health_bind = parse_optional_string(DIMSE_SERVICE_NAME, "DICOM_DIMSE_HEALTH_BIND")?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let allowed_hosts = parse_allowed_hosts("DICOM_DIMSE_ALLOWED_HOSTS")?;

        Ok(Self {
            bind,
            limits,
            network_limits,
            dimse_limits,
            read_timeout,
            write_timeout,
            max_in_flight_associations,
            max_connections,
            max_in_flight_operations,
            max_query_response_count,
            tls_policy,
            transport_security,
            tls_material,
            roles,
            operation_sizes,
            auth,
            auth_mode,
            wal_path,
            wal_max_bytes,
            wal_max_rotated_files,
            called_ae,
            health_bind,
            allowed_hosts,
        })
    }
}

/// Start the DIMSE listener.
fn main() -> std::io::Result<()> {
    let contract = dicom_dimse_env_contract(cfg!(test));

    let envelope_version = validate_envelope_version(
        DIMSE_SERVICE_NAME,
        DICOM_ENVELOPE_VERSION,
        SUPPORTED_DICOM_ENVELOPE_VERSIONS,
        DEFAULT_DICOM_ENVELOPE_VERSION,
    )?;

    if has_flag("--print-env-contract") {
        println!("{}", contract.snapshot_json(&envelope_version));
        return Ok(());
    }

    contract.validate()?;

    let runtime_config = DimseRuntimeConfig::from_env()?;
    let observability =
        DimseObservability::from_env(DICOM_DIMSE_ENV_PREFIX, "dicom-dimse-service")?;
    let bind = runtime_config.bind;
    let bind_addr = bind.parse::<std::net::SocketAddr>().map_err(|err| {
        IoError::new(
            IoErrorKind::InvalidInput,
            format!("invalid bind address: {err}"),
        )
    })?;
    let limits = runtime_config.limits;
    let network_limits = runtime_config.network_limits;
    let dimse_limits = runtime_config.dimse_limits;
    let read_timeout = runtime_config.read_timeout;
    let write_timeout = runtime_config.write_timeout;
    let max_connections = runtime_config.max_connections;
    let max_in_flight_operations = runtime_config.max_in_flight_operations;
    let max_query_response_count = runtime_config.max_query_response_count;
    let tls_policy = runtime_config.tls_policy;
    let transport_security = runtime_config.transport_security;
    let tls_material = runtime_config.tls_material;
    let roles = runtime_config.roles;
    let operation_sizes = runtime_config.operation_sizes;
    let auth = runtime_config.auth;
    let auth_mode = runtime_config.auth_mode;
    let wal_path = runtime_config.wal_path;
    let wal_max_bytes = runtime_config.wal_max_bytes;
    let wal_max_rotated_files = runtime_config.wal_max_rotated_files;
    let allowed_hosts = runtime_config.allowed_hosts;
    let called_ae = runtime_config.called_ae;
    let health_bind = runtime_config.health_bind;

    preflight_storage(&wal_path, wal_max_rotated_files)?;

    let service = StorageBackedDimseService::open(limits.clone(), &wal_path)
        .map_err(|err| io_error("failed to open DIMSE storage WAL", err))?;

    let mut server_config = DimseServerConfig::default();
    server_config.network_limits = network_limits;
    server_config.dimse_limits = dimse_limits;
    server_config.limits = limits;
    server_config.read_timeout = read_timeout;
    server_config.write_timeout = write_timeout;
    server_config.max_connections = max_connections;
    server_config.operation_resource_limits = DimseOperationResourceLimits {
        max_in_flight_operations,
        max_query_responses: max_query_response_count,
    };
    server_config.tls_policy = tls_policy;
    server_config.transport_security = transport_security;
    server_config.roles = roles;
    server_config.operation_sizes = operation_sizes;
    server_config.auth = auth;
    server_config.tls_material = tls_material;
    if !allowed_hosts.is_empty() {
        server_config.allowed_hosts = Some(allowed_hosts);
    }
    if let Some(called_ae) = called_ae {
        server_config.policy.called_ae = Some(called_ae);
    }
    let readiness = Arc::new(AtomicBool::new(false));
    let health_running = Arc::new(AtomicBool::new(true));
    let health_server = health_bind
        .as_deref()
        .map(|addr| {
            spawn_health_listener(addr, Arc::clone(&readiness), Arc::clone(&health_running))
        })
        .transpose()?;
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service contract: bind={bind}, tls_policy={:?}, transport_security={:?}, auth={}, max_connections={}, read_timeout={:?}, write_timeout={:?}",
            server_config.tls_policy,
            server_config.transport_security,
            auth_mode.as_str(),
            server_config.max_connections,
            server_config.read_timeout,
            server_config.write_timeout,
        ),
    );
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service operation cap config: max_in_flight_operations={}, max_query_responses={}",
            server_config.operation_resource_limits.max_in_flight_operations,
            server_config.operation_resource_limits.max_query_responses,
        ),
    );
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service storage contract: wal={wal_path}, wal_max_bytes={wal_max_bytes}, wal_max_rotated_files={wal_max_rotated_files}",
        ),
    );
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service feature contract: {}",
            dimse_feature_contract()
        ),
    );
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service role config: c_echo={}, c_store={}, c_find={}, c_move={}, c_get={}",
            server_config.roles.c_echo_enabled,
            server_config.roles.c_store_enabled,
            server_config.roles.c_find_enabled,
            server_config.roles.c_move_enabled,
            server_config.roles.c_get_enabled,
        ),
    );
    observability.log(
        DimseLogLevel::Info,
        &format!(
            "dicom-dimse-service operation size config: c_echo_data_set_bytes={}, c_store_data_set_bytes={}, c_find_data_set_bytes={}, c_move_data_set_bytes={}, c_get_data_set_bytes={}",
            server_config.operation_sizes.c_echo_data_set_bytes,
            server_config.operation_sizes.c_store_data_set_bytes,
            server_config.operation_sizes.c_find_data_set_bytes,
            server_config.operation_sizes.c_move_data_set_bytes,
            server_config.operation_sizes.c_get_data_set_bytes,
        ),
    );
    if let Some(material) = &server_config.tls_material {
        observability.log(
            DimseLogLevel::Info,
            &format!(
                "dicom-dimse-service tls contract: cert={}, key={}, ca_bundle={}, cert_rotation_interval_secs={:?}",
                material.certificate_path,
                material.private_key_path,
                material.ca_bundle_path.as_deref().unwrap_or("<not-set>"),
                material.cert_rotation_interval_secs,
            ),
        );
    } else {
        observability.log(
            DimseLogLevel::Info,
            "dicom-dimse-service tls contract: <not-configured>",
        );
    }
    if let Some(addr) = &health_bind {
        observability.log(
            DimseLogLevel::Info,
            &format!("dicom-dimse-service health endpoint enabled: {addr}"),
        );
    } else {
        observability.log(
            DimseLogLevel::Info,
            "dicom-dimse-service health endpoint: disabled",
        );
    }
    observability.emit_telemetry(
        "service_start",
        &[
            ("bind", &bind),
            ("tls", &format!("{:?}", server_config.tls_policy)),
            (
                "transport",
                &format!("{:?}", server_config.transport_security),
            ),
            ("auth", auth_mode.as_str()),
            (
                "max_connections",
                &server_config.max_connections.to_string(),
            ),
            (
                "max_in_flight_operations",
                &server_config
                    .operation_resource_limits
                    .max_in_flight_operations
                    .to_string(),
            ),
            (
                "max_query_responses",
                &server_config
                    .operation_resource_limits
                    .max_query_responses
                    .to_string(),
            ),
            (
                "read_timeout_ms",
                &server_config.read_timeout.as_secs().to_string(),
            ),
            (
                "write_timeout_ms",
                &server_config.write_timeout.as_secs().to_string(),
            ),
            ("wal", &wal_path),
        ],
    );

    let server = DimseServer::bind(bind_addr, server_config, service)
        .map_err(|err| io_error("failed to bind DIMSE service", err))?;

    readiness.store(true, Ordering::Release);
    let result = server
        .run()
        .map_err(|err| io_error("DIMSE server runtime failure", err));
    readiness.store(false, Ordering::Release);
    health_running.store(false, Ordering::Release);
    drop(health_server);
    result
}

fn parse_allowed_hosts(name: &str) -> std::io::Result<Vec<std::net::IpAddr>> {
    let raw = match parse_optional_string(DIMSE_SERVICE_NAME, name)? {
        Some(raw) => raw,
        None => return Ok(Vec::new()),
    };

    let mut out = Vec::new();
    for entry in raw.split(',') {
        let host = entry.trim();
        if host.is_empty() {
            continue;
        }
        out.push(host.parse::<std::net::IpAddr>().map_err(|_| {
            IoError::new(
                IoErrorKind::InvalidInput,
                format!("invalid IP in {name}: {host}"),
            )
        })?);
    }
    Ok(out)
}

fn parse_auth_config(auth_mode: DimseAuthMode) -> DimseAuthConfig {
    match auth_mode {
        DimseAuthMode::AllowAll => DimseAuthConfig::allow_all(),
        DimseAuthMode::DenyAll => DimseAuthConfig::deny_all(),
    }
}

fn dimse_env_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!(
            "service={DIMSE_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}",
        ),
    )
}

fn dimse_feature_contract() -> String {
    let find_supported = if cfg!(feature = "dimse-c-find") {
        "enabled"
    } else {
        "disabled"
    };
    let move_supported = if cfg!(feature = "dimse-c-move") {
        "enabled"
    } else {
        "disabled"
    };
    let get_supported = if cfg!(feature = "dimse-c-get") {
        "enabled"
    } else {
        "disabled"
    };

    format!("c_echo,c_store,c_find={find_supported},c_move={move_supported},c_get={get_supported}")
}

fn parse_tls_policy(tls_policy_raw: Option<&str>) -> std::io::Result<TlsPolicy> {
    match tls_policy_raw {
        None | Some("require_tls") => Ok(TlsPolicy::RequireTls),
        Some("allow_insecure") => Ok(TlsPolicy::AllowInsecure),
        Some(value) => Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("unsupported DICOM_DIMSE_TLS_POLICY: {value}"),
        )),
    }
}

fn parse_transport_security(
    transport_security_raw: Option<&str>,
) -> std::io::Result<TransportSecurity> {
    match transport_security_raw {
        None | Some("insecure") => Ok(TransportSecurity::Insecure),
        Some("tls") => Ok(TransportSecurity::Tls),
        Some(value) => Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("unsupported DICOM_DIMSE_TRANSPORT_SECURITY: {value}"),
        )),
    }
}

fn parse_tls_material(
    transport_security: TransportSecurity,
    cert_path: Option<&str>,
    key_path: Option<&str>,
    ca_bundle: Option<&str>,
    cert_rotation_interval_secs: Option<u64>,
) -> std::io::Result<Option<DimseTlsMaterialConfig>> {
    if transport_security != TransportSecurity::Tls {
        if cert_path.is_some() || key_path.is_some() || ca_bundle.is_some() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "TLS material is only valid when DICOM_DIMSE_TRANSPORT_SECURITY=tls",
            ));
        }
        if cert_rotation_interval_secs.is_some() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "TLS cert rotation interval is only valid when DICOM_DIMSE_TRANSPORT_SECURITY=tls",
            ));
        }
        return Ok(None);
    }

    let (certificate_path, private_key_path) = match (cert_path, key_path) {
        (Some(cert), Some(key)) => (cert, key),
        (None, None) => {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_DIMSE_TRANSPORT_SECURITY=tls requires DICOM_DIMSE_TLS_CERT_PATH and DICOM_DIMSE_TLS_KEY_PATH",
            ))
        }
        (None, Some(_)) => {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_DIMSE_TRANSPORT_SECURITY=tls requires DICOM_DIMSE_TLS_CERT_PATH",
            ))
        }
        (Some(_), None) => {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_DIMSE_TRANSPORT_SECURITY=tls requires DICOM_DIMSE_TLS_KEY_PATH",
            ))
        }
    };

    if !Path::new(&certificate_path).is_file() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!(
                "DICOM_DIMSE_TLS_CERT_PATH must reference an existing file: {certificate_path}"
            ),
        ));
    }
    if !Path::new(&private_key_path).is_file() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("DICOM_DIMSE_TLS_KEY_PATH must reference an existing file: {private_key_path}"),
        ));
    }
    if let Some(path) = ca_bundle.as_ref() {
        if !Path::new(path).is_file() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                format!("DICOM_DIMSE_TLS_CA_BUNDLE_PATH must reference an existing file: {path}"),
            ));
        }
    }

    if let Some(interval_secs) = cert_rotation_interval_secs {
        if interval_secs == 0 {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS must be greater than zero",
            ));
        }
    }

    Ok(Some(DimseTlsMaterialConfig {
        certificate_path: certificate_path.to_string(),
        private_key_path: private_key_path.to_string(),
        ca_bundle_path: ca_bundle.map(str::to_string),
        cert_rotation_interval_secs,
    }))
}

fn parse_limits() -> std::io::Result<Limits> {
    // Defaults and bounds in this parser are the runtime source for docs/09 and docs/14.
    let mut limits = Limits::default();
    limits.set_max_input_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_INPUT_BYTES",
        limits.max_input_bytes(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_dataset_elements(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_DATASET_ELEMENTS",
        limits.max_dataset_elements(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_sequence_depth(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_SEQUENCE_DEPTH",
        limits.max_sequence_depth(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_string_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_STRING_BYTES",
        limits.max_string_bytes(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_element_vl_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_ELEMENT_VL_BYTES",
        limits.max_element_vl_bytes(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_frames_per_instance(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE",
        limits.max_frames_per_instance(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_pixels_per_frame(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_PIXELS_PER_FRAME",
        limits.max_pixels_per_frame(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_decompressed_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_DECOMPRESSED_BYTES",
        limits.max_decompressed_bytes(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_gpu_texture_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES",
        limits.max_gpu_texture_bytes(),
        NumericBounds::at_least(1),
    )?);
    limits.set_max_cache_bytes(parse_u64(
        DIMSE_SERVICE_NAME,
        "DICOM_DIMSE_MAX_CACHE_BYTES",
        limits.max_cache_bytes(),
        NumericBounds::at_least(1),
    )?);
    Ok(limits)
}

fn parse_network_limits() -> std::io::Result<NetworkLimits> {
    Ok(NetworkLimits {
        max_pdu_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_PDU_BYTES",
            1024 * 1024,
            NumericBounds::at_least(1),
        )?,
        max_pdv_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_PDV_BYTES",
            256 * 1024,
            NumericBounds::at_least(1),
        )?,
        max_presentation_contexts: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS",
            128,
            NumericBounds::at_least(1),
        )?,
    })
}

fn parse_dimse_limits() -> std::io::Result<DimseLimits> {
    Ok(DimseLimits {
        max_command_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_COMMAND_BYTES",
            64 * 1024,
            NumericBounds::at_least(1),
        )?,
    })
}

fn parse_duration_secs(name: &str, default_secs: u64) -> std::io::Result<Duration> {
    parse_u64(
        DIMSE_SERVICE_NAME,
        name,
        default_secs,
        NumericBounds::at_least(1),
    )
    .map(Duration::from_secs)
}

fn parse_role_config() -> std::io::Result<DimseRoleConfig> {
    Ok(DimseRoleConfig {
        c_echo_enabled: parse_bool(DIMSE_SERVICE_NAME, "DICOM_DIMSE_ROLE_C_ECHO_ENABLED", true)?,
        c_store_enabled: parse_bool(DIMSE_SERVICE_NAME, "DICOM_DIMSE_ROLE_C_STORE_ENABLED", true)?,
        c_find_enabled: parse_bool(DIMSE_SERVICE_NAME, "DICOM_DIMSE_ROLE_C_FIND_ENABLED", true)?,
        c_move_enabled: parse_bool(DIMSE_SERVICE_NAME, "DICOM_DIMSE_ROLE_C_MOVE_ENABLED", true)?,
        c_get_enabled: parse_bool(DIMSE_SERVICE_NAME, "DICOM_DIMSE_ROLE_C_GET_ENABLED", true)?,
    })
}

fn parse_operation_size_limits(limits: &Limits) -> std::io::Result<DimseOperationSizeConfig> {
    let default_limit = limits.max_input_bytes();
    Ok(DimseOperationSizeConfig {
        c_echo_data_set_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES",
            0,
            NumericBounds::at_least(0),
        )?,
        c_store_data_set_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES",
            default_limit,
            NumericBounds::at_least(1),
        )?,
        c_find_data_set_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES",
            default_limit,
            NumericBounds::at_least(1),
        )?,
        c_move_data_set_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES",
            default_limit,
            NumericBounds::at_least(1),
        )?,
        c_get_data_set_bytes: parse_u64(
            DIMSE_SERVICE_NAME,
            "DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES",
            default_limit,
            NumericBounds::at_least(1),
        )?,
    })
}

fn parse_env_u64(name: &str, default: u64) -> std::io::Result<u64> {
    parse_u64(
        DIMSE_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

fn parse_env_usize(name: &str, default: usize) -> std::io::Result<usize> {
    parse_usize(
        DIMSE_SERVICE_NAME,
        name,
        default,
        NumericBounds::at_least(1),
    )
}

fn parse_bool(service: &'static str, name: &str, default: bool) -> std::io::Result<bool> {
    let raw = parse_optional_string(service, name)?;
    let Some(raw) = raw else {
        return Ok(default);
    };
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(dimse_env_parse_error(
            name,
            raw.trim(),
            "must be true/false",
        )),
    }
}

fn parse_optional_u64(
    service: &'static str,
    name: &str,
    bounds: NumericBounds,
) -> std::io::Result<Option<u64>> {
    let raw = parse_optional_string(service, name)?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let normalized = raw.trim();
    let parsed = normalized.parse::<u64>().map_err(|_| {
        dimse_env_parse_error(
            name,
            normalized,
            "must be a non-negative integer for parse_optional_u64",
        )
    })?;
    if let Some(min) = bounds.min {
        if parsed < min {
            return Err(dimse_env_parse_error(
                name,
                normalized,
                "below minimum bound",
            ));
        }
    }
    if let Some(max) = bounds.max {
        if parsed > max {
            return Err(dimse_env_parse_error(
                name,
                normalized,
                "above maximum bound",
            ));
        }
    }
    Ok(Some(parsed))
}

fn preflight_storage(path: &str, max_rotated_files: usize) -> std::io::Result<()> {
    let parent = Path::new(path)
        .parent()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "invalid DIMSE storage path"))?;
    if !parent.as_os_str().is_empty() {
        std::fs::create_dir_all(parent).map_err(|err| {
            IoError::new(
                err.kind(),
                format!("failed to create DIMSE storage parent directory: {err}"),
            )
        })?;
    }

    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut handle| handle.write_all(&[]))
        .map_err(|err| {
            IoError::new(
                err.kind(),
                format!("DIMSE storage file is not writable: {err}"),
            )
        })?;

    if max_rotated_files == 0 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES must be >= 1",
        ));
    }

    Ok(())
}

fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|arg| arg == name)
}

fn parse_bind_arg(flag: &str, env_name: &str) -> Option<String> {
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next();
        }
    }
    env::var(env_name).ok()
}

fn io_error<T: core::fmt::Display>(message: &str, err: T) -> IoError {
    IoError::new(IoErrorKind::Other, format!("{message}: {err}"))
}

fn json_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('\"', "\\\"")
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

fn spawn_health_listener(
    addr: &str,
    readiness: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = std::net::TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    Ok(thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            match listener.incoming().next() {
                Some(Ok(mut stream)) => {
                    let mut buffer = [0u8; 512];
                    let read = stream.read(&mut buffer).unwrap_or_default();
                    let request = String::from_utf8_lossy(&buffer[..read]);

                    let (status_line, body) = if request.starts_with("GET /healthz") {
                        ("200 OK", "ok")
                    } else if request.starts_with("GET /readyz") {
                        if readiness.load(Ordering::Acquire) {
                            ("200 OK", "ready")
                        } else {
                            ("503 Service Unavailable", "not ready")
                        }
                    } else {
                        ("404 Not Found", "not found")
                    };

                    let response = format!(
                        "HTTP/1.1 {status_line}\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                }
                Some(Err(err)) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Some(Err(_)) => {
                    break;
                }
                None => break,
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{ErrorKind as IoErrorKind, Read};
    use std::sync::Mutex;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

    fn next_free_port() -> u16 {
        let listener =
            std::net::TcpListener::bind("127.0.0.1:0").expect("allocate free loopback port");
        listener.local_addr().expect("listener local addr").port()
    }

    fn now_epoch_millis() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after unix epoch")
            .as_millis()
    }

    #[test]
    fn parse_env_u64_uses_default_when_unset() {
        with_env_var("DICOM_DIMSE_TEST_MAX_BYTES", None, || {
            let value = parse_env_u64("DICOM_DIMSE_TEST_MAX_BYTES", 4096).expect("default");
            assert_eq!(value, 4096);
        });
    }

    #[test]
    fn parse_env_u64_rejects_invalid_value() {
        with_env_var("DICOM_DIMSE_TEST_MAX_BYTES", Some("bad"), || {
            let err = parse_env_u64("DICOM_DIMSE_TEST_MAX_BYTES", 4096)
                .expect_err("invalid value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("DICOM_DIMSE_TEST_MAX_BYTES"));
        });
    }

    #[test]
    fn parse_env_u64_rejects_out_of_range_value() {
        with_env_var("DICOM_DIMSE_TEST_MAX_BYTES", Some("0"), || {
            let err = parse_env_u64("DICOM_DIMSE_TEST_MAX_BYTES", 4096)
                .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_env_u64_accepts_positive_value() {
        with_env_var("DICOM_DIMSE_TEST_MAX_BYTES", Some("8192"), || {
            let value = parse_env_u64("DICOM_DIMSE_TEST_MAX_BYTES", 4096).expect("positive value");
            assert_eq!(value, 8192);
        });
    }

    #[test]
    fn parse_env_u64_rejects_negative_value() {
        with_env_var("DICOM_DIMSE_TEST_MAX_BYTES", Some("-1"), || {
            let err = parse_env_u64("DICOM_DIMSE_TEST_MAX_BYTES", 4096)
                .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("non-negative integer"));
        });
    }

    #[test]
    fn parse_optional_u64_rejects_invalid_value() {
        with_env_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", Some("bad"), || {
            let err = parse_optional_u64(
                DIMSE_SERVICE_NAME,
                "DICOM_DIMSE_TEST_OPTIONAL_SECONDS",
                NumericBounds::at_least(0),
            )
            .expect_err("invalid optional value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err
                .to_string()
                .contains("DICOM_DIMSE_TEST_OPTIONAL_SECONDS"));
        });
    }

    #[test]
    fn parse_optional_u64_returns_none_when_unset() {
        with_env_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", None, || {
            let value = parse_optional_u64(
                DIMSE_SERVICE_NAME,
                "DICOM_DIMSE_TEST_OPTIONAL_SECONDS",
                NumericBounds::at_least(0),
            )
            .expect("optional parse");
            assert_eq!(value, None);
        });
    }

    #[test]
    fn parse_optional_u64_allows_zero() {
        with_env_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", Some("0"), || {
            let value = parse_optional_u64(
                DIMSE_SERVICE_NAME,
                "DICOM_DIMSE_TEST_OPTIONAL_SECONDS",
                NumericBounds::at_least(0),
            )
            .expect("optional parse");
            assert_eq!(value, Some(0));
        });
    }

    #[test]
    fn parse_optional_u64_rejects_negative_value() {
        with_env_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", Some("-1"), || {
            let err = parse_optional_u64(
                DIMSE_SERVICE_NAME,
                "DICOM_DIMSE_TEST_OPTIONAL_SECONDS",
                NumericBounds::at_least(0),
            )
            .expect_err("negative optional value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("non-negative integer"));
        });
    }

    #[test]
    fn parse_auth_mode_rejects_invalid_value() {
        let err = DimseAuthMode::parse(Some("invalid")).expect_err("invalid auth mode should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_DIMSE_AUTH_MODE"));
    }

    #[test]
    fn parse_auth_mode_defaults_to_deny_all() {
        assert_eq!(
            DimseAuthMode::parse(None).expect("default auth mode"),
            DimseAuthMode::DenyAll,
        );
    }

    #[test]
    fn parse_tls_policy_defaults_to_require_tls() {
        let value = parse_tls_policy(None).expect("default tls policy");
        assert_eq!(value, TlsPolicy::RequireTls);
    }

    #[test]
    fn parse_transport_security_defaults_to_insecure() {
        let value = parse_transport_security(None).expect("default transport security");
        assert_eq!(value, TransportSecurity::Insecure);
    }

    #[test]
    fn preflight_storage_rejects_zero_rotated_file_retention() {
        let root = std::env::temp_dir().join(format!(
            "dimse_preflight_zero_retention_{}",
            now_epoch_millis()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let wal_path = root.join("storage.wal");

        let err = preflight_storage(&wal_path.to_string_lossy(), 0)
            .expect_err("zero rotation retention should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains("DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn preflight_storage_creates_wal_for_positive_rotated_file_retention() {
        let root = std::env::temp_dir().join(format!(
            "dimse_preflight_positive_retention_{}",
            now_epoch_millis()
        ));
        std::fs::create_dir_all(&root).expect("create temp root");
        let wal_path = root.join("storage.wal");

        preflight_storage(&wal_path.to_string_lossy(), 2)
            .expect("positive rotation retention should pass");
        assert!(wal_path.exists());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn diagnostics_redact_phi_pii_uid_path_and_host_tokens() {
        let raw = "patient_id=PX-44 uid=1.2.840.10008 email=alice@example.org path=/var/dimse/storage.wal peer=dimse.internal:11112";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("PX-44"));
        assert!(!redacted.contains("1.2.840.10008"));
        assert!(!redacted.contains("alice@example.org"));
        assert!(!redacted.contains("/var/dimse/storage.wal"));
        assert!(!redacted.contains("dimse.internal:11112"));
        assert!(redacted.contains("patient_id=[REDACTED_PII]"));
        assert!(redacted.contains("uid=[REDACTED_UID]"));
        assert!(redacted.contains("email=[REDACTED_PII]"));
        assert!(redacted.contains("path=[REDACTED_PATH]"));
        assert!(redacted.contains("peer=[REDACTED_HOST]"));
    }

    #[test]
    fn test_only_env_vars_do_not_change_production_limits_defaults() {
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let prev_test_max = std::env::var("DICOM_DIMSE_TEST_MAX_BYTES").ok();
        let prev_test_optional = std::env::var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS").ok();
        let prev_max_input = std::env::var("DICOM_DIMSE_MAX_INPUT_BYTES").ok();

        std::env::set_var("DICOM_DIMSE_TEST_MAX_BYTES", "9999");
        std::env::set_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", "9999");
        std::env::remove_var("DICOM_DIMSE_MAX_INPUT_BYTES");

        let limits = parse_limits().expect("production limits defaults");
        assert_eq!(
            limits.max_input_bytes(),
            Limits::default().max_input_bytes()
        );

        match prev_test_max {
            Some(value) => std::env::set_var("DICOM_DIMSE_TEST_MAX_BYTES", value),
            None => std::env::remove_var("DICOM_DIMSE_TEST_MAX_BYTES"),
        }
        match prev_test_optional {
            Some(value) => std::env::set_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS", value),
            None => std::env::remove_var("DICOM_DIMSE_TEST_OPTIONAL_SECONDS"),
        }
        match prev_max_input {
            Some(value) => std::env::set_var("DICOM_DIMSE_MAX_INPUT_BYTES", value),
            None => std::env::remove_var("DICOM_DIMSE_MAX_INPUT_BYTES"),
        }
    }

    fn get_response(addr: &str, path: &str) -> String {
        let mut stream = std::net::TcpStream::connect(addr).expect("connect to health listener");
        let request =
            format!("GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n");
        stream
            .write_all(request.as_bytes())
            .expect("send health request");
        let mut buffer = String::new();
        stream
            .read_to_string(&mut buffer)
            .expect("read health response");
        buffer
    }

    fn wait_for_listen(addr: &str) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if std::net::TcpStream::connect(addr).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("health listener did not start on {addr}");
    }

    #[test]
    fn health_listener_exposes_health_and_readiness_contract() {
        let readiness = Arc::new(AtomicBool::new(false));
        let running = Arc::new(AtomicBool::new(true));
        let health_port = next_free_port();
        let addr = format!("127.0.0.1:{health_port}");
        let listener = spawn_health_listener(&addr, Arc::clone(&readiness), Arc::clone(&running))
            .expect("spawn health listener");

        wait_for_listen(&addr);

        assert!(get_response(&addr, "/healthz").starts_with("HTTP/1.1 200 OK"));
        assert!(get_response(&addr, "/readyz").starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(get_response(&addr, "/").starts_with("HTTP/1.1 404 Not Found"));

        readiness.store(true, Ordering::Release);
        assert!(get_response(&addr, "/readyz").starts_with("HTTP/1.1 200 OK"));

        readiness.store(false, Ordering::Release);
        running.store(false, Ordering::Release);
        listener.join().expect("health listener thread should stop");
    }
}
