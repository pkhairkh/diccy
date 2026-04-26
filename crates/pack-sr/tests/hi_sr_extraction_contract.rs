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
    item.insert(Element::new(TAG_CODE_VALUE, Vr::Sh, Value::Str(code.to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_CODING_SCHEME_DESIGNATOR, Vr::Sh, Value::Str(scheme.to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_CODE_MEANING, Vr::Lo, Value::Str(meaning.to_string()),
    ).unwrap());
    item
}

fn build_num_item(value: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("NUM".to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item("G-D7FE", "SRT", "Length")]),
    ).unwrap());

    let mut measured = Dataset::new();
    measured.insert(Element::new(TAG_NUMERIC_VALUE, Vr::Ds, Value::Str(value.to_string()),
    ).unwrap());
    measured.insert(Element::new(TAG_MEASUREMENT_UNITS_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item("mm", "UCUM", "millimeter")]),
    ).unwrap());
    item.insert(Element::new(TAG_MEASURED_VALUE_SEQUENCE, Vr::Sq, Value::Sequence(vec![measured]),
    ).unwrap());
    item
}

fn build_text_item(text: &str) -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("TEXT".to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item("121106", "DCM", "Finding")]),
    ).unwrap());
    item.insert(Element::new(TAG_TEXT_VALUE, Vr::Ut, Value::Str(text.to_string()),
    ).unwrap());
    item
}

fn build_code_item() -> Dataset {
    let mut item = Dataset::new();
    item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CODE".to_string()),
    ).unwrap());
    item.insert(Element::new(TAG_CONCEPT_NAME_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item("121071", "DCM", "Interpretation")]),
    ).unwrap());
    item.insert(Element::new(TAG_CONCEPT_CODE_SEQUENCE, Vr::Sq, Value::Sequence(vec![concept_item("R-404FB", "SRT", "Normal")]),
    ).unwrap());
    item
}

#[test]
fn sr_nested_extraction_is_deterministic_across_num_text_and_code_items() {
    // REQ-HI-431
    let mut container = Dataset::new();
    container.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CONTAINER".to_string()),
    ).unwrap());
    container.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![
            build_num_item("12.5"),
            build_text_item("Nested note"),
            build_code_item(),
        ]),
    ).unwrap());

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![container]),
    ).unwrap());

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
    non_finite.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_num_item("NaN")]),
    ).unwrap());
    let num_err = extract_measurements(&non_finite).expect_err("non-finite numeric");
    assert!(matches!(num_err.kind(), ErrorKind::InvalidTagValue { .. }));

    let mut empty_text = Dataset::new();
    empty_text.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![build_text_item("   ")]),
    ).unwrap());
    let text_err = extract_text_observations(&empty_text).expect_err("empty text");
    assert!(matches!(text_err.kind(), ErrorKind::InvalidTagValue { .. }));

    let mut missing_code = Dataset::new();
    let mut invalid_code_item = Dataset::new();
    invalid_code_item.insert(Element::new(TAG_VALUE_TYPE, Vr::Cs, Value::Str("CODE".to_string()),
    ).unwrap());
    missing_code.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![invalid_code_item]),
    ).unwrap());
    let code_err = extract_code_observations(&missing_code).expect_err("missing code seq");
    assert!(matches!(
        code_err.kind(),
        ErrorKind::MissingRequiredTag { .. }
    ));
}

#[test]
fn sr_provenance_exposes_referenced_sop_instance_uids() {
    // REQ-HI-432
    let mut ref_item = Dataset::new();
    ref_item.insert(Element::new(TAG_REFERENCED_SOP_INSTANCE_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string()),
    ).unwrap());

    let mut num_item = build_num_item("7.0");
    num_item.insert(Element::new(TAG_REFERENCED_SOP_SEQUENCE, Vr::Sq, Value::Sequence(vec![ref_item]),
    ).unwrap());

    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![num_item]),
    ).unwrap());

    let measurements = extract_measurements(&dataset).expect("extract measurements");
    assert_eq!(measurements.len(), 1);
    assert_eq!(
        measurements[0].referenced_sop_instance_uid.as_deref(),
        Some("1.2.3.4")
    );
}
