#![deny(missing_docs)]

//! CT modality pack: geometry validation, volume constraints, and measurement helpers.

use dicom_core::{Error, ErrorKind, Result, Tag};

/// Enhanced CT Image Storage SOP Class UID (Tier 1, requires CT pack).
pub const SOP_CLASS_ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";

/// Marker type for CT modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CtPack;

impl CtPack {
    /// Return true when CT pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-ct")
    }

    /// Require the CT pack to be enabled for enhanced CT support.
    pub fn ensure_enhanced_ct_supported() -> Result<()> {
        if dicom_core::capabilities().pack_enhanced {
            return Ok(());
        }
        Err(Box::new(Error::from_kind(
            ErrorKind::UnsupportedSopClass {
                sop_class_uid: SOP_CLASS_ENHANCED_CT.to_string(),
            },
            "enhanced CT requires pack-enhanced feature",
        )))
    }

    /// Require the CT pack to be enabled for physical-unit measurements.
    pub fn ensure_physical_measurements_enabled() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "measurement_mode".to_string(),
                    detail: "physical-unit measurements require modality-ct feature".to_string(),
                },
                "physical-unit measurements require modality-ct feature",
            )))
        }
    }
}

/// DICOM tag for Image Position (Patient).
pub const TAG_IMAGE_POSITION: Tag = Tag(0x0020, 0x0032);
/// DICOM tag for Image Orientation (Patient).
pub const TAG_IMAGE_ORIENTATION: Tag = Tag(0x0020, 0x0037);
/// DICOM tag for Pixel Spacing.
pub const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
/// DICOM tag for Rows.
pub const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
/// DICOM tag for Columns.
pub const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);

/// Tolerances used for CT geometry validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeometryTolerance {
    /// Epsilon used for IOP orthonormal checks.
    pub iop_epsilon: f64,
    /// Epsilon used for pixel spacing equality checks.
    pub spacing_epsilon: f64,
    /// Epsilon used for slice spacing consistency checks.
    pub slice_spacing_epsilon: f64,
}

impl GeometryTolerance {
    /// Validate that tolerances are finite and non-negative.
    pub fn validate(&self) -> Result<()> {
        let values = [
            self.iop_epsilon,
            self.spacing_epsilon,
            self.slice_spacing_epsilon,
        ];
        if values.iter().any(|v| !v.is_finite() || *v < 0.0) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "geometry tolerances must be finite and non-negative".to_string(),
                },
                "geometry tolerances must be finite and non-negative",
            )));
        }
        Ok(())
    }
}

/// CT geometry inputs with required tags represented explicitly.
#[derive(Debug, Clone, PartialEq)]
pub struct CtGeometryInput {
    /// Image rows.
    pub rows: Option<u16>,
    /// Image columns.
    pub cols: Option<u16>,
    /// Pixel spacing (sx, sy) in mm.
    pub pixel_spacing: Option<(f64, f64)>,
    /// Image orientation (patient) direction cosines (row, col).
    pub iop: Option<[f64; 6]>,
    /// Image position (patient) coordinates.
    pub ipp: Option<[f64; 3]>,
}

/// Validated CT geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct CtGeometry {
    /// Image rows.
    pub rows: u16,
    /// Image columns.
    pub cols: u16,
    /// Pixel spacing (sx, sy) in mm.
    pub pixel_spacing: (f64, f64),
    /// Image orientation (patient) direction cosines (row, col).
    pub iop: [f64; 6],
    /// Image position (patient) coordinates.
    pub ipp: [f64; 3],
}

/// Validates CT geometry required for volume assembly.
///
/// Enforces REQ-CONF-030/031/032 and prepares validated geometry for volume checks.
pub fn validate_ct_geometry(input: CtGeometryInput, iop_epsilon: f64) -> Result<CtGeometry> {
    if !iop_epsilon.is_finite() || iop_epsilon < 0.0 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidGeometry {
                detail: "iop_epsilon must be finite and non-negative".to_string(),
            },
            "iop_epsilon must be finite and non-negative",
        )));
    }

    let rows = input.rows.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag { tag: TAG_ROWS },
            "rows missing",
        ))
    })?;
    let cols = input.cols.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag { tag: TAG_COLUMNS },
            "columns missing",
        ))
    })?;
    if rows == 0 || cols == 0 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_ROWS,
                detail: "rows/columns must be non-zero".to_string(),
            },
            "rows/columns must be non-zero",
        )));
    }

    let pixel_spacing = input.pixel_spacing.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag {
                tag: TAG_PIXEL_SPACING,
            },
            "pixel spacing missing",
        ))
    })?;
    if !pixel_spacing.0.is_finite()
        || !pixel_spacing.1.is_finite()
        || pixel_spacing.0 <= 0.0
        || pixel_spacing.1 <= 0.0
    {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_PIXEL_SPACING,
                detail: "pixel spacing must be finite and > 0".to_string(),
            },
            "pixel spacing must be finite and > 0",
        )));
    }

    let iop = input.iop.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag {
                tag: TAG_IMAGE_ORIENTATION,
            },
            "image orientation missing",
        ))
    })?;
    validate_iop(iop, iop_epsilon)?;

    let ipp = input.ipp.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag {
                tag: TAG_IMAGE_POSITION,
            },
            "image position missing",
        ))
    })?;
    if ipp.iter().any(|v| !v.is_finite()) {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_IMAGE_POSITION,
                detail: "image position must be finite".to_string(),
            },
            "image position must be finite",
        )));
    }

    Ok(CtGeometry {
        rows,
        cols,
        pixel_spacing,
        iop,
        ipp,
    })
}

/// Validate IOP orthonormality per REQ-CONF-031.
pub fn validate_iop(iop: [f64; 6], epsilon: f64) -> Result<()> {
    let row = [iop[0], iop[1], iop[2]];
    let col = [iop[3], iop[4], iop[5]];
    if row.iter().chain(col.iter()).any(|v| !v.is_finite()) {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_IMAGE_ORIENTATION,
                detail: "IOP values must be finite".to_string(),
            },
            "IOP values must be finite",
        )));
    }
    let row_norm = dot(row, row).sqrt();
    let col_norm = dot(col, col).sqrt();
    let dot_rc = dot(row, col).abs();
    if (row_norm - 1.0).abs() > epsilon || (col_norm - 1.0).abs() > epsilon || dot_rc > epsilon {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_IMAGE_ORIENTATION,
                detail: "IOP must be approximately orthonormal".to_string(),
            },
            "IOP must be approximately orthonormal",
        )));
    }
    Ok(())
}

/// Order slices by projection of IPP onto the normal vector (REQ-CONF-030).
pub fn order_slices_by_ipp(slices: &[CtGeometry]) -> Result<Vec<usize>> {
    if slices.is_empty() {
        return Ok(Vec::new());
    }
    let normal = normal_from_iop(slices[0].iop)?;
    let mut projections: Vec<(usize, f64)> = slices
        .iter()
        .enumerate()
        .map(|(idx, slice)| {
            let t = dot(normal, slice.ipp);
            (idx, t)
        })
        .collect();

    if projections.iter().any(|(_, t)| !t.is_finite()) {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidGeometry {
                detail: "slice projection must be finite".to_string(),
            },
            "slice projection must be finite",
        )));
    }

    projections.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    Ok(projections.into_iter().map(|(idx, _)| idx).collect())
}

/// Validate volume-level geometry constraints (REQ-VOL-903/904/905).
pub fn validate_volume_geometry(
    slices: &[CtGeometry],
    tolerances: GeometryTolerance,
) -> Result<()> {
    tolerances.validate()?;
    if slices.is_empty() {
        return Ok(());
    }

    let first = &slices[0];
    for slice in slices.iter().skip(1) {
        if slice.rows != first.rows || slice.cols != first.cols {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "rows/columns mismatch".to_string(),
                },
                "rows/columns mismatch",
            )));
        }
        if !approx_eq(
            slice.pixel_spacing.0,
            first.pixel_spacing.0,
            tolerances.spacing_epsilon,
        ) || !approx_eq(
            slice.pixel_spacing.1,
            first.pixel_spacing.1,
            tolerances.spacing_epsilon,
        ) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "pixel spacing mismatch".to_string(),
                },
                "pixel spacing mismatch",
            )));
        }
        if !approx_iop_eq(slice.iop, first.iop, tolerances.iop_epsilon) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "IOP mismatch".to_string(),
                },
                "IOP mismatch",
            )));
        }
    }

    let order = order_slices_by_ipp(slices)?;
    if order.len() >= 2 {
        let normal = normal_from_iop(slices[order[0]].iop)?;
        let mut prev_t = dot(normal, slices[order[0]].ipp);
        for idx in order.into_iter().skip(1) {
            let t = dot(normal, slices[idx].ipp);
            let delta = t - prev_t;
            if !delta.is_finite() || delta.abs() <= tolerances.slice_spacing_epsilon {
                return Err(Box::new(Error::from_kind(
                    ErrorKind::InvalidGeometry {
                        detail: "non-monotonic slice positions".to_string(),
                    },
                    "non-monotonic slice positions",
                )));
            }
            prev_t = t;
        }
    }

    Ok(())
}

/// Slice spacing computation result.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceSpacing {
    /// Computed slice spacing in mm.
    pub spacing_mm: f64,
    /// True when spacing is unknown (nz < 2).
    pub unknown: bool,
    /// True when spacing deltas were non-uniform and allowed.
    pub non_uniform: bool,
}

/// Compute slice spacing per REQ-VOL-906/907/908.
pub fn compute_slice_spacing(
    sorted_slices: &[CtGeometry],
    tolerances: GeometryTolerance,
    allow_non_uniform: bool,
) -> Result<SliceSpacing> {
    tolerances.validate()?;

    if sorted_slices.len() < 2 {
        return Ok(SliceSpacing {
            spacing_mm: 1.0,
            unknown: true,
            non_uniform: false,
        });
    }

    let normal = normal_from_iop(sorted_slices[0].iop)?;
    let mut deltas = Vec::with_capacity(sorted_slices.len() - 1);
    for window in sorted_slices.windows(2) {
        let t0 = dot(normal, window[0].ipp);
        let t1 = dot(normal, window[1].ipp);
        let delta = (t1 - t0).abs();
        if !delta.is_finite() || delta <= 0.0 {
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "invalid slice spacing delta".to_string(),
                },
                "invalid slice spacing delta",
            )));
        }
        deltas.push(delta);
    }

    let mut sorted = deltas.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    let spacing = if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    };

    for delta in deltas.iter() {
        if (*delta - spacing).abs() > tolerances.slice_spacing_epsilon {
            if allow_non_uniform {
                return Ok(SliceSpacing {
                    spacing_mm: spacing,
                    unknown: false,
                    non_uniform: true,
                });
            }
            return Err(Box::new(Error::from_kind(
                ErrorKind::InvalidGeometry {
                    detail: "non-uniform slice spacing".to_string(),
                },
                "non-uniform slice spacing",
            )));
        }
    }

    Ok(SliceSpacing {
        spacing_mm: spacing,
        unknown: false,
        non_uniform: false,
    })
}

/// Measurement units used for CT tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementUnit {
    /// Pixel-domain units.
    Pixels,
    /// Physical units in millimeters.
    Millimeters,
}

/// Measurement record with provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// Measurement value.
    pub value: f64,
    /// Unit for the measurement.
    pub unit: MeasurementUnit,
    /// True when measurement is calibrated.
    pub calibrated: bool,
    /// Provenance tag used for calibration.
    pub provenance: Option<Tag>,
}

/// Measurement modes (pixel vs physical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementMode {
    /// Pixel-domain measurements.
    Pixels,
    /// Physical-unit measurements in mm.
    PhysicalMillimeters,
}

/// Compute a distance measurement in the selected mode.
pub fn measure_distance(
    p0: (f64, f64),
    p1: (f64, f64),
    pixel_spacing: Option<(f64, f64)>,
    mode: MeasurementMode,
) -> Result<Measurement> {
    match mode {
        MeasurementMode::Pixels => measure_distance_pixels(p0, p1),
        MeasurementMode::PhysicalMillimeters => measure_distance_mm(p0, p1, pixel_spacing),
    }
}

/// Compute a pixel-domain distance measurement (REQ-MEAS-001).
pub fn measure_distance_pixels(p0: (f64, f64), p1: (f64, f64)) -> Result<Measurement> {
    let dist = distance_pixels(p0, p1)?;
    Ok(Measurement {
        value: dist,
        unit: MeasurementUnit::Pixels,
        calibrated: false,
        provenance: None,
    })
}

/// Compute a physical-unit distance measurement (REQ-MEAS-010/020/050/070).
pub fn measure_distance_mm(
    p0: (f64, f64),
    p1: (f64, f64),
    pixel_spacing: Option<(f64, f64)>,
) -> Result<Measurement> {
    CtPack::ensure_physical_measurements_enabled()?;
    let spacing = pixel_spacing.ok_or_else(|| {
        Box::new(Error::from_kind(
            ErrorKind::MissingRequiredTag {
                tag: TAG_PIXEL_SPACING,
            },
            "pixel spacing missing",
        ))
    })?;
    if !spacing.0.is_finite() || !spacing.1.is_finite() || spacing.0 <= 0.0 || spacing.1 <= 0.0 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag: TAG_PIXEL_SPACING,
                detail: "pixel spacing must be finite and > 0".to_string(),
            },
            "pixel spacing must be finite and > 0",
        )));
    }

    let (dx, dy) = delta(p0, p1)?;
    let dx_mm = dx * spacing.0;
    let dy_mm = dy * spacing.1;
    let dist = (dx_mm * dx_mm + dy_mm * dy_mm).sqrt();
    if !dist.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "distance_mm".to_string(),
                detail: "distance computation produced non-finite value".to_string(),
            },
            "distance computation produced non-finite value",
        )));
    }

    Ok(Measurement {
        value: dist,
        unit: MeasurementUnit::Millimeters,
        calibrated: true,
        provenance: Some(TAG_PIXEL_SPACING),
    })
}

/// Compute an angle measurement in degrees (REQ-MEAS-060/061).
pub fn measure_angle_degrees(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> Result<f64> {
    if !a.0.is_finite() || !a.1.is_finite() || !b.0.is_finite() || !b.1.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "angle".to_string(),
                detail: "non-finite input".to_string(),
            },
            "non-finite input",
        )));
    }
    if !c.0.is_finite() || !c.1.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "angle".to_string(),
                detail: "non-finite input".to_string(),
            },
            "non-finite input",
        )));
    }

    let ab = (b.0 - a.0, b.1 - a.1);
    let ac = (c.0 - a.0, c.1 - a.1);
    let ab_len = (ab.0 * ab.0 + ab.1 * ab.1).sqrt();
    let ac_len = (ac.0 * ac.0 + ac.1 * ac.1).sqrt();
    if ab_len == 0.0 || ac_len == 0.0 {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "angle".to_string(),
                detail: "zero-length segment".to_string(),
            },
            "zero-length segment",
        )));
    }

    let mut cos_theta = (ab.0 * ac.0 + ab.1 * ac.1) / (ab_len * ac_len);
    if !cos_theta.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "angle".to_string(),
                detail: "non-finite cos_theta".to_string(),
            },
            "non-finite cos_theta",
        )));
    }
    cos_theta = cos_theta.clamp(-1.0, 1.0);
    let angle = cos_theta.acos().to_degrees();
    if !angle.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "angle".to_string(),
                detail: "non-finite angle".to_string(),
            },
            "non-finite angle",
        )));
    }
    Ok(angle)
}

/// Format a measurement with fixed decimal precision (REQ-MEAS-080).
pub fn format_fixed(value: f64, decimals: u32) -> Result<String> {
    if !value.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "format".to_string(),
                detail: "non-finite value".to_string(),
            },
            "non-finite value",
        )));
    }
    Ok(format!("{:.*}", decimals as usize, value))
}

fn delta(p0: (f64, f64), p1: (f64, f64)) -> Result<(f64, f64)> {
    if !p0.0.is_finite() || !p0.1.is_finite() || !p1.0.is_finite() || !p1.1.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "distance".to_string(),
                detail: "non-finite input".to_string(),
            },
            "non-finite input",
        )));
    }
    Ok((p1.0 - p0.0, p1.1 - p0.1))
}

fn distance_pixels(p0: (f64, f64), p1: (f64, f64)) -> Result<f64> {
    let (dx, dy) = delta(p0, p1)?;
    let dist = (dx * dx + dy * dy).sqrt();
    if !dist.is_finite() {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "distance".to_string(),
                detail: "distance computation produced non-finite value".to_string(),
            },
            "distance computation produced non-finite value",
        )));
    }
    Ok(dist)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normal_from_iop(iop: [f64; 6]) -> Result<[f64; 3]> {
    let row = [iop[0], iop[1], iop[2]];
    let col = [iop[3], iop[4], iop[5]];
    let normal = [
        row[1] * col[2] - row[2] * col[1],
        row[2] * col[0] - row[0] * col[2],
        row[0] * col[1] - row[1] * col[0],
    ];
    if normal.iter().any(|v| !v.is_finite()) {
        return Err(Box::new(Error::from_kind(
            ErrorKind::InvalidGeometry {
                detail: "invalid IOP normal".to_string(),
            },
            "invalid IOP normal",
        )));
    }
    Ok(normal)
}

fn approx_eq(a: f64, b: f64, epsilon: f64) -> bool {
    (a - b).abs() <= epsilon
}

fn approx_vec_eq(a: [f64; 3], b: [f64; 3], epsilon: f64) -> bool {
    approx_eq(a[0], b[0], epsilon)
        && approx_eq(a[1], b[1], epsilon)
        && approx_eq(a[2], b[2], epsilon)
}

fn approx_iop_eq(a: [f64; 6], b: [f64; 6], epsilon: f64) -> bool {
    let a_row = [a[0], a[1], a[2]];
    let a_col = [a[3], a[4], a[5]];
    let b_row = [b[0], b[1], b[2]];
    let b_col = [b[3], b[4], b[5]];

    let row_match =
        approx_vec_eq(a_row, b_row, epsilon) || approx_vec_eq(neg(a_row), b_row, epsilon);
    let col_match =
        approx_vec_eq(a_col, b_col, epsilon) || approx_vec_eq(neg(a_col), b_col, epsilon);
    row_match && col_match
}

fn neg(v: [f64; 3]) -> [f64; 3] {
    [-v[0], -v[1], -v[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_slice(z: f64) -> CtGeometry {
        CtGeometry {
            rows: 512,
            cols: 512,
            pixel_spacing: (0.5, 0.5),
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            ipp: [0.0, 0.0, z],
        }
    }

    #[test]
    fn enhanced_ct_requires_pack() {
        // REQ-FEAT-302, REQ-SOP-301: enhanced CT requires explicit pack feature.
        if dicom_core::capabilities().pack_enhanced {
            CtPack::ensure_enhanced_ct_supported().expect("pack-enhanced enabled");
        } else {
            let err = CtPack::ensure_enhanced_ct_supported().unwrap_err();
            assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
        }
    }

    #[test]
    fn ct_geometry_missing_required_tag() {
        // REQ-CONF-032: missing geometry tags must reject CT series assembly.
        let input = CtGeometryInput {
            rows: Some(512),
            cols: Some(512),
            pixel_spacing: Some((0.5, 0.5)),
            iop: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            ipp: None,
        };
        let err = validate_ct_geometry(input, 1e-4).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn ct_geometry_rejects_non_orthonormal_iop() {
        // REQ-CONF-031: IOP must be approximately orthonormal.
        let input = CtGeometryInput {
            rows: Some(512),
            cols: Some(512),
            pixel_spacing: Some((0.5, 0.5)),
            iop: Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            ipp: Some([0.0, 0.0, 0.0]),
        };
        let err = validate_ct_geometry(input, 1e-4).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn ct_slice_ordering_by_projection() {
        // REQ-CONF-030: order slices by IPP projection onto normal.
        let slices = vec![base_slice(5.0), base_slice(1.0), base_slice(3.0)];
        let order = order_slices_by_ipp(&slices).expect("order slices");
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn ct_volume_geometry_checks() {
        // REQ-VOL-903/904/905: enforce geometry consistency and monotonic positions.
        let slices = vec![base_slice(0.0), base_slice(1.0), base_slice(2.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        validate_volume_geometry(&slices, tol).expect("volume geometry ok");
    }

    #[test]
    fn ct_volume_geometry_rejects_spacing_mismatch() {
        // REQ-VOL-903/905: mismatched spacing fails closed.
        let mut slices = vec![base_slice(0.0), base_slice(1.0)];
        slices[1].pixel_spacing = (0.8, 0.5);
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        let err = validate_volume_geometry(&slices, tol).unwrap_err();
        assert_eq!(err.code, "DVF.GEOM.INVALID");
    }

    #[test]
    fn ct_slice_spacing_single_slice() {
        // REQ-VOL-906: nz < 2 defaults to 1.0 mm and unknown.
        let slices = vec![base_slice(0.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        let spacing = compute_slice_spacing(&slices, tol, false).expect("spacing");
        assert_eq!(spacing.spacing_mm, 1.0);
        assert!(spacing.unknown);
    }

    #[test]
    fn ct_slice_spacing_rejects_non_uniform() {
        // REQ-VOL-908: non-uniform spacing fails closed unless allowed.
        let slices = vec![base_slice(0.0), base_slice(1.0), base_slice(3.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 0.1,
        };
        let err = compute_slice_spacing(&slices, tol, false).unwrap_err();
        assert_eq!(err.code, "DVF.GEOM.INVALID");
    }

    #[test]
    #[cfg(feature = "modality-ct")]
    fn ct_measure_distance_mm_requires_spacing() {
        // REQ-MEAS-020: missing pixel spacing must fail closed in mm mode.
        let err = measure_distance_mm((0.0, 0.0), (1.0, 1.0), None).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    #[cfg(not(feature = "modality-ct"))]
    fn ct_measure_distance_mm_rejects_when_pack_disabled() {
        // REQ-FEAT-302: physical measurements require CT pack feature.
        let err = measure_distance_mm((0.0, 0.0), (1.0, 1.0), None).unwrap_err();
        assert_eq!(err.code, "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    #[cfg(feature = "modality-ct")]
    fn ct_measure_distance_mm_provenance() {
        // REQ-MEAS-050/070: calibrated mm outputs include provenance.
        let measurement =
            measure_distance_mm((0.0, 0.0), (2.0, 0.0), Some((0.5, 0.5))).expect("distance mm");
        assert_eq!(measurement.unit, MeasurementUnit::Millimeters);
        assert!(measurement.calibrated);
        assert_eq!(measurement.provenance, Some(TAG_PIXEL_SPACING));
    }

    #[test]
    fn ct_angle_zero_length_rejected() {
        // REQ-MEAS-060: zero-length segments fail closed.
        let err = measure_angle_degrees((0.0, 0.0), (0.0, 0.0), (1.0, 0.0)).unwrap_err();
        assert_eq!(err.code, "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn ct_format_fixed_deterministic() {
        // REQ-MEAS-080: fixed precision formatting is deterministic.
        let formatted = format_fixed(1.2345, 2).expect("format");
        assert_eq!(formatted, "1.23");
    }
}
