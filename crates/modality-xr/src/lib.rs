#![deny(missing_docs)]

//! XR modality pack: CR/DR SOP gating and measurement helpers.

use dicom_core::{Error, ErrorKind, Result};

/// Computed Radiography Image Storage.
pub const SOP_CLASS_CR: &str = "1.2.840.10008.5.1.4.1.1.1";
/// Digital X-Ray Image Storage - For Presentation.
pub const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";

/// Supported XR SOP Class UIDs.
pub const XR_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_CR, SOP_CLASS_DX_PRESENTATION];

/// Marker type for XR modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XrPack;

impl XrPack {
    /// Return true when XR pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-xr")
    }

    /// Require the XR pack to be enabled for XR SOP classes.
    pub fn ensure_xr_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "XR requires modality-xr feature",
            )));
        }
        if !XR_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported XR SOP class",
            )));
        }
        Ok(())
    }

    /// Require the XR pack to be enabled for physical-unit measurements.
    pub fn ensure_physical_measurements_enabled() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "measurement_mode".to_string(),
                    detail: "physical-unit measurements require modality-xr feature".to_string(),
                },
                "physical-unit measurements require modality-xr feature",
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(feature = "modality-xr"))]
    fn xr_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301: XR pack requires explicit Cargo feature.
        assert!(!XrPack::enabled());
        let err = XrPack::ensure_xr_supported(SOP_CLASS_CR).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "modality-xr")]
    fn xr_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301: XR pack requires explicit Cargo feature.
        assert!(XrPack::enabled());
        XrPack::ensure_xr_supported(SOP_CLASS_CR).expect("xr pack enabled");
    }

    #[test]
    fn xr_measurements_gate() {
        // REQ-MEAS-010
        if XrPack::enabled() {
            XrPack::ensure_physical_measurements_enabled().expect("xr pack enabled");
        } else {
            let err = XrPack::ensure_physical_measurements_enabled().unwrap_err();
            assert_eq!(err.code, "DVF.PIXEL.INVALID_TRANSFORM");
        }
    }
}
