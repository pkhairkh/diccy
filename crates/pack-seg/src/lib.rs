#![deny(missing_docs)]

//! Segmentation pack: binary segmentation parsing and deterministic overlays.

use dicom_core::{parse_i32_strict, Dataset, Error, ErrorKind, Result, Tag, Value};
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
}

/// Parsed segmentation data (binary, single-frame or multi-frame).
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
    pub mask: Vec<u8>,
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
    /// Parse a binary segmentation from a dataset.
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
        let seg_type = read_str(dataset, TAG_SEGMENTATION_TYPE)?
            .ok_or_else(|| missing_required_tag(TAG_SEGMENTATION_TYPE))?;
        if seg_type != "BINARY" {
            return Err(invalid_tag_value(
                TAG_SEGMENTATION_TYPE,
                "only BINARY segmentation supported",
            ));
        }
        let segment_number = read_u16(dataset, TAG_SEGMENT_NUMBER)?;
        let referenced_sop_instance_uid = read_referenced_sop_instance_uid(dataset)?;
        let bits_allocated = read_u16(dataset, TAG_BITS_ALLOCATED)?;
        if bits_allocated != 1 {
            return Err(invalid_tag_value(
                TAG_BITS_ALLOCATED,
                "segmentation BitsAllocated must be 1",
            ));
        }
        let bits_stored = read_u16(dataset, TAG_BITS_STORED)?;
        if bits_stored != 1 {
            return Err(invalid_tag_value(
                TAG_BITS_STORED,
                "segmentation BitsStored must be 1",
            ));
        }
        let high_bit = read_u16(dataset, TAG_HIGH_BIT)?;
        if high_bit != 0 {
            return Err(invalid_tag_value(
                TAG_HIGH_BIT,
                "segmentation HighBit must be 0",
            ));
        }
        let pixel_data = read_bytes(dataset, TAG_PIXEL_DATA)?;
        let expected_pixels = rows as usize * cols as usize * frames as usize;
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
            seg_type: SegmentationType::Binary,
        })
    }

    /// Apply this segmentation frame 0 as a deterministic RGBA overlay onto a frame.
    pub fn overlay_on(
        &self,
        frame: &DisplayFrame,
        reference: SegReference<'_>,
    ) -> Result<DisplayFrame> {
        self.overlay_on_frame(frame, reference, 0)
    }

    /// Apply a selected segmentation frame as a deterministic RGBA overlay onto a frame.
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
        let frame_mask = &self.mask[frame_start..frame_end];
        let mut out = vec![0u8; (frame.width * frame.height * 4) as usize];
        let color = segment_color(self.segment_number);
        for (idx, &mask) in frame_mask.iter().enumerate() {
            if mask == 0 {
                continue;
            }
            let base = idx * 4;
            out[base] = color[0];
            out[base + 1] = color[1];
            out[base + 2] = color[2];
            out[base + 3] = color[3];
        }
        Ok(DisplayFrame {
            width: frame.width,
            height: frame.height,
            format: PixelFormat::Rgba8,
            bytes: out,
        })
    }
}

const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
const TAG_SEGMENTATION_TYPE: Tag = Tag(0x0062, 0x0001);
const TAG_SEGMENT_NUMBER: Tag = Tag(0x0062, 0x0004);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_REFERENCED_SERIES_SEQUENCE: Tag = Tag(0x0008, 0x1115);
const TAG_REFERENCED_INSTANCE_SEQUENCE: Tag = Tag(0x0008, 0x114A);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

fn segment_color(segment_number: u16) -> [u8; 4] {
    let seed = segment_number as u32;
    let r = (seed.wrapping_mul(73) % 200 + 30) as u8;
    let g = (seed.wrapping_mul(151) % 200 + 30) as u8;
    let b = (seed.wrapping_mul(199) % 200 + 30) as u8;
    [r, g, b, 180]
}

fn read_str(dataset: &Dataset, tag: Tag) -> Result<Option<&str>> {
    match dataset.get(tag) {
        Some(element) => match &element.value {
            Value::Str(value) => Ok(Some(value.as_str())),
            Value::Uid(value) => Ok(Some(value.as_str())),
            _ => Err(invalid_tag_value(tag, "expected string")),
        },
        None => Ok(None),
    }
}

fn read_u16(dataset: &Dataset, tag: Tag) -> Result<u16> {
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

fn read_u16_optional(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    if dataset.get(tag).is_none() {
        return Ok(None);
    }
    Ok(Some(read_u16(dataset, tag)?))
}

fn read_bytes(dataset: &Dataset, tag: Tag) -> Result<&[u8]> {
    match dataset.get(tag) {
        Some(element) => match &element.value {
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

fn read_referenced_sop_instance_uid(dataset: &Dataset) -> Result<Option<String>> {
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
        Some(element) => match &element.value {
            Value::Sequence(items) => Ok(items.as_slice()),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Err(missing_required_tag(tag)),
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
    use dicom_core::{Element, Vr};

    const SEG_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        SEG_MANIFEST
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

    #[test]
    #[cfg(not(feature = "pack-seg"))]
    fn seg_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!SegPack::enabled());
        let err = SegPack::ensure_supported(SOP_CLASS_SEG).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-seg")]
    fn seg_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(SegPack::enabled());
        SegPack::ensure_supported(SOP_CLASS_SEG).expect("seg pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in SEG_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn binary_seg_overlay_is_deterministic() {
        // REQ-UI-063, REQ-VOL-923, REQ-VOL-927, REQ-SEG-300, REQ-SEG-303
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("2".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("2".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0011]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0, 0, 0, 0],
        };
        let reference = SegReference {
            rows: 2,
            cols: 2,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let overlay = seg.overlay_on(&base, reference).expect("overlay");
        let overlay_repeat = seg.overlay_on(&base, reference).expect("overlay repeat");
        assert_eq!(overlay.bytes, overlay_repeat.bytes);
        assert_eq!(overlay.bytes.len(), 16);
        assert!(overlay.bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn seg_reference_uid_mismatch_fails() {
        // REQ-CONF-086, REQ-SEG-300
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0001]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "9.9.9",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn seg_frame_of_reference_mismatch_fails() {
        // REQ-CONF-086, REQ-SEG-300
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0001]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "9.9.9",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code, "DVF.GEOM.INVALID");
    }

    #[test]
    fn seg_multiframe_overlay_is_deterministic() {
        // REQ-CONF-086, REQ-SEG-301, REQ-SEG-303
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_NUMBER_OF_FRAMES,
            vr: Vr::Is,
            value: Value::Str("2".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0001]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        assert_eq!(seg.frames, 2);
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let frame0 = seg
            .overlay_on_frame(&base, reference, 0)
            .expect("frame0 overlay");
        let frame1 = seg
            .overlay_on_frame(&base, reference, 1)
            .expect("frame1 overlay");
        let frame0_repeat = seg
            .overlay_on_frame(&base, reference, 0)
            .expect("frame0 repeat");
        assert_eq!(frame0.bytes, frame0_repeat.bytes);
        assert!(frame0.bytes.iter().any(|&value| value != 0));
        assert!(frame1.bytes.iter().all(|&value| value == 0));

        // Backward-compatible entrypoint uses frame 0.
        let default_overlay = seg.overlay_on(&base, reference).expect("default overlay");
        assert_eq!(default_overlay.bytes, frame0.bytes);
    }

    #[test]
    fn seg_multiframe_frame_index_out_of_range_fails_closed() {
        // REQ-CONF-086, REQ-SEG-301
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_NUMBER_OF_FRAMES,
            vr: Vr::Is,
            value: Value::Str("2".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0001]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on_frame(&base, reference, 2).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn seg_grid_mismatch_fails_closed_without_resampling() {
        // REQ-VOL-924, REQ-SEG-302
        let mut ref_instance = Dataset::new();
        ref_instance.insert(Element {
            tag: TAG_REFERENCED_SOP_INSTANCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let mut ref_series = Dataset::new();
        ref_series.insert(Element {
            tag: TAG_REFERENCED_INSTANCE_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_instance]),
        });

        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ROWS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_COLUMNS,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_FRAME_OF_REFERENCE_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENTATION_TYPE,
            vr: Vr::Cs,
            value: Value::Str("BINARY".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SEGMENT_NUMBER,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_ALLOCATED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_BITS_STORED,
            vr: Vr::Us,
            value: Value::Str("1".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_HIGH_BIT,
            vr: Vr::Us,
            value: Value::Str("0".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_PIXEL_DATA,
            vr: Vr::Ob,
            value: Value::Bytes(vec![0b0000_0001]),
        });
        dataset.insert(Element {
            tag: TAG_REFERENCED_SERIES_SEQUENCE,
            vr: Vr::Sq,
            value: Value::Sequence(vec![ref_series]),
        });

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0; 4],
        };
        let reference = SegReference {
            rows: 2,
            cols: 2,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code, "DVF.GEOM.INVALID");
    }
}
