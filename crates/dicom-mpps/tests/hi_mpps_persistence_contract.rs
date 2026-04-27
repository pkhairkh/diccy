use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};
use dicom_mpps::{IngestOutcome, MppsService, MppsServiceConfig, MppsStatus, MppsStore};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);
const TAG_END_DATE: Tag = Tag(0x0040, 0x0250);
const TAG_END_TIME: Tag = Tag(0x0040, 0x0251);

fn temp_snapshot_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("diccy_{name}_{nonce}.snapshot"))
}

fn dataset_with_status(status: &str, sop_uid: &str, with_end: bool) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            TAG_SOP_INSTANCE_UID,
            Vr::Ui,
            Value::Uid(sop_uid.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(Element::new(TAG_STATUS, Vr::Cs, Value::Str(status.to_string())).unwrap());
    dataset.insert(
        Element::new(
            TAG_PERFORMED_STEP_ID,
            Vr::Sh,
            Value::Str("STEP-1".to_string()),
        )
        .unwrap(),
    );
    dataset
        .insert(Element::new(TAG_START_DATE, Vr::Da, Value::Str("20260214".to_string())).unwrap());
    dataset.insert(Element::new(TAG_START_TIME, Vr::Tm, Value::Str("101500".to_string())).unwrap());
    if with_end {
        dataset.insert(
            Element::new(TAG_END_DATE, Vr::Da, Value::Str("20260214".to_string())).unwrap(),
        );
        dataset
            .insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("103000".to_string())).unwrap());
    }
    dataset
}

#[test]
fn mpps_transition_contract_enforces_terminal_state_rules() {
    // REQ-HI-438, REQ-HI-439
    let mut store = MppsStore::new(Limits::default());

    let in_progress = dataset_with_status("IN PROGRESS", "1.2.3", false);
    let inserted = store.ingest_update(&in_progress).expect("insert");
    assert_eq!(inserted, IngestOutcome::Inserted);

    let completed = dataset_with_status("COMPLETED", "1.2.3", true);
    let updated = store.ingest_update(&completed).expect("complete");
    assert_eq!(updated, IngestOutcome::Updated);

    let invalid_reopen = dataset_with_status("IN PROGRESS", "1.2.3", false);
    let err = store
        .ingest_update(&invalid_reopen)
        .expect_err("terminal immutability");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));

    let current = store.get("1.2.3").expect("stored update");
    assert_eq!(current.status, MppsStatus::Completed);
}

#[test]
fn mpps_snapshot_reload_preserves_updates_and_deterministic_order() {
    // REQ-HI-439
    let path = temp_snapshot_path("hi_mpps_persistence");
    let mut service = MppsService::with_persistence(
        MppsServiceConfig {
            limits: Limits::default(),
            audit: None,
        },
        &path,
    )
    .expect("open service");

    service
        .ingest(&dataset_with_status("IN PROGRESS", "1.2.3", false))
        .expect("ingest 1");
    service
        .ingest(&dataset_with_status("IN PROGRESS", "1.2.4", false))
        .expect("ingest 2");
    drop(service);

    let reopened = MppsService::with_persistence(
        MppsServiceConfig {
            limits: Limits::default(),
            audit: None,
        },
        &path,
    )
    .expect("reopen service");

    assert_eq!(reopened.len(), 2);
    let updates = reopened.all_updates();
    assert_eq!(updates.len(), 2);
    assert!(updates[0].sop_instance_uid <= updates[1].sop_instance_uid);

    let _ = fs::remove_file(path);
}

#[test]
fn mpps_corrupt_snapshot_fails_closed() {
    // REQ-HI-439
    let path = temp_snapshot_path("hi_mpps_corrupt");
    fs::write(&path, b"BAD").expect("write corrupt snapshot");
    let err = MppsStore::open(Limits::default(), &path).expect_err("corrupt snapshot");
    assert!(matches!(
        err.kind(),
        ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
    ));
    let _ = fs::remove_file(path);
}
