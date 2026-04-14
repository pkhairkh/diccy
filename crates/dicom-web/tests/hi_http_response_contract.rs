use dicom_core::{Error, ErrorKind, Limits};
use dicom_web::{
    dicomweb_http_status_contract, dicomweb_status_for_error, http_preread_hard_cap_bytes,
    parse_http_request, TransportSecurity, DICOM_WEB_NOT_FOUND_CODE,
};

#[test]
fn http_status_contract_rows_are_stable() {
    // REQ-HI-288, REQ-HI-325, REQ-HI-379
    let rows = dicomweb_http_status_contract();
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].class, "limit_exceeded");
    assert_eq!(rows[0].status, 413);
    assert_eq!(rows[1].class, "auth_denied");
    assert_eq!(rows[1].status, 403);
    assert_eq!(rows[2].class, "not_found");
    assert_eq!(rows[2].status, 404);
}

#[test]
fn status_mapping_is_deterministic_for_core_error_classes() {
    // REQ-HI-288, REQ-HI-325, REQ-HI-379
    let not_found = Error::new(
        DICOM_WEB_NOT_FOUND_CODE,
        ErrorKind::DecodeError {
            stage: "dicom-web".to_string(),
            detail: "not found".to_string(),
        },
        "missing",
    );
    assert_eq!(dicomweb_status_for_error(&not_found), (404, "Not Found"));

    let denied = Error::new(
        "DVF.DICOM.DECODE_ERROR",
        ErrorKind::DecodeError {
            stage: "dicom-auth".to_string(),
            detail: "denied".to_string(),
        },
        "forbidden",
    );
    assert_eq!(dicomweb_status_for_error(&denied), (403, "Forbidden"));

    let malformed = Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-web".to_string(),
            detail: "bad request".to_string(),
        },
        "bad request",
    );
    assert_eq!(dicomweb_status_for_error(&malformed), (400, "Bad Request"));
}

#[test]
fn framing_contract_exposes_preread_cap_and_fails_closed_on_malformed_requests() {
    // REQ-HI-323, REQ-HI-324, REQ-HI-375, REQ-HI-376
    let limits = Limits {
        max_input_bytes: 64,
        max_string_bytes: 16,
        ..Limits::default()
    };
    assert_eq!(http_preread_hard_cap_bytes(&limits), 80);

    let err = parse_http_request(b"GET /studies HTTP/1.1", &limits, TransportSecurity::Tls)
        .expect_err("missing header terminator must fail");
    assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
}
