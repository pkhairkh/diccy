//! Shared test utilities for the diccy workspace.
//!
//! Provides factory functions, assertion macros, and shared test fixtures
//! to reduce test boilerplate across the 48-crate workspace.

use dicom_core::{Dataset, Element, Error, ErrorKind, Limits, Tag, Value, Vr};

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
