// Auto-extracted from /home/z/diccy/crates/dicom-index/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_index::*;
use dicom_core::{Dataset, Element, ErrorKind, Limits, Value, Vr};

fn limits() -> Limits {
    Limits::default()
}

fn dataset_with_uids(study: &str, series: &str, sop: &str, sop_class: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study.to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series.to_string())).unwrap());
    dataset.insert(Element::new(TAG_SOP_UID, Vr::Ui, Value::Uid(sop.to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SOP_CLASS_UID, Vr::Ui, Value::Uid(sop_class.to_string())).unwrap(),
    );
    dataset
}

#[test]
fn extract_requires_required_uids() {
    // REQ-META-300: required UIDs must be present and valid.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    let err = extract_indexed_instance(&dataset, &limits(), "hash".to_string(), 10)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn index_orders_deterministically() {
    // REQ-META-301: index ordering is deterministic by UID.
    let mut index = Index::new(limits());
    let d1 = dataset_with_uids("2.2", "9.9", "1.1", "1.2.840.10008.1.1");
    let d2 = dataset_with_uids("1.1", "5.5", "2.2", "1.2.840.10008.1.1");
    let i1 = extract_indexed_instance(&d1, &limits(), "h1".to_string(), 10).unwrap();
    let i2 = extract_indexed_instance(&d2, &limits(), "h2".to_string(), 10).unwrap();
    index.insert(i1).unwrap();
    index.insert(i2).unwrap();
    assert_eq!(
        index.study_uids(),
        vec!["1.1".to_string(), "2.2".to_string()]
    );
    assert_eq!(index.series_uids("1.1"), Some(vec!["5.5".to_string()]));
}

#[test]
fn index_rejects_uid_conflict() {
    // REQ-META-301: SOP UID conflicts with different hashes must fail closed.
    let mut index = Index::new(limits());
    let dataset = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
    let i1 = extract_indexed_instance(&dataset, &limits(), "h1".to_string(), 10).unwrap();
    let i2 = extract_indexed_instance(&dataset, &limits(), "h2".to_string(), 10).unwrap();
    index.insert(i1).unwrap();
    let err = index.insert(i2).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}

#[test]
fn index_enforces_max_instances() {
    // REQ-META-302: max_dataset_elements bounds total indexed instances.
    let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
    let mut index = Index::new(limits);
    let d1 = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
    let d2 = dataset_with_uids("1.1", "2.2", "4.4", "1.2.840.10008.1.1");
    let i1 = extract_indexed_instance(&d1, &Limits::default(), "h1".to_string(), 10).unwrap();
    let i2 = extract_indexed_instance(&d2, &Limits::default(), "h2".to_string(), 10).unwrap();
    index.insert(i1).unwrap();
    let err = index.insert(i2).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn location_for_sop_returns_study_series_and_hash() {
    // REQ-META-301: SOP UID lookup returns deterministic UID/hash mapping.
    let mut index = Index::new(limits());
    let dataset = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
    let instance = extract_indexed_instance(&dataset, &Limits::default(), "h1".to_string(), 10)
        .expect("indexed instance");
    index.insert(instance).expect("insert");

    let location = index.location_for_sop("3.3").expect("location");
    assert_eq!(location.0, "1.1");
    assert_eq!(location.1, "2.2");
    assert_eq!(location.2, "h1");
}

#[test]
fn high_frequency_index_strategy_lists_core_uids() {
    let rows = high_frequency_index_strategy();
    assert!(rows.iter().any(|row| row.tag == TAG_STUDY_UID));
    assert!(rows.iter().any(|row| row.tag == TAG_SERIES_UID));
    assert!(rows.iter().any(|row| row.tag == TAG_SOP_UID));
}
