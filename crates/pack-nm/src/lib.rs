#![deny(missing_docs)]

//! Nuclear medicine pack: calibration and timing extraction for measurement gating.

use dicom_core::{Dataset, Error, ErrorKind, Limits, Result, Tag};
use pack_shared::{parse_positive_time, parse_spacing_pair, parse_uniform_time_vector};

/// NM Image Storage SOP Class UID.
pub const SOP_CLASS_NM: &str = "1.2.840.10008.5.1.4.1.1.20";

/// NM SOP Class manifest list.
pub const NM_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_NM];

/// Marker type for nuclear medicine pack features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NmPack;

impl NmPack {
    /// Return true when the nuclear medicine pack is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "pack-nm")
    }

    /// Require the NM pack to be enabled for NM SOP classes.
    pub fn ensure_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "nuclear medicine requires pack-nm feature",
            )));
        }
        if !NM_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported nuclear medicine SOP class",
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

/// Extracted measurement context for NM.
#[derive(Debug, Clone, PartialEq)]
pub struct NmMeasurementContext {
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
) -> Result<NmMeasurementContext> {
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

    Ok(NmMeasurementContext {
        calibration,
        frame_time_ms,
        warnings,
    })
}

const TAG_PIXEL_SPACING: Tag = Tag(0x0028, 0x0030);
const TAG_IMAGER_PIXEL_SPACING: Tag = Tag(0x0018, 0x1164);
const TAG_FRAME_TIME: Tag = Tag(0x0018, 0x1063);
const TAG_FRAME_TIME_VECTOR: Tag = Tag(0x0018, 0x1065);
const FRAME_TIME_EPS: f64 = 1e-6;

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Value, Vr};

    const NM_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        NM_MANIFEST
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
    #[cfg(not(feature = "pack-nm"))]
    fn nm_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(!NmPack::enabled());
        let err = NmPack::ensure_supported(SOP_CLASS_NM).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-nm")]
    fn nm_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(NmPack::enabled());
        NmPack::ensure_supported(SOP_CLASS_NM).expect("nm pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083, REQ-SOP-300
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in NM_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn calibration_prefers_pixel_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("0.8\\0.9".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGER_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("1\\1".to_string()),
            )
            .unwrap(),
        );
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(
            ctx.calibration,
            Some((CalibrationSource::PixelSpacing, (0.8, 0.9)))
        );
    }

    #[test]
    fn calibration_falls_back_to_imager_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_IMAGER_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("1\\2".to_string()),
            )
            .unwrap(),
        );
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
        dataset
            .insert(Element::new(TAG_FRAME_TIME, Vr::Ds, Value::Bytes(vec![0xff, 0xfe])).unwrap());
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert!(ctx.warnings.contains(&MeasurementWarning::InvalidFrameTime));
    }

    #[test]
    fn frame_time_vector_fallback() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_FRAME_TIME_VECTOR,
                Vr::Ds,
                Value::Str("40\\40".to_string()),
            )
            .unwrap(),
        );
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(ctx.frame_time_ms, Some(40.0));
    }
}
