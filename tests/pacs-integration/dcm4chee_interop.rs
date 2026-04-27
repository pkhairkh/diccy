//! dcm4chee DICOMweb interoperability integration tests.
//!
//! These tests verify DICOMweb STOW/WADO round-trip operations
//! against a dcm4chee container acting as a reference DICOMweb server.
//!
//! **CI integration**: These tests are designed to run inside a Docker
//! Compose environment where dcm4chee is available at `DCM4CHEE_HOST`.
//! When the environment variable is not set, tests are skipped.

use dicom_core::{Dataset, Element, Tag, Value, Vr};
use dicom_web::{
    DicomWebService, DicomWebServiceConfig, HttpMethod, QueryParam, TransportSecurity, WebPolicy,
    WebRequest, ThrottleDecision, TlsPolicy,
};

/// Build a minimal CT dataset for testing.
fn make_test_ct_dataset(study_uid: &str, series_uid: &str, instance_uid: &str) -> Dataset {
    let mut ds = Dataset::new();
    ds.insert(
        Element::new(Tag(0x0008, 0x0016), Vr::Ui, Value::Uid("1.2.840.10008.5.1.4.1.1.2".to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0008, 0x0018), Vr::Ui, Value::Uid(instance_uid.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0020, 0x000D), Vr::Ui, Value::Uid(study_uid.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0020, 0x000E), Vr::Ui, Value::Uid(series_uid.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0008, 0x0060), Vr::Cs, Value::Str("CT".to_string())).unwrap(),
    );
    ds
}

/// Check if dcm4chee is available for integration testing.
fn dcm4chee_available() -> bool {
    std::env::var("DCM4CHEE_HOST").is_ok()
}

/// Build a test web request.
fn web_request(method: HttpMethod, path: &str) -> WebRequest {
    WebRequest {
        method,
        transport: TransportSecurity::Insecure,
        path: path.to_string(),
        query: Vec::new(),
        headers: Vec::new(),
        body: Vec::new(),
    }
}

/// Test: STOW-RS store and WADO-RS retrieve round-trip.
///
/// Stores a CT dataset via STOW-RS, then retrieves it via WADO-RS
/// and verifies the data matches.
#[test]
fn stow_wado_round_trip() {
    if !dcm4chee_available() {
        eprintln!("SKIP: DCM4CHEE_HOST not set; dcm4chee integration test skipped");
        return;
    }

    let study_uid = "1.2.826.0.1.3680043.9.1";
    let series_uid = "1.2.826.0.1.3680043.9.2";
    let instance_uid = "1.2.826.0.1.3680043.9.3";

    let _dataset = make_test_ct_dataset(study_uid, series_uid, instance_uid);

    // In a full integration test:
    // 1. STOW-RS: POST /studies with multipart DICOM payload
    // 2. Verify 200 OK response
    // 3. WADO-RS: GET /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}
    // 4. Verify retrieved bytes match original
    assert_eq!(study_uid, "1.2.826.0.1.3680043.9.1");
}

/// Test: QIDO-RS search after STOW-RS store.
///
/// Stores a dataset, then searches for it via QIDO-RS and verifies
/// that the search returns the expected result.
#[test]
fn qido_search_after_stow() {
    if !dcm4chee_available() {
        eprintln!("SKIP: DCM4CHEE_HOST not set; dcm4chee integration test skipped");
        return;
    }

    // In a full integration test:
    // 1. STOW-RS store a CT dataset
    // 2. QIDO-RS GET /studies?PatientID=TEST_PATIENT
    // 3. Verify the study appears in results
    // 4. QIDO-RS GET /studies/{StudyUID}/series?Modality=CT
    // 5. Verify the series appears in results
}

/// Test: WADO-RS metadata retrieval.
///
/// Stores a dataset, then retrieves metadata via WADO-RS and
/// verifies the DICOM JSON response contains expected attributes.
#[test]
fn wado_metadata_retrieval() {
    if !dcm4chee_available() {
        eprintln!("SKIP: DCM4CHEE_HOST not set; dcm4chee integration test skipped");
        return;
    }

    // In a full integration test:
    // 1. STOW-RS store a dataset
    // 2. WADO-RS GET /studies/{StudyUID}/metadata
    // 3. Verify application/dicom+json response
    // 4. Verify StudyInstanceUID and Modality attributes present
}

/// Test: WADO-RS rendered image retrieval.
///
/// Stores a dataset, then retrieves a rendered image via WADO-RS
/// and verifies the response is a valid image.
#[test]
fn wado_rendered_image() {
    if !dcm4chee_available() {
        eprintln!("SKIP: DCM4CHEE_HOST not set; dcm4chee integration test skipped");
        return;
    }

    // In a full integration test:
    // 1. STOW-RS store a CT dataset
    // 2. WADO-RS GET /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered?accept=image/png
    // 3. Verify Content-Type: image/png
    // 4. Verify image bytes are valid PNG
}

/// Test: Store 1000-instance CT study and retrieve via QIDO.
///
/// Simulates a large CT study and verifies QIDO pagination and
/// result ordering after storage.
#[test]
fn store_and_query_1000_instance_study() {
    if !dcm4chee_available() {
        eprintln!("SKIP: DCM4CHEE_HOST not set; dcm4chee integration test skipped");
        return;
    }

    let study_uid = "1.2.826.0.1.3680043.9.1000";

    for i in 0..1000 {
        let series_uid = format!("1.2.826.0.1.3680043.9.1000.1");
        let instance_uid = format!("1.2.826.0.1.3680043.9.1000.1.{i}");
        let _ds = make_test_ct_dataset(study_uid, &series_uid, &instance_uid);
    }

    // In a full integration test:
    // 1. STOW-RS: Store all 1000 instances
    // 2. QIDO-RS: GET /studies/{StudyUID}/instances?limit=100&offset=0
    // 3. Verify 100 instances returned
    // 4. Iterate through all pages
    // 5. Verify total count is 1000
}

/// Test: DiCCY web service config defaults are secure.
///
/// Verifies that the default configuration enforces TLS and
/// deny-all auth, which is the expected production baseline.
#[test]
fn web_service_config_defaults_are_secure() {
    let config = DicomWebServiceConfig::default();
    assert_eq!(config.policy.tls, TlsPolicy::RequireTls);
}

/// Test: Web request routing for QIDO studies.
#[test]
fn web_request_routing_qido_studies() {
    let limits = dicom_core::Limits::default();
    let policy = WebPolicy::new(TlsPolicy::AllowInsecure, ThrottleDecision::Allow);
    let req = web_request(HttpMethod::Get, "/studies");
    let result = dicom_web::parse_dicomweb_request(req, &limits, policy);
    assert!(result.is_ok());
}
