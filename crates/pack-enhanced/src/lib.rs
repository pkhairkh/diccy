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

fn read_number_of_frames(dataset: &Dataset, limits: &Limits) -> Result<u32> {
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

fn read_str(dataset: &Dataset, tag: Tag) -> Result<Option<&str>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Str(value) => Ok(Some(value.as_str())),
            Value::Uid(value) => Ok(Some(value.as_str())),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
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
    use dicom_core::{Element, Limits, Vr};
    use dicom_io::parse_dataset_bytes;
    use sha2::{Digest, Sha256};
    use std::fs;
    use std::path::PathBuf;

    const ENHANCED_MANIFEST: &str = include_str!("../manifest.toml");
    const CORPUS_SAMPLE_ID: &str = "synthetic-enhanced-multiframe";
    const CORPUS_SAMPLE_FILE: &str = "enhanced_multiframe_sample.bin";
    const CORPUS_CONFIG_ID: &str = "enhanced-geometry-v1";
    const CORPUS_FORMAT: &str = "GeometryHash";

    fn parse_manifest_uids() -> Vec<String> {
        ENHANCED_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn corpus_sample_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/")
            .join(CORPUS_SAMPLE_FILE)
    }

    fn manifest_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/manifest.toml")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        let mut out = String::with_capacity(digest.len() * 2);
        for byte in digest {
            out.push_str(&format!("{byte:02x}"));
        }
        out
    }

    fn extract_quoted(line: &str) -> Option<String> {
        line.split('"').nth(1).map(|value| value.to_string())
    }

    fn manifest_entry() -> (String, String) {
        let text = fs::read_to_string(manifest_path()).expect("manifest read failed");
        let mut in_sample = false;
        let mut file_hash: Option<String> = None;
        let mut output_hash: Option<String> = None;
        let mut current_config: Option<String> = None;
        let mut current_format: Option<String> = None;

        for line in text.lines() {
            let line = line.trim();
            if line == "[[samples]]" {
                in_sample = false;
                current_config = None;
                current_format = None;
                continue;
            }
            if line.starts_with("id =") {
                let id = extract_quoted(line).expect("sample id parse");
                in_sample = id == CORPUS_SAMPLE_ID;
                continue;
            }
            if !in_sample {
                continue;
            }
            if line.starts_with("sha256 =") && file_hash.is_none() {
                file_hash = extract_quoted(line);
                continue;
            }
            if line == "[[samples.expected_outputs]]" {
                current_config = None;
                current_format = None;
                continue;
            }
            if line.starts_with("config_id =") {
                current_config = extract_quoted(line);
                continue;
            }
            if line.starts_with("format =") {
                current_format = extract_quoted(line);
                continue;
            }
            if line.starts_with("sha256 =")
                && current_config.as_deref() == Some(CORPUS_CONFIG_ID)
                && current_format.as_deref() == Some(CORPUS_FORMAT)
            {
                output_hash = extract_quoted(line);
            }
        }

        (
            file_hash.expect("sample sha256 missing in manifest"),
            output_hash.expect("expected output hash missing in manifest"),
        )
    }

    fn encode_text_bytes(value: &str) -> Vec<u8> {
        let bytes = value.as_bytes().to_vec();
        assert!(
            bytes.len().is_multiple_of(2),
            "text value must be even length for synthetic corpus"
        );
        bytes
    }

    fn encode_uid_bytes(value: &str) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        bytes
    }

    fn encode_item(item: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0xFFFEu16.to_le_bytes());
        out.extend_from_slice(&0xE000u16.to_le_bytes());
        out.extend_from_slice(&(item.len() as u32).to_le_bytes());
        out.extend_from_slice(item);
        out
    }

    fn encode_sequence_delim() -> [u8; 8] {
        let mut out = [0u8; 8];
        out[0..2].copy_from_slice(&0xFFFEu16.to_le_bytes());
        out[2..4].copy_from_slice(&0xE0DDu16.to_le_bytes());
        out
    }

    fn vr_bytes(vr: Vr) -> [u8; 2] {
        match vr {
            Vr::Ae => *b"AE",
            Vr::As => *b"AS",
            Vr::Cs => *b"CS",
            Vr::Da => *b"DA",
            Vr::Ds => *b"DS",
            Vr::Is => *b"IS",
            Vr::Lo => *b"LO",
            Vr::Lt => *b"LT",
            Vr::Pn => *b"PN",
            Vr::Sh => *b"SH",
            Vr::St => *b"ST",
            Vr::Tm => *b"TM",
            Vr::Ui => *b"UI",
            Vr::Ut => *b"UT",
            Vr::Ob => *b"OB",
            Vr::Ow => *b"OW",
            Vr::Sq => *b"SQ",
            Vr::Un => *b"UN",
            Vr::Other(bytes) => bytes,
            _ => *b"UN",
        }
    }

    fn encode_dataset_explicit(dataset: &Dataset) -> Vec<u8> {
        let mut out = Vec::new();
        for element in dataset.iter() {
            out.extend_from_slice(&encode_element_explicit(element));
        }
        out
    }

    fn encode_element_explicit(element: &Element) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&element.tag().0.to_le_bytes());
        out.extend_from_slice(&element.tag().1.to_le_bytes());
        match element.value() {
            Value::Sequence(items) => {
                let mut seq_bytes = Vec::new();
                for item in items {
                    let item_bytes = encode_dataset_explicit(item);
                    seq_bytes.extend_from_slice(&encode_item(&item_bytes));
                }
                out.extend_from_slice(b"SQ");
                out.extend_from_slice(&0u16.to_le_bytes());
                out.extend_from_slice(&u32::MAX.to_le_bytes());
                out.extend_from_slice(&seq_bytes);
                out.extend_from_slice(&encode_sequence_delim());
            }
            Value::Str(value) => {
                let bytes = encode_text_bytes(value);
                out.extend_from_slice(&vr_bytes(*element.vr()));
                out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                out.extend_from_slice(&bytes);
            }
            Value::Uid(value) => {
                let bytes = encode_uid_bytes(value);
                out.extend_from_slice(b"UI");
                out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                out.extend_from_slice(&bytes);
            }
            Value::Bytes(value) => {
                out.extend_from_slice(&vr_bytes(*element.vr()));
                out.extend_from_slice(&(value.len() as u16).to_le_bytes());
                out.extend_from_slice(value);
            }
            _ => panic!("unsupported value for synthetic corpus encoding"),
        }
        out
    }

    fn geometry_hash(dataset: &Dataset, limits: &Limits) -> Result<String> {
        let groups = select_frame_groups(dataset, 0, limits)?;
        let geometry = extract_frame_geometry(&groups, limits)?;
        let rescale = extract_frame_rescale(&groups, limits)?;
        let mut text = String::new();
        text.push_str(&format!(
            "ipp={:.6},{:.6},{:.6};",
            geometry.ipp[0], geometry.ipp[1], geometry.ipp[2]
        ));
        text.push_str(&format!(
            "iop={:.6},{:.6},{:.6},{:.6},{:.6},{:.6};",
            geometry.iop[0],
            geometry.iop[1],
            geometry.iop[2],
            geometry.iop[3],
            geometry.iop[4],
            geometry.iop[5]
        ));
        text.push_str(&format!(
            "spacing={:.6},{:.6};",
            geometry.pixel_spacing.0, geometry.pixel_spacing.1
        ));
        match geometry.slice_thickness {
            Some(value) => text.push_str(&format!("thickness={value:.6};")),
            None => text.push_str("thickness=none;"),
        }
        match rescale {
            Some(rescale) => text.push_str(&format!(
                "slope={:.6};intercept={:.6};",
                rescale.slope, rescale.intercept
            )),
            None => text.push_str("slope=none;intercept=none;"),
        }
        Ok(sha256_hex(text.as_bytes()))
    }

    #[test]
    #[cfg(not(feature = "pack-enhanced"))]
    fn pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(!EnhancedPack::enabled());
        let err = EnhancedPack::ensure_supported(SOP_CLASS_ENHANCED_CT).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-enhanced")]
    fn pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(EnhancedPack::enabled());
        EnhancedPack::ensure_supported(SOP_CLASS_ENHANCED_CT).expect("enhanced pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083, REQ-SOP-300
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in ENHANCED_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn manifest_uids_look_like_uids() {
        // REQ-CONF-083, REQ-SOP-300
        for uid in ENHANCED_SOP_CLASS_UIDS {
            assert!(uid.chars().all(|ch| ch.is_ascii_digit() || ch == '.'));
        }
    }

    fn build_enhanced_dataset_with_values(
        number_of_frames: &str,
        pixel_spacing_value: &str,
        slice_thickness_value: Option<&str>,
    ) -> Dataset {
        let mut shared = Dataset::new();
        let mut pixel_measures = Dataset::new();
        pixel_measures.insert(Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str(pixel_spacing_value.to_string()),
        ).unwrap());
        if let Some(slice_thickness) = slice_thickness_value {
            pixel_measures.insert(Element::new(TAG_SLICE_THICKNESS, Vr::Ds, Value::Str(slice_thickness.to_string()),
            ).unwrap());
        }
        shared.insert(Element::new(TAG_PIXEL_MEASURES_SEQUENCE, Vr::Sq, Value::Sequence(vec![pixel_measures]),
        ).unwrap());

        let mut per_frame = Dataset::new();
        let mut plane_position = Dataset::new();
        plane_position.insert(Element::new(TAG_IMAGE_POSITION, Vr::Ds, Value::Str("0\\0\\00".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PLANE_POSITION_SEQUENCE, Vr::Sq, Value::Sequence(vec![plane_position]),
        ).unwrap());
        let mut plane_orientation = Dataset::new();
        plane_orientation.insert(Element::new(TAG_IMAGE_ORIENTATION, Vr::Ds, Value::Str("1\\0\\0\\0\\1\\00".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PLANE_ORIENTATION_SEQUENCE, Vr::Sq, Value::Sequence(vec![plane_orientation]),
        ).unwrap());
        let mut rescale = Dataset::new();
        rescale.insert(Element::new(TAG_RESCALE_SLOPE, Vr::Ds, Value::Str("2.00".to_string()),
        ).unwrap());
        rescale.insert(Element::new(TAG_RESCALE_INTERCEPT, Vr::Ds, Value::Str("5.00".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE, Vr::Sq, Value::Sequence(vec![rescale]),
        ).unwrap());
        let mut frame_content = Dataset::new();
        frame_content.insert(Element::new(TAG_IN_STACK_POSITION_NUMBER, Vr::Is, Value::Str("1".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_FRAME_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![frame_content]),
        ).unwrap());

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str(number_of_frames.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![shared]),
        ).unwrap());
        dataset.insert(Element::new(TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![per_frame]),
        ).unwrap());
        dataset
    }

    fn build_enhanced_dataset() -> Dataset {
        build_enhanced_dataset_with_values("01", "0.5\\0.50", None)
    }

    #[test]
    fn enhanced_groups_extract_rescale() {
        // REQ-CONF-084, REQ-ENH-350
        let dataset = build_enhanced_dataset();
        let limits = Limits::default();
        let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
        let rescale = extract_frame_rescale(&groups, &limits).expect("rescale");
        assert_eq!(
            rescale.unwrap(),
            EnhancedRescale {
                slope: 2.0,
                intercept: 5.0
            }
        );
    }

    #[test]
    fn enhanced_missing_per_frame_groups_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![Dataset::new()]),
        ).unwrap());
        let limits = Limits::default();
        let err = select_frame_groups(&dataset, 0, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn enhanced_per_frame_count_mismatch_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let dataset = build_enhanced_dataset_with_values("2", "0.5\\0.50", None);
        let limits = Limits::default();
        let err = select_frame_groups(&dataset, 0, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn enhanced_missing_plane_position_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let mut shared = Dataset::new();
        let mut pixel_measures = Dataset::new();
        pixel_measures.insert(Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string()),
        ).unwrap());
        shared.insert(Element::new(TAG_PIXEL_MEASURES_SEQUENCE, Vr::Sq, Value::Sequence(vec![pixel_measures]),
        ).unwrap());

        let mut per_frame = Dataset::new();
        let mut plane_orientation = Dataset::new();
        plane_orientation.insert(Element::new(TAG_IMAGE_ORIENTATION, Vr::Ds, Value::Str("1\\0\\0\\0\\1\\0".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PLANE_ORIENTATION_SEQUENCE, Vr::Sq, Value::Sequence(vec![plane_orientation]),
        ).unwrap());

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![shared]),
        ).unwrap());
        dataset.insert(Element::new(TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![per_frame]),
        ).unwrap());

        let limits = Limits::default();
        let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
        let err = extract_frame_geometry(&groups, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn enhanced_invalid_frame_content_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let mut shared = Dataset::new();
        let mut pixel_measures = Dataset::new();
        pixel_measures.insert(Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string()),
        ).unwrap());
        shared.insert(Element::new(TAG_PIXEL_MEASURES_SEQUENCE, Vr::Sq, Value::Sequence(vec![pixel_measures]),
        ).unwrap());

        let mut per_frame = Dataset::new();
        let mut plane_position = Dataset::new();
        plane_position.insert(Element::new(TAG_IMAGE_POSITION, Vr::Ds, Value::Str("0\\0\\0".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PLANE_POSITION_SEQUENCE, Vr::Sq, Value::Sequence(vec![plane_position]),
        ).unwrap());
        let mut plane_orientation = Dataset::new();
        plane_orientation.insert(Element::new(TAG_IMAGE_ORIENTATION, Vr::Ds, Value::Str("1\\0\\0\\0\\1\\0".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_PLANE_ORIENTATION_SEQUENCE, Vr::Sq, Value::Sequence(vec![plane_orientation]),
        ).unwrap());
        let mut frame_content = Dataset::new();
        frame_content.insert(Element::new(TAG_IN_STACK_POSITION_NUMBER, Vr::Is, Value::Str("0".to_string()),
        ).unwrap());
        per_frame.insert(Element::new(TAG_FRAME_CONTENT_SEQUENCE, Vr::Sq, Value::Sequence(vec![frame_content]),
        ).unwrap());

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![shared]),
        ).unwrap());
        dataset.insert(Element::new(TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![per_frame]),
        ).unwrap());

        let limits = Limits::default();
        let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
        let err = extract_frame_geometry(&groups, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn enhanced_non_positive_pixel_spacing_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let dataset = build_enhanced_dataset_with_values("01", "0\\1", None);
        let limits = Limits::default();
        let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
        let err = extract_frame_geometry(&groups, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn enhanced_non_positive_slice_thickness_fails() {
        // REQ-CONF-084, REQ-ENH-350
        let dataset = build_enhanced_dataset_with_values("01", "1\\1", Some("0"));
        let limits = Limits::default();
        let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
        let err = extract_frame_geometry(&groups, &limits).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn enhanced_corpus_sample_matches_manifest() {
        // REQ-ENH-350: enhanced multi-frame functional groups must parse deterministically.
        // REQ-SOP-300: enhanced SOP classes are covered by corpus validation.
        // REQ-TEST-720, REQ-TEST-722: corpus samples must be hash-validated.
        if std::env::var("UPDATE_ENHANCED_CORPUS").is_ok() {
            return;
        }
        let bytes = fs::read(corpus_sample_path()).expect("corpus sample read failed");
        let file_hash = sha256_hex(&bytes);
        let (manifest_hash, expected_output_hash) = manifest_entry();
        assert_eq!(file_hash, manifest_hash);

        let limits = Limits::default();
        let dataset = parse_dataset_bytes(&bytes, "1.2.840.10008.1.2.1", &limits)
            .expect("parse corpus dataset");
        let output_hash = geometry_hash(&dataset, &limits).expect("geometry hash");
        assert_eq!(output_hash, expected_output_hash);
    }

    #[test]
    fn update_enhanced_corpus_sample() {
        if std::env::var("UPDATE_ENHANCED_CORPUS").is_err() {
            return;
        }
        let dataset = build_enhanced_dataset();
        let bytes = encode_dataset_explicit(&dataset);
        fs::write(corpus_sample_path(), &bytes).expect("write corpus sample");
        let file_hash = sha256_hex(&bytes);
        let output_hash = geometry_hash(&dataset, &Limits::default()).expect("geometry hash");
        println!("enhanced corpus sha256={file_hash}");
        println!("enhanced corpus output sha256={output_hash}");
    }
}
