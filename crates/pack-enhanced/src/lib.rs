#![deny(missing_docs)]

//! Enhanced CT/MR pack: multi-frame functional group parsing and validation.

use dicom_core::{
    parse_f64_strict, parse_i32_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};

/// Enhanced CT Image Storage SOP Class UID.
pub const SOP_CLASS_ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";
/// Enhanced MR Image Storage SOP Class UID.
pub const SOP_CLASS_ENHANCED_MR: &str = "1.2.840.10008.5.1.4.1.1.4.1";

/// Enhanced SOP Class manifest list.
pub const ENHANCED_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_ENHANCED_CT, SOP_CLASS_ENHANCED_MR];

/// Marker type for enhanced multi-frame pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnhancedPack;

impl EnhancedPack {
    /// Return true when the enhanced pack is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-enhanced")
    }

    /// Require the enhanced pack to be enabled for enhanced SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "enhanced pack requires pack-enhanced feature",
            )));
        }
        if !ENHANCED_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported enhanced SOP class",
            )));
        }
        Ok(())
    }
}

/// Number of Frames tag.
pub const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
/// Shared Functional Groups Sequence tag.
pub const TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE: Tag = Tag(0x5200, 0x9229);
/// Per-frame Functional Groups Sequence tag.
pub const TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE: Tag = Tag(0x5200, 0x9230);
/// Plane Position Sequence tag.
pub const TAG_PLANE_POSITION_SEQUENCE: Tag = Tag(0x0020, 0x9113);
/// Plane Orientation Sequence tag.
pub const TAG_PLANE_ORIENTATION_SEQUENCE: Tag = Tag(0x0020, 0x9116);
/// Pixel Measures Sequence tag.
pub const TAG_PIXEL_MEASURES_SEQUENCE: Tag = Tag(0x0028, 0x9110);
/// Frame Content Sequence tag.
pub const TAG_FRAME_CONTENT_SEQUENCE: Tag = Tag(0x0020, 0x9111);
/// Image Position (Patient) tag.
pub const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
/// Image Orientation (Patient) tag.
pub const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);
/// In-Stack Position Number tag.
pub const TAG_IN_STACK_POSITION_NUMBER: Tag = Tag(0x0020, 0x9057);
/// Pixel Spacing tag.
pub const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
/// Slice Thickness tag.
pub const TAG_SLICE_THICKNESS: Tag = Tag(0x0018, 0x0050);
/// Rescale Slope tag.
pub const TAG_RESCALE_SLOPE: Tag = Tag(0x0028, 0x1053);
/// Rescale Intercept tag.
pub const TAG_RESCALE_INTERCEPT: Tag = Tag(0x0028, 0x1052);
/// Pixel Value Transformation Sequence tag.
pub const TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE: Tag = Tag(0x0028, 0x9145);

/// Per-frame geometry extracted from functional groups.
#[derive(Debug, Clone, PartialEq)]
pub struct EnhancedFrameGeometry {
    /// Image Position (Patient).
    pub ipp: [f64; 3],
    /// Image Orientation (Patient).
    pub iop: [f64; 6],
    /// Pixel spacing (sx, sy) in mm.
    pub pixel_spacing: (f64, f64),
    /// Slice thickness, if provided.
    pub slice_thickness: Option<f64>,
}

/// Per-frame rescale parameters.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct EnhancedRescale {
    /// Rescale slope.
    pub slope: f64,
    /// Rescale intercept.
    pub intercept: f64,
}

/// Per-frame functional group selection.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameGroups<'a> {
    /// Shared functional group dataset.
    pub shared: &'a Dataset,
    /// Per-frame functional group dataset.
    pub per_frame: &'a Dataset,
}

/// Extract the shared/per-frame group datasets for a specific frame index.
pub fn select_frame_groups<'a>(
    dataset: &'a Dataset,
    frame_index: u32,
    limits: &Limits,
) -> Result<FrameGroups<'a>> {
    let shared = first_sequence_item(dataset, TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE)?;
    let per_frame_items = sequence_items(dataset, TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE)?;
    if per_frame_items.is_empty() {
        return Err(missing_required_tag(
            TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
        ));
    }
    let frame_count = read_number_of_frames(dataset, limits)?;
    if per_frame_items.len() != frame_count as usize {
        return Err(invalid_tag_value(
            TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            "per-frame group count must match NumberOfFrames",
        ));
    }
    let index = frame_index as usize;
    if index >= per_frame_items.len() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
                detail: "frame index out of range".to_string(),
            },
            "frame index out of range",
        )));
    }
    Ok(FrameGroups {
        shared,
        per_frame: &per_frame_items[index],
    })
}

/// Extract per-frame geometry from functional groups.
pub fn extract_frame_geometry(
    groups: &FrameGroups<'_>,
    limits: &Limits,
) -> Result<EnhancedFrameGeometry> {
    validate_frame_content(groups.shared, groups.per_frame)?;
    let shared_pixel_measures =
        resolve_sequence(groups.shared, groups.per_frame, TAG_PIXEL_MEASURES_SEQUENCE)?;
    let plane_position =
        resolve_sequence(groups.shared, groups.per_frame, TAG_PLANE_POSITION_SEQUENCE)?;
    let plane_orientation = resolve_sequence(
        groups.shared,
        groups.per_frame,
        TAG_PLANE_ORIENTATION_SEQUENCE,
    )?;

    let ipp_vals = parse_f64_vec(plane_position, TAG_IMAGE_POSITION, 3, limits)?;
    let iop_vals = parse_f64_vec(plane_orientation, TAG_IMAGE_ORIENTATION, 6, limits)?;
    let spacing = parse_f64_vec(shared_pixel_measures, TAG_PIXEL_SPACING, 2, limits)?;
    let ipp = [ipp_vals[0], ipp_vals[1], ipp_vals[2]];
    let iop = [
        iop_vals[0],
        iop_vals[1],
        iop_vals[2],
        iop_vals[3],
        iop_vals[4],
        iop_vals[5],
    ];
    if spacing[0] <= 0.0 || spacing[1] <= 0.0 {
        return Err(invalid_tag_value(
            TAG_PIXEL_SPACING,
            "pixel spacing values must be > 0",
        ));
    }
    let pixel_spacing = (spacing[0], spacing[1]);
    let slice_thickness = parse_optional_f64(shared_pixel_measures, TAG_SLICE_THICKNESS, limits)?;
    if let Some(value) = slice_thickness {
        if value <= 0.0 {
            return Err(invalid_tag_value(
                TAG_SLICE_THICKNESS,
                "slice thickness must be > 0 when present",
            ));
        }
    }

    Ok(EnhancedFrameGeometry {
        ipp,
        iop,
        pixel_spacing,
        slice_thickness,
    })
}

fn validate_frame_content(shared: &Dataset, per_frame: &Dataset) -> Result<()> {
    let Some(frame_content) =
        resolve_sequence_optional(shared, per_frame, TAG_FRAME_CONTENT_SEQUENCE)?
    else {
        return Ok(());
    };
    let _ = parse_positive_u32(frame_content, TAG_IN_STACK_POSITION_NUMBER)?;
    Ok(())
}

fn parse_positive_u32(dataset: &Dataset, tag: Tag) -> Result<u32> {
    if let Some(raw) = read_str(dataset, tag)? {
        let value = parse_i32_strict(tag, raw)?;
        if value < 1 {
            return Err(invalid_tag_value(tag, "value must be >= 1"));
        }
        return Ok(value as u32);
    }
    if let Some(value) = dataset.get_i32(tag) {
        if value < 1 {
            return Err(invalid_tag_value(tag, "value must be >= 1"));
        }
        return Ok(value as u32);
    }
    Err(missing_required_tag(tag))
}

/// Extract per-frame rescale parameters from functional groups.
pub fn extract_frame_rescale(
    groups: &FrameGroups<'_>,
    limits: &Limits,
) -> Result<Option<EnhancedRescale>> {
    let seq = resolve_sequence_optional(
        groups.shared,
        groups.per_frame,
        TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE,
    )?;
    let Some(seq_dataset) = seq else {
        return Ok(None);
    };
    let slope = parse_optional_f64(seq_dataset, TAG_RESCALE_SLOPE, limits)?;
    let intercept = parse_optional_f64(seq_dataset, TAG_RESCALE_INTERCEPT, limits)?;
    if let (Some(slope), Some(intercept)) = (slope, intercept) {
        if !slope.is_finite() || !intercept.is_finite() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidTagValue {
                    tag: TAG_RESCALE_SLOPE,
                    detail: "non-finite rescale parameters".to_string(),
                },
                "non-finite rescale parameters",
            )));
        }
        return Ok(Some(EnhancedRescale { slope, intercept }));
    }
    Ok(None)
}

/// Helper function for read_number_of_frames
pub fn read_number_of_frames(dataset: &Dataset, limits: &Limits) -> Result<u32> {
    let Some(raw) = read_str(dataset, TAG_NUMBER_OF_FRAMES)? else {
        return Err(missing_required_tag(TAG_NUMBER_OF_FRAMES));
    };
    let value = parse_i32_strict(TAG_NUMBER_OF_FRAMES, raw)?;
    if value < 1 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_NUMBER_OF_FRAMES,
                detail: "number of frames must be >= 1".to_string(),
            },
            "number of frames must be >= 1",
        )));
    }
    let count = value as u32;
    if count as u64 > limits.max_frames_per_instance() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "max_frames_per_instance",
                allowed: limits.max_frames_per_instance(),
                observed: count as u64,
            },
            "frame count exceeds limit",
        )));
    }
    Ok(count)
}

fn sequence_items(dataset: &Dataset, tag: Tag) -> Result<&[Dataset]> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Sequence(items) => Ok(items.as_slice()),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn first_sequence_item(dataset: &Dataset, tag: Tag) -> Result<&Dataset> {
    let items = sequence_items(dataset, tag)?;
    items.first().ok_or_else(|| missing_required_tag(tag))
}

fn resolve_sequence<'a>(
    shared: &'a Dataset,
    per_frame: &'a Dataset,
    tag: Tag,
) -> Result<&'a Dataset> {
    if let Ok(item) = first_sequence_item(per_frame, tag) {
        return Ok(item);
    }
    first_sequence_item(shared, tag)
}

fn resolve_sequence_optional<'a>(
    shared: &'a Dataset,
    per_frame: &'a Dataset,
    tag: Tag,
) -> Result<Option<&'a Dataset>> {
    if let Ok(item) = first_sequence_item(per_frame, tag) {
        return Ok(Some(item));
    }
    if let Ok(item) = first_sequence_item(shared, tag) {
        return Ok(Some(item));
    }
    Ok(None)
}

fn parse_f64_vec(
    dataset: &Dataset,
    tag: Tag,
    expected: usize,
    _limits: &Limits,
) -> Result<Vec<f64>> {
    let raw = read_str(dataset, tag)?.ok_or_else(|| missing_required_tag(tag))?;
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.len() != expected {
        return Err(invalid_tag_value(tag, "unexpected number of values"));
    }
    let mut values = Vec::with_capacity(expected);
    for part in parts {
        let value = parse_f64_strict(tag, part)?;
        if !value.is_finite() {
            return Err(invalid_tag_value(tag, "non-finite value"));
        }
        values.push(value);
    }
    Ok(values)
}

fn parse_optional_f64(dataset: &Dataset, tag: Tag, _limits: &Limits) -> Result<Option<f64>> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Ok(None);
    };
    let value = parse_f64_strict(tag, raw)?;
    if !value.is_finite() {
        return Err(invalid_tag_value(tag, "non-finite value"));
    }
    Ok(Some(value))
}

/// Helper function for read_str
pub fn read_str(dataset: &Dataset, tag: Tag) -> Result<Option<&str>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Str(value) => Ok(Some(value.as_str())),
            Value::Uid(value) => Ok(Some(value.as_str())),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

/// Helper function for missing_required_tag
pub fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
}

/// Helper function for invalid_tag_value
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
