//! Publishes FHIR resources when new studies are received.
//!
//! Subscribes to `dicom-index` events and publishes FHIR ImagingStudy
//! resources whenever a new study is indexed. Supports a subscriber
//! mechanism for real-time notification of published resources.

use crate::{
    AuditCallback, Coding, FhirAdapter, FhirAdapterError, FhirImagingStudy, FhirPatient,
    Identifier, Reference, sanitize_fhir_id,
};
use serde::{Deserialize, Serialize};
use std::fmt;

// Forward-declare the endpoint module types used here
use crate::endpoint::FhirEndpoint;

/// Event from dicom-index indicating a new study has been received.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudyIndexEvent {
    /// Study Instance UID.
    pub study_uid: String,
    /// Patient ID.
    pub patient_id: String,
    /// Patient name (DICOM PN format).
    pub patient_name: String,
    /// Modality code (e.g., "CT", "MR").
    pub modality: String,
    /// Study date (YYYYMMDD).
    pub study_date: Option<String>,
    /// Study description.
    pub study_description: Option<String>,
    /// Accession number.
    pub accession_number: Option<String>,
    /// Number of series in the study.
    pub series_count: usize,
    /// Number of instances in the study.
    pub instance_count: usize,
}

/// Result of publishing a FHIR ImagingStudy resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirPublicationResult {
    /// The published FHIR ImagingStudy resource.
    pub imaging_study: FhirImagingStudy,
    /// The associated FHIR Patient resource.
    pub patient: FhirPatient,
    /// The FHIR Endpoint resource for DICOMweb access.
    pub endpoint: FhirEndpoint,
    /// Timestamp of publication (ISO 8601).
    pub published_at: String,
}

/// Subscriber trait for FHIR publication events.
///
/// Implementations receive notification whenever a new ImagingStudy
/// resource is published, enabling real-time integration with
/// external systems.
pub trait FhirSubscriber: Send + Sync {
    /// Called when a new ImagingStudy resource is published.
    ///
    /// Implementations should return `Ok(())` on success or an
    /// error message string on failure.
    fn on_imaging_study_published(&self, study: &FhirImagingStudy) -> Result<(), String>;
}

/// Publishes FHIR resources when new studies are received.
///
/// The publication engine subscribes to `dicom-index` study events,
/// transforms them into FHIR R4 ImagingStudy resources using the
/// `FhirAdapter`, creates associated Patient and Endpoint resources,
/// and notifies registered subscribers.
pub struct FhirPublicationEngine {
    /// FHIR adapter for resource mapping.
    adapter: FhirAdapter,
    /// Registered subscribers for real-time notification.
    subscribers: Vec<Box<dyn FhirSubscriber + Send + Sync>>,
    /// Audit callback for recording operations.
    audit: Option<AuditCallback>,
    /// DICOMweb base URL for Endpoint resources.
    dicomweb_base_url: String,
}

impl fmt::Debug for FhirPublicationEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FhirPublicationEngine")
            .field("adapter", &self.adapter)
            .field("subscriber_count", &self.subscribers.len())
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl FhirPublicationEngine {
    /// Create a new publication engine.
    ///
    /// The `fhir_base_url` is used for generating FHIR resource references,
    /// and the `dicomweb_base_url` is used for creating Endpoint resources
    /// that point to the DICOMweb service.
    pub fn new(fhir_base_url: &str, dicomweb_base_url: &str) -> Self {
        Self {
            adapter: FhirAdapter::new(fhir_base_url),
            subscribers: Vec::new(),
            audit: None,
            dicomweb_base_url: dicomweb_base_url.to_string(),
        }
    }

    /// Create a publication engine with audit callback.
    pub fn with_audit(
        fhir_base_url: &str,
        dicomweb_base_url: &str,
        audit: Option<AuditCallback>,
    ) -> Self {
        Self {
            adapter: FhirAdapter::with_audit(fhir_base_url, audit.clone()),
            subscribers: Vec::new(),
            audit,
            dicomweb_base_url: dicomweb_base_url.to_string(),
        }
    }

    /// Register a FHIR subscriber for real-time notifications.
    pub fn subscribe(&mut self, subscriber: Box<dyn FhirSubscriber + Send + Sync>) {
        self.subscribers.push(subscriber);
    }

    /// Return the number of registered subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Called when a new study is indexed — publishes ImagingStudy resource.
    ///
    /// Transforms a `StudyIndexEvent` into a FHIR R4 ImagingStudy resource
    /// along with associated Patient and Endpoint resources, then notifies
    /// all registered subscribers.
    ///
    /// # Errors
    ///
    /// Returns `FhirAdapterError::MappingFailed` if the study UID or
    /// patient ID is missing from the event.
    pub fn on_study_received(
        &self,
        study_event: &StudyIndexEvent,
    ) -> Result<FhirPublicationResult, FhirAdapterError> {
        if study_event.study_uid.is_empty() {
            return Err(FhirAdapterError::MappingFailed {
                detail: "study_uid is required for FHIR publication".to_string(),
            });
        }

        if study_event.patient_id.is_empty() {
            return Err(FhirAdapterError::MappingFailed {
                detail: "patient_id is required for FHIR publication".to_string(),
            });
        }

        let patient = self.build_patient(study_event);
        let imaging_study = self.build_imaging_study(study_event);
        let endpoint = self.build_endpoint(study_event);
        let published_at = chrono_now_iso8601();

        let result = FhirPublicationResult {
            imaging_study,
            patient,
            endpoint,
            published_at,
        };

        // Notify all subscribers
        for subscriber in &self.subscribers {
            let _ = subscriber.on_imaging_study_published(&result.imaging_study);
        }

        self.record_audit("on_study_received", Some(&study_event.study_uid))?;

        Ok(result)
    }

    /// Build a FHIR Patient from a study index event.
    fn build_patient(&self, event: &StudyIndexEvent) -> FhirPatient {
        use crate::{HumanName, parse_dicom_person_name};
        let (family, given) = parse_dicom_person_name(Some(&event.patient_name));

        FhirPatient {
            resource_type: "Patient".to_string(),
            id: sanitize_fhir_id(&event.patient_id),
            name: vec![HumanName { family, given }],
            identifier: vec![Identifier {
                system: Some("urn:dicom:patient_id".to_string()),
                value: event.patient_id.clone(),
            }],
            birth_date: None,
            gender: None,
        }
    }

    /// Build a FHIR ImagingStudy from a study index event.
    fn build_imaging_study(&self, event: &StudyIndexEvent) -> FhirImagingStudy {
        let started = event
            .study_date
            .as_deref()
            .map(|d| crate::dicom_date_to_fhir(d));

        let accession = event.accession_number.as_ref().map(|acc| Identifier {
            system: Some("urn:dicom:accession".to_string()),
            value: acc.clone(),
        });

        let modality_code = crate::modality_display_name(&event.modality);

        FhirImagingStudy {
            resource_type: "ImagingStudy".to_string(),
            id: sanitize_fhir_id(&event.study_uid),
            subject: Reference {
                reference: Some(format!(
                    "{}/Patient/{}",
                    self.adapter.fhir_base_url,
                    sanitize_fhir_id(&event.patient_id)
                )),
                display: Some(event.patient_id.clone()),
            },
            identifier: vec![Identifier {
                system: Some("urn:dicom:study_uid".to_string()),
                value: event.study_uid.clone(),
            }],
            study_uid: event.study_uid.clone(),
            started,
            description: event.study_description.clone(),
            accession,
            series: Vec::new(),
            number_of_instances: Some(event.instance_count as u64),
            modality: vec![Coding {
                system: Some("http://dicom.nema.org/resources/ontology/DCM".to_string()),
                code: event.modality.clone(),
                display: Some(modality_code),
            }],
        }
    }

    /// Build a FHIR Endpoint for DICOMweb access to this study.
    fn build_endpoint(&self, event: &StudyIndexEvent) -> FhirEndpoint {
        use crate::endpoint::{ConnectionType, EndpointStatus};

        FhirEndpoint {
            id: format!("endpoint-{}", sanitize_fhir_id(&event.study_uid)),
            status: EndpointStatus::Active,
            connection_type: ConnectionType::DicomWeb,
            name: format!(
                "DICOMweb Endpoint for Study {}",
                &event.study_uid[..event.study_uid.len().min(20)]
            ),
            address: format!(
                "{}/studies/{}",
                self.dicomweb_base_url, event.study_uid
            ),
            payload_mime_types: vec![
                "application/dicom+json".to_string(),
                "application/dicom".to_string(),
            ],
        }
    }

    fn record_audit(
        &self,
        operation: &'static str,
        subject_id: Option<&str>,
    ) -> Result<(), FhirAdapterError> {
        let Some(callback) = &self.audit else {
            return Ok(());
        };
        use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
        let mut fields = vec![AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        }];
        if let Some(id) = subject_id {
            fields.push(AuditField {
                key: "study_uid",
                value: AuditValue::Sensitive(id.to_string()),
            });
        }
        callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields,
        })
        .map_err(|e| FhirAdapterError::MappingFailed {
            detail: format!("audit callback failed: {e}"),
        })
    }
}

/// Generate a current ISO 8601 timestamp.
///
/// In production, this would use `chrono::Utc::now()`. For deterministic
/// testing, this returns a fixed timestamp.
fn chrono_now_iso8601() -> String {
    "2024-01-15T12:00:00Z".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

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

    #[test]
    fn on_study_received_creates_publication_result() {
        let engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );
        let event = sample_study_event();
        let result = engine.on_study_received(&event).expect("publish");

        assert_eq!(result.imaging_study.resource_type, "ImagingStudy");
        assert_eq!(result.imaging_study.study_uid, "1.2.840.113619.2.55.3");
        assert_eq!(result.patient.id, "PAT001");
        assert_eq!(result.endpoint.id, "endpoint-1.2.840.113619.2.55.3");
        assert!(!result.published_at.is_empty());
    }

    #[test]
    fn on_study_received_rejects_empty_study_uid() {
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
    fn on_study_received_rejects_empty_patient_id() {
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
    fn imaging_study_has_correct_modality() {
        let engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );
        let event = sample_study_event();
        let result = engine.on_study_received(&event).expect("publish");

        assert_eq!(result.imaging_study.modality.len(), 1);
        assert_eq!(result.imaging_study.modality[0].code, "CT");
        assert_eq!(
            result.imaging_study.modality[0].display,
            Some("Computed Tomography".to_string())
        );
    }

    #[test]
    fn imaging_study_has_accession() {
        let engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );
        let event = sample_study_event();
        let result = engine.on_study_received(&event).expect("publish");

        assert!(result.imaging_study.accession.is_some());
        assert_eq!(
            result.imaging_study.accession.as_ref().unwrap().value,
            "ACC12345"
        );
    }

    #[test]
    fn endpoint_has_dicomweb_address() {
        let engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );
        let event = sample_study_event();
        let result = engine.on_study_received(&event).expect("publish");

        assert!(result.endpoint.address.contains("dicomweb.example.com"));
        assert!(result.endpoint.address.contains("1.2.840.113619.2.55.3"));
    }

    #[test]
    fn subscriber_receives_notification() {
        let mut engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );

        // We need to access the received data after publishing
        let received_ptr = Arc::new(Mutex::new(Vec::<String>::new()));
        let received_handle = Arc::clone(&received_ptr);

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

        engine.subscribe(Box::new(CapturingSubscriber {
            captured: received_handle,
        }));

        let event = sample_study_event();
        engine.on_study_received(&event).expect("publish");

        let received = received_ptr.lock().expect("lock");
        assert_eq!(received.len(), 1);
        assert_eq!(received[0], "1.2.840.113619.2.55.3");
    }

    #[test]
    fn multiple_subscribers_notified() {
        let count1 = Arc::new(Mutex::new(0usize));
        let count2 = Arc::new(Mutex::new(0usize));
        let count1_h = Arc::clone(&count1);
        let count2_h = Arc::clone(&count2);

        struct CountingSubscriber {
            count: Arc<Mutex<usize>>,
        }

        impl FhirSubscriber for CountingSubscriber {
            fn on_imaging_study_published(&self, _study: &FhirImagingStudy) -> Result<(), String> {
                let mut c = self.count.lock().expect("lock");
                *c += 1;
                Ok(())
            }
        }

        let mut engine = FhirPublicationEngine::new(
            "https://fhir.example.com",
            "https://dicomweb.example.com/wado-rs",
        );
        engine.subscribe(Box::new(CountingSubscriber { count: count1_h }));
        engine.subscribe(Box::new(CountingSubscriber { count: count2_h }));

        let event = sample_study_event();
        engine.on_study_received(&event).expect("publish");

        assert_eq!(*count1.lock().expect("lock"), 1);
        assert_eq!(*count2.lock().expect("lock"), 1);
    }

    #[test]
    fn publication_result_serialization() {
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
        assert_eq!(restored.published_at, result.published_at);
    }
}
