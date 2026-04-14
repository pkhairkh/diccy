use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};
use dicom_worklist::{validate_worklist_item, WorklistQuery, WorklistStore};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);

fn temp_snapshot_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.snapshot"))
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

#[test]
fn worklist_validation_rejects_multi_item_sps_and_empty_filters() {
    // REQ-HI-433, REQ-HI-435
    let mut first = Dataset::new();
    first.insert(Element {
        tag: TAG_SPS_ID,
        vr: Vr::Sh,
        value: Value::Str("A".to_string()),
    });
    first.insert(Element {
        tag: TAG_MODALITY,
        vr: Vr::Cs,
        value: Value::Str("CT".to_string()),
    });
    first.insert(Element {
        tag: TAG_SPS_START_DATE,
        vr: Vr::Da,
        value: Value::Str("20260214".to_string()),
    });
    first.insert(Element {
        tag: TAG_SPS_START_TIME,
        vr: Vr::Tm,
        value: Value::Str("090000".to_string()),
    });

    let mut second = first.clone();
    second.insert(Element {
        tag: TAG_SPS_ID,
        vr: Vr::Sh,
        value: Value::Str("B".to_string()),
    });

    let mut invalid = Dataset::new();
    invalid.insert(Element {
        tag: TAG_SPS_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![first, second]),
    });

    let err = validate_worklist_item(&invalid, &Limits::default()).expect_err("single-item SPS");
    assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));

    let store = WorklistStore::new(Limits::default());
    let query_err = store
        .query(&WorklistQuery {
            modality: Some("".to_string()),
            scheduled_step_id: None,
            patient_id: None,
            requested_procedure_id: None,
        })
        .expect_err("empty filter");
    assert!(matches!(query_err.kind, ErrorKind::DecodeError { .. }));
}

#[test]
fn worklist_snapshot_reload_preserves_deterministic_ordering() {
    // REQ-HI-434, REQ-HI-437
    let path = temp_snapshot_path("hi_worklist_persistence");
    let mut store = WorklistStore::open(Limits::default(), &path).expect("open");

    store
        .upsert_dataset(&worklist_dataset("STEP-2", "CT", "20260214", "101500"))
        .expect("upsert step-2");
    store
        .upsert_dataset(&worklist_dataset("STEP-1", "CT", "20260214", "101000"))
        .expect("upsert step-1");
    drop(store);

    let reopened = WorklistStore::open(Limits::default(), &path).expect("reopen");
    let rows = reopened.query(&WorklistQuery::default()).expect("query");
    assert_eq!(rows.len(), 2);

    let first = validate_worklist_item(&rows[0], &Limits::default()).expect("first item");
    let second = validate_worklist_item(&rows[1], &Limits::default()).expect("second item");
    assert_eq!(first.scheduled_step_id, "STEP-1");
    assert_eq!(second.scheduled_step_id, "STEP-2");

    let _ = fs::remove_file(path);
}

#[test]
fn worklist_corrupt_snapshot_fails_closed() {
    // REQ-HI-436
    let path = temp_snapshot_path("hi_worklist_corrupt");
    fs::write(&path, b"BAD").expect("write corrupt snapshot");
    let err = WorklistStore::open(Limits::default(), &path).expect_err("corrupt snapshot");
    assert!(matches!(
        err.kind,
        ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
    ));
    let _ = fs::remove_file(path);
}
