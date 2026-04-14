use dicom_core::{Dataset, Element, Limits, Tag, Value, Vr};
use dicom_mpps::{MppsService, MppsServiceConfig};
use dicom_workflow_server::{
    path_with_suffix, prepare_persistence_file_with_diagnostics, workflow_policy_diagnostics,
    workflow_recovery_diagnostic,
};
use dicom_worklist::WorklistStore;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);

const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);

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

fn worklist_dataset(step_id: &str, modality: &str, start_date: &str, start_time: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_SPS_ID,
        vr: Vr::Sh,
        value: Value::Str(step_id.to_string()),
    });
    item.insert(Element {
        tag: TAG_MODALITY,
        vr: Vr::Cs,
        value: Value::Str(modality.to_string()),
    });
    item.insert(Element {
        tag: TAG_SPS_START_DATE,
        vr: Vr::Da,
        value: Value::Str(start_date.to_string()),
    });
    item.insert(Element {
        tag: TAG_SPS_START_TIME,
        vr: Vr::Tm,
        value: Value::Str(start_time.to_string()),
    });

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_SPS_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![item]),
    });
    dataset
}

fn mpps_dataset(status: &str, sop_instance_uid: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_SOP_INSTANCE_UID,
        vr: Vr::Ui,
        value: Value::Uid(sop_instance_uid.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_STATUS,
        vr: Vr::Cs,
        value: Value::Str(status.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_PERFORMED_STEP_ID,
        vr: Vr::Sh,
        value: Value::Str("STEP-RUNTIME".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_START_DATE,
        vr: Vr::Da,
        value: Value::Str("20260214".to_string()),
    });
    dataset.insert(Element {
        tag: TAG_START_TIME,
        vr: Vr::Tm,
        value: Value::Str("101500".to_string()),
    });
    dataset
}

#[test]
fn workflow_policy_defaults_to_deny_all_and_flags_token_activation() {
    // REQ-HI-335, REQ-HI-306, REQ-HI-317
    let defaults = workflow_policy_diagnostics(None, None, None);
    assert_eq!(defaults.auth_mode_label, "deny_all");
    assert_eq!(defaults.transport_security, "insecure");
    assert!(!defaults.token_mode_active);
    assert!(defaults.fail_closed());

    let token = workflow_policy_diagnostics(Some("token"), Some("secret"), Some("tls"));
    assert_eq!(token.auth_mode_label, "token");
    assert_eq!(token.transport_security, "tls");
    assert!(token.token_mode_active);

    let invalid = workflow_policy_diagnostics(Some("token"), Some("  "), Some("???"));
    assert_eq!(invalid.auth_mode_label, "deny_all");
    assert_eq!(invalid.transport_security, "insecure");
    assert!(invalid.auth_defaulted);
    assert!(invalid.transport_defaulted);
    assert!(invalid.fail_closed());
}

#[test]
fn workflow_preflight_reports_creation_rotation_and_directory_rejection() {
    // REQ-HI-266, REQ-HI-302, REQ-HI-331, REQ-HI-332, REQ-HI-383, REQ-HI-384, REQ-HI-385
    let root = temp_file_path("workflow_hi_preflight", "dir");
    let snapshot = root.join("state/worklist.snapshot");
    let created = prepare_persistence_file_with_diagnostics(
        &snapshot.to_string_lossy(),
        1024,
        2,
        "worklist snapshot",
    )
    .expect("preflight create");
    assert!(created.parent_created);
    assert!(created.file_ready);

    fs::write(&snapshot, vec![9u8; 64]).expect("seed snapshot");
    let rotated = prepare_persistence_file_with_diagnostics(
        &snapshot.to_string_lossy(),
        16,
        0,
        "worklist snapshot",
    )
    .expect("preflight rotate");
    assert!(rotated.rotated);
    assert!(rotated.destructive_rollover);

    let dir_target = temp_file_path("workflow_hi_dir", "snapshot");
    fs::create_dir_all(&dir_target).expect("mkdir");
    let err = prepare_persistence_file_with_diagnostics(
        &dir_target.to_string_lossy(),
        1024,
        2,
        "worklist snapshot",
    )
    .expect_err("directory target must fail");
    assert!(err.to_string().contains("must reference a file"));

    let _ = fs::remove_dir_all(&dir_target);
    cleanup_with_rotations(&snapshot, 2);
}

#[test]
fn workflow_recovery_summary_reports_restored_counts() {
    // REQ-HI-268, REQ-HI-269, REQ-HI-340
    let worklist_path = temp_file_path("workflow_hi_recover_worklist", "snapshot");
    let mpps_path = temp_file_path("workflow_hi_recover_mpps", "snapshot");

    prepare_persistence_file_with_diagnostics(
        &worklist_path.to_string_lossy(),
        1_048_576,
        2,
        "worklist snapshot",
    )
    .expect("worklist preflight");
    prepare_persistence_file_with_diagnostics(
        &mpps_path.to_string_lossy(),
        1_048_576,
        2,
        "mpps snapshot",
    )
    .expect("mpps preflight");

    let limits = Limits::default();
    {
        let mut worklist =
            WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist");
        let mut mpps = MppsService::with_persistence(
            MppsServiceConfig {
                limits: limits.clone(),
                audit: None,
            },
            &mpps_path,
        )
        .expect("open mpps");

        worklist
            .upsert_dataset(&worklist_dataset("STEP-HI", "CT", "20260214", "101010"))
            .expect("worklist upsert");
        mpps.ingest(&mpps_dataset("IN PROGRESS", "1.2.840.1000.77"))
            .expect("mpps ingest");
    }

    let worklist = WorklistStore::open(limits.clone(), &worklist_path).expect("reopen worklist");
    let mpps = MppsService::with_persistence(
        MppsServiceConfig {
            limits,
            audit: None,
        },
        &mpps_path,
    )
    .expect("reopen mpps");

    let first = workflow_recovery_diagnostic(&worklist, &mpps);
    let second = workflow_recovery_diagnostic(&worklist, &mpps);
    assert!(first.durable);
    assert_eq!(first.worklist_items, 1);
    assert_eq!(first.mpps_updates, 1);
    assert_eq!(first, second);

    cleanup_with_rotations(&worklist_path, 2);
    cleanup_with_rotations(&mpps_path, 2);
}

#[test]
fn workflow_sr_state_and_audit_paths_support_preflight_and_rotation() {
    // REQ-HI-266, REQ-HI-331, REQ-HI-332, REQ-HI-383, REQ-HI-384, REQ-HI-385
    let root = temp_file_path("workflow_hi_sr_preflight", "dir");
    let sr_snapshot = root.join("state/sr.snapshot");
    let sr_audit = root.join("state/sr.audit.log");

    let snapshot_created = prepare_persistence_file_with_diagnostics(
        &sr_snapshot.to_string_lossy(),
        1024,
        2,
        "sr snapshot",
    )
    .expect("sr snapshot preflight create");
    assert!(snapshot_created.parent_created);
    assert!(snapshot_created.file_ready);

    let audit_created =
        prepare_persistence_file_with_diagnostics(&sr_audit.to_string_lossy(), 1024, 2, "sr audit")
            .expect("sr audit preflight create");
    assert!(audit_created.file_ready);

    fs::write(&sr_snapshot, vec![7u8; 128]).expect("seed sr snapshot");
    fs::write(&sr_audit, vec![8u8; 128]).expect("seed sr audit");

    let snapshot_rotated = prepare_persistence_file_with_diagnostics(
        &sr_snapshot.to_string_lossy(),
        32,
        0,
        "sr snapshot",
    )
    .expect("sr snapshot rotate");
    assert!(snapshot_rotated.rotated);

    let audit_rotated =
        prepare_persistence_file_with_diagnostics(&sr_audit.to_string_lossy(), 32, 0, "sr audit")
            .expect("sr audit rotate");
    assert!(audit_rotated.rotated);

    let dir_target = temp_file_path("workflow_hi_sr_dir", "snapshot");
    fs::create_dir_all(&dir_target).expect("mkdir");
    let err = prepare_persistence_file_with_diagnostics(
        &dir_target.to_string_lossy(),
        1024,
        2,
        "sr snapshot",
    )
    .expect_err("directory target must fail");
    assert!(err.to_string().contains("must reference a file"));

    let _ = fs::remove_dir_all(&dir_target);
    cleanup_with_rotations(&sr_snapshot, 2);
    cleanup_with_rotations(&sr_audit, 2);
}

#[test]
fn graceful_shutdown_flushes_workflow_snapshots_for_durable_recovery() {
    // REQ-HI-268, REQ-HI-269, REQ-HI-340
    let worklist_path = temp_file_path("workflow_hi_graceful_worklist", "snapshot");
    let mpps_path = temp_file_path("workflow_hi_graceful_mpps", "snapshot");

    prepare_persistence_file_with_diagnostics(
        &worklist_path.to_string_lossy(),
        1_048_576,
        2,
        "worklist snapshot",
    )
    .expect("worklist preflight");
    prepare_persistence_file_with_diagnostics(
        &mpps_path.to_string_lossy(),
        1_048_576,
        2,
        "mpps snapshot",
    )
    .expect("mpps preflight");

    let limits = Limits::default();
    {
        let mut worklist =
            WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist");
        let mut mpps = MppsService::with_persistence(
            MppsServiceConfig {
                limits: limits.clone(),
                audit: None,
            },
            &mpps_path,
        )
        .expect("open mpps");

        worklist
            .upsert_dataset(&worklist_dataset("STEP-GRACE", "MR", "20260214", "121212"))
            .expect("worklist upsert");
        mpps.ingest(&mpps_dataset("IN PROGRESS", "1.2.840.1000.99"))
            .expect("mpps ingest");
    }

    assert!(
        fs::metadata(&worklist_path)
            .expect("worklist metadata")
            .len()
            > 0,
        "worklist snapshot should be non-empty after shutdown"
    );
    assert!(
        fs::metadata(&mpps_path).expect("mpps metadata").len() > 0,
        "mpps snapshot should be non-empty after shutdown"
    );

    let worklist = WorklistStore::open(limits.clone(), &worklist_path).expect("reopen worklist");
    let mpps = MppsService::with_persistence(
        MppsServiceConfig {
            limits,
            audit: None,
        },
        &mpps_path,
    )
    .expect("reopen mpps");
    let recovery = workflow_recovery_diagnostic(&worklist, &mpps);
    assert!(recovery.durable);
    assert_eq!(recovery.worklist_items, 1);
    assert_eq!(recovery.mpps_updates, 1);

    cleanup_with_rotations(&worklist_path, 2);
    cleanup_with_rotations(&mpps_path, 2);
}
