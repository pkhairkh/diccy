//! Environment-variable parsing, TLS/transport defaults, storage preflight,
//! diagnostic redaction, and health-listener utilities shared between the
//! library and the DIMSE service binary.

use dicom_core::Limits;
use dicom_dimse::DimseLimits;
#[allow(missing_docs)]
pub use dicom_env_contract::{
    parse_optional_string, parse_optional_u64, parse_u64, NumericBounds,
    DICOM_DIMSE_ENV_PREFIX,
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

use crate::transport::{TlsPolicy, TransportSecurity};

/// Service name constant used in diagnostics and env-var parsing.
pub const DIMSE_SERVICE_NAME: &str = "dicom-dimse-service";

/// DIMSE authorization mode parsed from `DICOM_DIMSE_AUTH_MODE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimseAuthMode {
    /// Allow all requests (insecure – testing only).
    AllowAll,
    /// Deny all requests (fail-closed default).
    DenyAll,
}

impl DimseAuthMode {
    /// Parse an auth-mode value from the environment variable.
    pub fn parse(raw: Option<&str>) -> std::io::Result<Self> {
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
}

/// Parse a required `u64` environment variable with a minimum bound of 1.
pub fn parse_env_u64(name: &str, default: u64) -> std::io::Result<u64> {
    parse_u64(DIMSE_SERVICE_NAME, name, default, NumericBounds::at_least(1))
}

/// Parse a TLS policy from an optional string value.
pub fn parse_tls_policy(tls_policy_raw: Option<&str>) -> std::io::Result<TlsPolicy> {
    match tls_policy_raw {
        None | Some("require_tls") => Ok(TlsPolicy::RequireTls),
        Some("allow_insecure") => Ok(TlsPolicy::AllowInsecure),
        Some(value) => Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("unsupported DICOM_DIMSE_TLS_POLICY: {value}"),
        )),
    }
}

/// Parse a transport-security value from an optional string value.
pub fn parse_transport_security(
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

/// Validate storage paths and writable WAL before startup.
pub fn preflight_storage(path: &str, max_rotated_files: usize) -> std::io::Result<()> {
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

/// Parse production `Limits` from environment variables.
pub fn parse_limits() -> std::io::Result<Limits> {
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

/// Redact PHI, PII, UIDs, paths, and host:port tokens from a diagnostic message.
pub fn redact_diagnostic_message(message: &str) -> String {
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

/// Spawn a simple HTTP health-listener thread.
pub fn spawn_health_listener(
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

fn dimse_env_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!(
            "service={DIMSE_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}",
        ),
    )
}
