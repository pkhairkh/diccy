// Auto-extracted from /home/z/diccy/crates/dicom-query/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_query::*;
use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};
use std::sync::Arc;
use std::thread;

fn limits() -> Limits {
    Limits::default()
}

fn dataset_with_uids(study: &str, series: &str, sop: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study.to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series.to_string())).unwrap());
    dataset.insert(Element::new(TAG_SOP_UID, Vr::Ui, Value::Uid(sop.to_string())).unwrap());
    dataset
}

fn dataset_with_metadata(
    study: &str,
    series: &str,
    sop: &str,
    patient_id: &str,
    modality: &str,
) -> Dataset {
    let mut dataset = dataset_with_uids(study, series, sop);
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str(patient_id.to_string())).unwrap(),
    );
    dataset
        .insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(modality.to_string())).unwrap());
    dataset
}

fn dataset_with_clinical_metadata(
    study: &str,
    series: &str,
    sop: &str,
    patient_id: &str,
    modality: &str,
    accession_number: &str,
    study_date: &str,
) -> Dataset {
    let mut dataset = dataset_with_metadata(study, series, sop, patient_id, modality);
    dataset.insert(
        Element::new(
            TAG_ACCESSION_NUMBER,
            Vr::Lo,
            Value::Str(accession_number.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str(study_date.to_string())).unwrap(),
    );
    dataset
}

#[test]
fn query_matches_study_uid() {
    // REQ-QR-300: only supported UID keys may be used for matching.
    let datasets = vec![
        dataset_with_uids("1.2.3", "2.3.4", "3.4.5"),
        dataset_with_uids("9.9", "8.8", "7.7"),
    ];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_STUDY_UID,
            value: "1.2.3".to_string(),
        }],
    };
    let matches = query(&datasets, &query_request, &limits()).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[test]
fn query_matches_patient_id_at_study_level() {
    // REQ-QR-300: study-level queries support Patient ID in addition to Study UID.
    let datasets = vec![
        dataset_with_metadata("1.2.3", "2.3.4", "3.4.5", "PATIENT_A", "CT"),
        dataset_with_metadata("9.9", "8.8", "7.7", "PATIENT_B", "MR"),
    ];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_PATIENT_ID,
            value: "PATIENT_A".to_string(),
        }],
    };
    let matches = query(&datasets, &query_request, &limits()).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[test]
fn query_matches_modality_at_series_level() {
    // REQ-QR-300: series-level queries support modality filters.
    let datasets = vec![
        dataset_with_metadata("1.2.3", "2.3.4", "3.4.5", "PATIENT_A", "CT"),
        dataset_with_metadata("1.2.3", "2.3.5", "3.4.6", "PATIENT_A", "MR"),
    ];
    let query_request = Query {
        level: QueryLevel::Series,
        keys: vec![
            QueryKey {
                tag: TAG_STUDY_UID,
                value: "1.2.3".to_string(),
            },
            QueryKey {
                tag: TAG_MODALITY,
                value: "MR".to_string(),
            },
        ],
    };
    let matches = query(&datasets, &query_request, &limits()).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].series_uid.as_deref(), Some("2.3.5"));
}

#[test]
fn query_rejects_unsupported_key() {
    // REQ-QR-300: unsupported keys must fail closed.
    let datasets = vec![dataset_with_uids("1.2.3", "2.3.4", "3.4.5")];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: Tag(0x0008, 0x1030),
            value: "CT HEAD".to_string(),
        }],
    };
    let err = query(&datasets, &query_request, &limits()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn query_matches_accession_number_and_study_date() {
    // REQ-QR-300: Study-level queries support Accession Number and Study Date filters.
    let datasets = vec![
        dataset_with_clinical_metadata(
            "1.2.3",
            "2.3.4",
            "3.4.5",
            "PATIENT_A",
            "CT",
            "ACC123",
            "20260211",
        ),
        dataset_with_clinical_metadata(
            "9.9",
            "8.8",
            "7.7",
            "PATIENT_B",
            "MR",
            "ACC999",
            "20260101",
        ),
    ];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![
            QueryKey {
                tag: TAG_ACCESSION_NUMBER,
                value: "ACC123".to_string(),
            },
            QueryKey {
                tag: TAG_STUDY_DATE,
                value: "20260211".to_string(),
            },
        ],
    };
    let matches = query(&datasets, &query_request, &limits()).expect("query");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[test]
fn query_rejects_invalid_study_date() {
    // REQ-QR-302: Study Date query values must be exact valid YYYYMMDD strings.
    let datasets = vec![dataset_with_clinical_metadata(
        "1.2.3",
        "2.3.4",
        "3.4.5",
        "PATIENT_A",
        "CT",
        "ACC123",
        "20260211",
    )];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_STUDY_DATE,
            value: "2026-02-11".to_string(),
        }],
    };
    let err = query(&datasets, &query_request, &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn query_orders_deterministically() {
    // REQ-QR-301: results must be ordered deterministically by UID.
    let datasets = vec![
        dataset_with_uids("2.2", "9.9", "1.1"),
        dataset_with_uids("1.1", "5.5", "2.2"),
    ];
    let query_request = Query {
        level: QueryLevel::Instance,
        keys: Vec::new(),
    };
    let matches = query(&datasets, &query_request, &limits()).expect("query");
    assert_eq!(matches[0].study_uid, "1.1");
    assert_eq!(matches[1].study_uid, "2.2");
}

#[test]
fn query_enforces_limit() {
    // REQ-QR-302: results must be bounded by max_dataset_elements.
    let datasets = vec![
        dataset_with_uids("1.1", "2.2", "3.3"),
        dataset_with_uids("4.4", "5.5", "6.6"),
    ];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: Vec::new(),
    };
    let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
    let err = query(&datasets, &query_request, &limits).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn query_rejects_invalid_uid_value() {
    // REQ-QR-302: invalid UID values must fail closed.
    let datasets = vec![dataset_with_uids("1.2.3", "2.3.4", "3.4.5")];
    let query_request = Query {
        level: QueryLevel::Study,
        keys: vec![QueryKey {
            tag: TAG_STUDY_UID,
            value: "1.2.x".to_string(),
        }],
    };
    let err = query(&datasets, &query_request, &limits()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn query_rejects_candidate_missing_required_uid_for_level() {
    // REQ-QR-303: candidate datasets missing required UIDs must fail closed.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());

    let query_request = Query {
        level: QueryLevel::Series,
        keys: Vec::new(),
    };
    let err = query(&[dataset], &query_request, &limits()).expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::MissingRequiredTag {
            tag: TAG_SERIES_UID
        }
    ));
}

#[test]
fn query_rejects_candidate_invalid_required_uid_for_level() {
    // REQ-QR-303: candidate datasets with invalid required UIDs must fail closed.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("2.3.x".to_string())).unwrap());

    let query_request = Query {
        level: QueryLevel::Series,
        keys: Vec::new(),
    };
    let err = query(&[dataset], &query_request, &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn identifier_query_builds_from_dataset() {
    // REQ-QR-300: supported UID keys are parsed from identifier datasets.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    let query = query_from_identifier(&dataset, &limits()).expect("query");
    assert_eq!(query.level, QueryLevel::Series);
    assert_eq!(query.keys.len(), 2);
}

#[test]
fn identifier_query_rejects_unsupported_key() {
    // REQ-QR-300: unsupported keys must fail closed.
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            Tag(0x0008, 0x1030),
            Vr::Lo,
            Value::Str("CT HEAD".to_string()),
        )
        .unwrap(),
    );
    let err = query_from_identifier(&dataset, &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn identifier_query_accepts_patient_id() {
    // REQ-QR-300: Patient ID is accepted as a supported identifier key.
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );
    let query = query_from_identifier(&dataset, &limits()).expect("query");
    assert_eq!(query.level, QueryLevel::Study);
    assert_eq!(query.keys.len(), 1);
    assert_eq!(query.keys[0].tag, TAG_PATIENT_ID);
    assert_eq!(query.keys[0].value, "PATIENT_A");
}

#[test]
fn identifier_query_accepts_accession_and_study_date() {
    // REQ-QR-300: Accession Number and Study Date are accepted identifier keys.
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            TAG_ACCESSION_NUMBER,
            Vr::Lo,
            Value::Str("ACC123".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20260211".to_string())).unwrap(),
    );
    let query = query_from_identifier(&dataset, &limits()).expect("query");
    assert_eq!(query.level, QueryLevel::Study);
    assert_eq!(query.keys.len(), 2);
}

#[test]
fn identifier_query_duplicate_tag_replaced() {
    // REQ-QR-302: Duplicate tags are replaced by BTreeMap, so the latest value wins.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    let query = query_from_identifier(&dataset, &limits()).expect("query");
    assert_eq!(query.level, QueryLevel::Study);
    assert_eq!(query.keys.len(), 1);
    assert_eq!(query.keys[0].tag, TAG_STUDY_UID);
    assert_eq!(query.keys[0].value, "9.9");
}

#[test]
fn query_key_coverage_matrix_lists_supported_hot_keys() {
    let rows = query_key_coverage_matrix();
    assert!(!rows.is_empty());
    assert!(rows.iter().any(|row| row.tag == TAG_STUDY_UID));
    assert!(rows.iter().any(|row| row.tag == TAG_SERIES_UID));
    assert!(rows.iter().any(|row| row.tag == TAG_SOP_UID));
}

#[test]
fn paginate_query_matches_emits_next_cursor() {
    let matches = vec![
        QueryMatch {
            study_uid: "1".to_string(),
            series_uid: Some("1.1".to_string()),
            instance_uid: Some("1.1.1".to_string()),
        },
        QueryMatch {
            study_uid: "2".to_string(),
            series_uid: Some("2.1".to_string()),
            instance_uid: Some("2.1.1".to_string()),
        },
    ];
    let first = paginate_query_matches(&matches, None, 1, &limits()).expect("first page");
    assert_eq!(first.matches.len(), 1);
    assert_eq!(first.next, Some(QueryCursor { offset: 1 }));
    let second =
        paginate_query_matches(&matches, first.next, 1, &limits()).expect("second page");
    assert_eq!(second.matches.len(), 1);
    assert_eq!(second.next, None);
}

#[test]
fn query_guardrails_and_failure_classification_are_deterministic() {
    let err = enforce_query_guardrails(
        1,
        100,
        QueryGuardrailConfig {
            max_inflight: 10,
            max_duration_ms: 10,
        },
    )
    .expect_err("duration limit");
    assert_eq!(
        classify_query_failure(err.as_ref()),
        QueryFailureMetric::Timeout
    );
}

#[test]
fn concurrent_query_replay_is_idempotent() {
    let datasets = Arc::new(vec![
        dataset_with_uids("1.2.3", "2.3.4", "3.4.5"),
        dataset_with_uids("1.2.3", "2.3.4", "3.4.6"),
    ]);
    let query_request = Query {
        level: QueryLevel::Instance,
        keys: vec![QueryKey {
            tag: TAG_STUDY_UID,
            value: "1.2.3".to_string(),
        }],
    };

    let mut handles = Vec::new();
    for _ in 0..6 {
        let datasets = Arc::clone(&datasets);
        let query_request = query_request.clone();
        handles.push(thread::spawn(move || {
            query(&datasets, &query_request, &Limits::default()).expect("query")
        }));
    }

    let baseline = handles
        .pop()
        .expect("baseline handle")
        .join()
        .expect("baseline join");
    for handle in handles {
        let current = handle.join().expect("join");
        assert_eq!(current, baseline);
    }
}
