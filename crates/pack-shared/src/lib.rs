#![deny(missing_docs)]

//! Shared parsing helpers for modality packs.

use dicom_core::{
    enforce_limit, parse_f64_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};

/// Read a `u16` (US VR) value from a DICOM dataset element.
///
/// Returns `Ok(None)` when the tag is absent.
pub fn read_u16(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::I32(v) => {
                let v = u16::try_from(*v).map_err(|_| {
                    invalid_tag_value(tag, "u16 value out of range")
                })?;
                Ok(Some(v))
            }
            Value::Str(s) => s
                .parse::<u16>()
                .map(Some)
                .map_err(|_| invalid_tag_value(tag, "expected u16 string")),
            _ => Err(invalid_tag_value(tag, "expected u16")),
        },
        None => Ok(None),
    }
}

/// Read raw bytes from a DICOM dataset element (OW/OB VR).
///
/// Returns `Ok(None)` when the tag is absent.
pub fn read_bytes<'a>(dataset: &'a Dataset, tag: Tag, limits: &Limits) -> Result<Option<&'a [u8]>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Bytes(bytes) => {
                enforce_limit("max_string_bytes", bytes.len() as u64, limits.max_string_bytes())?;
                Ok(Some(bytes.as_slice()))
            }
            _ => Err(invalid_tag_value(tag, "expected bytes")),
        },
        None => Ok(None),
    }
}

/// Construct a `MissingRequiredTag` error for the given tag.
pub fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
}

/// Parse all items in a DICOM sequence (SQ VR), returning each sub-dataset.
///
/// Returns an empty `Vec` when the tag is absent or the sequence is empty.
pub fn sequence_items(dataset: &Dataset, tag: Tag) -> Result<Vec<Dataset>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Sequence(items) => Ok(items.clone()),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Ok(Vec::new()),
    }
}

/// Parse the first item in a DICOM sequence, returning `Ok(None)` when absent or empty.
pub fn first_sequence_item(dataset: &Dataset, tag: Tag) -> Result<Option<Dataset>> {
    let items = sequence_items(dataset, tag)?;
    Ok(items.into_iter().next())
}

/// Read a DICOM sequence and call a parser closure on each item, collecting results.
///
/// This is the primary high-level helper for extracting typed data from
/// DICOM SQ attributes in pack crates.
pub fn read_sequence<T>(
    dataset: &Dataset,
    tag: Tag,
    mut parse_item: impl FnMut(&Dataset) -> Result<T>,
) -> Result<Vec<T>> {
    let items = sequence_items(dataset, tag)?;
    items.iter().map(|item| parse_item(item)).collect()
}

/// Parse a DICOM DS spacing pair from `tag`, returning `(row, col)` values.
pub fn parse_spacing_pair(
    dataset: &Dataset,
    tag: Tag,
    limits: &Limits,
) -> Result<Option<(f64, f64)>> {
    let Some(raw) = read_str(dataset, tag, limits)? else {
        return Ok(None);
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.len() != 2 {
        return Err(invalid_tag_value(tag, "expected two spacing values"));
    }
    let a = parse_f64_strict(tag, parts[0])?;
    let b = parse_f64_strict(tag, parts[1])?;
    if !a.is_finite() || !b.is_finite() || a <= 0.0 || b <= 0.0 {
        return Err(invalid_tag_value(
            tag,
            "spacing values must be finite and > 0",
        ));
    }
    Ok(Some((a, b)))
}

/// Parse a positive DICOM DS frame time value from `tag`.
pub fn parse_positive_time(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<f64>> {
    let Some(raw) = read_str(dataset, tag, limits)? else {
        return Ok(None);
    };
    let value = parse_f64_strict(tag, raw)?;
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid_tag_value(tag, "frame time must be finite and > 0"));
    }
    Ok(Some(value))
}

/// Parse a uniform positive DICOM DS frame-time vector from `tag`.
///
/// The returned value is the first frame time when all entries are within `eps`.
pub fn parse_uniform_time_vector(
    dataset: &Dataset,
    tag: Tag,
    eps: f64,
    limits: &Limits,
) -> Result<Option<f64>> {
    let Some(raw) = read_str(dataset, tag, limits)? else {
        return Ok(None);
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.is_empty() {
        return Err(invalid_tag_value(
            tag,
            "frame time vector must contain values",
        ));
    }
    let mut values = Vec::with_capacity(parts.len());
    for part in parts {
        let value = parse_f64_strict(tag, part)?;
        if !value.is_finite() || value <= 0.0 {
            return Err(invalid_tag_value(
                tag,
                "frame time vector values must be finite and > 0",
            ));
        }
        values.push(value);
    }
    let first = values[0];
    if values.iter().any(|value| (*value - first).abs() > eps) {
        return Err(invalid_tag_value(
            tag,
            "frame time vector values must match in initial scope",
        ));
    }
    Ok(Some(first))
}

/// Read a string value from a DICOM dataset element, enforcing string-byte limits.
///
/// Returns `Ok(None)` when the tag is absent. Handles `Str`, `Uid`, and
/// UTF-8 `Bytes` value variants.
pub fn read_str<'a>(dataset: &'a Dataset, tag: Tag, limits: &Limits) -> Result<Option<&'a str>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Str(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(Some(value.as_str()))
            }
            Value::Uid(value) => {
                enforce_limit(
                    "max_string_bytes",
                    value.len() as u64,
                    limits.max_string_bytes(),
                )?;
                Ok(Some(value.as_str()))
            }
            Value::Bytes(bytes) => {
                enforce_limit(
                    "max_string_bytes",
                    bytes.len() as u64,
                    limits.max_string_bytes(),
                )?;
                std::str::from_utf8(bytes)
                    .map(Some)
                    .map_err(|_| invalid_tag_value(tag, "expected UTF-8 string bytes"))
            }
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

/// Construct an `InvalidTagValue` error for the given tag and detail message.
pub fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidTagValue {
            tag,
            detail: detail.into(),
        },
        "invalid tag value",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Vr};

    const TAG_TEST: Tag = Tag(0x0018, 0x1063);
    const TAG_VEC: Tag = Tag(0x0018, 0x1065);
    const TAG_U16: Tag = Tag(0x0018, 0x1066);
    const TAG_BYTES: Tag = Tag(0x7FE0, 0x0010);

    #[test]
    fn parse_spacing_pair_rejects_non_utf8_bytes() {
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_TEST, Vr::Ds, Value::Bytes(vec![0xff, 0xfe]),
        ).unwrap());
        let err = parse_spacing_pair(&dataset, TAG_TEST, &Limits::default())
            .expect_err("expected invalid utf8");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn parse_uniform_time_vector_accepts_uniform_values() {
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_VEC, Vr::Ds, Value::Str("40\\40\\40".to_string()),
        ).unwrap());
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
        dataset.insert(
            Element::new(TAG_BYTES, Vr::Ow, Value::Bytes(vec![1, 2, 3])).unwrap(),
        );
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
}
