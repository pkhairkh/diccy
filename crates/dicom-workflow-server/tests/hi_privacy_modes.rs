use dicom_core::{Dataset, Element, Limits, Result, Tag, Value, Vr};
use dicom_mpps::{MppsService, MppsServiceConfig};
use dicom_worklist::{WorklistQuery, WorklistStore};
use std::sync::{Arc, Mutex};

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
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);

fn mpps_in_progress(uid: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(uid.to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_STATUS, Vr::Cs, Value::Str("IN PROGRESS".to_string())).unwrap());
    dataset.insert(
        Element::new(
            TAG_PERFORMED_STEP_ID,
            Vr::Sh,
            Value::Str("STEP-PRIV".to_string()),
        )
        .unwrap(),
    );
    dataset
        .insert(Element::new(TAG_START_DATE, Vr::Da, Value::Str("20260214".to_string())).unwrap());
    dataset.insert(Element::new(TAG_START_TIME, Vr::Tm, Value::Str("101500".to_string())).unwrap());
    dataset
}

fn worklist_entry(step_id: &str, patient_id: &str) -> Dataset {
    let mut sps_item = Dataset::new();
    sps_item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(step_id.to_string())).unwrap());
    sps_item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
    sps_item.insert(
        Element::new(
            TAG_SPS_START_DATE,
            Vr::Da,
            Value::Str("20260214".to_string()),
        )
        .unwrap(),
    );
    sps_item.insert(
        Element::new(TAG_SPS_START_TIME, Vr::Tm, Value::Str("102000".to_string())).unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![sps_item])).unwrap());
    dataset
        .insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str(patient_id.to_string())).unwrap());
    dataset
}

#[test]
fn mpps_audit_marks_identifiers_as_sensitive() {
    // REQ-HI-195, REQ-HI-201, REQ-HI-209
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);

    let mut service = MppsService::new(MppsServiceConfig {
        limits: Limits::default(),
        audit: Some(Arc::new(move |event| -> Result<()> {
            let mut guard = sink.lock().expect("audit sink lock");
            guard.push(format!("{event:?}"));
            Ok(())
        })),
    });

    service
        .ingest(&mpps_in_progress("1.2.840.55"))
        .expect("ingest should pass");

    let events = events.lock().expect("events lock");
    assert!(!events.is_empty());
    assert!(events
        .iter()
        .any(|line| line.contains("Sensitive(\"1.2.840.55\")")));
    assert!(!events
        .iter()
        .any(|line| line.contains("Plain(\"1.2.840.55\")")));
}

#[test]
fn worklist_audit_marks_patient_identifiers_as_sensitive() {
    // REQ-HI-195, REQ-HI-200, REQ-HI-201
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);

    let mut store = WorklistStore::with_audit(
        Limits::default(),
        Some(Arc::new(move |event| -> Result<()> {
            let mut guard = sink.lock().expect("audit sink lock");
            guard.push(format!("{event:?}"));
            Ok(())
        })),
    );

    store
        .upsert_dataset(&worklist_entry("STEP-PRIV-1", "PATIENT-42"))
        .expect("upsert should pass");

    let events = events.lock().expect("events lock");
    assert!(!events.is_empty());
    assert!(events
        .iter()
        .any(|line| line.contains("Sensitive(\"PATIENT-42\")")));
    assert!(!events
        .iter()
        .any(|line| line.contains("Plain(\"PATIENT-42\")")));
}

#[test]
fn repeated_privacy_audit_events_remain_deterministic() {
    // REQ-HI-183, REQ-HI-205, REQ-HI-206
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);

    let mut store = WorklistStore::with_audit(
        Limits::default(),
        Some(Arc::new(move |event| -> Result<()> {
            let mut guard = sink.lock().expect("audit sink lock");
            guard.push(format!("{event:?}"));
            Ok(())
        })),
    );

    let dataset = worklist_entry("STEP-PRIV-2", "PATIENT-99");
    store.upsert_dataset(&dataset).expect("insert");
    let query = WorklistQuery {
        modality: Some("CT".to_string()),
        scheduled_step_id: None,
        patient_id: None,
        requested_procedure_id: None,
    };
    store.query(&query).expect("query one");
    store.query(&query).expect("query two");

    let events = events.lock().expect("events lock");
    assert_eq!(events.len(), 3);
    assert_eq!(events[1], events[2]);
}
