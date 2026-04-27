// Auto-extracted from /home/z/diccy/crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs
// S13-T8: Move inline tests to tests/ directories


use dicom_dimse_service::*;
use dicom_dimse_service::env_config::{
    parse_env_u64, parse_limits, parse_optional_u64, parse_tls_policy,
    parse_transport_security, preflight_storage, redact_diagnostic_message,
    spawn_health_listener, DimseAuthMode, DIMSE_SERVICE_NAME,
};
use dicom_core::Limits;
use dicom_env_contract::NumericBounds;
use std::io::{ErrorKind as IoErrorKind, Read, Write};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use dicom_core::ErrorKind;
use std::sync::Arc;

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
