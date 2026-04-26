use dicom_core::{Error, ErrorKind};
use dicom_web::dicomweb_status_for_error;

#[test]
fn http_status_contract_maps_auth_decode_to_forbidden() {
    // REQ-HI-287, REQ-HI-325, REQ-HI-379
    let denied = Error::from_kind(
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
    let not_found = Error::from_kind(
        ErrorKind::NotFound {
            detail: "not found".to_string(),
        },
        "missing",
    );
    assert_eq!(dicomweb_status_for_error(&not_found), (404, "Not Found"));
}
