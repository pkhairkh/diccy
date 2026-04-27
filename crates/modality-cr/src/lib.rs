#![deny(missing_docs)]

//! CR/DR modality pack: Computed Radiography SOP gating and measurement helpers.
//!
//! Renamed from `modality-xr` to `modality-cr` to disambiguate from
//! `dicom-xr` (Extended Reality visualization crate, Sprint 5).

use dicom_core::{Error, ErrorKind, Result};

/// Computed Radiography Image Storage.
pub const SOP_CLASS_CR: &str = "1.2.840.10008.5.1.4.1.1.1";
/// Digital X-Ray Image Storage - For Presentation.
pub const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";

/// Supported CR/DR SOP Class UIDs.
pub const CR_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_CR, SOP_CLASS_DX_PRESENTATION];

/// Marker type for CR/DR modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrPack;

impl CrPack {
    /// Return true when CR pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-cr")
    }

    /// Require the CR pack to be enabled for CR/DR SOP classes.
    pub fn ensure_cr_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "CR requires modality-cr feature",
            )));
        }
        if !CR_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported CR SOP class",
            )));
        }
        Ok(())
    }

    /// Require the CR pack to be enabled for physical-unit measurements.
    pub fn ensure_physical_measurements_enabled() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "measurement_mode".to_string(),
                    detail: "physical-unit measurements require modality-cr feature".to_string(),
                },
                "physical-unit measurements require modality-cr feature",
            )))
        }
    }
}
