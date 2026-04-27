// Auto-extracted from /home/z/diccy/crates/pack-shared/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


use pack_shared::*;
use dicom_core::{Dataset, Element, ErrorKind, Limits, Tag, Value, Vr};

const TAG_TEST: Tag = Tag(0x0018, 0x1063);
const TAG_VEC: Tag = Tag(0x0018, 0x1065);
const TAG_U16: Tag = Tag(0x0018, 0x1066);
const TAG_BYTES: Tag = Tag(0x7FE0, 0x0010);

#[test]
fn parse_spacing_pair_rejects_non_utf8_bytes() {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_TEST, Vr::Ds, Value::Bytes(vec![0xff, 0xfe])).unwrap());
    let err = parse_spacing_pair(&dataset, TAG_TEST, &Limits::default())
        .expect_err("expected invalid utf8");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn parse_uniform_time_vector_accepts_uniform_values() {
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_VEC, Vr::Ds, Value::Str("40\\40\\40".to_string())).unwrap());
    let value = parse_uniform_time_vector(&dataset, TAG_VEC, 1e-6, &Limits::default())
        .expect("parse")
        .expect("value");
    assert_eq!(value, 40.0);
}

#[test]
fn read_u16_parses_numeric_value() {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_U16, Vr::Us, Value::I32(42)).unwrap());
    let v = read_u16(&dataset, TAG_U16).expect("parse").expect("value");
    assert_eq!(v, 42);
}

#[test]
fn read_u16_returns_none_when_absent() {
    let dataset = Dataset::new();
    assert!(read_u16(&dataset, TAG_U16).unwrap().is_none());
}

#[test]
fn read_bytes_parses_ow_value() {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_BYTES, Vr::Ow, Value::Bytes(vec![1, 2, 3])).unwrap());
    let v = read_bytes(&dataset, TAG_BYTES, &Limits::default())
        .expect("parse")
        .expect("value");
    assert_eq!(v, &[1, 2, 3]);
}

#[test]
fn missing_required_tag_produces_correct_error_kind() {
    let tag = Tag(0x0010, 0x0010);
    let err = missing_required_tag(tag);
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn sequence_items_returns_empty_for_absent_tag() {
    let dataset = Dataset::new();
    let items = sequence_items(&dataset, Tag(0x0008, 0x1115)).unwrap();
    assert!(items.is_empty());
}

#[test]
fn first_sequence_item_returns_none_for_absent() {
    let dataset = Dataset::new();
    assert!(first_sequence_item(&dataset, Tag(0x0008, 0x1115))
        .unwrap()
        .is_none());
}

#[test]
fn read_sequence_collects_parsed_items() {
    let dataset = Dataset::new();
    // Empty sequence — just verify it doesn't panic
    let result: Vec<Tag> =
        read_sequence(&dataset, Tag(0x0008, 0x1115), |_ds| Ok(Tag(0, 0))).unwrap();
    assert!(result.is_empty());
}
