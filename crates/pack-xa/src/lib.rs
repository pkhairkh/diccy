#![deny(missing_docs)]

//! XA/XRF pack: calibration and timing extraction for measurement gating.

use dicom_core::{Dataset, Error, ErrorKind, Limits, Result, Tag};
use pack_shared::{parse_positive_time, parse_spacing_pair, parse_uniform_time_vector};

/// XA Image Storage SOP Class UID.
pub const SOP_CLASS_XA: &str = "1.2.840.10008.5.1.4.1.1.12.1";
/// XRF Image Storage SOP Class UID.
pub const SOP_CLASS_XRF: &str = "1.2.840.10008.5.1.4.1.1.12.2";

/// XA/XRF SOP Class manifest list.
pub const XA_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_XA, SOP_CLASS_XRF];

/// Marker type for XA/XRF pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XaPack;

impl XaPack {
    /// Return true when the XA/XRF pack is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-xa")
    }

    /// Require the XA/XRF pack to be enabled for XA SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "xa/xrf requires pack-xa feature",
            )));
        }
        if !XA_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported XA/XRF SOP class",
            )));
        }
        Ok(())
    }
}

/// Calibration source for physical spacing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationSource {
    /// Pixel Spacing (0028,0030).
    PixelSpacing,
    /// Imager Pixel Spacing (0018,1164).
    ImagerPixelSpacing,
}

/// Measurement warning types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeasurementWarning {
    /// Pixel Spacing missing.
    MissingPixelSpacing,
    /// Pixel Spacing invalid.
    InvalidPixelSpacing,
    /// Imager Pixel Spacing missing.
    MissingImagerPixelSpacing,
    /// Imager Pixel Spacing invalid.
    InvalidImagerPixelSpacing,
    /// Frame Time missing.
    MissingFrameTime,
    /// Frame Time invalid.
    InvalidFrameTime,
    /// Frame Time Vector missing.
    MissingFrameTimeVector,
    /// Frame Time Vector invalid.
    InvalidFrameTimeVector,
}

/// Extracted measurement context for XA/XRF.
#[derive(Debug, Clone, PartialEq)]
pub struct XaMeasurementContext {
    /// Optional calibrated spacing and its source.
    pub calibration: Option<(CalibrationSource, (f64, f64))>,
    /// Optional frame time in milliseconds.
    pub frame_time_ms: Option<f64>,
    /// Structured warnings for missing/invalid calibration or timing.
    pub warnings: Vec<MeasurementWarning>,
}

/// Extract calibration and timing metadata for measurement gating.
pub fn extract_measurement_context(
    dataset: &Dataset,
    limits: &Limits,
) -> Result<XaMeasurementContext> {
    let mut warnings = Vec::new();
    let pixel_spacing = match parse_spacing_pair(dataset, TAG_PIXEL_SPACING, limits) {
        Ok(Some(value)) => Some((CalibrationSource::PixelSpacing, value)),
        Ok(None) => {
            warnings.push(MeasurementWarning::MissingPixelSpacing);
            None
        }
        Err(_) => {
            warnings.push(MeasurementWarning::InvalidPixelSpacing);
            None
        }
    };
    let imager_spacing = match parse_spacing_pair(dataset, TAG_IMAGER_PIXEL_SPACING, limits) {
        Ok(Some(value)) => Some((CalibrationSource::ImagerPixelSpacing, value)),
        Ok(None) => {
            warnings.push(MeasurementWarning::MissingImagerPixelSpacing);
            None
        }
        Err(_) => {
            warnings.push(MeasurementWarning::InvalidImagerPixelSpacing);
            None
        }
    };
    let calibration = pixel_spacing.or(imager_spacing);

    let frame_time_ms = match parse_positive_time(dataset, TAG_FRAME_TIME, limits) {
        Ok(Some(value)) => Some(value),
        Ok(None) => {
            match parse_uniform_time_vector(dataset, TAG_FRAME_TIME_VECTOR, FRAME_TIME_EPS, limits)
            {
                Ok(Some(value)) => Some(value),
                Ok(None) => {
                    warnings.push(MeasurementWarning::MissingFrameTime);
                    warnings.push(MeasurementWarning::MissingFrameTimeVector);
                    None
                }
                Err(_) => {
                    warnings.push(MeasurementWarning::MissingFrameTime);
                    warnings.push(MeasurementWarning::InvalidFrameTimeVector);
                    None
                }
            }
        }
        Err(_) => {
            warnings.push(MeasurementWarning::InvalidFrameTime);
            None
        }
    };

    Ok(XaMeasurementContext {
        calibration,
        frame_time_ms,
        warnings,
    })
}

/// DICOM Tag for Pixel Spacing (0028,0030).
pub const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
/// DICOM Tag for Imager Pixel Spacing (0018,1164).
pub const TAG_IMAGER_PIXEL_SPACING: Tag = Tag(0x0018, 0x1164);
/// DICOM Tag for Frame Time (0018,1063).
pub const TAG_FRAME_TIME: Tag = Tag(0x0018, 0x1063);
/// DICOM Tag for Frame Time Vector (0018,1065).
pub const TAG_FRAME_TIME_VECTOR: Tag = Tag(0x0018, 0x1065);
/// Epsilon value for FRAME_TIME_EPS
pub const FRAME_TIME_EPS: f64 = 1e-6;
