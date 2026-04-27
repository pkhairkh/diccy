// Auto-extracted from /home/z/diccy/crates/dicom-env-contract/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_env_contract::*;
use std::io;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_env_var<F, R>(name: &str, value: Option<&str>, f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_LOCK.lock().expect("env lock");
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

#[test]
fn parse_u64_respects_default_when_unset() {
    with_env_var("DICOM_TEST_ALPHA", None, || {
        let value = parse_u64(
            "dicom-test",
            "DICOM_TEST_ALPHA",
            100,
            NumericBounds::unbounded(),
        )
        .expect("default");
        assert_eq!(value, 100);
    });
}

#[test]
fn parse_u64_rejects_out_of_range() {
    with_env_var("DICOM_TEST_ALPHA", Some("200"), || {
        let err = parse_u64(
            "dicom-test",
            "DICOM_TEST_ALPHA",
            100,
            NumericBounds::at_most(128),
        )
        .expect_err("range violation");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("must be <= 128"));
    });
}

#[test]
fn parse_usize_rejects_invalid_default() {
    let bounds = NumericBounds::with_range(10, 20);
    let err = parse_usize("dicom-test", "DICOM_TEST_BETA", 4, bounds).expect_err("bad default");
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert!(err.to_string().contains("DICOM_TEST_BETA"));
}

#[test]
fn parse_bool_defaults_when_unset() {
    with_env_var("DICOM_TEST_BOOL", None, || {
        assert!(parse_bool("dicom-test", "DICOM_TEST_BOOL", true).expect("default"));
    });
}

#[test]
fn validate_envelope_version_uses_default_when_unset() {
    with_env_var(DICOM_ENVELOPE_VERSION, None, || {
        let version = validate_envelope_version(
            "dicom-test",
            DICOM_ENVELOPE_VERSION,
            SUPPORTED_DICOM_ENVELOPE_VERSIONS,
            DEFAULT_DICOM_ENVELOPE_VERSION,
        )
        .expect("default envelope version");
        assert_eq!(version, DEFAULT_DICOM_ENVELOPE_VERSION);
    });
}

#[test]
fn validate_envelope_version_rejects_unknown_value() {
    with_env_var(DICOM_ENVELOPE_VERSION, Some("9.9"), || {
        let err = validate_envelope_version(
            "dicom-test",
            DICOM_ENVELOPE_VERSION,
            SUPPORTED_DICOM_ENVELOPE_VERSIONS,
            DEFAULT_DICOM_ENVELOPE_VERSION,
        )
        .expect_err("unknown envelope must fail closed");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("unsupported"));
    });
}

#[test]
fn production_contract_excludes_web_test_only_env_vars() {
    let prod = dicom_web_env_contract(false);
    assert!(!prod.allowed_exact.contains(&"DICOM_WEB_TEST_STORAGE_BYTES"));
    assert!(!prod.allowed_exact.contains(&"DICOM_WEB_TEST_WORKERS"));

    let test = dicom_web_env_contract(true);
    assert!(test.allowed_exact.contains(&"DICOM_WEB_TEST_STORAGE_BYTES"));
    assert!(test.allowed_exact.contains(&"DICOM_WEB_TEST_WORKERS"));
}

#[test]
fn production_contract_excludes_workflow_test_only_env_vars() {
    let prod = dicom_workflow_env_contract(false);
    assert!(!prod
        .allowed_exact
        .contains(&"DICOM_WORKFLOW_TEST_RATE_LIMIT"));
    assert!(!prod
        .allowed_exact
        .contains(&"DICOM_WORKFLOW_TEST_ROTATIONS"));

    let test = dicom_workflow_env_contract(true);
    assert!(test
        .allowed_exact
        .contains(&"DICOM_WORKFLOW_TEST_RATE_LIMIT"));
    assert!(test
        .allowed_exact
        .contains(&"DICOM_WORKFLOW_TEST_ROTATIONS"));
}

#[test]
fn production_contract_excludes_dimse_test_only_env_vars() {
    let prod = dicom_dimse_env_contract(false);
    assert!(!prod.allowed_exact.contains(&"DICOM_DIMSE_TEST_MAX_BYTES"));
    assert!(!prod
        .allowed_exact
        .contains(&"DICOM_DIMSE_TEST_OPTIONAL_SECONDS"));

    let test = dicom_dimse_env_contract(true);
    assert!(test.allowed_exact.contains(&"DICOM_DIMSE_TEST_MAX_BYTES"));
    assert!(test
        .allowed_exact
        .contains(&"DICOM_DIMSE_TEST_OPTIONAL_SECONDS"));
}
