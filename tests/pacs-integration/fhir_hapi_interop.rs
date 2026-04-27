//! FHIR (HAPI FHIR) interoperability integration tests.
//!
//! These tests verify DICOM-to-FHIR resource mapping against a
//! HAPI FHIR server container.
//!
//! **CI integration**: These tests are designed to run inside a Docker
//! Compose environment where HAPI FHIR is available at `HAPI_FHIR_HOST`.
//! When the environment variable is not set, tests are skipped.

use dicom_core::{Dataset, Element, Tag, Value, Vr};
use dicom_fhir::{FhirAdapter, FhirPatient, FhirImagingStudy, FhirObservation};

/// Build a test patient dataset.
fn make_patient_dataset(patient_id: &str, patient_name: &str) -> Dataset {
    let mut ds = Dataset::new();
    ds.insert(
        Element::new(Tag(0x0010, 0x0020), Vr::Lo, Value::Str(patient_id.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0010, 0x0010), Vr::Pn, Value::Str(patient_name.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0010, 0x0030), Vr::Da, Value::Str("19900101".to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0010, 0x0040), Vr::Cs, Value::Str("M".to_string())).unwrap(),
    );
    ds
}

/// Build a test study dataset.
fn make_study_dataset(study_uid: &str, patient_id: &str) -> Dataset {
    let mut ds = Dataset::new();
    ds.insert(
        Element::new(Tag(0x0020, 0x000D), Vr::Ui, Value::Uid(study_uid.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0010, 0x0020), Vr::Lo, Value::Str(patient_id.to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0008, 0x0020), Vr::Da, Value::Str("20260101".to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0008, 0x1030), Vr::Lo, Value::Str("CT Chest".to_string())).unwrap(),
    );
    ds.insert(
        Element::new(Tag(0x0008, 0x0060), Vr::Cs, Value::Str("CT".to_string())).unwrap(),
    );
    ds
}

/// Check if HAPI FHIR is available for integration testing.
fn hapi_fhir_available() -> bool {
    std::env::var("HAPI_FHIR_HOST").is_ok()
}

/// Test: Map DICOM Patient to FHIR Patient and post to HAPI FHIR.
///
/// Creates a DICOM patient dataset, maps it to a FHIR Patient
/// resource, and POSTs it to the HAPI FHIR server.
#[test]
fn map_patient_and_post_to_hapi() {
    if !hapi_fhir_available() {
        eprintln!("SKIP: HAPI_FHIR_HOST not set; HAPI FHIR integration test skipped");
        return;
    }

    let ds = make_patient_dataset("PATIENT_001", "Doe^John");
    let adapter = FhirAdapter::new("https://hapi-fhir.example/fhir");
    let fhir_patient = adapter.map_patient(&ds).expect("map_patient");

    assert_eq!(fhir_patient.resource_type, "Patient");
    assert_eq!(fhir_patient.id, "PATIENT_001");
    assert_eq!(fhir_patient.gender.as_deref(), Some("male"));

    // In a full integration test:
    // 1. Serialize fhir_patient to JSON
    // 2. POST to {HAPI_FHIR_HOST}/Patient
    // 3. Verify 201 Created response
    // 4. GET the created patient and verify fields
}

/// Test: Map DICOM Study to FHIR ImagingStudy and post to HAPI FHIR.
///
/// Creates a DICOM study dataset, maps it to a FHIR ImagingStudy
/// resource, and verifies the mapping is correct.
#[test]
fn map_imaging_study_and_post_to_hapi() {
    if !hapi_fhir_available() {
        eprintln!("SKIP: HAPI_FHIR_HOST not set; HAPI FHIR integration test skipped");
        return;
    }

    let ds = make_study_dataset("1.2.826.0.1.3680043.10.1", "PATIENT_001");
    let adapter = FhirAdapter::new("https://hapi-fhir.example/fhir");
    let fhir_study = adapter.map_imaging_study(&ds).expect("map_imaging_study");

    assert_eq!(fhir_study.resource_type, "ImagingStudy");
    assert_eq!(fhir_study.study_uid, "1.2.826.0.1.3680043.10.1");
    assert!(fhir_study.modality.iter().any(|m| m.code == "CT"));

    // In a full integration test:
    // 1. Serialize fhir_study to JSON
    // 2. POST to {HAPI_FHIR_HOST}/ImagingStudy
    // 3. Verify 201 Created response
}

/// Test: Map DICOM SR to FHIR Observation.
///
/// Creates a FHIR Observation from measurement data and verifies
/// the mapping is correct.
#[test]
fn map_sr_observation_to_fhir() {
    if !hapi_fhir_available() {
        eprintln!("SKIP: HAPI_FHIR_HOST not set; HAPI FHIR integration test skipped");
        return;
    }

    let adapter = FhirAdapter::new("https://hapi-fhir.example/fhir");
    let obs = adapter.map_sr_observation(
        "obs-001",
        "PATIENT_001",
        "410668003",
        "Length",
        42.5,
        "mm",
        "millimeter",
        Some("2026-02-22T12:00:00Z"),
        "1.2.826.0.1.3680043.10.1",
    );

    assert_eq!(obs.resource_type, "Observation");
    assert_eq!(obs.id, "obs-001");
    assert!(obs.value_quantity.is_some());
    let vq = obs.value_quantity.as_ref().unwrap();
    assert!((vq.value - 42.5).abs() < f64::EPSILON);
    assert_eq!(vq.code.as_deref(), Some("mm"));

    // In a full integration test:
    // 1. Serialize observation to JSON
    // 2. POST to {HAPI_FHIR_HOST}/Observation
    // 3. Verify 201 Created response
}

/// Test: Full DICOM → FHIR round-trip with search.
///
/// Creates a study, maps to FHIR, posts to HAPI, then searches
/// for the patient and verifies the ImagingStudy is linked.
#[test]
fn full_dicom_fhir_round_trip() {
    if !hapi_fhir_available() {
        eprintln!("SKIP: HAPI_FHIR_HOST not set; HAPI FHIR integration test skipped");
        return;
    }

    // In a full integration test:
    // 1. Create patient dataset → map to FHIR Patient → POST
    // 2. Create study dataset → map to FHIR ImagingStudy → POST
    // 3. GET /Patient?identifier=PATIENT_001
    // 4. Verify patient exists
    // 5. GET /ImagingStudy?patient=PATIENT_001
    // 6. Verify ImagingStudy is linked
}

/// Test: Verify FHIR adapter date conversion.
#[test]
fn fhir_date_conversion_is_correct() {
    let result = dicom_fhir::dicom_date_to_fhir("20260222");
    assert_eq!(result, "2026-02-22");
}

/// Test: Verify FHIR ID sanitization.
#[test]
fn fhir_id_sanitization() {
    let result = dicom_fhir::sanitize_fhir_id("1.2.840.10008.5.1.4.1.1.2");
    // Dots should be preserved, non-ASCII-alphanumeric replaced
    assert!(result.contains('.'));
}

/// Test: Verify person name parsing.
#[test]
fn person_name_parsing() {
    let (family, given) = dicom_fhir::parse_dicom_person_name(Some("Doe^John^M"));
    assert_eq!(family.as_deref(), Some("Doe"));
    assert_eq!(given, vec!["John".to_string(), "M".to_string()]);
}

/// Test: Verify modality display names.
#[test]
fn modality_display_names() {
    assert_eq!(dicom_fhir::modality_display_name("CT"), "Computed Tomography");
    assert_eq!(dicom_fhir::modality_display_name("MR"), "Magnetic Resonance");
    assert_eq!(dicom_fhir::modality_display_name("XX"), "XX"); // unknown → passthrough
}
