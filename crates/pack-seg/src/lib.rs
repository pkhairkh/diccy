#![deny(missing_docs)]

//! Segmentation pack: binary and fractional segmentation parsing, encoding,
//! and deterministic overlays.

use dicom_core::{parse_i32_strict, Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
#[cfg(feature = "rendering")]
use dicom_pixel::{DisplayFrame, PixelFormat};

/// Segmentation Storage SOP Class UID.
pub const SOP_CLASS_SEG: &str = "1.2.840.10008.5.1.4.1.1.66.4";

/// SEG SOP Class manifest list.
pub const SEG_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_SEG];

/// Marker type for segmentation pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegPack;

impl SegPack {
    /// Return true when SEG pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-seg")
    }

    /// Require the SEG pack to be enabled for segmentation SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "segmentation requires pack-seg feature",
            )));
        }
        if !SEG_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported segmentation SOP class",
            )));
        }
        Ok(())
    }
}

/// Segmentation type.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SegmentationType {
    /// Binary segmentation (0/1).
    Binary,
    /// Fractional segmentation (probability 0.0–1.0).
    Fractional,
}

impl SegmentationType {
    /// Return the DICOM CS string for this segmentation type.
    pub fn to_cs_string(self) -> &'static str {
        match self {
            SegmentationType::Binary => "BINARY",
            SegmentationType::Fractional => "FRACTIONAL",
        }
    }

    /// Parse a DICOM CS string into a segmentation type.
    pub fn from_cs_string(s: &str) -> Result<Self> {
        match s {
            "BINARY" => Ok(SegmentationType::Binary),
            "FRACTIONAL" => Ok(SegmentationType::Fractional),
            _ => Err(invalid_tag_value(
                TAG_SEGMENTATION_TYPE,
                "expected BINARY or FRACTIONAL",
            )),
        }
    }
}

/// Parsed segmentation data (binary or fractional, single-frame or multi-frame).
#[derive(Debug, Clone, PartialEq)]
pub struct Segmentation {
    /// Rows.
    pub rows: u16,
    /// Columns.
    pub cols: u16,
    /// Number of frames.
    pub frames: u16,
    /// Frame of Reference UID.
    pub frame_of_reference_uid: String,
    /// Segment number.
    pub segment_number: u16,
    /// Referenced SOP Instance UID, if present.
    pub referenced_sop_instance_uid: Option<String>,
    /// Mask bytes (one byte per pixel, 0 or 255), frame-major order.
    /// Used for binary segmentation.
    pub mask: Vec<u8>,
    /// Fractional probability values (0.0–1.0 per pixel), frame-major order.
    /// Used for fractional segmentation.
    pub fractional_probability: Vec<f32>,
    /// Segmentation type.
    pub seg_type: SegmentationType,
}

/// Reference geometry for applying a segmentation overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegReference<'a> {
    /// Rows of the referenced image.
    pub rows: u16,
    /// Columns of the referenced image.
    pub cols: u16,
    /// Frame of Reference UID of the referenced image.
    pub frame_of_reference_uid: &'a str,
    /// SOP Instance UID of the referenced image.
    pub sop_instance_uid: &'a str,
}

impl Segmentation {
    /// Parse a segmentation from a dataset.
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let frames = match read_u16_optional(dataset, TAG_NUMBER_OF_FRAMES)? {
            Some(0) => {
                return Err(invalid_tag_value(
                    TAG_NUMBER_OF_FRAMES,
                    "number of frames must be >= 1",
                ))
            }
            Some(value) => value,
            None => 1,
        };
        let rows = read_u16(dataset, TAG_ROWS)?;
        let cols = read_u16(dataset, TAG_COLUMNS)?;
        let frame_of_reference_uid = read_str(dataset, TAG_FRAME_OF_REFERENCE_UID)?
            .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?
            .to_string();
        let seg_type_str = read_str(dataset, TAG_SEGMENTATION_TYPE)?
            .ok_or_else(|| missing_required_tag(TAG_SEGMENTATION_TYPE))?;
        let seg_type = SegmentationType::from_cs_string(seg_type_str)?;
        let segment_number = read_u16(dataset, TAG_SEGMENT_NUMBER)?;
        let referenced_sop_instance_uid = read_referenced_sop_instance_uid(dataset)?;

        let expected_pixels = rows as usize * cols as usize * frames as usize;

        match seg_type {
            SegmentationType::Binary => {
                let bits_allocated = read_u16(dataset, TAG_BITS_ALLOCATED)?;
                if bits_allocated != 1 {
                    return Err(invalid_tag_value(
                        TAG_BITS_ALLOCATED,
                        "binary segmentation BitsAllocated must be 1",
                    ));
                }
                let bits_stored = read_u16(dataset, TAG_BITS_STORED)?;
                if bits_stored != 1 {
                    return Err(invalid_tag_value(
                        TAG_BITS_STORED,
                        "binary segmentation BitsStored must be 1",
                    ));
                }
                let high_bit = read_u16(dataset, TAG_HIGH_BIT)?;
                if high_bit != 0 {
                    return Err(invalid_tag_value(
                        TAG_HIGH_BIT,
                        "binary segmentation HighBit must be 0",
                    ));
                }
                let pixel_data = read_bytes(dataset, TAG_PIXEL_DATA)?;
                let expected_bytes = expected_pixels.div_ceil(8);
                if pixel_data.len() < expected_bytes {
                    return Err(invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "segmentation pixel data too short",
                    ));
                }
                let mut mask = vec![0u8; expected_pixels];
                for i in 0..expected_pixels {
                    let byte = pixel_data[i / 8];
                    let bit = (byte >> (i % 8)) & 1;
                    mask[i] = if bit == 1 { 0xFF } else { 0x00 };
                }
                Ok(Self {
                    rows,
                    cols,
                    frames,
                    frame_of_reference_uid,
                    segment_number,
                    referenced_sop_instance_uid,
                    mask,
                    fractional_probability: Vec::new(),
                    seg_type: SegmentationType::Binary,
                })
            }
            SegmentationType::Fractional => {
                let bits_allocated = read_u16(dataset, TAG_BITS_ALLOCATED)?;
                if bits_allocated != 8 {
                    return Err(invalid_tag_value(
                        TAG_BITS_ALLOCATED,
                        "fractional segmentation BitsAllocated must be 8",
                    ));
                }
                let bits_stored = read_u16(dataset, TAG_BITS_STORED)?;
                if bits_stored != 8 {
                    return Err(invalid_tag_value(
                        TAG_BITS_STORED,
                        "fractional segmentation BitsStored must be 8",
                    ));
                }
                let high_bit = read_u16(dataset, TAG_HIGH_BIT)?;
                if high_bit != 7 {
                    return Err(invalid_tag_value(
                        TAG_HIGH_BIT,
                        "fractional segmentation HighBit must be 7",
                    ));
                }
                let pixel_data = read_bytes(dataset, TAG_PIXEL_DATA)?;
                if pixel_data.len() < expected_pixels {
                    return Err(invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "fractional segmentation pixel data too short",
                    ));
                }
                let fractional_probability: Vec<f32> = pixel_data[..expected_pixels]
                    .iter()
                    .map(|&b| b as f32 / 255.0)
                    .collect();
                Ok(Self {
                    rows,
                    cols,
                    frames,
                    frame_of_reference_uid,
                    segment_number,
                    referenced_sop_instance_uid,
                    mask: Vec::new(),
                    fractional_probability,
                    seg_type: SegmentationType::Fractional,
                })
            }
        }
    }

    /// Apply this segmentation frame 0 as a deterministic RGBA overlay onto a frame.
    #[cfg(feature = "rendering")]
    pub fn overlay_on(
        &self,
        frame: &DisplayFrame,
        reference: SegReference<'_>,
    ) -> Result<DisplayFrame> {
        self.overlay_on_frame(frame, reference, 0)
    }

    /// Apply a selected segmentation frame as a deterministic RGBA overlay onto a frame.
    #[cfg(feature = "rendering")]
    pub fn overlay_on_frame(
        &self,
        frame: &DisplayFrame,
        reference: SegReference<'_>,
        frame_index: u16,
    ) -> Result<DisplayFrame> {
        if self.frame_of_reference_uid != reference.frame_of_reference_uid {
            return Err(invalid_geometry("frame of reference UID mismatch"));
        }
        let seg_ref_uid = self
            .referenced_sop_instance_uid
            .as_deref()
            .ok_or_else(|| missing_required_tag(TAG_REFERENCED_SOP_INSTANCE_UID))?;
        if seg_ref_uid != reference.sop_instance_uid {
            return Err(invalid_tag_value(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                "referenced SOP Instance UID mismatch",
            ));
        }
        if self.rows != reference.rows || self.cols != reference.cols {
            return Err(invalid_geometry(
                "segmentation grid does not match reference grid",
            ));
        }
        if frame.width != reference.cols as u32 || frame.height != reference.rows as u32 {
            return Err(invalid_tag_value(
                TAG_PIXEL_DATA,
                "reference dimensions do not match frame",
            ));
        }
        if frame_index >= self.frames {
            return Err(invalid_tag_value(
                TAG_NUMBER_OF_FRAMES,
                "frame index out of range",
            ));
        }
        let frame_pixels = self.rows as usize * self.cols as usize;
        let frame_start = frame_index as usize * frame_pixels;
        let frame_end = frame_start + frame_pixels;
        let mut out = vec![0u8; (frame.width * frame.height * 4) as usize];
        let color = segment_color(self.segment_number);

        match self.seg_type {
            SegmentationType::Binary => {
                let frame_mask = &self.mask[frame_start..frame_end];
                for (idx, &mask_val) in frame_mask.iter().enumerate() {
                    if mask_val == 0 {
                        continue;
                    }
                    let base = idx * 4;
                    out[base] = color[0];
                    out[base + 1] = color[1];
                    out[base + 2] = color[2];
                    out[base + 3] = color[3];
                }
            }
            SegmentationType::Fractional => {
                let frame_probs = &self.fractional_probability[frame_start..frame_end];
                for (idx, &prob) in frame_probs.iter().enumerate() {
                    if prob <= 0.0 {
                        continue;
                    }
                    let base = idx * 4;
                    let alpha = (prob.clamp(0.0, 1.0) * color[3] as f32) as u8;
                    out[base] = color[0];
                    out[base + 1] = color[1];
                    out[base + 2] = color[2];
                    out[base + 3] = alpha;
                }
            }
        }

        Ok(DisplayFrame {
            width: frame.width,
            height: frame.height,
            format: PixelFormat::Rgba8,
            bytes: out,
        })
    }
}

/// Domain-level descriptor for a segmentation overlay, containing grid
/// dimensions, mask/probability data, and reference geometry without any
/// rendering dependency.
///
/// This is a type alias for [`Segmentation`] since the segmentation fields
/// are all pure domain data. Use this type when you need a rendering-
/// independent reference to the parsed segmentation data.
pub type SegOverlayDescriptor = Segmentation;

// ---------------------------------------------------------------------------
// Encoder
// ---------------------------------------------------------------------------

/// Builder for encoding segmentation data into a DICOM Dataset.
///
/// Construct with [`SegmentationEncoder::new`], then set required fields via
/// builder methods, and finally call [`SegmentationEncoder::encode_to_dataset`].
///
/// # Example
///
/// ```ignore
/// use pack_seg::{SegmentationEncoder, SegmentationType};
///
/// let dataset = SegmentationEncoder::new(SegmentationType::Binary)
///     .rows(256)
///     .cols(256)
///     .frames(1)
///     .frame_of_reference_uid("1.2.3.4.5")
///     .segment_number(1)
///     .binary_mask(vec![0u8; 256 * 256])
///     .encode_to_dataset()
///     .expect("encode");
/// ```
#[derive(Debug, Clone)]
pub struct SegmentationEncoder {
    seg_type: SegmentationType,
    rows: Option<u16>,
    cols: Option<u16>,
    frames: Option<u16>,
    frame_of_reference_uid: Option<String>,
    segment_number: Option<u16>,
    binary_mask: Option<Vec<u8>>,
    fractional_mask: Option<Vec<f32>>,
    referenced_sop_instance_uid: Option<String>,
}

impl SegmentationEncoder {
    /// Create a new encoder for the given segmentation type.
    pub fn new(seg_type: SegmentationType) -> Self {
        Self {
            seg_type,
            rows: None,
            cols: None,
            frames: None,
            frame_of_reference_uid: None,
            segment_number: None,
            binary_mask: None,
            fractional_mask: None,
            referenced_sop_instance_uid: None,
        }
    }

    /// Set the number of rows.
    pub fn rows(mut self, rows: u16) -> Self {
        self.rows = Some(rows);
        self
    }

    /// Set the number of columns.
    pub fn cols(mut self, cols: u16) -> Self {
        self.cols = Some(cols);
        self
    }

    /// Set the number of frames (defaults to 1 if not called).
    pub fn frames(mut self, frames: u16) -> Self {
        self.frames = Some(frames);
        self
    }

    /// Set the Frame of Reference UID.
    pub fn frame_of_reference_uid(mut self, uid: impl Into<String>) -> Self {
        self.frame_of_reference_uid = Some(uid.into());
        self
    }

    /// Set the segment number.
    pub fn segment_number(mut self, num: u16) -> Self {
        self.segment_number = Some(num);
        self
    }

    /// Set the binary mask data (one byte per pixel, 0 or non-zero).
    pub fn binary_mask(mut self, mask: Vec<u8>) -> Self {
        self.binary_mask = Some(mask);
        self
    }

    /// Set the fractional probability mask data (0.0–1.0 per pixel).
    pub fn fractional_mask(mut self, mask: Vec<f32>) -> Self {
        self.fractional_mask = Some(mask);
        self
    }

    /// Set the referenced SOP Instance UID.
    pub fn referenced_sop_instance_uid(mut self, uid: impl Into<String>) -> Self {
        self.referenced_sop_instance_uid = Some(uid.into());
        self
    }

    /// Encode the segmentation data into a DICOM Dataset.
    ///
    /// Produces a deterministic dataset with all required DICOM tags for the
    /// Segmentation IOD, including bit-packed pixel data for binary and raw
    /// bytes for fractional.
    pub fn encode_to_dataset(self) -> Result<Dataset> {
        let rows = self.rows.ok_or_else(|| missing_required_tag(TAG_ROWS))?;
        let cols = self.cols.ok_or_else(|| missing_required_tag(TAG_COLUMNS))?;
        let frames = self.frames.unwrap_or(1);
        if frames == 0 {
            return Err(invalid_tag_value(
                TAG_NUMBER_OF_FRAMES,
                "number of frames must be >= 1",
            ));
        }
        let frame_of_reference_uid = self
            .frame_of_reference_uid
            .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?;
        let segment_number = self
            .segment_number
            .ok_or_else(|| missing_required_tag(TAG_SEGMENT_NUMBER))?;

        let expected_pixels = rows as usize * cols as usize * frames as usize;

        let pixel_data = match self.seg_type {
            SegmentationType::Binary => {
                let mask = self.binary_mask.ok_or_else(|| {
                    invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "binary mask data required for BINARY segmentation",
                    )
                })?;
                if mask.len() < expected_pixels {
                    return Err(invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "binary mask data too short for rows*cols*frames",
                    ));
                }
                // Bit-pack: LSB-first within each byte.
                let byte_count = expected_pixels.div_ceil(8);
                let mut packed = vec![0u8; byte_count];
                for i in 0..expected_pixels {
                    if mask[i] != 0 {
                        packed[i / 8] |= 1 << (i % 8);
                    }
                }
                packed
            }
            SegmentationType::Fractional => {
                let frac = self.fractional_mask.ok_or_else(|| {
                    invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "fractional mask data required for FRACTIONAL segmentation",
                    )
                })?;
                if frac.len() < expected_pixels {
                    return Err(invalid_tag_value(
                        TAG_PIXEL_DATA,
                        "fractional mask data too short for rows*cols*frames",
                    ));
                }
                // Convert f32 probability [0.0, 1.0] to u8 [0, 255].
                frac[..expected_pixels]
                    .iter()
                    .map(|&p| (p.clamp(0.0, 1.0) * 255.0).round() as u8)
                    .collect()
            }
        };

        let (bits_allocated, bits_stored, high_bit) = match self.seg_type {
            SegmentationType::Binary => (1u16, 1u16, 0u16),
            SegmentationType::Fractional => (8u16, 8u16, 7u16),
        };

        let mut dataset = Dataset::new();

        // Rows
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str(rows.to_string())).unwrap());
        // Columns
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str(cols.to_string())).unwrap());
        // NumberOfFrames
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str(frames.to_string())).unwrap(),
        );
        // FrameOfReferenceUID
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid(frame_of_reference_uid),
            )
            .unwrap(),
        );
        // SegmentationType
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str(self.seg_type.to_cs_string().to_string()),
            )
            .unwrap(),
        );
        // SegmentNumber
        dataset.insert(
            Element::new(
                TAG_SEGMENT_NUMBER,
                Vr::Us,
                Value::Str(segment_number.to_string()),
            )
            .unwrap(),
        );
        // BitsAllocated
        dataset.insert(
            Element::new(
                TAG_BITS_ALLOCATED,
                Vr::Us,
                Value::Str(bits_allocated.to_string()),
            )
            .unwrap(),
        );
        // BitsStored
        dataset.insert(
            Element::new(TAG_BITS_STORED, Vr::Us, Value::Str(bits_stored.to_string())).unwrap(),
        );
        // HighBit
        dataset
            .insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str(high_bit.to_string())).unwrap());
        // PixelData
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(pixel_data)).unwrap());

        // ReferencedSeriesSequence (optional)
        if let Some(ref_uid) = self.referenced_sop_instance_uid {
            let mut ref_instance = Dataset::new();
            ref_instance.insert(
                Element::new(TAG_REFERENCED_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(ref_uid)).unwrap(),
            );
            let mut ref_series = Dataset::new();
            ref_series.insert(
                Element::new(
                    TAG_REFERENCED_INSTANCE_SEQUENCE,
                    Vr::Sq,
                    Value::Sequence(vec![ref_instance]),
                )
                .unwrap(),
            );
            dataset.insert(
                Element::new(
                    TAG_REFERENCED_SERIES_SEQUENCE,
                    Vr::Sq,
                    Value::Sequence(vec![ref_series]),
                )
                .unwrap(),
            );
        }

        Ok(dataset)
    }
}

// ---------------------------------------------------------------------------
// Tag constants
// ---------------------------------------------------------------------------

/// DICOM Tag (0x0028,0x0010)
pub const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
/// DICOM Tag (0x0028,0x0011)
pub const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
/// DICOM Tag (0x0028,0x0008)
pub const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
/// DICOM Tag (0x0020,0x0052)
pub const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
/// DICOM Tag (0x0062,0x0001)
pub const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
/// DICOM Tag (0x0062,0x0004)
pub const TAG_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x0004);
/// DICOM Tag (0x0028,0x0100)
pub const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
/// DICOM Tag (0x0028,0x0101)
pub const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
/// DICOM Tag (0x0028,0x0102)
pub const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
/// DICOM Tag (0x7FE0,0x0010)
pub const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
/// DICOM Tag (0x0008,0x1115)
pub const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
/// DICOM Tag (0x0008,0x114A)
pub const TAG_REFERENCED_INSTANCE_SEQUENCE: Tag = Tag(0x0008, 0x114A);
/// DICOM Tag (0x0008,0x1155)
pub const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

#[cfg(feature = "rendering")]
fn segment_color(segment_number: u16) -> [u8; 4] {
    let seed = segment_number as u32;
    let r = (seed.wrapping_mul(73) % 200 + 30) as u8;
    let g = (seed.wrapping_mul(151) % 200 + 30) as u8;
    let b = (seed.wrapping_mul(199) % 200 + 30) as u8;
    [r, g, b, 180]
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

/// Helper function for read_u16
pub fn read_u16(dataset: &Dataset, tag: Tag) -> Result<u16> {
    if let Some(value) = dataset.get_i32(tag) {
        if value >= 0 && value <= u16::MAX as i32 {
            return Ok(value as u16);
        }
    }
    if let Some(raw) = read_str(dataset, tag)? {
        let value = parse_i32_strict(tag, raw)?;
        if value >= 0 && value <= u16::MAX as i32 {
            return Ok(value as u16);
        }
    }
    Err(invalid_tag_value(tag, "expected u16"))
}

/// Helper function for read_u16_optional
pub fn read_u16_optional(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    if dataset.get(tag).is_none() {
        return Ok(None);
    }
    Ok(Some(read_u16(dataset, tag)?))
}

/// Helper function for read_bytes
pub fn read_bytes(dataset: &Dataset, tag: Tag) -> Result<&[u8]> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Bytes(bytes) => Ok(bytes.as_slice()),
            _ => Err(invalid_tag_value(tag, "expected bytes")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn invalid_geometry(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidGeometry {
            detail: detail.into(),
        },
        "invalid geometry",
    )
    .into()
}

/// Helper function for read_referenced_sop_instance_uid
pub fn read_referenced_sop_instance_uid(dataset: &Dataset) -> Result<Option<String>> {
    if let Ok(series_items) = sequence_items(dataset, TAG_REFERENCED_SERIES_SEQUENCE) {
        if let Some(series) = series_items.first() {
            if let Ok(instances) = sequence_items(series, TAG_REFERENCED_INSTANCE_SEQUENCE) {
                if let Some(instance) = instances.first() {
                    if let Some(uid) = read_str(instance, TAG_REFERENCED_SOP_INSTANCE_UID)? {
                        return Ok(Some(uid.to_string()));
                    }
                }
            }
        }
    }
    Ok(None)
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
