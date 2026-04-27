// Auto-extracted from /home/z/diccy/crates/dicom-worklist/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_worklist::*;
use dicom_audit::{AuditEvent, AuditValue};
use dicom_core::{Dataset, Element, ErrorKind, Limits, Value, Vr};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

fn limits() -> Limits {
    Limits::default()
}

fn worklist_dataset(step_id: &str, modality: &str, date: &str, time: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(step_id.to_string())).unwrap());
    item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(modality.to_string())).unwrap());
    item.insert(
        Element::new(TAG_SPS_START_DATE, Vr::Da, Value::Str(date.to_string())).unwrap(),
    );
    item.insert(
        Element::new(TAG_SPS_START_TIME, Vr::Tm, Value::Str(time.to_string())).unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![item])).unwrap());
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
fn worklist_requires_sps_sequence() {
    // REQ-WL-300: missing Scheduled Procedure Step Sequence must fail closed.
    let dataset = Dataset::new();
    let err = validate_worklist_item(&dataset, &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn worklist_rejects_empty_sequence() {
    // REQ-WL-300: empty sequence must fail closed.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(Vec::new())).unwrap());
    let err = validate_worklist_item(&dataset, &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn worklist_orders_deterministically() {
    // REQ-WL-301: response ordering is deterministic.
    let a = WorklistItem {
        scheduled_step_id: "B".to_string(),
        modality: "CT".to_string(),
        start_date: "20240102".to_string(),
        start_time: "120000".to_string(),
        requested_procedure_id: None,
        scheduled_station_ae_title: None,
        patient_id: None,
        accession_number: None,
    };
    let b = WorklistItem {
        scheduled_step_id: "A".to_string(),
        modality: "MR".to_string(),
        start_date: "20240101".to_string(),
        start_time: "080000".to_string(),
        requested_procedure_id: None,
        scheduled_station_ae_title: None,
        patient_id: None,
        accession_number: None,
    };
    let ordered = build_worklist_response(&[a, b], &limits()).expect("response");
    let first = validate_worklist_item(&ordered[0], &limits()).expect("item");
    assert_eq!(first.start_date, "20240101");
}

#[test]
fn worklist_enforces_limits() {
    // REQ-WL-302: string limits must be enforced.
    let limits = Limits::builder().max_string_bytes(2).build().unwrap();
    let dataset = worklist_dataset("AAA", "CT", "20240101", "120000");
    let err = validate_worklist_item(&dataset, &limits).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn response_enforces_item_limit() {
    // REQ-WL-302: response size is bounded by max_dataset_elements.
    let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
    let item = validate_worklist_item(
        &worklist_dataset("A", "CT", "20240101", "090000"),
        &Limits::default(),
    )
    .expect("item");
    let err = build_worklist_response(&[item.clone(), item], &limits).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn worklist_store_persists_and_queries_items() {
    // REQ-WL-303: persisted worklist queries are deterministic and filterable.
    let mut store = WorklistStore::new(Limits::default());
    let mut dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_ACCESSION_NUMBER,
            Vr::Sh,
            Value::Str("ACC123".to_string()),
        )
        .unwrap(),
    );
    let outcome = store.upsert_dataset(&dataset).expect("upsert");
    assert_eq!(outcome, UpsertOutcome::Inserted);

    let response = store
        .query(&WorklistQuery {
            modality: Some("CT".to_string()),
            scheduled_step_id: None,
            patient_id: Some("PATIENT_A".to_string()),
            requested_procedure_id: None,
        })
        .expect("query");
    assert_eq!(response.len(), 1);
    let item = validate_worklist_item(&response[0], &Limits::default()).expect("item");
    assert_eq!(item.patient_id.as_deref(), Some("PATIENT_A"));
    assert_eq!(item.accession_number.as_deref(), Some("ACC123"));
}

#[test]
fn worklist_store_emits_audit_for_upsert_and_query() {
    // REQ-WL-303, REQ-AUDIT-350: persisted operations emit deterministic audit events.
    let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
    let events_handle = Arc::clone(&events);
    let audit: AuditCallback = Arc::new(move |event| {
        let mut guard = events_handle.lock().expect("audit lock");
        guard.push(event);
        Ok(())
    });
    let mut store = WorklistStore::with_audit(Limits::default(), Some(audit));
    let dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
    store.upsert_dataset(&dataset).expect("upsert");
    let _ = store.query(&WorklistQuery::default()).expect("query");

    let events = events.lock().expect("audit lock");
    assert_eq!(events.len(), 2);
    assert!(events[0].fields.iter().any(|field| {
        field.key == "operation"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "upsert")
    }));
    assert!(events[1].fields.iter().any(|field| {
        field.key == "operation"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "query")
    }));
}

#[test]
fn durable_worklist_store_recovers_snapshot() {
    // REQ-WL-303: durable snapshot reload must preserve deterministic worklist state.
    let path = temp_snapshot_path("worklist");
    let mut store = WorklistStore::open(Limits::default(), &path).expect("open");
    let dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
    store.upsert_dataset(&dataset).expect("upsert");
    drop(store);

    let reopened = WorklistStore::open(Limits::default(), &path).expect("reopen");
    assert_eq!(reopened.len(), 1);
    let rows = reopened.query(&WorklistQuery::default()).expect("query");
    assert_eq!(rows.len(), 1);
    let _ = fs::remove_file(path);
}

#[test]
fn durable_worklist_store_rejects_invalid_header() {
    // REQ-WL-303: corrupted worklist snapshots fail closed.
    let path = temp_snapshot_path("worklist_corrupt");
    fs::write(&path, b"BAD").expect("write");
    let err = WorklistStore::open(Limits::default(), &path).expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
    ));
    let _ = fs::remove_file(path);
}
