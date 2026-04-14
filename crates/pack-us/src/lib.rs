#![deny(missing_docs)]

//! Ultrasound pack: calibration and timing extraction for measurement gating.

use dicom_core::{Dataset, Error, ErrorKind, Limits, Result, Tag};
use pack_shared::{parse_positive_time, parse_spacing_pair};

/// Ultrasound Image Storage SOP Class UID.
pub const SOP_CLASS_US: &str = "1.2.840.10008.5.1.4.1.1.6.1";
/// Ultrasound Multi-frame Image Storage SOP Class UID.
pub const SOP_CLASS_US_MF: &str = "1.2.840.10008.5.1.4.1.1.3.1";

/// Ultrasound SOP Class manifest list.
pub const US_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_US, SOP_CLASS_US_MF];

/// Marker type for ultrasound pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsPack;

impl UsPack {
    /// Return true when the ultrasound pack is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-us")
    }

    /// Require the ultrasound pack to be enabled for ultrasound SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "ultrasound requires pack-us feature",
            )));
        }
        if !US_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported ultrasound SOP class",
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
}

/// Extracted measurement context for ultrasound.
#[derive(Debug, Clone, PartialEq)]
pub struct UsMeasurementContext {
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
) -> Result<UsMeasurementContext> {
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
            warnings.push(MeasurementWarning::MissingFrameTime);
            None
        }
        Err(_) => {
            warnings.push(MeasurementWarning::InvalidFrameTime);
            None
        }
    };

    Ok(UsMeasurementContext {
        calibration,
        frame_time_ms,
        warnings,
    })
}

const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
const TAG_IMAGER_PIXEL_SPACING: Tag = Tag(0x0018, 0x1164);
const TAG_FRAME_TIME: Tag = Tag(0x0018, 0x1063);

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Value, Vr};

    const US_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        US_MANIFEST
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
    #[cfg(not(feature = "pack-us"))]
    fn us_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(!UsPack::enabled());
        let err = UsPack::ensure_supported(SOP_CLASS_US).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-us")]
    fn us_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(UsPack::enabled());
        UsPack::ensure_supported(SOP_CLASS_US).expect("us pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083, REQ-SOP-300
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in US_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn calibration_prefers_pixel_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_PIXEL_SPACING,
            vr: Vr::Ds,
            value: Value::Str("0.5\\0.5".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_IMAGER_PIXEL_SPACING,
            vr: Vr::Ds,
            value: Value::Str("1\\1".to_string()),
        });
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(
            ctx.calibration,
            Some((CalibrationSource::PixelSpacing, (0.5, 0.5)))
        );
    }

    #[test]
    fn calibration_falls_back_to_imager_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_IMAGER_PIXEL_SPACING,
            vr: Vr::Ds,
            value: Value::Str("1\\2".to_string()),
        });
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(
            ctx.calibration,
            Some((CalibrationSource::ImagerPixelSpacing, (1.0, 2.0)))
        );
    }

    #[test]
    fn calibration_missing_warns() {
        // REQ-CONF-085
        let dataset = Dataset::new();
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert!(ctx.calibration.is_none());
        assert!(ctx
            .warnings
            .contains(&MeasurementWarning::MissingPixelSpacing));
        assert!(ctx
            .warnings
            .contains(&MeasurementWarning::MissingImagerPixelSpacing));
    }

    #[test]
    fn invalid_utf8_bytes_emit_invalid_warning() {
        // REQ-CONF-085: malformed string bytes must fail closed as invalid metadata.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_FRAME_TIME,
            vr: Vr::Ds,
            value: Value::Bytes(vec![0xff, 0xfe]),
        });
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert!(ctx.warnings.contains(&MeasurementWarning::InvalidFrameTime));
    }
}
