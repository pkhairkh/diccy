// Auto-extracted from /home/z/diccy/crates/dicom-core/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_core::{
    enforce_limit, parse_f64_strict, parse_i32_strict, validate_dataset, validate_uid_strict,
    Dataset, Element, Error, ErrorKind, Limits, Tag, Value, Vr,
};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

#[test]
fn default_limits_match_spec() {
    // REQ-SEC-401: default limits are explicitly defined.
    let limits = Limits::default();

    assert_eq!(limits.max_input_bytes(), 512 * MIB);
    assert_eq!(limits.max_dataset_elements(), 250_000);
    assert_eq!(limits.max_sequence_depth(), 64);
    assert_eq!(limits.max_string_bytes(), MIB);
    assert_eq!(limits.max_element_vl_bytes(), 64 * MIB);
    assert_eq!(limits.max_frames_per_instance(), 4_096);
    assert_eq!(limits.max_pixels_per_frame(), 16_777_216);
    assert_eq!(limits.max_decompressed_bytes(), GIB);
    assert_eq!(limits.max_gpu_texture_bytes(), 512 * MIB);
    assert_eq!(limits.max_cache_bytes(), GIB);
}

#[test]
fn error_codes_match_required_kinds() {
    // REQ-ERR-030: required error kinds map to stable codes.
    let unsupported_sop = ErrorKind::UnsupportedSopClass {
        sop_class_uid: "1.2.3".to_string(),
    };
    let unsupported_ts = ErrorKind::UnsupportedTransferSyntax {
        transfer_syntax_uid: "1.2.840.10008.1.2".to_string(),
    };
    let missing_tag = ErrorKind::MissingRequiredTag {
        tag: Tag(0x0028, 0x0010),
    };
    let limit_exceeded = ErrorKind::LimitExceeded {
        limit_name: "max_input_bytes",
        observed: 1,
        allowed: 0,
    };
    let invalid_geometry = ErrorKind::InvalidGeometry {
        detail: "bad spacing".to_string(),
    };

    assert_eq!(unsupported_sop.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    assert_eq!(unsupported_ts.code(), "DVF.DICOM.UNSUPPORTED_TS");
    assert_eq!(missing_tag.code(), "DVF.DICOM.MISSING_TAG");
    assert_eq!(limit_exceeded.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
    assert_eq!(invalid_geometry.code(), "DVF.GEOM.INVALID");

    let err = Error::from_kind(missing_tag, "missing tag");
    assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
}

#[test]
fn tag_parsing_accepts_known_formats() {
    // REQ-API-202: parsing helpers are portable and deterministic.
    let tag = Tag::parse_str("0010,0010").expect("parse tag");
    assert_eq!(tag, Tag(0x0010, 0x0010));

    let packed = Tag::parse_str("7FE00010").expect("parse packed tag");
    assert_eq!(packed, Tag(0x7FE0, 0x0010));

    let wrapped = Tag::parse_str("(0028,0010)").expect("parse wrapped tag");
    assert_eq!(wrapped, Tag(0x0028, 0x0010));
}

#[test]
fn tag_parsing_rejects_invalid_formats() {
    // REQ-ERR-008: invalid tag strings must yield structured errors.
    assert!(Tag::parse_str("0010,010").is_err());
    assert!(Tag::parse_str("0010 0010").is_err());
    assert!(Tag::parse_str("ZZZZ,0010").is_err());
}

#[test]
fn vr_parse_accepts_known_and_custom() {
    // REQ-ERR-002: VR enum must cover required variants and parsing.
    let tag = Tag(0x0010, 0x0010);
    assert_eq!(Vr::parse_for_tag(*b"PN", tag).unwrap(), Vr::Pn);
    assert_eq!(Vr::parse_for_tag(*b"ZZ", tag).unwrap(), Vr::Other(*b"ZZ"));
    assert!(Vr::parse_for_tag(*b"pN", tag).is_err());
}

#[test]
fn parse_numbers_strictly() {
    // REQ-ERR-008: invalid numeric formats must be rejected.
    let tag = Tag(0x0028, 0x1050);
    assert_eq!(parse_i32_strict(tag, "42").unwrap(), 42);
    assert_eq!(parse_i32_strict(tag, "-7").unwrap(), -7);
    assert!(parse_i32_strict(tag, " 1").is_err());
    assert!(parse_i32_strict(tag, "+1").is_err());

    assert_eq!(parse_f64_strict(tag, "1.25").unwrap(), 1.25);
    assert_eq!(parse_f64_strict(tag, "-1e-3").unwrap(), -1e-3);
    assert!(parse_f64_strict(tag, "NaN").is_err());
    assert!(parse_f64_strict(tag, " 1.0").is_err());
}

#[test]
fn uid_validation_is_strict() {
    // REQ-ERR-008: invalid UID formats must be rejected.
    let tag = Tag(0x0008, 0x0018);
    assert!(validate_uid_strict(tag, "1.2.840.10008.1.2").is_ok());
    assert!(validate_uid_strict(tag, "").is_err());
    assert!(validate_uid_strict(tag, "1..2").is_err());
    assert!(validate_uid_strict(tag, "1.2.3.").is_err());
    assert!(validate_uid_strict(tag, "1.2.3a").is_err());
}

#[test]
fn enforce_limits_returns_limit_exceeded() {
    // REQ-SEC-404: enforce limits before allocations.
    let err = enforce_limit("max_input_bytes", 10, 5).unwrap_err();
    assert_eq!(err.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
    assert!(matches!(
        err.kind(),
        ErrorKind::LimitExceeded {
            limit_name: "max_input_bytes",
            observed: 10,
            allowed: 5
        }
    ));
}

#[test]
fn dataset_validation_enforces_limits() {
    // REQ-SEC-401..404: limit enforcement covers datasets, strings, and bytes.
    let mut dataset = Dataset::new();
    let tag = Tag(0x0010, 0x0010);
    dataset.insert(Element::new(tag, Vr::Pn, Value::Str("abc".to_string())).unwrap());

    let limits = Limits::builder()
        .max_dataset_elements(1)
        .max_string_bytes(2)
        .build()
        .unwrap();
    assert!(validate_dataset(&dataset, &limits).is_err());
}

#[test]
fn dataset_strict_getters_parse_and_validate() {
    // REQ-ERR-008: typed getters enforce strict parsing.
    let mut dataset = Dataset::new();
    let tag_int = Tag(0x0028, 0x0002);
    let tag_uid = Tag(0x0008, 0x0018);

    dataset.insert(Element::new(tag_int, Vr::Is, Value::Str("12".to_string())).unwrap());
    dataset.insert(Element::new(tag_uid, Vr::Ui, Value::Uid("1.2.840.10008.1.2".to_string())).unwrap());

    let limits = Limits::default();
    assert_eq!(dataset.get_i32_strict(tag_int, &limits).unwrap(), Some(12));
    assert_eq!(
        dataset.get_uid_strict(tag_uid, &limits).unwrap(),
        Some("1.2.840.10008.1.2")
    );
}
