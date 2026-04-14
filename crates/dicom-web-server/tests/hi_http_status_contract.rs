use dicom_core::{Error, ErrorKind};
use dicom_web::{dicomweb_status_for_error, DICOM_WEB_NOT_FOUND_CODE};

#[test]
fn http_status_contract_maps_auth_decode_to_forbidden() {
    // REQ-HI-287, REQ-HI-325, REQ-HI-379
    let denied = Error::new(
        "DVF.DICOM.DECODE_ERROR",
        ErrorKind::DecodeError {
            stage: "dicom-auth".to_string(),
            detail: "denied".to_string(),
        },
        "forbidden",
    );
    assert_eq!(dicomweb_status_for_error(&denied), (403, "Forbidden"));
}

#[test]
fn http_status_contract_maps_structured_not_found_to_404() {
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
}
