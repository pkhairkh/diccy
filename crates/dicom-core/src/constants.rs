//! DICOM standard constants and well-known UID values.
//!
//! Centralizes magic numbers from DICOM PS3.5, PS3.6, PS3.7, and PS3.8
//! that were previously hardcoded across the workspace.

/// Maximum UID length in bytes (DICOM PS3.5 9.1).
pub const MAX_UID_LENGTH: usize = 64;

/// Maximum AE title length in bytes (DICOM PS3.8).
pub const MAX_AE_TITLE_LENGTH: usize = 16;

/// Typical maximum PDU length (negotiable, common default).
pub const MAX_PDU_LENGTH: u32 = 131_072;

/// DICOM Part 10 preamble length: 128 zero bytes + 4-byte "DICM" prefix.
pub const DICOM_PREAMBLE_LENGTH: usize = 132;

/// DICOM Part 10 magic prefix.
pub const DICOM_PREFIX: &[u8; 4] = b"DICM";

// ---------------------------------------------------------------------------
// Transfer Syntax UIDs (DICOM PS3.6 Table A-1)
// ---------------------------------------------------------------------------

/// Implicit VR Little Endian: Default Transfer Syntax for DICOM.
pub const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";

/// Explicit VR Little Endian.
pub const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

/// Explicit VR Big Endian (retired).
pub const EXPLICIT_VR_BE: &str = "1.2.840.10008.1.2.2";

/// JPEG Baseline (Process 1).
pub const JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";

/// JPEG Lossless, Non-Hierarchical (Process 14).
pub const JPEG_LOSSLESS: &str = "1.2.840.10008.1.2.4.70";

/// JPEG-LS Lossless.
pub const JPEG_LS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";

/// JPEG-LS Near-Lossless.
pub const JPEG_LS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";

/// JPEG 2000 Lossless.
pub const JPEG_2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";

/// JPEG 2000 Lossy.
pub const JPEG_2000: &str = "1.2.840.10008.1.2.4.91";

/// RLE Lossless.
pub const RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";

// ---------------------------------------------------------------------------
// Application Context UID
// ---------------------------------------------------------------------------

/// DICOM Application Context Name.
pub const APPLICATION_CONTEXT: &str = "1.2.840.10008.3.1.1.1";

// ---------------------------------------------------------------------------
// Well-Known SOP Class UIDs (DICOM PS3.6)
// ---------------------------------------------------------------------------

/// Verification SOP Class.
pub const SOP_VERIFICATION: &str = "1.2.840.10008.1.1";

/// CT Image Storage.
pub const SOP_CT_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.2";

/// MR Image Storage.
pub const SOP_MR_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.4";

/// Ultrasound Image Storage (retired).
pub const SOP_US_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.6";

/// Nuclear Medicine Image Storage.
pub const SOP_NM_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.20";

/// Secondary Capture Image Storage.
pub const SOP_SC_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.7";

/// X-Ray Angiographic Image Storage.
pub const SOP_XA_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.12.1";

/// X-Ray Radiofluoroscopic Image Storage.
pub const SOP_XRF_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.12.2";

/// PET Image Storage.
pub const SOP_PET_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.77.1";

/// RT Image Storage.
pub const SOP_RT_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.481.1";

/// RT Dose Storage.
pub const SOP_RT_DOSE: &str = "1.2.840.10008.5.1.4.1.1.481.2";

/// RT Structure Set Storage.
pub const SOP_RT_STRUCTURE: &str = "1.2.840.10008.5.1.4.1.1.481.3";

/// RT Plan Storage.
pub const SOP_RT_PLAN: &str = "1.2.840.10008.5.1.4.1.1.481.5";

/// Segmentation Storage.
pub const SOP_SEG: &str = "1.2.840.10008.5.1.4.1.1.66.4";

/// Grayscale Softcopy Presentation State Storage.
pub const SOP_GSPS: &str = "1.2.840.10008.5.1.4.1.1.11.1";

/// SR Basic Text.
pub const SOP_SR_BASIC_TEXT: &str = "1.2.840.10008.5.1.4.1.1.88.11";

/// SR Comprehensive.
pub const SOP_SR_COMPREHENSIVE: &str = "1.2.840.10008.5.1.4.1.1.88.33";

/// Enhanced CT Image Storage.
pub const SOP_ENHANCED_CT: &str = "1.2.840.10008.5.1.4.1.1.2.1";

/// Enhanced MR Image Storage.
pub const SOP_ENHANCED_MR: &str = "1.2.840.10008.5.1.4.1.1.4.1";

/// Mammography (MG) Image Storage.
pub const SOP_MG_IMAGE: &str = "1.2.840.10008.5.1.4.1.1.1.2";

/// Breast Tomosynthesis Image Storage.
pub const SOP_BREAST_TOMO: &str = "1.2.840.10008.5.1.4.1.1.13.1.1";

/// VL Whole Slide Microscopy Image Storage.
pub const SOP_WSI: &str = "1.2.840.10008.5.1.4.1.1.77.1.6";

/// Encapsulated PDF Storage.
pub const SOP_ENCAPSULATED_PDF: &str = "1.2.840.10008.5.1.4.1.1.104.1";

/// Encapsulated CDA Storage.
pub const SOP_ENCAPSULATED_CDA: &str = "1.2.840.10008.5.1.4.1.1.104.2";

/// Video Endoscopic Image Storage.
pub const SOP_VIDEO_ENDOSCOPIC: &str = "1.2.840.10008.5.1.4.1.1.77.1.1.1";

/// Hanging Protocol Storage.
pub const SOP_HANGING_PROTOCOL: &str = "1.2.840.10008.5.1.4.38.1";

/// Storage Commitment Push Model SOP Class.
pub const SOP_STORAGE_COMMITMENT: &str = "1.2.840.10008.1.20.1";

/// AI Results Storage.
pub const SOP_AI_RESULTS: &str = "1.2.840.10008.5.1.4.1.1.90.1";
