#![deny(missing_docs)]

//! MG modality pack: mammography SOP support and measurement gating.

use dicom_core::{Error, ErrorKind, Result, Tag};

/// Mammography X-Ray Image Storage - For Presentation.
pub const SOP_CLASS_MG_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.2";
/// Mammography X-Ray Image Storage - For Processing.
pub const SOP_CLASS_MG_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.1.2.1";

/// Mammography SOP Class manifest (static list).
pub const MG_SOP_CLASS_UIDS: &[&str] = &[SOP_CLASS_MG_PRESENTATION, SOP_CLASS_MG_PROCESSING];

/// Marker type for MG modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MgPack;

impl MgPack {
    /// Return true when MG pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-mg")
    }

    /// Require the MG pack to be enabled for mammography SOP classes.
    pub fn ensure_mg_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "mammography requires modality-mg feature",
            )));
        }
        if !MG_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported mammography SOP class",
            )));
        }
        Ok(())
    }

    /// Require the MG pack to be enabled for physical-unit measurements.
    pub fn ensure_physical_measurements_enabled() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "measurement_mode".to_string(),
                    detail: "physical-unit measurements require modality-mg feature".to_string(),
                },
                "physical-unit measurements require modality-mg feature",
            )))
        }
    }
}

/// DICOM tag for SOP Class UID.
pub const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);

#[cfg(test)]
mod tests {
    use super::*;
    const MG_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        MG_MANIFEST
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

    #[test]
    #[cfg(not(feature = "modality-mg"))]
    fn mg_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(!MgPack::enabled());
        let err = MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).unwrap_err();
        assert_eq!(err.code, "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "modality-mg")]
    fn mg_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(MgPack::enabled());
        MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).expect("mg pack enabled");
    }

    #[test]
    fn mg_manifest_matches_constants() {
        // REQ-CONF-012: MG pack includes a SOP class manifest.
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in MG_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn mg_manifest_uids_look_like_uids() {
        // REQ-CONF-012: manifest entries must be valid UID-like strings.
        for uid in MG_SOP_CLASS_UIDS {
            if uid.is_empty() || uid.starts_with('.') || uid.ends_with('.') {
                let err = invalid_tag_value(TAG_SOP_CLASS_UID, "invalid UID format");
                assert_eq!(err.code, "DVF.DICOM.INVALID_TAG_VALUE");
            }
            assert!(uid.chars().all(|ch| ch.is_ascii_digit() || ch == '.'));
        }
    }

    #[test]
    fn mg_measurements_gate() {
        // REQ-MEAS-010
        if MgPack::enabled() {
            MgPack::ensure_physical_measurements_enabled().expect("mg pack enabled");
        } else {
            let err = MgPack::ensure_physical_measurements_enabled().unwrap_err();
            assert_eq!(err.code, "DVF.PIXEL.INVALID_TRANSFORM");
        }
    }
}
