#![deny(missing_docs)]

//! Shared parsing helpers for modality packs.

use dicom_core::{
    enforce_limit, parse_f64_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};

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

fn read_str<'a>(dataset: &'a Dataset, tag: Tag, limits: &Limits) -> Result<Option<&'a str>> {
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

fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
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
}
