use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};
use dicom_mpps::{IngestOutcome, MppsService, MppsServiceConfig, MppsStatus};
use dicom_worklist::{WorklistQuery, WorklistStore};

const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);
const TAG_END_DATE: Tag = Tag(0x0040, 0x0250);
const TAG_END_TIME: Tag = Tag(0x0040, 0x0251);

const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);

fn mpps_dataset(status: &str, sop_instance_uid: &str, with_end: bool) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            TAG_SOP_INSTANCE_UID,
            Vr::Ui,
            Value::Uid(sop_instance_uid.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(Element::new(TAG_STATUS, Vr::Cs, Value::Str(status.to_string())).unwrap());
    dataset.insert(
        Element::new(
            TAG_PERFORMED_STEP_ID,
            Vr::Sh,
            Value::Str("PS-100".to_string()),
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
            .insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("102000".to_string())).unwrap());
    }
    dataset
}

fn worklist_dataset(step_id: &str, modality: &str, start_date: &str, start_time: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(step_id.to_string())).unwrap());
    item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(modality.to_string())).unwrap());
    item.insert(
        Element::new(
            TAG_SPS_START_DATE,
            Vr::Da,
            Value::Str(start_date.to_string()),
        )
        .unwrap(),
    );
    item.insert(
        Element::new(
            TAG_SPS_START_TIME,
            Vr::Tm,
            Value::Str(start_time.to_string()),
        )
        .unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![item])).unwrap());
    dataset
}

#[test]
fn mpps_state_machine_is_deterministic_for_repeated_sequences() {
    // REQ-HI-136, REQ-HI-137, REQ-HI-141
    let mut service = MppsService::new(MppsServiceConfig {
        limits: Limits::default(),
        audit: None,
    });

    let in_progress = mpps_dataset("IN PROGRESS", "1.2.840.1001", false);
    let completed = mpps_dataset("COMPLETED", "1.2.840.1001", true);

    assert_eq!(
        service.ingest(&in_progress).expect("insert"),
        IngestOutcome::Inserted
    );
    assert_eq!(
        service.ingest(&completed).expect("update"),
        IngestOutcome::Updated
    );
    assert_eq!(
        service.ingest(&completed).expect("duplicate"),
        IngestOutcome::Duplicate
    );

    let first_snapshot = service.all_updates();
    let second_snapshot = service.all_updates();
    assert_eq!(first_snapshot, second_snapshot);

    let update = service.get("1.2.840.1001").expect("persisted update");
    assert_eq!(update.status, MppsStatus::Completed);
}

#[test]
fn mpps_terminal_state_rejects_invalid_reopen_transition() {
    // REQ-HI-136, REQ-HI-138, REQ-HI-144
    let mut service = MppsService::new(MppsServiceConfig {
        limits: Limits::default(),
        audit: None,
    });

    let in_progress = mpps_dataset("IN PROGRESS", "1.2.840.1002", false);
    let completed = mpps_dataset("COMPLETED", "1.2.840.1002", true);
    let invalid_reopen = mpps_dataset("IN PROGRESS", "1.2.840.1002", false);

    service.ingest(&in_progress).expect("insert");
    service.ingest(&completed).expect("complete");

    let err = service
        .ingest(&invalid_reopen)
        .expect_err("reopen must fail");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));

    let update = service.get("1.2.840.1002").expect("stored update");
    assert_eq!(update.status, MppsStatus::Completed);
}

#[test]
fn worklist_query_results_remain_deterministic_across_repeated_reads() {
    // REQ-HI-130, REQ-HI-135, REQ-HI-141
    let mut store = WorklistStore::new(Limits::default());
    store
        .upsert_dataset(&worklist_dataset("STEP-2", "CT", "20260214", "101000"))
        .expect("insert step-2");
    store
        .upsert_dataset(&worklist_dataset("STEP-1", "CT", "20260214", "100500"))
        .expect("insert step-1");

    let query = WorklistQuery {
        modality: Some("CT".to_string()),
        scheduled_step_id: None,
        patient_id: None,
        requested_procedure_id: None,
    };

    let first = store.query(&query).expect("first query");
    let second = store.query(&query).expect("second query");
    assert_eq!(first, second);

    let first_step = first[0]
        .get(TAG_SPS_SEQUENCE)
        .and_then(|el| match el.value() {
            Value::Sequence(items) => items.first(),
            _ => None,
        })
        .and_then(|item| item.get_str(TAG_SPS_ID))
        .expect("first step id");
    assert_eq!(first_step, "STEP-1");
}
