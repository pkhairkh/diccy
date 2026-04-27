// Auto-extracted from /home/z/diccy/crates/dicom-mpps/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_mpps::*;
use dicom_audit::{AuditEvent, AuditValue};
use dicom_core::{Dataset, Element, ErrorKind, Limits, Value, Vr};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn dataset_with_status(status: &str, sop_uid: &str) -> Dataset {
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
            Value::Str("STEP1".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_START_DATE, Vr::Da, Value::Str("20240101".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(TAG_START_TIME, Vr::Tm, Value::Str("120000".to_string())).unwrap(),
    );
    dataset
}

fn temp_snapshot_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("diccy_{name}_{nonce}.snapshot"))
}

#[test]
fn mpps_requires_required_tags() {
    // REQ-MPPS-350: required tags must be present.
    let dataset = Dataset::new();
    let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn mpps_rejects_invalid_status() {
    // REQ-MPPS-351: unsupported status must fail closed.
    let dataset = dataset_with_status("BAD", "1.2.3");
    let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn mpps_requires_end_time_for_completed() {
    // REQ-MPPS-351: completed status requires end date/time.
    let dataset = dataset_with_status("COMPLETED", "1.2.3");
    let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn mpps_transition_enforced() {
    // REQ-MPPS-352: invalid transitions must fail closed.
    let mut store = MppsStore::new(Limits::default());
    let dataset = dataset_with_status("IN PROGRESS", "1.2.3");
    store.ingest_update(&dataset).expect("insert");
    let mut completed = dataset_with_status("COMPLETED", "1.2.3");
    completed.insert(
        Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string())).unwrap(),
    );
    completed
        .insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("130000".to_string())).unwrap());
    let outcome = store.ingest_update(&completed).expect("update");
    assert_eq!(outcome, IngestOutcome::Updated);

    let reverted = dataset_with_status("IN PROGRESS", "1.2.3");
    let err = store.ingest_update(&reverted).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}

#[test]
fn mpps_enforces_limit() {
    // REQ-MPPS-353: store size is bounded by max_dataset_elements.
    let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
    let mut store = MppsStore::new(limits);
    let d1 = dataset_with_status("IN PROGRESS", "1.2.3");
    let d2 = dataset_with_status("IN PROGRESS", "1.2.4");
    store.ingest_update(&d1).expect("insert");
    let err = store.ingest_update(&d2).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn mpps_service_persists_and_emits_audit() {
    // REQ-MPPS-354, REQ-AUDIT-350: persisted MPPS ingest emits deterministic audit events.
    let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
    let events_handle = Arc::clone(&events);
    let audit: AuditCallback = Arc::new(move |event| {
        let mut guard = events_handle.lock().expect("audit lock");
        guard.push(event);
        Ok(())
    });

    let mut service = MppsService::new(MppsServiceConfig {
        limits: Limits::default(),
        audit: Some(audit),
    });

    let in_progress = dataset_with_status("IN PROGRESS", "1.2.3");
    let inserted = service.ingest(&in_progress).expect("insert");
    assert_eq!(inserted, IngestOutcome::Inserted);

    let mut completed = dataset_with_status("COMPLETED", "1.2.3");
    completed.insert(
        Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string())).unwrap(),
    );
    completed
        .insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("130000".to_string())).unwrap());
    let updated = service.ingest(&completed).expect("update");
    assert_eq!(updated, IngestOutcome::Updated);

    let persisted = service.get("1.2.3").expect("persisted update");
    assert_eq!(persisted.status, MppsStatus::Completed);

    let events = events.lock().expect("audit lock");
    assert_eq!(events.len(), 2);
    assert!(events[0].fields.iter().any(|field| {
        field.key == "operation"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "ingest")
    }));
    assert!(events[1].fields.iter().any(|field| {
        field.key == "status"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "COMPLETED")
    }));
}

#[test]
fn durable_mpps_store_recovers_snapshot() {
    // REQ-MPPS-354: durable MPPS snapshot reload preserves tracked transitions.
    let path = temp_snapshot_path("mpps");
    let mut store = MppsStore::open(Limits::default(), &path).expect("open");
    let mut completed = dataset_with_status("COMPLETED", "1.2.3");
    completed.insert(
        Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string())).unwrap(),
    );
    completed
        .insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("121500".to_string())).unwrap());
    store.ingest_update(&completed).expect("ingest");
    drop(store);

    let reopened = MppsStore::open(Limits::default(), &path).expect("reopen");
    assert_eq!(reopened.len(), 1);
    assert!(reopened.get("1.2.3").is_some());
    let _ = fs::remove_file(path);
}

#[test]
fn durable_mpps_store_rejects_invalid_header() {
    // REQ-MPPS-354: corrupted MPPS snapshot headers fail closed.
    let path = temp_snapshot_path("mpps_corrupt");
    fs::write(&path, b"BAD").expect("write");
    let err = MppsStore::open(Limits::default(), &path).expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
    ));
    let _ = fs::remove_file(path);
}
