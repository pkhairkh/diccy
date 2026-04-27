use dicom_core::{Limits, Tag};
use dicom_storage::{IngestOutcome, Storage};
use dicom_web::{TlsPolicy, TransportSecurity};
use dicom_web_server::{
    classify_accept_queue_state, path_with_suffix, prepare_persistence_file_with_diagnostics,
    queue_backpressure_guidance, startup_policy_diagnostics, storage_recovery_diagnostic,
    worker_queue_contract, QueueBackpressureState,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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
fn startup_policy_fallbacks_default_secure_and_flag_unknown_values() {
    // REQ-HI-330, REQ-HI-317, REQ-HI-382
    let diagnostics = startup_policy_diagnostics(Some("bad"), Some("unknown"), Some("???"));
    assert_eq!(diagnostics.tls_policy, TlsPolicy::RequireTls);
    assert_eq!(diagnostics.auth_mode, "deny_all");
    assert_eq!(diagnostics.transport_security, TransportSecurity::Insecure);
    assert!(diagnostics.tls_defaulted);
    assert!(diagnostics.auth_defaulted);
    assert!(diagnostics.transport_defaulted);
    assert!(diagnostics.fail_closed());
}

#[test]
fn preflight_reports_parent_creation_and_rejects_directory_targets() {
    // REQ-HI-266, REQ-HI-302, REQ-HI-331, REQ-HI-383, REQ-HI-384
    let root = temp_file_path("web_hi_preflight", "dir");
    let wal = root.join("runtime/storage.wal");
    let diag =
        prepare_persistence_file_with_diagnostics(&wal.to_string_lossy(), 1024, 2, "storage WAL")
            .expect("preflight");
    assert!(diag.parent_created);
    assert!(diag.file_ready);
    assert!(wal.exists());

    let dir_target = temp_file_path("web_hi_preflight_dir", "wal");
    fs::create_dir_all(&dir_target).expect("mkdir");
    let err = prepare_persistence_file_with_diagnostics(
        &dir_target.to_string_lossy(),
        1024,
        2,
        "storage WAL",
    )
    .expect_err("directory targets must fail closed");
    assert!(err.to_string().contains("must reference a file"));

    let _ = fs::remove_dir_all(&dir_target);
    cleanup_with_rotations(&wal, 2);
}

#[test]
fn rotation_supports_zero_retention_destructive_rollover() {
    // REQ-HI-267, REQ-HI-303, REQ-HI-332, REQ-HI-385
    let wal = temp_file_path("web_hi_rotation", "wal");
    fs::write(&wal, vec![5u8; 64]).expect("seed wal");
    let diag =
        prepare_persistence_file_with_diagnostics(&wal.to_string_lossy(), 16, 0, "storage WAL")
            .expect("preflight");
    assert!(diag.rotated);
    assert!(diag.destructive_rollover);
    assert!(wal.exists());
    assert!(!path_with_suffix(&wal, 1).exists());

    cleanup_with_rotations(&wal, 1);
}

#[test]
fn worker_queue_contract_enforces_minimum_bounds_and_backpressure_guidance() {
    // REQ-HI-318, REQ-HI-333, REQ-HI-334
    let err = worker_queue_contract(0, 8).expect_err("zero workers must fail");
    assert!(err.to_string().contains("DICOM_WEB_WORKERS"));

    let err = worker_queue_contract(4, 0).expect_err("zero queue depth must fail");
    assert!(err.to_string().contains("DICOM_WEB_ACCEPT_QUEUE_DEPTH"));

    let contract = worker_queue_contract(2, 8).expect("valid contract");
    let state = classify_accept_queue_state(contract.queue_depth, contract.queue_depth);
    assert_eq!(state, QueueBackpressureState::Saturated);
    assert_eq!(
        queue_backpressure_guidance(state),
        "accept queue saturated; allow workers to drain before retrying"
    );
}

#[test]
fn recovery_summary_reports_durable_reopen_record_count() {
    // REQ-HI-268, REQ-HI-269, REQ-HI-304
    let wal = temp_file_path("web_hi_recovery", "wal");
    prepare_persistence_file_with_diagnostics(&wal.to_string_lossy(), 1_048_576, 2, "storage WAL")
        .expect("preflight");

    let payload = sample_p10("1.2.840.12", "1.2.840.12.1", "1.2.840.12.1.1");
    let mut storage = Storage::open(Limits::default(), &wal).expect("open");
    let outcome = storage.ingest_bytes(payload).expect("ingest");
    assert!(matches!(outcome, IngestOutcome::Inserted { .. }));
    drop(storage);

    let reopened = Storage::open(Limits::default(), &wal).expect("reopen");
    let recovery = storage_recovery_diagnostic(&reopened).expect("recovery summary");
    assert!(recovery.durable);
    assert_eq!(recovery.recovered_records, 1);

    cleanup_with_rotations(&wal, 2);
}

#[test]
fn graceful_shutdown_flushes_wal_and_preserves_reopen_integrity() {
    // REQ-HI-268, REQ-HI-269, REQ-HI-304
    let wal = temp_file_path("web_hi_graceful_shutdown", "wal");
    prepare_persistence_file_with_diagnostics(&wal.to_string_lossy(), 1_048_576, 2, "storage WAL")
        .expect("preflight");

    let payload = sample_p10("1.2.840.52", "1.2.840.52.1", "1.2.840.52.1.1");
    {
        let mut storage = Storage::open(Limits::default(), &wal).expect("open");
        let outcome = storage.ingest_bytes(payload).expect("ingest");
        assert!(matches!(outcome, IngestOutcome::Inserted { .. }));
    }

    let wal_size = fs::metadata(&wal).expect("wal metadata").len();
    assert!(
        wal_size > 0,
        "expected non-empty WAL after graceful shutdown"
    );

    let reopened = Storage::open(Limits::default(), &wal).expect("reopen");
    let datasets = reopened.datasets().expect("datasets");
    assert_eq!(datasets.len(), 1);

    cleanup_with_rotations(&wal, 2);
}
