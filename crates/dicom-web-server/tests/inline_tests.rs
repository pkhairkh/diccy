// Auto-extracted from /home/z/diccy/crates/dicom-web-server/src/main.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_core::{ErrorKind, Tag};
use dicom_web_server::*;
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
    std::env::temp_dir().join(format!("diccy_{name}_{nonce}.{ext}"))
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
