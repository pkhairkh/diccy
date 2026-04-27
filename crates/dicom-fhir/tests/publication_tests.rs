//! Integration tests for Sprint 15 FHIR publication.
//!
//! Tests cover:
//! - ImagingStudy publication on study receipt
//! - FHIR Endpoint creation
//! - Subscription management
//! - Subscriber notification
//! - DICOM-to-FHIR mapping tables

use dicom_fhir::*;
use dicom_fhir::endpoint::{ConnectionType, EndpointStatus, FhirEndpoint};
use dicom_fhir::publication::{FhirPublicationEngine, FhirPublicationResult, FhirSubscriber, StudyIndexEvent};
use dicom_fhir::subscription::{
    ChannelType, FhirSubscription, SubscriptionManager, SubscriptionStatus,
};
use dicom_fhir::mapping_tables::{
    modality_mapping, patient_mapping, study_mapping, sr_observation_mapping, mapping_summary,
};
use std::sync::{Arc, Mutex};

// --- Helper constructors ---

fn sample_study_event() -> StudyIndexEvent {
    StudyIndexEvent {
        study_uid: "1.2.840.113619.2.55.3".to_string(),
        patient_id: "PAT001".to_string(),
        patient_name: "Smith^John^M".to_string(),
        modality: "CT".to_string(),
        study_date: Some("20240115".to_string()),
        study_description: Some("CT Chest with Contrast".to_string()),
        accession_number: Some("ACC12345".to_string()),
        series_count: 3,
        instance_count: 512,
    }
}

fn sample_mr_study_event() -> StudyIndexEvent {
    StudyIndexEvent {
        study_uid: "1.2.840.113619.2.55.4".to_string(),
        patient_id: "PAT002".to_string(),
        patient_name: "Doe^Jane".to_string(),
        modality: "MR".to_string(),
        study_date: Some("20240220".to_string()),
        study_description: Some("MR Brain with Contrast".to_string()),
        accession_number: Some("ACC67890".to_string()),
        series_count: 5,
        instance_count: 1024,
    }
}

// --- Test: ImagingStudy publication on study receipt ---

#[test]
fn study_receipt_publishes_imaging_study() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = sample_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    assert_eq!(result.imaging_study.resource_type, "ImagingStudy");
    assert_eq!(result.imaging_study.study_uid, "1.2.840.113619.2.55.3");
    assert_eq!(
        result.imaging_study.started.as_deref(),
        Some("2024-01-15")
    );
    assert_eq!(
        result.imaging_study.description.as_deref(),
        Some("CT Chest with Contrast")
    );
    assert!(result.imaging_study.accession.is_some());
    assert_eq!(
        result.imaging_study.accession.as_ref().unwrap().value,
        "ACC12345"
    );
}

#[test]
fn study_receipt_creates_patient_resource() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = sample_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    assert_eq!(result.patient.resource_type, "Patient");
    assert_eq!(result.patient.id, "PAT001");
    assert_eq!(result.patient.name.len(), 1);
    assert_eq!(result.patient.name[0].family.as_deref(), Some("Smith"));
}

#[test]
fn study_receipt_rejects_empty_study_uid() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let mut event = sample_study_event();
    event.study_uid = String::new();

    let result = engine.on_study_received(&event);
    assert!(result.is_err());
}

#[test]
fn study_receipt_rejects_empty_patient_id() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let mut event = sample_study_event();
    event.patient_id = String::new();

    let result = engine.on_study_received(&event);
    assert!(result.is_err());
}

#[test]
fn study_receipt_with_mr_modality() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = sample_mr_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    assert_eq!(result.imaging_study.modality[0].code, "MR");
    assert_eq!(
        result.imaging_study.modality[0].display,
        Some("Magnetic Resonance".to_string())
    );
}

#[test]
fn study_receipt_without_optional_fields() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = StudyIndexEvent {
        study_uid: "1.2.3.4.5".to_string(),
        patient_id: "PAT001".to_string(),
        patient_name: "Smith^John".to_string(),
        modality: "CT".to_string(),
        study_date: None,
        study_description: None,
        accession_number: None,
        series_count: 0,
        instance_count: 0,
    };

    let result = engine.on_study_received(&event).expect("publish");
    assert!(result.imaging_study.started.is_none());
    assert!(result.imaging_study.description.is_none());
    assert!(result.imaging_study.accession.is_none());
}

// --- Test: FHIR Endpoint creation ---

#[test]
fn endpoint_created_for_study() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = sample_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    assert_eq!(result.endpoint.status, EndpointStatus::Active);
    assert_eq!(result.endpoint.connection_type, ConnectionType::DicomWeb);
    assert!(result.endpoint.address.contains("dicomweb.example.com"));
    assert!(result.endpoint.address.contains("1.2.840.113619.2.55.3"));
    assert!(result.endpoint.payload_mime_types.contains(&"application/dicom+json".to_string()));
}

#[test]
fn endpoint_direct_creation() {
    let ep = FhirEndpoint::new_dicomweb(
        "ep-test",
        "Test DICOMweb",
        "https://dicomweb.example.com/wado-rs",
    );
    assert_eq!(ep.id, "ep-test");
    assert_eq!(ep.status, EndpointStatus::Active);
    assert_eq!(ep.connection_type, ConnectionType::DicomWeb);
    assert!(ep.is_available());
    assert_eq!(ep.resource_type(), "Endpoint");
}

#[test]
fn endpoint_serialization_roundtrip() {
    let ep = FhirEndpoint::new_dicomweb(
        "ep-1",
        "Test",
        "https://example.com",
    );
    let json = serde_json::to_string(&ep).expect("serialize");
    let restored: FhirEndpoint = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.id, ep.id);
    assert_eq!(restored.status, ep.status);
}

#[test]
fn endpoint_status_variants() {
    assert!(!EndpointStatus::Suspended.to_string().is_empty());
    assert!(!EndpointStatus::Error.to_string().is_empty());
    assert!(!EndpointStatus::Off.to_string().is_empty());
}

// --- Test: Subscription management ---

#[test]
fn subscription_lifecycle() {
    let mut sub = FhirSubscription::new(
        "sub-1",
        "ImagingStudy?patient=PAT001",
        ChannelType::RestHook,
        "https://example.com/hook",
    );

    // Initially requested (not active)
    assert_eq!(sub.status, SubscriptionStatus::Requested);
    assert!(!sub.is_active());

    // Activate
    sub.activate();
    assert_eq!(sub.status, SubscriptionStatus::Active);
    assert!(sub.is_active());

    // Deactivate
    sub.deactivate();
    assert_eq!(sub.status, SubscriptionStatus::Off);
    assert!(!sub.is_active());
}

#[test]
fn subscription_criteria_matching() {
    let mut sub = FhirSubscription::new(
        "sub-1",
        "ImagingStudy",
        ChannelType::RestHook,
        "https://example.com/hook",
    );
    sub.activate();

    assert!(sub.matches("ImagingStudy", "any-id"));
    assert!(!sub.matches("Patient", "any-id"));
}

#[test]
fn subscription_id_matching() {
    let mut sub = FhirSubscription::new(
        "sub-1",
        "ImagingStudy?_id=study-123",
        ChannelType::RestHook,
        "https://example.com/hook",
    );
    sub.activate();

    assert!(sub.matches("ImagingStudy", "study-123"));
    assert!(!sub.matches("ImagingStudy", "study-456"));
}

#[test]
fn subscription_manager_add_remove() {
    let mut mgr = SubscriptionManager::new();
    assert!(mgr.is_empty());

    let sub1 = FhirSubscription::new(
        "sub-1",
        "ImagingStudy",
        ChannelType::RestHook,
        "https://example.com/hook1",
    );
    mgr.add_subscription(sub1);
    assert_eq!(mgr.len(), 1);

    let sub2 = FhirSubscription::new(
        "sub-2",
        "Patient",
        ChannelType::Websocket,
        "wss://example.com/ws",
    );
    mgr.add_subscription(sub2);
    assert_eq!(mgr.len(), 2);

    assert!(mgr.remove_subscription("sub-1"));
    assert_eq!(mgr.len(), 1);
    assert!(mgr.get("sub-1").is_none());
    assert!(mgr.get("sub-2").is_some());
}

// --- Test: Subscriber notification ---

struct CapturingSubscriber {
    captured: Arc<Mutex<Vec<String>>>,
}

impl FhirSubscriber for CapturingSubscriber {
    fn on_imaging_study_published(&self, study: &FhirImagingStudy) -> Result<(), String> {
        self.captured
            .lock()
            .expect("lock")
            .push(study.study_uid.clone());
        Ok(())
    }
}

#[test]
fn subscriber_notified_on_publication() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_handle = Arc::clone(&captured);

    let mut engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    engine.subscribe(Box::new(CapturingSubscriber {
        captured: captured_handle,
    }));

    let event = sample_study_event();
    engine.on_study_received(&event).expect("publish");

    let received = captured.lock().expect("lock");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], "1.2.840.113619.2.55.3");
}

#[test]
fn multiple_subscribers_notified() {
    let count1 = Arc::new(Mutex::new(0usize));
    let count2 = Arc::new(Mutex::new(0usize));
    let count1_h = Arc::clone(&count1);
    let count2_h = Arc::clone(&count2);

    struct CountingSub {
        count: Arc<Mutex<usize>>,
    }
    impl FhirSubscriber for CountingSub {
        fn on_imaging_study_published(&self, _: &FhirImagingStudy) -> Result<(), String> {
            *self.count.lock().expect("lock") += 1;
            Ok(())
        }
    }

    let mut engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    engine.subscribe(Box::new(CountingSub { count: count1_h }));
    engine.subscribe(Box::new(CountingSub { count: count2_h }));

    let event = sample_study_event();
    engine.on_study_received(&event).expect("publish");

    assert_eq!(*count1.lock().expect("lock"), 1);
    assert_eq!(*count2.lock().expect("lock"), 1);
    assert_eq!(engine.subscriber_count(), 2);
}

#[test]
fn subscription_manager_notify_matching() {
    let mut mgr = SubscriptionManager::new();
    let mut sub = FhirSubscription::new(
        "sub-1",
        "ImagingStudy",
        ChannelType::RestHook,
        "https://example.com/hook",
    );
    sub.activate();
    mgr.add_subscription(sub);

    let results = mgr.notify_subscribers("ImagingStudy", "study-123");
    assert_eq!(results.len(), 1);
    assert!(results[0].is_ok());
}

#[test]
fn subscription_manager_notify_no_match() {
    let mut mgr = SubscriptionManager::new();
    let mut sub = FhirSubscription::new(
        "sub-1",
        "ImagingStudy",
        ChannelType::RestHook,
        "https://example.com/hook",
    );
    sub.activate();
    mgr.add_subscription(sub);

    let results = mgr.notify_subscribers("Patient", "PAT001");
    assert!(results.is_empty());
}

// --- Test: DICOM-to-FHIR mapping tables ---

#[test]
fn modality_mapping_lookup() {
    assert_eq!(modality_mapping::display_for_code("CT"), Some("Computed Tomography"));
    assert_eq!(modality_mapping::display_for_code("MR"), Some("Magnetic Resonance"));
    assert_eq!(modality_mapping::display_for_code("US"), Some("Ultrasound"));
    assert_eq!(modality_mapping::display_for_code("UNKNOWN"), None);
}

#[test]
fn modality_code_system() {
    assert_eq!(
        modality_mapping::MODALITY_CODE_SYSTEM,
        "http://dicom.nema.org/resources/ontology/DCM"
    );
}

#[test]
fn patient_gender_mapping() {
    assert_eq!(patient_mapping::map_gender("M"), "male");
    assert_eq!(patient_mapping::map_gender("F"), "female");
    assert_eq!(patient_mapping::map_gender("O"), "other");
    assert_eq!(patient_mapping::map_gender("U"), "unknown");
}

#[test]
fn patient_date_mapping() {
    assert_eq!(patient_mapping::map_date("20240115"), "2024-01-15");
    assert_eq!(patient_mapping::map_date("19800101"), "1980-01-01");
}

#[test]
fn patient_name_mapping() {
    let (family, given) = patient_mapping::parse_person_name("Smith^John^M");
    assert_eq!(family, Some("Smith"));
    assert_eq!(given, vec!["John", "M"]);
}

#[test]
fn study_uid_to_fhir_id() {
    assert_eq!(
        study_mapping::study_uid_to_fhir_id("1.2.840.113619.2.55.3"),
        "1.2.840.113619.2.55.3"
    );
}

#[test]
fn sr_measurement_codes() {
    assert!(sr_observation_mapping::find_measurement("410668003").is_some());
    assert!(sr_observation_mapping::find_measurement("nonexistent").is_none());
}

#[test]
fn sr_unit_codes() {
    assert!(sr_observation_mapping::find_unit("mm").is_some());
    assert!(sr_observation_mapping::find_unit("deg").is_some());
}

#[test]
fn mapping_summary_consistency() {
    assert_eq!(mapping_summary::MODALITY_MAPPINGS, modality_mapping::MODALITY_MAP.len());
    assert_eq!(mapping_summary::PATIENT_MAPPINGS, patient_mapping::PATIENT_MAP.len());
    assert_eq!(mapping_summary::STUDY_MAPPINGS, study_mapping::STUDY_MAP.len());
}

// --- Test: Full end-to-end publication pipeline ---

#[test]
fn end_to_end_study_publication_with_subscription() {
    // Set up publication engine
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_handle = Arc::clone(&captured);

    let mut engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    engine.subscribe(Box::new(CapturingSubscriber {
        captured: captured_handle,
    }));

    // Simulate study received event (as if from dicom-index)
    let event = sample_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    // Verify ImagingStudy resource
    assert_eq!(result.imaging_study.study_uid, "1.2.840.113619.2.55.3");
    assert_eq!(result.imaging_study.modality[0].code, "CT");
    assert_eq!(result.patient.id, "PAT001");
    assert!(result.endpoint.address.contains("dicomweb"));

    // Verify subscriber was notified
    let received = captured.lock().expect("lock");
    assert_eq!(received.len(), 1);

    // Verify serialization
    let json = serde_json::to_string_pretty(&result).expect("serialize");
    let restored: FhirPublicationResult = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.imaging_study.study_uid, result.imaging_study.study_uid);
}

// --- Test: Publication result serialization ---

#[test]
fn publication_result_json_roundtrip() {
    let engine = FhirPublicationEngine::new(
        "https://fhir.example.com",
        "https://dicomweb.example.com/wado-rs",
    );
    let event = sample_study_event();
    let result = engine.on_study_received(&event).expect("publish");

    let json = serde_json::to_string(&result).expect("serialize");
    let restored: FhirPublicationResult =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.imaging_study.study_uid, result.imaging_study.study_uid);
    assert_eq!(restored.patient.id, result.patient.id);
    assert_eq!(restored.published_at, result.published_at);
}
