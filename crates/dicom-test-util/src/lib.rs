//! Shared test utilities for the diccy workspace.
//!
//! Provides factory functions, assertion macros, and shared test fixtures
//! to reduce test boilerplate across the 48-crate workspace.

use dicom_core::{Dataset, Element, Limits, Tag, Value, Vr};

/// Create a minimal valid DICOM dataset with Patient ID and Study UID.
pub fn make_minimal_dataset() -> Dataset {
    let mut ds = Dataset::new();
    let _ = ds.insert(Element::new(
        Tag(0x0010, 0x0020),
        Vr::Lo,
        Value::Str("TEST-PATIENT".to_string()),
    ).unwrap());
    let _ = ds.insert(Element::new(
        Tag(0x0020, 0x000D),
        Vr::Ui,
        Value::Uid("1.2.3.4.5".to_string()),
    ).unwrap());
    ds
}

/// Create a minimal CT dataset.
pub fn make_ct_dataset() -> Dataset {
    let mut ds = make_minimal_dataset();
    let _ = ds.insert(Element::new(
        Tag(0x0008, 0x0060),
        Vr::Cs,
        Value::Str("CT".to_string()),
    ).unwrap());
    ds
}

/// Create a minimal MR dataset.
pub fn make_mr_dataset() -> Dataset {
    let mut ds = make_minimal_dataset();
    let _ = ds.insert(Element::new(
        Tag(0x0008, 0x0060),
        Vr::Cs,
        Value::Str("MR".to_string()),
    ).unwrap());
    ds
}

/// Create a minimal SR dataset.
pub fn make_sr_dataset() -> Dataset {
    let mut ds = make_minimal_dataset();
    let _ = ds.insert(Element::new(
        Tag(0x0008, 0x0060),
        Vr::Cs,
        Value::Str("SR".to_string()),
    ).unwrap());
    ds
}

/// Assert that a result has the expected ErrorKind.
#[macro_export]
macro_rules! assert_error_kind {
    ($result:expr, $kind:pat) => {
        match $result.unwrap_err().kind() {
            $kind => {}
            other => panic!("Expected error kind matching {}, got {:?}", stringify!($kind), other),
        }
    };
}

/// Assert that a result has the expected error code.
#[macro_export]
macro_rules! assert_error_code {
    ($result:expr, $code:expr) => {
        let err = $result.unwrap_err();
        assert_eq!(err.code(), $code, "Expected error code {}, got {}", $code, err.code());
    };
}

// ===========================================================================
// Shared DICOM Part 10 Test Fixture Data
// ===========================================================================

/// Minimal valid DICOM Part 10 preamble (128 zero bytes + "DICM" magic).
///
/// This is the standard 132-byte DICOM Part 10 header that every valid
/// DICOM file starts with. Use this as the prefix when constructing test
/// fixture files.
pub const DICOM_P10_PREAMBLE: [u8; 132] = {
    let mut buf = [0u8; 132];
    buf[128] = b'D';
    buf[129] = b'I';
    buf[130] = b'C';
    buf[131] = b'M';
    buf
};

/// Return a minimal valid DICOM Part 10 file as raw bytes.
///
/// The file contains:
/// - 128-byte preamble + "DICM" magic (132 bytes)
/// - File Meta Information group (0002,xxxx):
///   - (0002,0000) File Meta Information Group Length
///   - (0002,0001) File Meta Information Version
///   - (0002,0002) Media Storage SOP Class UID (Secondary Capture)
///   - (0002,0003) Media Storage SOP Instance UID
///   - (0002,0010) Transfer Syntax UID (Explicit VR Little Endian)
/// - Patient ID (0010,0020)
/// - Study Instance UID (0020,000D)
///
/// Total size is approximately 300 bytes.
pub fn minimal_p10_bytes() -> Vec<u8> {
    let mut buf = Vec::with_capacity(512);

    // Preamble + magic
    buf.extend_from_slice(&DICOM_P10_PREAMBLE);

    // File Meta Information Group Length (0002,0000) = UL, 4 bytes
    // We'll compute the length after encoding all 0002 elements
    let meta_elements = encode_meta_elements();
    // Group length = total bytes of all meta elements (excluding the group length element itself)
    let group_length = meta_elements.len() as u32;

    // (0002,0000) UL 4 — File Meta Information Group Length
    buf.extend_from_slice(&0x0002u16.to_le_bytes()); // group
    buf.extend_from_slice(&0x0000u16.to_le_bytes()); // element
    buf.extend_from_slice(b"UL");                     // VR
    buf.extend_from_slice(&0u16.to_le_bytes());       // reserved
    buf.extend_from_slice(&4u32.to_le_bytes());       // length
    buf.extend_from_slice(&group_length.to_le_bytes()); // value

    // Meta elements
    buf.extend_from_slice(&meta_elements);

    // Patient ID (0010,0020) LO — Explicit VR Little Endian
    let patient_id = b"TEST-PATIENT";
    buf.extend_from_slice(&0x0010u16.to_le_bytes());
    buf.extend_from_slice(&0x0020u16.to_le_bytes());
    buf.extend_from_slice(b"LO");
    buf.extend_from_slice(&(patient_id.len() as u16).to_le_bytes());
    buf.extend_from_slice(patient_id);

    // Study Instance UID (0020,000D) UI
    let study_uid = b"1.2.3.4.5.6.7.8";
    buf.extend_from_slice(&0x0020u16.to_le_bytes());
    buf.extend_from_slice(&0x000Du16.to_le_bytes());
    buf.extend_from_slice(b"UI");
    buf.extend_from_slice(&(study_uid.len() as u16).to_le_bytes());
    buf.extend_from_slice(study_uid);
    // Pad to even length if needed
    if study_uid.len() % 2 != 0 {
        buf.push(0u8);
    }

    buf
}

/// Encode the File Meta Information elements (excluding group length).
fn encode_meta_elements() -> Vec<u8> {
    let mut buf = Vec::with_capacity(256);

    // (0002,0001) OB — File Meta Information Version = [0, 1]
    buf.extend_from_slice(&0x0002u16.to_le_bytes());
    buf.extend_from_slice(&0x0001u16.to_le_bytes());
    buf.extend_from_slice(b"OB");
    buf.extend_from_slice(&0u16.to_le_bytes()); // reserved
    buf.extend_from_slice(&2u32.to_le_bytes()); // length
    buf.extend_from_slice(&[0u8, 1u8]);

    // (0002,0002) UI — Media Storage SOP Class UID (Secondary Capture: 1.2.840.10008.5.1.4.1.1.7)
    let sop_class = b"1.2.840.10008.5.1.4.1.1.7";
    buf.extend_from_slice(&0x0002u16.to_le_bytes());
    buf.extend_from_slice(&0x0002u16.to_le_bytes());
    buf.extend_from_slice(b"UI");
    buf.extend_from_slice(&(sop_class.len() as u16).to_le_bytes());
    buf.extend_from_slice(sop_class);
    if sop_class.len() % 2 != 0 {
        buf.push(0u8); // pad to even
    }

    // (0002,0003) UI — Media Storage SOP Instance UID
    let sop_instance = b"1.2.3.4.5.6.7.8.9";
    buf.extend_from_slice(&0x0002u16.to_le_bytes());
    buf.extend_from_slice(&0x0003u16.to_le_bytes());
    buf.extend_from_slice(b"UI");
    buf.extend_from_slice(&(sop_instance.len() as u16).to_le_bytes());
    buf.extend_from_slice(sop_instance);
    if sop_instance.len() % 2 != 0 {
        buf.push(0u8);
    }

    // (0002,0010) UI — Transfer Syntax UID (Explicit VR Little Endian: 1.2.840.10008.1.2.1)
    let ts_uid = b"1.2.840.10008.1.2.1";
    buf.extend_from_slice(&0x0002u16.to_le_bytes());
    buf.extend_from_slice(&0x0010u16.to_le_bytes());
    buf.extend_from_slice(b"UI");
    buf.extend_from_slice(&(ts_uid.len() as u16).to_le_bytes());
    buf.extend_from_slice(ts_uid);
    if ts_uid.len() % 2 != 0 {
        buf.push(0u8);
    }

    buf
}

/// Return the default Limits used for testing.
///
/// Provides a permissive set of limits suitable for unit tests that
/// should not hit resource constraints.
pub fn test_limits() -> Limits {
    Limits::builder()
        .max_input_bytes(1024 * 1024 * 1024)
        .max_dataset_elements(1_000_000)
        .max_sequence_depth(128)
        .max_string_bytes(10 * 1024 * 1024)
        .max_element_vl_bytes(512 * 1024 * 1024)
        .max_frames_per_instance(10_000)
        .max_pixels_per_frame(100_000_000)
        .max_decompressed_bytes(10 * 1024 * 1024 * 1024)
        .max_gpu_texture_bytes(1024 * 1024 * 1024)
        .max_cache_bytes(10 * 1024 * 1024 * 1024)
        .build()
        .expect("test limits should be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Error, ErrorKind};

    #[test]
    fn make_minimal_dataset_has_patient_id() {
        let ds = make_minimal_dataset();
        let patient_id = ds.get_str(Tag(0x0010, 0x0020));
        assert_eq!(patient_id, Some("TEST-PATIENT"));
    }

    #[test]
    fn make_minimal_dataset_has_study_uid() {
        let ds = make_minimal_dataset();
        let study_uid = ds.get_uid(Tag(0x0020, 0x000D));
        assert_eq!(study_uid, Some("1.2.3.4.5"));
    }

    #[test]
    fn make_ct_dataset_has_modality() {
        let ds = make_ct_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("CT"));
    }

    #[test]
    fn make_mr_dataset_has_modality() {
        let ds = make_mr_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("MR"));
    }

    #[test]
    fn make_sr_dataset_has_modality() {
        let ds = make_sr_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("SR"));
    }

    #[test]
    fn minimal_p10_has_dicm_magic() {
        let bytes = minimal_p10_bytes();
        assert!(bytes.len() > 132, "P10 data must include preamble + magic");
        assert_eq!(&bytes[128..132], b"DICM", "P10 magic must be DICM");
    }

    #[test]
    fn minimal_p10_contains_meta_group() {
        let bytes = minimal_p10_bytes();
        // After the 132-byte preamble, the first element should be (0002,0000)
        assert!(bytes.len() > 140);
        let group = u16::from_le_bytes([bytes[132], bytes[133]]);
        let element = u16::from_le_bytes([bytes[134], bytes[135]]);
        assert_eq!(group, 0x0002);
        assert_eq!(element, 0x0000);
    }

    #[test]
    fn test_limits_are_valid() {
        let limits = test_limits();
        assert!(limits.validate().is_ok(), "test limits should validate");
    }

    #[test]
    fn assert_error_kind_macro_works() {
        let result: Result<Dataset, Box<Error>> = Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "test",
                observed: 100,
                allowed: 50,
            },
            "test error",
        ).into());
        assert_error_kind!(result, ErrorKind::LimitExceeded { .. });
    }

    #[test]
    fn assert_error_code_macro_works() {
        let result: Result<Dataset, Box<Error>> = Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "test",
                observed: 100,
                allowed: 50,
            },
            "test error",
        ).into());
        assert_error_code!(result, "DVF.SECURITY.LIMIT_EXCEEDED");
    }

    #[test]
    fn dicom_p10_preamble_is_all_zeros_then_dicm() {
        for i in 0..128 {
            assert_eq!(DICOM_P10_PREAMBLE[i], 0, "preamble byte {i} should be zero");
        }
        assert_eq!(&DICOM_P10_PREAMBLE[128..132], b"DICM");
    }
}
