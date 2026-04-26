use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};
use dicom_query::{query, query_from_identifier, Query, QueryKey, QueryLevel};

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);

fn dataset_with_uids(study_uid: &str, series_uid: &str, sop_uid: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study_uid.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series_uid.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SOP_UID, Vr::Ui, Value::Uid(sop_uid.to_string()),
    ).unwrap());
    dataset
}

#[test]
fn query_identifier_builder_enforces_supported_key_sets_by_level() {
    // REQ-HI-296, REQ-HI-348, REQ-HI-349
    let limits = Limits::default();

    let series_identifier = dataset_with_uids("1.2.840.10", "1.2.840.10.1", "1.2.840.10.1.1");
    let series_query = query_from_identifier(&series_identifier, &limits).expect("series query");
    assert_eq!(series_query.level, QueryLevel::Instance);

    let mut unsupported = Dataset::new();
    unsupported.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.840.11".to_string()),
    ).unwrap());
    unsupported.insert(Element::new(Tag(0x0008, 0x1030), Vr::Lo, Value::Str("UNSUPPORTED".to_string())).unwrap());
    let err = query_from_identifier(&unsupported, &limits).expect_err("unsupported key must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("unsupported query key"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn query_controls_reject_invalid_study_date_and_non_ascii_text_values() {
    // REQ-HI-297, REQ-HI-307
    let limits = Limits::default();
    let invalid_date = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_STUDY_DATE,
            value: "20261301".to_string(),
        }],
    };
    let err = query(&[], &invalid_date, &limits).expect_err("invalid month must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("YYYYMMDD") || detail.contains("invalid month"));
        }
        _ => panic!("expected decode error"),
    }

    let invalid_patient = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_PATIENT_ID,
            value: "PÄTIENT".to_string(),
        }],
    };
    let err = query(&[], &invalid_patient, &limits).expect_err("non-ascii must fail");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("non-ASCII"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn query_execution_fails_closed_on_missing_required_uids_and_orders_deterministically() {
    // REQ-HI-298, REQ-HI-350
    let limits = Limits::default();

    let mut missing_series = Dataset::new();
    missing_series.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.840.20".to_string()),
    ).unwrap());
    let err = query(
        &[missing_series],
        &Query {
            level: QueryLevel::Series,
            keys: Vec::new(),
        },
        &limits,
    )
    .expect_err("missing required series UID must fail");
    assert!(matches!(
        err.kind(),
        ErrorKind::MissingRequiredTag {
            tag: TAG_SERIES_UID
        }
    ));

    let dataset_b = dataset_with_uids("1.2.840.30", "1.2.840.30.2", "1.2.840.30.2.9");
    let dataset_a = dataset_with_uids("1.2.840.30", "1.2.840.30.1", "1.2.840.30.1.7");
    let matches = query(
        &[dataset_b, dataset_a],
        &Query {
            level: QueryLevel::Instance,
            keys: Vec::new(),
        },
        &limits,
    )
    .expect("query");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.840.30.1"));
    assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.840.30.1.7"));
    assert_eq!(matches[1].series_uid.as_deref(), Some("1.2.840.30.2"));
    assert_eq!(matches[1].instance_uid.as_deref(), Some("1.2.840.30.2.9"));
}
