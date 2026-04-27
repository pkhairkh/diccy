#![deny(missing_docs)]

//! Shared parsing helpers and trait definitions for modality packs.

use dicom_core::{
    enforce_limit, parse_f64_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};

// ---------------------------------------------------------------------------
// S13-T1: Pack trait
// ---------------------------------------------------------------------------

/// A trait for pack marker types that encapsulates feature gating and
/// SOP class validation.
///
/// Each pack crate defines a zero-sized marker type (e.g. `EnhancedPack`,
/// `GspsPack`) that implements this trait to declare which SOP Class UIDs it
/// supports and which Cargo feature enables it.
pub trait Pack: Sized {
    /// Human-readable feature name (e.g. `"pack-enhanced"`, `"gsps"`).
    const FEATURE: &'static str;

    /// The list of SOP Class UIDs this pack supports.
    const SOP_CLASS_UIDS: &'static [&'static str];

    /// Return `true` when the pack's Cargo feature is enabled for the
    /// current build.
    fn enabled() -> bool;

    /// Verify that the given `sop_class_uid` is supported by this pack and
    /// that the pack feature is enabled.
    ///
    /// Returns `Err(UnsupportedSopClass)` if the pack is disabled or the
    /// SOP class is not in [`Pack::SOP_CLASS_UIDS`].
    fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                format!("{} requires {} feature", sop_class_uid, Self::FEATURE),
            )));
        }
        if !Self::SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                format!("unsupported SOP class for {}", Self::FEATURE),
            )));
        }
        Ok(())
    }
}

/// Generic helper that delegates to [`Pack::ensure_supported`] for any pack
/// marker type `P`.
///
/// Useful when the pack type is known at compile time and callers want the
/// ergonomic `<P as Pack>::ensure_supported(uid)` shorthand.
pub fn ensure_pack_supported<P: Pack>(uid: &str) -> Result<()> {
    P::ensure_supported(uid)
}

// ---------------------------------------------------------------------------
// S13-T2: FromDataset trait
// ---------------------------------------------------------------------------

/// Trait for types that can be parsed from a DICOM [`Dataset`] within
/// configurable resource [`Limits`].
///
/// This is the shared interface behind the various `from_dataset` methods
/// that already exist on pack-domain types such as `RtDoseGrid`,
/// `RtStructureSet`, `RtPlanSummary`, `Segmentation`, and
/// `PresentationState`.
pub trait FromDataset: Sized {
    /// Parse `Self` from the given `dataset`, enforcing `limits`.
    fn from_dataset(dataset: &Dataset, limits: &Limits) -> Result<Self>;
}

// ---------------------------------------------------------------------------
// S13-T2: OverlayRenderable trait
// ---------------------------------------------------------------------------

/// Trait for types that can render an overlay onto a pixel buffer.
///
/// This provides a uniform interface for the overlay-on-frame operations
/// that exist in `pack-gsps`, `pack-seg`, and `pack-rt`. Implementations
/// are feature-gated behind the `"rendering"` feature which brings in
/// `dicom-pixel::DisplayFrame`.
#[cfg(feature = "rendering")]
pub trait OverlayRenderable {
    /// Render `self` as an overlay on top of `frame`.
    fn overlay_on(&self, frame: &mut dicom_pixel::DisplayFrame);
}

/// Read a `u16` (US VR) value from a DICOM dataset element.
///
/// Returns `Ok(None)` when the tag is absent.
pub fn read_u16(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::I32(v) => {
                let v = u16::try_from(*v)
                    .map_err(|_| invalid_tag_value(tag, "u16 value out of range"))?;
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
                enforce_limit(
                    "max_string_bytes",
                    bytes.len() as u64,
                    limits.max_string_bytes(),
                )?;
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
