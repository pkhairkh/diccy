#![deny(missing_docs)]

//! RT Dose pack: dose grid parsing, scaling, and deterministic overlays.

use dicom_core::{
    parse_f64_strict, parse_i32_strict, Dataset, Error, ErrorKind, Result, Tag, Value,
};
#[cfg(feature = "rendering")]
use dicom_pixel::{DisplayFrame, PixelFormat};

/// RT Dose Storage SOP Class UID.
pub const SOP_CLASS_RT_DOSE: &str = "1.2.840.10008.5.1.4.1.1.481.2";
/// RT Structure Set Storage SOP Class UID.
pub const SOP_CLASS_RT_STRUCTURE: &str = "1.2.840.10008.5.1.4.1.1.481.3";
/// RT Plan Storage SOP Class UID.
pub const SOP_CLASS_RT_PLAN: &str = "1.2.840.10008.5.1.4.1.1.481.5";

/// RT SOP Class manifest list.
pub const RT_SOP_CLASS_UIDS: &[&str] =
    &[SOP_CLASS_RT_DOSE, SOP_CLASS_RT_STRUCTURE, SOP_CLASS_RT_PLAN];

/// Marker type for RT pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RtPack;

impl RtPack {
    /// Return true when RT pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-rt")
    }

    /// Require the RT pack to be enabled for RT SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "rt requires pack-rt feature",
            )));
        }
        if !RT_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported RT SOP class",
            )));
        }
        Ok(())
    }
}

/// Parsed RT Dose grid.
#[derive(Debug, Clone, PartialEq)]
pub struct RtDoseGrid {
    /// Rows.
    pub rows: u16,
    /// Columns.
    pub cols: u16,
    /// Number of frames.
    pub frames: u16,
    /// Pixel spacing (sx, sy).
    pub pixel_spacing: (f64, f64),
    /// Image Position (Patient).
    pub ipp: [f64; 3],
    /// Image Orientation (Patient).
    pub iop: [f64; 6],
    /// Frame of Reference UID.
    pub frame_of_reference_uid: String,
    /// Grid Frame Offset Vector (length == frames).
    pub grid_frame_offset_vector: Vec<f64>,
    /// Dose grid scaling factor.
    pub dose_grid_scaling: f64,
    /// Dose values (scaled) in row-major order.
    pub values: Vec<f64>,
}

/// Reference geometry for aligning RT Dose to an image/frame.
#[derive(Debug, Clone, PartialEq)]
pub struct RtReferenceGeometry {
    /// Rows of the referenced image.
    pub rows: u16,
    /// Columns of the referenced image.
    pub cols: u16,
    /// Pixel spacing (sx, sy).
    pub pixel_spacing: (f64, f64),
    /// Image Position (Patient).
    pub ipp: [f64; 3],
    /// Image Orientation (Patient).
    pub iop: [f64; 6],
    /// Frame of Reference UID.
    pub frame_of_reference_uid: String,
}

/// Parsed RT Structure Set with planar contours.
#[derive(Debug, Clone, PartialEq)]
pub struct RtStructureSet {
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Frame of Reference UID.
    pub frame_of_reference_uid: String,
    /// Contours grouped for overlay.
    pub contours: Vec<RtContour>,
}

/// Contour type classification.
///
/// Replaces the boolean trap of `closed: bool` in [`RtContour`] with a
/// semantically meaningful enum that mirrors DICOM Contour Geometric Type
/// values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContourType {
    /// Closed planar contour (CLOSED_PLANAR or CLOSEDPLANAR_XOR).
    ClosedPlanar,
    /// Open planar contour (OPEN_PLANAR).
    OpenPlanar,
}

impl ContourType {
    /// Return true when the contour is closed.
    pub fn is_closed(&self) -> bool {
        matches!(self, ContourType::ClosedPlanar)
    }
}

/// Parsed RT contour geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct RtContour {
    /// Contour points in patient coordinates.
    pub points: Vec<[f64; 3]>,
    /// RGB display color.
    pub color: [u8; 3],
    /// Contour type (closed or open planar).
    pub contour_type: ContourType,
}

/// Parsed RT Plan summary with structure references.
#[derive(Debug, Clone, PartialEq)]
pub struct RtPlanSummary {
    /// Frame of Reference UID.
    pub frame_of_reference_uid: String,
    /// Referenced Structure Set SOP Instance UID, if present.
    pub referenced_structure_set_uid: Option<String>,
    /// Optional RT Plan label.
    pub plan_label: Option<String>,
}

impl RtDoseGrid {
    /// Parse RT Dose grid from dataset (uncompressed, 16-bit).
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let rows = read_u16(dataset, TAG_ROWS)?;
        let cols = read_u16(dataset, TAG_COLUMNS)?;
        let frames = read_u16(dataset, TAG_NUMBER_OF_FRAMES)?;
        let pixel_spacing = read_f64_array::<2>(dataset, TAG_PIXEL_SPACING)?;
        let ipp = read_f64_array::<3>(dataset, TAG_IMAGE_POSITION)?;
        let iop = read_f64_array::<6>(dataset, TAG_IMAGE_ORIENTATION)?;
        let frame_of_reference_uid = read_str(dataset, TAG_FRAME_OF_REFERENCE_UID)?
            .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?
            .to_string();
        let grid_frame_offset_vector = read_f64_vec(dataset, TAG_GRID_FRAME_OFFSET_VECTOR)?;
        if grid_frame_offset_vector.len() != frames as usize {
            return Err(invalid_tag_value(
                TAG_GRID_FRAME_OFFSET_VECTOR,
                "grid frame offset vector length mismatch",
            ));
        }
        let scaling = read_f64(dataset, TAG_DOSE_GRID_SCALING)?;
        if !scaling.is_finite() || scaling <= 0.0 {
            return Err(invalid_tag_value(
                TAG_DOSE_GRID_SCALING,
                "dose grid scaling must be finite and > 0",
            ));
        }
        let bits_allocated = read_u16(dataset, TAG_BITS_ALLOCATED)?;
        if bits_allocated != 16 {
            return Err(invalid_tag_value(
                TAG_BITS_ALLOCATED,
                "RT Dose BitsAllocated must be 16",
            ));
        }
        let pixel_data = read_bytes(dataset, TAG_PIXEL_DATA)?;
        let expected = rows as usize * cols as usize * frames as usize;
        let expected_bytes = expected * 2;
        if pixel_data.len() < expected_bytes {
            return Err(invalid_tag_value(
                TAG_PIXEL_DATA,
                "RT Dose pixel data too short",
            ));
        }
        let mut values = Vec::with_capacity(expected);
        for idx in 0..expected {
            let start = idx * 2;
            let raw = u16::from_le_bytes([pixel_data[start], pixel_data[start + 1]]);
            values.push((raw as f64) * scaling);
        }
        Ok(Self {
            rows,
            cols,
            frames,
            pixel_spacing: (pixel_spacing[0], pixel_spacing[1]),
            ipp,
            iop,
            frame_of_reference_uid,
            grid_frame_offset_vector,
            dose_grid_scaling: scaling,
            values,
        })
    }

    /// Create a deterministic overlay for a single frame index.
    #[cfg(feature = "rendering")]
    pub fn overlay_frame(&self, frame_index: u16) -> Result<DisplayFrame> {
        if frame_index >= self.frames {
            return Err(invalid_tag_value(
                TAG_NUMBER_OF_FRAMES,
                "frame index out of range",
            ));
        }
        let offset = frame_index as usize * self.rows as usize * self.cols as usize;
        let slice = &self.values[offset..offset + (self.rows as usize * self.cols as usize)];
        let max = slice
            .iter()
            .cloned()
            .filter(|v| v.is_finite())
            .fold(0.0, f64::max);
        let mut bytes = vec![0u8; self.rows as usize * self.cols as usize * 4];
        for (idx, &dose) in slice.iter().enumerate() {
            let normalized = if max > 0.0 {
                (dose / max).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (r, g, b) = dose_colormap(normalized);
            let base = idx * 4;
            bytes[base] = r;
            bytes[base + 1] = g;
            bytes[base + 2] = b;
            bytes[base + 3] = (normalized * 200.0) as u8;
        }
        Ok(DisplayFrame {
            width: self.cols as u32,
            height: self.rows as u32,
            format: PixelFormat::Rgba8,
            bytes,
        })
    }

    /// Validate alignment against reference geometry before overlay.
    pub fn validate_alignment(
        &self,
        reference: &RtReferenceGeometry,
        frame_index: u16,
    ) -> Result<()> {
        if frame_index >= self.frames {
            return Err(invalid_tag_value(
                TAG_NUMBER_OF_FRAMES,
                "frame index out of range",
            ));
        }
        if self.frame_of_reference_uid != reference.frame_of_reference_uid {
            return Err(invalid_geometry("frame of reference UID mismatch"));
        }
        if self.rows != reference.rows || self.cols != reference.cols {
            return Err(invalid_geometry(
                "dose grid dimensions do not match reference grid",
            ));
        }
        if !approx_eq(self.pixel_spacing.0, reference.pixel_spacing.0)
            || !approx_eq(self.pixel_spacing.1, reference.pixel_spacing.1)
        {
            return Err(invalid_geometry("pixel spacing mismatch"));
        }
        if !iop_matches(&self.iop, &reference.iop) {
            return Err(invalid_geometry("image orientation mismatch"));
        }
        let normal = normal_from_iop(&self.iop)?;
        let offset = self.grid_frame_offset_vector[frame_index as usize];
        let expected = [
            self.ipp[0] + normal[0] * offset,
            self.ipp[1] + normal[1] * offset,
            self.ipp[2] + normal[2] * offset,
        ];
        if !approx_eq(expected[0], reference.ipp[0])
            || !approx_eq(expected[1], reference.ipp[1])
            || !approx_eq(expected[2], reference.ipp[2])
        {
            return Err(invalid_geometry("image position mismatch"));
        }
        Ok(())
    }

    /// Create a deterministic overlay for a single frame index after alignment checks.
    #[cfg(feature = "rendering")]
    pub fn overlay_on(
        &self,
        reference: &RtReferenceGeometry,
        frame_index: u16,
    ) -> Result<DisplayFrame> {
        self.validate_alignment(reference, frame_index)?;
        self.overlay_frame(frame_index)
    }
}

/// Domain-level descriptor for an RT dose overlay, containing grid geometry,
/// dose values, and spatial metadata without any rendering dependency.
///
/// This is a type alias for [`RtDoseGrid`] since the dose grid fields are
/// all pure domain data. Use this type when you need a rendering-independent
/// reference to the parsed RT dose data.
pub type RtOverlayDescriptor = RtDoseGrid;

impl RtStructureSet {
    /// Parse an RT Structure Set with planar contours.
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let sop_instance_uid = read_str(dataset, TAG_SOP_INSTANCE_UID)?
            .ok_or_else(|| missing_required_tag(TAG_SOP_INSTANCE_UID))?
            .to_string();
        let frame_of_reference_uid = read_str(dataset, TAG_FRAME_OF_REFERENCE_UID)?
            .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?
            .to_string();
        let roi_contours = read_sequence(dataset, TAG_ROI_CONTOUR_SEQUENCE)?
            .ok_or_else(|| missing_required_tag(TAG_ROI_CONTOUR_SEQUENCE))?;
        let mut contours = Vec::new();
        for roi in roi_contours {
            let color = read_rgb(roi, TAG_ROI_DISPLAY_COLOR)?.unwrap_or(DEFAULT_CONTOUR_COLOR);
            let contour_sequence = read_sequence(roi, TAG_CONTOUR_SEQUENCE)?
                .ok_or_else(|| missing_required_tag(TAG_CONTOUR_SEQUENCE))?;
            for contour in contour_sequence {
                let geom_type = read_str(contour, TAG_CONTOUR_GEOMETRIC_TYPE)?
                    .ok_or_else(|| missing_required_tag(TAG_CONTOUR_GEOMETRIC_TYPE))?;
                let contour_type = match geom_type {
                    "CLOSED_PLANAR" => ContourType::ClosedPlanar,
                    "CLOSEDPLANAR_XOR" => ContourType::ClosedPlanar,
                    "OPEN_PLANAR" => ContourType::OpenPlanar,
                    _ => {
                        return Err(invalid_tag_value(
                            TAG_CONTOUR_GEOMETRIC_TYPE,
                            "unsupported contour geometric type",
                        ))
                    }
                };
                let values = read_f64_vec(contour, TAG_CONTOUR_DATA)?;
                if values.len() < 6 || values.len() % 3 != 0 {
                    return Err(invalid_tag_value(
                        TAG_CONTOUR_DATA,
                        "contour data must be multiple of 3 with at least 2 points",
                    ));
                }
                if let Some(expected) = read_u16_optional(contour, TAG_NUMBER_OF_CONTOUR_POINTS)? {
                    if expected as usize != values.len() / 3 {
                        return Err(invalid_tag_value(
                            TAG_NUMBER_OF_CONTOUR_POINTS,
                            "contour point count mismatch",
                        ));
                    }
                }
                let points = values
                    .chunks_exact(3)
                    .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                    .collect::<Vec<_>>();
                contours.push(RtContour {
                    points,
                    color,
                    contour_type,
                });
            }
        }
        Ok(Self {
            sop_instance_uid,
            frame_of_reference_uid,
            contours,
        })
    }

    /// Create a deterministic overlay for contours aligned to a reference geometry.
    #[cfg(feature = "rendering")]
    pub fn overlay_on(&self, reference: &RtReferenceGeometry) -> Result<DisplayFrame> {
        if self.frame_of_reference_uid != reference.frame_of_reference_uid {
            return Err(invalid_geometry("frame of reference UID mismatch"));
        }
        if reference.rows == 0 || reference.cols == 0 {
            return Err(invalid_geometry("reference dimensions must be non-zero"));
        }
        if reference.pixel_spacing.0 <= 0.0 || reference.pixel_spacing.1 <= 0.0 {
            return Err(invalid_geometry("pixel spacing must be > 0"));
        }
        let normal = normal_from_iop(&reference.iop)?;
        let row_dir = [reference.iop[0], reference.iop[1], reference.iop[2]];
        let col_dir = [reference.iop[3], reference.iop[4], reference.iop[5]];
        let mut out = vec![0u8; reference.rows as usize * reference.cols as usize * 4];
        for contour in &self.contours {
            let projected = project_contour(contour, reference, &row_dir, &col_dir, &normal)?;
            draw_polyline(
                &mut out,
                reference.cols as i32,
                reference.rows as i32,
                &projected,
                contour.color,
                contour.contour_type.is_closed(),
            );
        }
        Ok(DisplayFrame {
            width: reference.cols as u32,
            height: reference.rows as u32,
            format: PixelFormat::Rgba8,
            bytes: out,
        })
    }
}

impl RtPlanSummary {
    /// Parse an RT Plan summary with structure references.
    pub fn from_dataset(dataset: &Dataset) -> Result<Self> {
        let frame_of_reference_uid = read_str(dataset, TAG_FRAME_OF_REFERENCE_UID)?
            .ok_or_else(|| missing_required_tag(TAG_FRAME_OF_REFERENCE_UID))?
            .to_string();
        let plan_label = read_str(dataset, TAG_RT_PLAN_LABEL)?.map(|label| label.to_string());
        let referenced_structure_set_uid = read_referenced_structure_set_uid(dataset)?;
        Ok(Self {
            frame_of_reference_uid,
            referenced_structure_set_uid,
            plan_label,
        })
    }

    /// Validate that this plan references the provided structure set.
    pub fn validate_structure_set(&self, structure: &RtStructureSet) -> Result<()> {
        if self.frame_of_reference_uid != structure.frame_of_reference_uid {
            return Err(invalid_geometry("frame of reference UID mismatch"));
        }
        if let Some(uid) = &self.referenced_structure_set_uid {
            if uid != &structure.sop_instance_uid {
                return Err(invalid_tag_value(
                    TAG_REFERENCED_SOP_INSTANCE_UID,
                    "referenced structure set UID mismatch",
                ));
            }
        }
        Ok(())
    }
}

const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);
const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_FRAME_OF_REFERENCE_UID: Tag = Tag(0x0020, 0x0052);
const TAG_DOSE_GRID_SCALING: Tag = Tag(0x3004, 0x000E);
const TAG_GRID_FRAME_OFFSET_VECTOR: Tag = Tag(0x3004, 0x000C);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_ROI_CONTOUR_SEQUENCE: Tag = Tag(0x3006, 0x0039);
const TAG_CONTOUR_SEQUENCE: Tag = Tag(0x3006, 0x0040);
const TAG_CONTOUR_DATA: Tag = Tag(0x3006, 0x0050);
const TAG_CONTOUR_GEOMETRIC_TYPE: Tag = Tag(0x3006, 0x0042);
const TAG_NUMBER_OF_CONTOUR_POINTS: Tag = Tag(0x3006, 0x0046);
const TAG_ROI_DISPLAY_COLOR: Tag = Tag(0x3006, 0x002A);
const TAG_RT_PLAN_LABEL: Tag = Tag(0x300A, 0x0002);
const TAG_REFERENCED_STRUCTURE_SET_SEQUENCE: Tag = Tag(0x300C, 0x0060);
const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

const DEFAULT_CONTOUR_COLOR: [u8; 3] = [0xFF, 0x00, 0x00];
#[cfg(feature = "rendering")]
const STRUCTURE_ALPHA: u8 = 200;

#[cfg(feature = "rendering")]
fn dose_colormap(value: f64) -> (u8, u8, u8) {
    let v = value.clamp(0.0, 1.0);
    let r = (v * 255.0) as u8;
    let g = (v * 128.0) as u8;
    let b = (255.0 - v * 255.0) as u8;
    (r, g, b)
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

fn read_f64(dataset: &Dataset, tag: Tag) -> Result<f64> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    parse_f64_strict(tag, raw)
}

fn read_f64_array<const N: usize>(dataset: &Dataset, tag: Tag) -> Result<[f64; N]> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.len() != N {
        return Err(invalid_tag_value(tag, "unexpected number of values"));
    }
    let mut values = [0.0f64; N];
    for (idx, part) in parts.iter().enumerate() {
        let value = parse_f64_strict(tag, part)?;
        if !value.is_finite() {
            return Err(invalid_tag_value(tag, "non-finite value"));
        }
        values[idx] = value;
    }
    Ok(values)
}

fn read_f64_vec(dataset: &Dataset, tag: Tag) -> Result<Vec<f64>> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Err(missing_required_tag(tag));
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    let mut values = Vec::with_capacity(parts.len());
    for part in parts {
        let value = parse_f64_strict(tag, part)?;
        if !value.is_finite() {
            return Err(invalid_tag_value(tag, "non-finite value"));
        }
        values.push(value);
    }
    Ok(values)
}

fn read_bytes(dataset: &Dataset, tag: Tag) -> Result<&[u8]> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Bytes(bytes) => Ok(bytes.as_slice()),
            _ => Err(invalid_tag_value(tag, "expected bytes")),
        },
        None => Err(missing_required_tag(tag)),
    }
}

fn read_sequence(dataset: &Dataset, tag: Tag) -> Result<Option<&[Dataset]>> {
    match dataset.get(tag) {
        Some(element) => match element.value() {
            Value::Sequence(items) => Ok(Some(items.as_slice())),
            _ => Err(invalid_tag_value(tag, "expected sequence")),
        },
        None => Ok(None),
    }
}

fn read_rgb(dataset: &Dataset, tag: Tag) -> Result<Option<[u8; 3]>> {
    let Some(raw) = read_str(dataset, tag)? else {
        return Ok(None);
    };
    let parts: Vec<&str> = raw.split('\\').collect();
    if parts.len() != 3 {
        return Err(invalid_tag_value(tag, "expected RGB triple"));
    }
    let mut color = [0u8; 3];
    for (idx, part) in parts.iter().enumerate() {
        let value = parse_i32_strict(tag, part)?;
        if !(0..=255).contains(&value) {
            return Err(invalid_tag_value(tag, "RGB values must be 0..255"));
        }
        color[idx] = value as u8;
    }
    Ok(Some(color))
}

fn read_u16_optional(dataset: &Dataset, tag: Tag) -> Result<Option<u16>> {
    match dataset.get(tag) {
        None => Ok(None),
        Some(element) => match element.value() {
            Value::I32(value) if *value >= 0 && *value <= u16::MAX as i32 => {
                Ok(Some(*value as u16))
            }
            Value::Str(value) => {
                let parsed = parse_i32_strict(tag, value)?;
                if parsed >= 0 && parsed <= u16::MAX as i32 {
                    Ok(Some(parsed as u16))
                } else {
                    Err(invalid_tag_value(tag, "expected u16"))
                }
            }
            _ => Err(invalid_tag_value(tag, "expected u16")),
        },
    }
}

fn read_referenced_structure_set_uid(dataset: &Dataset) -> Result<Option<String>> {
    let Some(seq) = read_sequence(dataset, TAG_REFERENCED_STRUCTURE_SET_SEQUENCE)? else {
        return Ok(None);
    };
    let first = seq.first().ok_or_else(|| {
        invalid_tag_value(TAG_REFERENCED_STRUCTURE_SET_SEQUENCE, "empty sequence")
    })?;
    let uid = read_str(first, TAG_REFERENCED_SOP_INSTANCE_UID)?
        .ok_or_else(|| missing_required_tag(TAG_REFERENCED_SOP_INSTANCE_UID))?;
    Ok(Some(uid.to_string()))
}

fn project_contour(
    contour: &RtContour,
    reference: &RtReferenceGeometry,
    row_dir: &[f64; 3],
    col_dir: &[f64; 3],
    normal: &[f64; 3],
) -> Result<Vec<(i32, i32)>> {
    let row_spacing = reference.pixel_spacing.0;
    let col_spacing = reference.pixel_spacing.1;
    let cols = reference.cols as i32;
    let rows = reference.rows as i32;
    let mut projected = Vec::with_capacity(contour.points.len());
    for point in &contour.points {
        let d = [
            point[0] - reference.ipp[0],
            point[1] - reference.ipp[1],
            point[2] - reference.ipp[2],
        ];
        let distance = dot(d, *normal);
        if distance.abs() > GEOM_EPS {
            return Err(invalid_geometry("contour plane mismatch"));
        }
        let column = dot(d, *row_dir) / col_spacing;
        let row = dot(d, *col_dir) / row_spacing;
        let x = clamp_index(column, cols);
        let y = clamp_index(row, rows);
        projected.push((x, y));
    }
    Ok(projected)
}

fn clamp_index(value: f64, max: i32) -> i32 {
    let mut idx = value.round() as i32;
    if idx < 0 {
        idx = 0;
    }
    if idx >= max {
        idx = max - 1;
    }
    idx
}

#[cfg(feature = "rendering")]
fn draw_polyline(
    out: &mut [u8],
    width: i32,
    height: i32,
    points: &[(i32, i32)],
    color: [u8; 3],
    closed: bool,
) {
    if points.len() < 2 {
        return;
    }
    for pair in points.windows(2) {
        draw_line(out, width, height, pair[0], pair[1], color);
    }
    if closed {
        let first = points[0];
        let last = points[points.len() - 1];
        draw_line(out, width, height, last, first, color);
    }
}

#[cfg(feature = "rendering")]
fn draw_line(
    out: &mut [u8],
    width: i32,
    height: i32,
    start: (i32, i32),
    end: (i32, i32),
    color: [u8; 3],
) {
    let (mut x0, mut y0) = start;
    let (x1, y1) = end;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        plot_rgba(out, width, height, x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

#[cfg(feature = "rendering")]
fn plot_rgba(out: &mut [u8], width: i32, height: i32, x: i32, y: i32, color: [u8; 3]) {
    if x < 0 || y < 0 || x >= width || y >= height {
        return;
    }
    let idx = (y as usize * width as usize + x as usize) * 4;
    if idx + 3 >= out.len() {
        return;
    }
    out[idx] = color[0];
    out[idx + 1] = color[1];
    out[idx + 2] = color[2];
    out[idx + 3] = STRUCTURE_ALPHA;
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= GEOM_EPS
}

fn iop_matches(a: &[f64; 6], b: &[f64; 6]) -> bool {
    (0..6).all(|i| approx_eq(a[i], b[i]))
}

fn normal_from_iop(iop: &[f64; 6]) -> Result<[f64; 3]> {
    let row = [iop[0], iop[1], iop[2]];
    let col = [iop[3], iop[4], iop[5]];
    let normal = cross(row, col);
    let norm = norm(normal);
    if !norm.is_finite() || norm <= GEOM_EPS {
        return Err(invalid_geometry("invalid image orientation"));
    }
    Ok([normal[0] / norm, normal[1] / norm, normal[2] / norm])
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
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

fn invalid_geometry(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidGeometry {
            detail: detail.into(),
        },
        "invalid geometry",
    )
    .into()
}

const GEOM_EPS: f64 = 1e-6;

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Dataset, Element, Vr};

    const RT_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        RT_MANIFEST
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
    #[cfg(not(feature = "pack-rt"))]
    fn rt_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!RtPack::enabled());
        let err = RtPack::ensure_supported(SOP_CLASS_RT_DOSE).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-rt")]
    fn rt_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(RtPack::enabled());
        RtPack::ensure_supported(SOP_CLASS_RT_DOSE).expect("rt pack enabled");
        RtPack::ensure_supported(SOP_CLASS_RT_STRUCTURE).expect("rt pack enabled");
        RtPack::ensure_supported(SOP_CLASS_RT_PLAN).expect("rt pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in RT_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn dose_grid_scaling_applied() {
        // REQ-VOL-925, REQ-RT-350, REQ-RT-352
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("0.5".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![2, 0])).unwrap());
        let dose = RtDoseGrid::from_dataset(&dataset).expect("dose parse");
        assert_eq!(dose.values, vec![1.0]);
        let reference = RtReferenceGeometry {
            rows: 1,
            cols: 1,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "1.2.3".to_string(),
        };
        let overlay = dose.overlay_on(&reference, 0).expect("overlay");
        let overlay_repeat = dose.overlay_on(&reference, 0).expect("overlay repeat");
        assert_eq!(overlay.bytes, overlay_repeat.bytes);
        assert_eq!(overlay.width, 1);
    }

    #[test]
    fn grid_frame_offset_vector_mismatch_fails() {
        // REQ-VOL-926, REQ-RT-351
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("2".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 0, 0, 0])).unwrap());
        let err = RtDoseGrid::from_dataset(&dataset).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn rt_alignment_mismatch_fails() {
        // REQ-VOL-926, REQ-RT-351
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 0])).unwrap());
        let dose = RtDoseGrid::from_dataset(&dataset).expect("dose parse");
        let reference = RtReferenceGeometry {
            rows: 1,
            cols: 1,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 1.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "1.2.3".to_string(),
        };
        let err = dose.overlay_on(&reference, 0).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    fn build_structure_dataset_with_type(points: &str, contour_type: &str) -> Dataset {
        let mut contour = Dataset::new();
        contour.insert(
            Element::new(
                TAG_CONTOUR_GEOMETRIC_TYPE,
                Vr::Cs,
                Value::Str(contour_type.to_string()),
            )
            .unwrap(),
        );
        contour.insert(
            Element::new(
                TAG_NUMBER_OF_CONTOUR_POINTS,
                Vr::Is,
                Value::Str("4".to_string()),
            )
            .unwrap(),
        );
        contour.insert(
            Element::new(TAG_CONTOUR_DATA, Vr::Ds, Value::Str(points.to_string())).unwrap(),
        );

        let mut roi = Dataset::new();
        roi.insert(
            Element::new(
                TAG_ROI_DISPLAY_COLOR,
                Vr::Is,
                Value::Str("255\\0\\0".to_string()),
            )
            .unwrap(),
        );
        roi.insert(
            Element::new(TAG_CONTOUR_SEQUENCE, Vr::Sq, Value::Sequence(vec![contour])).unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("9.8.7".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_ROI_CONTOUR_SEQUENCE, Vr::Sq, Value::Sequence(vec![roi])).unwrap(),
        );
        dataset
    }

    fn build_structure_dataset(points: &str) -> Dataset {
        build_structure_dataset_with_type(points, "CLOSED_PLANAR")
    }

    #[test]
    fn structure_set_overlay_applies() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset("0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0");
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let overlay = structure.overlay_on(&reference).expect("overlay");
        assert_eq!(overlay.format, PixelFormat::Rgba8);
        assert!(overlay.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn structure_set_closedplanar_xor_overlay_applies() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset_with_type(
            "0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0",
            "CLOSEDPLANAR_XOR",
        );
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let overlay = structure.overlay_on(&reference).expect("overlay");
        assert_eq!(overlay.format, PixelFormat::Rgba8);
        assert!(overlay.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn structure_set_plane_mismatch_fails() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset("0\\0\\1\\1\\0\\1\\1\\1\\1\\0\\1\\1");
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let err = structure.overlay_on(&reference).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    #[test]
    fn plan_summary_references_structure() {
        // REQ-CONF-090, REQ-RT-354
        let structure = RtStructureSet::from_dataset(&build_structure_dataset(
            "0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0",
        ))
        .expect("structure parse");
        let mut plan = Dataset::new();
        plan.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("9.8.7".to_string()),
            )
            .unwrap(),
        );
        plan.insert(
            Element::new(TAG_RT_PLAN_LABEL, Vr::Sh, Value::Str("PLAN".to_string())).unwrap(),
        );
        let mut ref_item = Dataset::new();
        ref_item.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        plan.insert(
            Element::new(
                TAG_REFERENCED_STRUCTURE_SET_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_item]),
            )
            .unwrap(),
        );
        let summary = RtPlanSummary::from_dataset(&plan).expect("plan parse");
        summary
            .validate_structure_set(&structure)
            .expect("reference valid");
    }
}
