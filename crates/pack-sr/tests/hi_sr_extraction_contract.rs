use dicom_core::{Dataset, Element, ErrorKind, Tag, Value, Vr};
use pack_sr::{extract_code_observations, extract_measurements, extract_text_observations};

const TAG_CONTENT_SEQUENCE: Tag = Tag(0x0040, 0xA730);
const TAG_VALUE_TYPE: Tag = Tag(0x0040, 0xA040);
const TAG_CONCEPT_NAME_CODE_SEQUENCE: Tag = Tag(0x0040, 0xA043);
const TAG_CONCEPT_CODE_SEQUENCE: Tag = Tag(0x0040, 0xA168);
const TAG_MEASURED_VALUE_SEQUENCE: Tag = Tag(0x0040, 0xA300);
const TAG_NUMERIC_VALUE: Tag = Tag(0x0040, 0xA30A);
const TAG_TEXT_VALUE: Tag = Tag(0x0040, 0xA160);
const TAG_MEASUREMENT_UNITS_CODE_SEQUENCE: Tag = Tag(0x0040, 0x08EA);
const TAG_CODE_VALUE: Tag = Tag(0x0008, 0x0100);
const TAG_CODING_SCHEME_DESIGNATOR: Tag = Tag(0x0008, 0x0102);
const TAG_CODE_MEANING: Tag = Tag(0x0008, 0x0104);
const TAG_REFERENCED_SOP_SEQUENCE: Tag = Tag(0x0008, 0x1199);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

fn concept_item(code: &str, scheme: &str, meaning: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_CODE_VALUE,
        vr: Vr::Sh,
        value: Value::Str(code.to_string()),
    });
    item.insert(Element {
        tag: TAG_CODING_SCHEME_DESIGNATOR,
        vr: Vr::Sh,
        value: Value::Str(scheme.to_string()),
    });
    item.insert(Element {
        tag: TAG_CODE_MEANING,
        vr: Vr::Lo,
        value: Value::Str(meaning.to_string()),
    });
    item
}

fn build_num_item(value: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_VALUE_TYPE,
        vr: Vr::Cs,
        value: Value::Str("NUM".to_string()),
    });
    item.insert(Element {
        tag: TAG_CONCEPT_NAME_CODE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![concept_item("G-D7FE", "SRT", "Length")]),
    });

    let mut measured = Dataset::new();
    measured.insert(Element {
        tag: TAG_NUMERIC_VALUE,
        vr: Vr::Ds,
        value: Value::Str(value.to_string()),
    });
    measured.insert(Element {
        tag: TAG_MEASUREMENT_UNITS_CODE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![concept_item("mm", "UCUM", "millimeter")]),
    });
    item.insert(Element {
        tag: TAG_MEASURED_VALUE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![measured]),
    });
    item
}

fn build_text_item(text: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_VALUE_TYPE,
        vr: Vr::Cs,
        value: Value::Str("TEXT".to_string()),
    });
    item.insert(Element {
        tag: TAG_CONCEPT_NAME_CODE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![concept_item("121106", "DCM", "Finding")]),
    });
    item.insert(Element {
        tag: TAG_TEXT_VALUE,
        vr: Vr::Ut,
        value: Value::Str(text.to_string()),
    });
    item
}

fn build_code_item() -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_VALUE_TYPE,
        vr: Vr::Cs,
        value: Value::Str("CODE".to_string()),
    });
    item.insert(Element {
        tag: TAG_CONCEPT_NAME_CODE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![concept_item("121071", "DCM", "Interpretation")]),
    });
    item.insert(Element {
        tag: TAG_CONCEPT_CODE_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![concept_item("R-404FB", "SRT", "Normal")]),
    });
    item
}

#[test]
fn sr_nested_extraction_is_deterministic_across_num_text_and_code_items() {
    // REQ-HI-431
    let mut container = Dataset::new();
    container.insert(Element {
        tag: TAG_VALUE_TYPE,
        vr: Vr::Cs,
        value: Value::Str("CONTAINER".to_string()),
    });
    container.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![
            build_num_item("12.5"),
            build_text_item("Nested note"),
            build_code_item(),
        ]),
    });

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![container]),
    });

    let m1 = extract_measurements(&dataset).expect("measurements one");
    let m2 = extract_measurements(&dataset).expect("measurements two");
    let t1 = extract_text_observations(&dataset).expect("text one");
    let t2 = extract_text_observations(&dataset).expect("text two");
    let c1 = extract_code_observations(&dataset).expect("code one");
    let c2 = extract_code_observations(&dataset).expect("code two");

    assert_eq!(m1, m2);
    assert_eq!(t1, t2);
    assert_eq!(c1, c2);
}

#[test]
fn sr_extractors_fail_closed_for_non_finite_empty_and_missing_code_content() {
    // REQ-HI-428, REQ-HI-429, REQ-HI-430
    let mut non_finite = Dataset::new();
    non_finite.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![build_num_item("NaN")]),
    });
    let num_err = extract_measurements(&non_finite).expect_err("non-finite numeric");
    assert!(matches!(num_err.kind, ErrorKind::InvalidTagValue { .. }));

    let mut empty_text = Dataset::new();
    empty_text.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![build_text_item("   ")]),
    });
    let text_err = extract_text_observations(&empty_text).expect_err("empty text");
    assert!(matches!(text_err.kind, ErrorKind::InvalidTagValue { .. }));

    let mut missing_code = Dataset::new();
    let mut invalid_code_item = Dataset::new();
    invalid_code_item.insert(Element {
        tag: TAG_VALUE_TYPE,
        vr: Vr::Cs,
        value: Value::Str("CODE".to_string()),
    });
    missing_code.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![invalid_code_item]),
    });
    let code_err = extract_code_observations(&missing_code).expect_err("missing code seq");
    assert!(matches!(
        code_err.kind,
        ErrorKind::MissingRequiredTag { .. }
    ));
}

#[test]
fn sr_provenance_exposes_referenced_sop_instance_uids() {
    // REQ-HI-432
    let mut ref_item = Dataset::new();
    ref_item.insert(Element {
        tag: TAG_REFERENCED_SOP_INSTANCE_UID,
        vr: Vr::Ui,
        value: Value::Uid("1.2.3.4".to_string()),
    });

    let mut num_item = build_num_item("7.0");
    num_item.insert(Element {
        tag: TAG_REFERENCED_SOP_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![ref_item]),
    });

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_CONTENT_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![num_item]),
    });

    let measurements = extract_measurements(&dataset).expect("extract measurements");
    assert_eq!(measurements.len(), 1);
    assert_eq!(
        measurements[0].referenced_sop_instance_uid.as_deref(),
        Some("1.2.3.4")
    );
}
