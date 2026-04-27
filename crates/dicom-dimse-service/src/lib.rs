#![deny(missing_docs)]

//! DIMSE SCU/SCP harness built on `dicom-net` and `dicom-dimse`.

#[allow(missing_docs)]
pub mod commitment;
#[allow(missing_docs)]
pub mod env_config;
#[allow(missing_docs)]
pub mod ian;
#[allow(missing_docs)]
pub mod middleware;
#[allow(missing_docs)]
pub mod protocol;
#[allow(missing_docs)]
pub mod transport;

pub use commitment::*;
pub use env_config::*;
pub use ian::*;
pub use middleware::*;
pub use protocol::*;
pub use transport::*;

use dicom_core::{Error, ErrorKind};
use dicom_net::AssociationPolicy;

// ---------------------------------------------------------------------------
// Shared SOP class and transfer syntax constants
// ---------------------------------------------------------------------------

#[allow(missing_docs)]
pub const SOP_CLASS_VERIFICATION: &str = "1.2.840.10008.1.1";
#[allow(missing_docs)]
pub const SOP_CLASS_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
pub(crate) const SOP_CLASS_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
pub(crate) const SOP_CLASS_SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
pub(crate) const SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
pub(crate) const SOP_CLASS_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128";
pub(crate) const SOP_CLASS_CR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1";
pub(crate) const SOP_CLASS_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";
#[allow(missing_docs)]
pub const SOP_CLASS_STUDY_ROOT_FIND: &str = "1.2.840.10008.5.1.4.1.2.2.1";
#[allow(missing_docs)]
pub const SOP_CLASS_STUDY_ROOT_MOVE: &str = "1.2.840.10008.5.1.4.1.2.2.2";
#[allow(missing_docs)]
pub const SOP_CLASS_STUDY_ROOT_GET: &str = "1.2.840.10008.5.1.4.1.2.2.3";
pub(crate) const TRANSFER_SYNTAX_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
pub(crate) const TRANSFER_SYNTAX_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

// ---------------------------------------------------------------------------
// Shared helper functions
// ---------------------------------------------------------------------------

pub(crate) fn io_error(err: std::io::Error) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: err.to_string(),
        },
        "io error",
    )
    .into()
}

#[allow(missing_docs)]
pub fn decode_error(detail: impl Into<String>) -> Box<Error> {
    dicom_util::decode_error("dicom-dimse-service", &detail.into())
}

pub(crate) fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    dicom_util::limit_exceeded(limit_name, observed, allowed)
}

// ---------------------------------------------------------------------------
// Shared policy builder
// ---------------------------------------------------------------------------

pub(crate) fn workstation_default_association_policy() -> AssociationPolicy {
    #[allow(unused_mut)]
    let mut supported_abstract_syntaxes = vec![
        SOP_CLASS_VERIFICATION,
        SOP_CLASS_CT_IMAGE_STORAGE,
        SOP_CLASS_MR_IMAGE_STORAGE,
        SOP_CLASS_SECONDARY_CAPTURE,
        SOP_CLASS_MULTI_FRAME_SC_BYTE,
        SOP_CLASS_MULTI_FRAME_SC_WORD,
        SOP_CLASS_MULTI_FRAME_SC_TRUE_COLOR,
        SOP_CLASS_PET_IMAGE_STORAGE,
        SOP_CLASS_CR_IMAGE_STORAGE,
        SOP_CLASS_DX_PRESENTATION,
    ];
    #[cfg(feature = "dimse-c-find")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_FIND);
    #[cfg(feature = "dimse-c-move")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_MOVE);
    #[cfg(feature = "dimse-c-get")]
    supported_abstract_syntaxes.push(SOP_CLASS_STUDY_ROOT_GET);

    AssociationPolicy {
        called_ae: None,
        supported_abstract_syntaxes: supported_abstract_syntaxes
            .into_iter()
            .map(str::to_string)
            .collect(),
        supported_transfer_syntaxes: vec![
            TRANSFER_SYNTAX_IMPLICIT_VR_LE.to_string(),
            TRANSFER_SYNTAX_EXPLICIT_VR_LE.to_string(),
        ],
        max_pdu_length: 16_384,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
