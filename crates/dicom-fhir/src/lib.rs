#![deny(missing_docs)]

//! FHIR R4 adapter for DICOM-to-FHIR resource mapping.
//!
//! Maps DICOM Patient/Study/Series to FHIR Patient/ImagingStudy resources,
//! and DICOM SR measurement reports to FHIR Observations per the
//! HL7 DICOM-SR on FHIR Implementation Guide.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Error, ErrorKind, Result, Tag};
use serde::{Deserialize, Serialize};

use std::fmt;
use std::sync::Arc;

// --- DICOM Tag Constants ---

const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
const TAG_PATIENT_NAME: Tag = Tag(0x0010, 0x0010);
const TAG_PATIENT_BIRTH_DATE: Tag = Tag(0x0010, 0x0030);
const TAG_PATIENT_SEX: Tag = Tag(0x0010, 0x0040);
const TAG_STUDY_INSTANCE_UID: Tag = Tag(0x0020, 0x000D);
const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);
const TAG_STUDY_DESCRIPTION: Tag = Tag(0x0008, 0x1030);
const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
const TAG_SERIES_INSTANCE_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SERIES_DESCRIPTION: Tag = Tag(0x0008, 0x103E);
const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
const TAG_NUMBER_OF_SERIES_RELATED_INSTANCES: Tag = Tag(0x0020, 0x1209);

// --- FHIR Resource Types ---

/// FHIR R4 Patient resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirPatient {
    /// FHIR resource type (always "Patient").
    pub resource_type: String,
    /// Logical id of this artifact.
    pub id: String,
    /// A name associated with the patient.
    pub name: Vec<HumanName>,
    /// An identifier for this patient.
    pub identifier: Vec<Identifier>,
    /// The patient's date of birth (FHIR date format: YYYY-MM-DD).
    pub birth_date: Option<String>,
    /// Administrative gender.
    pub gender: Option<String>,
}

/// FHIR R4 HumanName type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HumanName {
    /// Family name (surname).
    pub family: Option<String>,
    /// Given names.
    pub given: Vec<String>,
}

/// FHIR R4 Identifier type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identifier {
    /// The namespace for the identifier value.
    pub system: Option<String>,
    /// The value that is unique.
    pub value: String,
}

/// FHIR R4 ImagingStudy resource.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirImagingStudy {
    /// FHIR resource type (always "ImagingStudy").
    pub resource_type: String,
    /// Logical id of this artifact.
    pub id: String,
    /// Who/what is the subject of the study.
    pub subject: Reference,
    /// DI globally unique identifier for the study.
    pub identifier: Vec<Identifier>,
    /// Study instance UID.
    pub study_uid: String,
    /// Date and time the study started.
    pub started: Option<String>,
    /// Description of the study.
    pub description: Option<String>,
    /// Accession Number.
    pub accession: Option<Identifier>,
    /// All series in the study.
    pub series: Vec<ImagingStudySeries>,
    /// Number of instances in the study.
    pub number_of_instances: Option<u64>,
    /// Modalities of the study.
    pub modality: Vec<Coding>,
}

/// FHIR R4 ImagingStudy.series backbone element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagingStudySeries {
    /// Numeric identifier of this series.
    pub uid: String,
    /// The modality of this series.
    pub modality: Coding,
    /// Description of the series.
    pub description: Option<String>,
    /// Number of instances in the series.
    pub number_of_instances: Option<u64>,
    /// Instances in this series.
    pub instance: Vec<ImagingStudyInstance>,
}

/// FHIR R4 ImagingStudy.instance backbone element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagingStudyInstance {
    /// SOP Class UID.
    pub sop_class: Coding,
    /// SOP Instance UID.
    pub uid: String,
}

/// FHIR R4 Coding type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coding {
    /// Identity of the terminology system.
    pub system: Option<String>,
    /// Symbol in syntax defined by the system.
    pub code: String,
    /// Representation defined by the system.
    pub display: Option<String>,
}

/// FHIR R4 Reference type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    /// Literal reference, Relative, internal or absolute URL.
    pub reference: Option<String>,
    /// Text alternative for the resource.
    pub display: Option<String>,
}

/// FHIR R4 Observation resource for SR measurement mapping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FhirObservation {
    /// FHIR resource type (always "Observation").
    pub resource_type: String,
    /// Logical id of this artifact.
    pub id: String,
    /// Classification of type of observation.
    pub category: Vec<CodeableConcept>,
    /// Type of observation (code / type).
    pub code: CodeableConcept,
    /// Who and/or what the observation is about.
    pub subject: Reference,
    /// Clinically relevant time/time-period for observation.
    pub effective_date_time: Option<String>,
    /// Result of the observation.
    pub value_quantity: Option<Quantity>,
    /// Additional result values.
    pub component: Vec<ObservationComponent>,
    /// DICOM SR source reference.
    pub derived_from: Vec<Reference>,
}

/// FHIR R4 Observation.component backbone element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservationComponent {
    /// Type of component observation (code / type).
    pub code: CodeableConcept,
    /// Result of the component observation.
    pub value_quantity: Option<Quantity>,
}

/// FHIR R4 CodeableConcept type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodeableConcept {
    /// Code defined by a terminology system.
    pub coding: Vec<Coding>,
    /// Plain text representation of the concept.
    pub text: Option<String>,
}

/// FHIR R4 Quantity type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    /// Numerical value (with implicit precision).
    pub value: f64,
    /// Unit representation.
    pub unit: Option<String>,
    /// System that defines coded unit form.
    pub system: Option<String>,
    /// Coded form of the unit.
    pub code: Option<String>,
}

// --- Error Types ---

/// FHIR adapter error type.
#[derive(Debug, Clone, PartialEq)]
pub enum FhirAdapterError {
    /// Required DICOM tag is missing.
    MissingTag {
        /// Missing tag identifier.
        tag: String,
    },
    /// DICOM tag has an invalid value.
    InvalidValue {
        /// Tag identifier with invalid value.
        tag: String,
        /// Validation failure detail.
        detail: String,
    },
    /// Mapping failed for a specific reason.
    MappingFailed {
        /// Failure detail.
        detail: String,
    },
    /// Serialization failure.
    SerializationFailed {
        /// Failure detail.
        detail: String,
    },
}

impl fmt::Display for FhirAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FhirAdapterError::MissingTag { tag } => write!(f, "missing required DICOM tag: {tag}"),
            FhirAdapterError::InvalidValue { tag, detail } => {
                write!(f, "invalid value for DICOM tag {tag}: {detail}")
            }
            FhirAdapterError::MappingFailed { detail } => {
                write!(f, "FHIR mapping failed: {detail}")
            }
            FhirAdapterError::SerializationFailed { detail } => {
                write!(f, "FHIR serialization failed: {detail}")
            }
        }
    }
}

impl std::error::Error for FhirAdapterError {}

/// Audit callback for FHIR adapter operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

// --- FHIR R4 Adapter ---

/// FHIR R4 adapter that maps DICOM resources to FHIR resources.
pub struct FhirAdapter {
    /// FHIR server base URL for generating references.
    pub fhir_base_url: String,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

impl fmt::Debug for FhirAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FhirAdapter")
            .field("fhir_base_url", &self.fhir_base_url)
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl FhirAdapter {
    /// Create a new FHIR adapter with a server base URL.
    pub fn new(fhir_base_url: &str) -> Self {
        Self {
            fhir_base_url: fhir_base_url.to_string(),
            audit: None,
        }
    }

    /// Create a FHIR adapter with audit callback.
    pub fn with_audit(fhir_base_url: &str, audit: Option<AuditCallback>) -> Self {
        Self {
            fhir_base_url: fhir_base_url.to_string(),
            audit,
        }
    }

    /// Map a DICOM Patient dataset to a FHIR Patient resource.
    ///
    /// Extracts patient demographics from the DICOM dataset and creates
    /// a FHIR R4 Patient resource with appropriate identifiers and names.
    pub fn map_patient(&self, dataset: &Dataset) -> Result<FhirPatient> {
        let patient_id = dataset
            .get_str(TAG_PATIENT_ID)
            .ok_or_else(|| missing_tag_error(TAG_PATIENT_ID))?;

        let patient_name = dataset.get_str(TAG_PATIENT_NAME);
        let (family, given) = parse_dicom_person_name(patient_name);

        let birth_date = dataset
            .get_str(TAG_PATIENT_BIRTH_DATE)
            .map(|d| dicom_date_to_fhir(d));

        let gender = dataset.get_str(TAG_PATIENT_SEX).map(|s| match s {
            "M" => "male".to_string(),
            "F" => "female".to_string(),
            "O" => "other".to_string(),
            "U" => "unknown".to_string(),
            _ => "unknown".to_string(),
        });

        let fhir_patient = FhirPatient {
            resource_type: "Patient".to_string(),
            id: sanitize_fhir_id(patient_id),
            name: vec![HumanName { family, given }],
            identifier: vec![Identifier {
                system: Some("urn:dicom:patient_id".to_string()),
                value: patient_id.to_string(),
            }],
            birth_date,
            gender,
        };

        self.record_audit("map_patient", Some(patient_id))?;
        Ok(fhir_patient)
    }

    /// Map a DICOM Study dataset to a FHIR ImagingStudy resource.
    ///
    /// Extracts study-level metadata and creates a FHIR R4 ImagingStudy
    /// resource. Series and instance information can be added separately.
    pub fn map_imaging_study(&self, dataset: &Dataset) -> Result<FhirImagingStudy> {
        let study_uid = dataset
            .get_uid(TAG_STUDY_INSTANCE_UID)
            .or_else(|| dataset.get_str(TAG_STUDY_INSTANCE_UID))
            .ok_or_else(|| missing_tag_error(TAG_STUDY_INSTANCE_UID))?;

        let patient_id = dataset.get_str(TAG_PATIENT_ID).unwrap_or("UNKNOWN");

        let started = dataset
            .get_str(TAG_STUDY_DATE)
            .map(|d| dicom_date_to_fhir(d));

        let description = dataset
            .get_str(TAG_STUDY_DESCRIPTION)
            .map(|s| s.to_string());

        let accession = dataset.get_str(TAG_ACCESSION_NUMBER).map(|acc| Identifier {
            system: Some("urn:dicom:accession".to_string()),
            value: acc.to_string(),
        });

        let modality_codes = extract_modalities(dataset);

        let study = FhirImagingStudy {
            resource_type: "ImagingStudy".to_string(),
            id: sanitize_fhir_id(study_uid),
            subject: Reference {
                reference: Some(format!(
                    "{}/Patient/{}",
                    self.fhir_base_url,
                    sanitize_fhir_id(patient_id)
                )),
                display: Some(patient_id.to_string()),
            },
            identifier: vec![Identifier {
                system: Some("urn:dicom:study_uid".to_string()),
                value: study_uid.to_string(),
            }],
            study_uid: study_uid.to_string(),
            started,
            description,
            accession,
            series: Vec::new(),
            number_of_instances: None,
            modality: modality_codes,
        };

        self.record_audit("map_imaging_study", Some(study_uid))?;
        Ok(study)
    }

    /// Add a DICOM Series dataset to an existing FHIR ImagingStudy.
    pub fn add_series_to_study(
        &self,
        study: &mut FhirImagingStudy,
        dataset: &Dataset,
    ) -> Result<()> {
        let series_uid = dataset
            .get_uid(TAG_SERIES_INSTANCE_UID)
            .or_else(|| dataset.get_str(TAG_SERIES_INSTANCE_UID))
            .ok_or_else(|| missing_tag_error(TAG_SERIES_INSTANCE_UID))?;

        let modality_code = dataset.get_str(TAG_MODALITY).unwrap_or("UNKNOWN");

        let description = dataset
            .get_str(TAG_SERIES_DESCRIPTION)
            .map(|s| s.to_string());

        let num_instances = dataset
            .get_i32(TAG_NUMBER_OF_SERIES_RELATED_INSTANCES)
            .map(|n| n as u64);

        let series = ImagingStudySeries {
            uid: series_uid.to_string(),
            modality: Coding {
                system: Some("http://dicom.nema.org/resources/ontology/DCM".to_string()),
                code: modality_code.to_string(),
                display: Some(modality_display_name(modality_code)),
            },
            description,
            number_of_instances: num_instances,
            instance: Vec::new(),
        };

        study.series.push(series);
        Ok(())
    }

    /// Add a DICOM Instance to a series within an existing FHIR ImagingStudy.
    pub fn add_instance_to_series(
        &self,
        study: &mut FhirImagingStudy,
        series_uid: &str,
        dataset: &Dataset,
    ) -> Result<()> {
        let sop_instance_uid = dataset
            .get_str(TAG_SOP_INSTANCE_UID)
            .or_else(|| dataset.get_uid(TAG_SOP_INSTANCE_UID).map(|s| s))
            .ok_or_else(|| missing_tag_error(TAG_SOP_INSTANCE_UID))?;

        let sop_class_uid = dataset
            .get_str(TAG_SOP_CLASS_UID)
            .or_else(|| dataset.get_uid(TAG_SOP_CLASS_UID).map(|s| s))
            .unwrap_or("1.2.840.10008.5.1.4.1.1.2");

        let instance = ImagingStudyInstance {
            sop_class: Coding {
                system: Some("urn:ietf:rfc:3986".to_string()),
                code: format!("urn:oid:{}", sop_class_uid),
                display: None,
            },
            uid: sop_instance_uid.to_string(),
        };

        let series = study
            .series
            .iter_mut()
            .find(|s| s.uid == series_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::DecodeError {
                        stage: "dicom-fhir".to_string(),
                        detail: format!("series not found: {series_uid}"),
                    },
                    "series not found",
                )
            })?;

        series.instance.push(instance);
        Ok(())
    }

    /// Create a FHIR Observation from a DICOM SR measurement report.
    ///
    /// Maps measurement data (distance, angle, probe) from DICOM SR
    /// to a FHIR R4 Observation resource with appropriate codings
    /// from SNOMED CT and UCUM.
    pub fn map_sr_observation(
        &self,
        observation_id: &str,
        patient_id: &str,
        measurement_code: &str,
        measurement_display: &str,
        value: f64,
        unit_code: &str,
        unit_display: &str,
        effective_datetime: Option<&str>,
        sr_reference_uid: &str,
    ) -> FhirObservation {
        FhirObservation {
            resource_type: "Observation".to_string(),
            id: observation_id.to_string(),
            category: vec![CodeableConcept {
                coding: vec![Coding {
                    system: Some(
                        "http://terminology.hl7.org/CodeSystem/observation-category".to_string(),
                    ),
                    code: "imaging".to_string(),
                    display: Some("Imaging".to_string()),
                }],
                text: Some("Imaging".to_string()),
            }],
            code: CodeableConcept {
                coding: vec![Coding {
                    system: Some("http://snomed.info/sct".to_string()),
                    code: measurement_code.to_string(),
                    display: Some(measurement_display.to_string()),
                }],
                text: Some(measurement_display.to_string()),
            },
            subject: Reference {
                reference: Some(format!(
                    "{}/Patient/{}",
                    self.fhir_base_url,
                    sanitize_fhir_id(patient_id)
                )),
                display: Some(patient_id.to_string()),
            },
            effective_date_time: effective_datetime.map(|s| s.to_string()),
            value_quantity: Some(Quantity {
                value,
                unit: Some(unit_display.to_string()),
                system: Some("http://unitsofmeasure.org".to_string()),
                code: Some(unit_code.to_string()),
            }),
            component: Vec::new(),
            derived_from: vec![Reference {
                reference: Some(format!(
                    "{}/ImagingStudy/{}",
                    self.fhir_base_url,
                    sanitize_fhir_id(sr_reference_uid)
                )),
                display: None,
            }],
        }
    }

    /// Serialize a FHIR resource to JSON.
    pub fn to_json<T: Serialize>(&self, resource: &T) -> Result<String> {
        serde_json::to_string(resource).map_err(|e| {
            Error::from_kind(
                ErrorKind::DecodeError {
                    stage: "dicom-fhir".to_string(),
                    detail: format!("JSON serialization failed: {e}"),
                },
                "serialization failed",
            )
            .into()
        })
    }

    /// Serialize a FHIR resource to pretty-printed JSON.
    pub fn to_json_pretty<T: Serialize>(&self, resource: &T) -> Result<String> {
        serde_json::to_string_pretty(resource).map_err(|e| {
            Error::from_kind(
                ErrorKind::DecodeError {
                    stage: "dicom-fhir".to_string(),
                    detail: format!("JSON serialization failed: {e}"),
                },
                "serialization failed",
            )
            .into()
        })
    }

    fn record_audit(&self, operation: &'static str, subject_id: Option<&str>) -> Result<()> {
        let Some(callback) = &self.audit else {
            return Ok(());
        };
        let mut fields = vec![AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        }];
        if let Some(id) = subject_id {
            fields.push(AuditField {
                key: "subject_id",
                value: AuditValue::Sensitive(id.to_string()),
            });
        }
        callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields,
        })
    }
}

// --- Helper Functions ---

/// Parse a DICOM Person Name (PN) value into family and given names.
///
/// DICOM PN format: "Family^Given^Middle^Prefix^Suffix"
pub fn parse_dicom_person_name(pn: Option<&str>) -> (Option<String>, Vec<String>) {
    match pn {
        None => (None, Vec::new()),
        Some(name) => {
            let parts: Vec<&str> = name.split('^').collect();
            let family = parts.first().and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_string())
                }
            });
            let given = parts
                .iter()
                .skip(1)
                .filter_map(|s| {
                    if s.is_empty() {
                        None
                    } else {
                        Some(s.to_string())
                    }
                })
                .collect();
            (family, given)
        }
    }
}

/// Convert a DICOM DA (Date) value to FHIR date format.
///
/// DICOM DA: "YYYYMMDD" -> FHIR: "YYYY-MM-DD"
pub fn dicom_date_to_fhir(dicom_date: &str) -> String {
    if dicom_date.len() >= 8 {
        format!(
            "{}-{}-{}",
            &dicom_date[0..4],
            &dicom_date[4..6],
            &dicom_date[6..8]
        )
    } else {
        dicom_date.to_string()
    }
}

/// Sanitize a string for use as a FHIR id.
///
/// FHIR ids must contain only ASCII alphanumeric characters, hyphens, and dots.
pub fn sanitize_fhir_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Get a human-readable display name for a DICOM modality code.
pub fn modality_display_name(code: &str) -> String {
    match code {
        "CT" => "Computed Tomography",
        "MR" => "Magnetic Resonance",
        "MG" => "Mammography",
        "US" => "Ultrasound",
        "XR" => "X-Ray",
        "NM" => "Nuclear Medicine",
        "PT" => "PET",
        "XA" => "X-Ray Angiography",
        "RF" => "Radio Fluoroscopy",
        "SR" => "Structured Report",
        _ => code,
    }
    .to_string()
}

/// Extract modality codes from a DICOM dataset.
fn extract_modalities(dataset: &Dataset) -> Vec<Coding> {
    let mut modalities = Vec::new();

    // Try to get modality from the dataset
    if let Some(modality) = dataset.get_str(TAG_MODALITY) {
        modalities.push(Coding {
            system: Some("http://dicom.nema.org/resources/ontology/DCM".to_string()),
            code: modality.to_string(),
            display: Some(modality_display_name(modality)),
        });
    }

    modalities
}

fn missing_tag_error(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required DICOM tag",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Element, Limits, Value, Vr};

    fn limits() -> Limits {
        Limits::default()
    }

    fn patient_dataset() -> Dataset {
        let mut ds = Dataset::new();
        ds.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap());
        ds.insert(
            Element::new(
                TAG_PATIENT_NAME,
                Vr::Pn,
                Value::Str("Smith^John^M".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_PATIENT_BIRTH_DATE,
                Vr::Da,
                Value::Str("19800101".to_string()),
            )
            .unwrap(),
        );
        ds.insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("M".to_string())).unwrap());
        ds
    }

    fn study_dataset() -> Dataset {
        let mut ds = Dataset::new();
        ds.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap());
        ds.insert(
            Element::new(
                TAG_STUDY_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.840.113619.2.55.3".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20240115".to_string())).unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_STUDY_DESCRIPTION,
                Vr::Lo,
                Value::Str("CT Chest with Contrast".to_string()),
            )
            .unwrap(),
        );
        ds.insert(
            Element::new(
                TAG_ACCESSION_NUMBER,
                Vr::Sh,
                Value::Str("ACC12345".to_string()),
            )
            .unwrap(),
        );
        ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        ds
    }

    #[test]
    fn map_patient_extracts_demographics() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient mapping");

        assert_eq!(patient.resource_type, "Patient");
        assert_eq!(patient.id, "PAT001");
        assert_eq!(patient.name.len(), 1);
        assert_eq!(patient.name[0].family.as_deref(), Some("Smith"));
        assert_eq!(patient.name[0].given, vec!["John", "M"]);
        assert_eq!(patient.birth_date.as_deref(), Some("1980-01-01"));
        assert_eq!(patient.gender.as_deref(), Some("male"));
        assert_eq!(patient.identifier.len(), 1);
        assert_eq!(patient.identifier[0].value, "PAT001");
    }

    #[test]
    fn map_patient_missing_id_fails() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = Dataset::new();
        let result = adapter.map_patient(&ds);
        assert!(result.is_err());
    }

    #[test]
    fn map_imaging_study_extracts_study_metadata() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let study = adapter.map_imaging_study(&ds).expect("study mapping");

        assert_eq!(study.resource_type, "ImagingStudy");
        assert_eq!(study.study_uid, "1.2.840.113619.2.55.3");
        assert_eq!(study.started.as_deref(), Some("2024-01-15"));
        assert_eq!(study.description.as_deref(), Some("CT Chest with Contrast"));
        assert!(study.accession.is_some());
        assert_eq!(study.modality.len(), 1);
        assert_eq!(study.modality[0].code, "CT");
    }

    #[test]
    fn map_imaging_study_missing_uid_fails() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = Dataset::new();
        let result = adapter.map_imaging_study(&ds);
        assert!(result.is_err());
    }

    #[test]
    fn add_series_to_study() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let mut study = adapter.map_imaging_study(&ds).expect("study");

        let mut series_ds = Dataset::new();
        series_ds.insert(
            Element::new(
                TAG_SERIES_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.840.113619.2.55.3.1".to_string()),
            )
            .unwrap(),
        );
        series_ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        series_ds.insert(
            Element::new(
                TAG_SERIES_DESCRIPTION,
                Vr::Lo,
                Value::Str("Axial 5mm".to_string()),
            )
            .unwrap(),
        );

        adapter
            .add_series_to_study(&mut study, &series_ds)
            .expect("add series");
        assert_eq!(study.series.len(), 1);
        assert_eq!(study.series[0].uid, "1.2.840.113619.2.55.3.1");
        assert_eq!(study.series[0].modality.code, "CT");
    }

    #[test]
    fn add_instance_to_series() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let mut study = adapter.map_imaging_study(&ds).expect("study");

        // Add series first
        let mut series_ds = Dataset::new();
        series_ds.insert(
            Element::new(
                TAG_SERIES_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.3.4.5".to_string()),
            )
            .unwrap(),
        );
        series_ds.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
        adapter
            .add_series_to_study(&mut study, &series_ds)
            .expect("add series");

        // Add instance
        let mut instance_ds = Dataset::new();
        instance_ds.insert(
            Element::new(
                TAG_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Str("1.2.3.4.5.6".to_string()),
            )
            .unwrap(),
        );
        instance_ds.insert(
            Element::new(
                TAG_SOP_CLASS_UID,
                Vr::Ui,
                Value::Str("1.2.840.10008.5.1.4.1.1.2".to_string()),
            )
            .unwrap(),
        );

        adapter
            .add_instance_to_series(&mut study, "1.2.3.4.5", &instance_ds)
            .expect("add instance");
        assert_eq!(study.series[0].instance.len(), 1);
        assert_eq!(study.series[0].instance[0].uid, "1.2.3.4.5.6");
    }

    #[test]
    fn map_sr_observation_creates_fhir_observation() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let obs = adapter.map_sr_observation(
            "obs-001",
            "PAT001",
            "410668003",
            "Length of lesion",
            25.3,
            "mm",
            "millimeter",
            Some("2024-01-15T10:30:00Z"),
            "1.2.840.113619.2.55.3",
        );

        assert_eq!(obs.resource_type, "Observation");
        assert_eq!(obs.id, "obs-001");
        assert_eq!(obs.category.len(), 1);
        assert_eq!(obs.category[0].coding[0].code, "imaging");
        assert_eq!(obs.code.coding[0].code, "410668003");
        assert!(obs.value_quantity.is_some());
        let qty = obs.value_quantity.unwrap();
        assert!((qty.value - 25.3).abs() < 0.01);
        assert_eq!(qty.code.as_deref(), Some("mm"));
        assert_eq!(obs.derived_from.len(), 1);
    }

    #[test]
    fn serialize_patient_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient");
        let json = adapter.to_json(&patient).expect("json");
        assert!(json.contains("\"resource_type\":\"Patient\""));
        assert!(json.contains("\"id\":\"PAT001\""));
    }

    #[test]
    fn serialize_imaging_study_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = study_dataset();
        let study = adapter.map_imaging_study(&ds).expect("study");
        let json = adapter.to_json(&study).expect("json");
        assert!(json.contains("\"resource_type\":\"ImagingStudy\""));
    }

    #[test]
    fn serialize_observation_to_json() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let obs = adapter.map_sr_observation(
            "obs-001",
            "PAT001",
            "410668003",
            "Length of lesion",
            25.3,
            "mm",
            "millimeter",
            None,
            "1.2.3.4.5",
        );
        let json = adapter.to_json(&obs).expect("json");
        assert!(json.contains("\"resource_type\":\"Observation\""));
    }

    #[test]
    fn parse_person_name_with_components() {
        let (family, given) = parse_dicom_person_name(Some("Smith^John^M"));
        assert_eq!(family.as_deref(), Some("Smith"));
        assert_eq!(given, vec!["John", "M"]);
    }

    #[test]
    fn parse_person_name_family_only() {
        let (family, given) = parse_dicom_person_name(Some("Smith"));
        assert_eq!(family.as_deref(), Some("Smith"));
        assert!(given.is_empty());
    }

    #[test]
    fn parse_person_name_none() {
        let (family, given) = parse_dicom_person_name(None);
        assert!(family.is_none());
        assert!(given.is_empty());
    }

    #[test]
    fn dicom_date_conversion() {
        assert_eq!(dicom_date_to_fhir("20240115"), "2024-01-15");
        assert_eq!(dicom_date_to_fhir("19800101"), "1980-01-01");
    }

    #[test]
    fn sanitize_fhir_id_replaces_invalid_chars() {
        assert_eq!(sanitize_fhir_id("1.2.840.113619"), "1.2.840.113619");
        assert_eq!(
            sanitize_fhir_id("patient id with spaces"),
            "patient_id_with_spaces"
        );
    }

    #[test]
    fn modality_display_names() {
        assert_eq!(modality_display_name("CT"), "Computed Tomography");
        assert_eq!(modality_display_name("MR"), "Magnetic Resonance");
        assert_eq!(modality_display_name("MG"), "Mammography");
        assert_eq!(modality_display_name("US"), "Ultrasound");
        assert_eq!(modality_display_name("UNKNOWN"), "UNKNOWN");
    }

    #[test]
    fn gender_mapping() {
        let adapter = FhirAdapter::new("https://fhir.example.com");

        let mut male_ds = Dataset::new();
        male_ds.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT001".to_string())).unwrap(),
        );
        male_ds.insert(
            Element::new(
                TAG_PATIENT_NAME,
                Vr::Pn,
                Value::Str("Smith^John".to_string()),
            )
            .unwrap(),
        );
        male_ds.insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("M".to_string())).unwrap());
        let male = adapter.map_patient(&male_ds).expect("male");
        assert_eq!(male.gender.as_deref(), Some("male"));

        let mut female_ds = Dataset::new();
        female_ds.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PAT002".to_string())).unwrap(),
        );
        female_ds.insert(
            Element::new(TAG_PATIENT_NAME, Vr::Pn, Value::Str("Doe^Jane".to_string())).unwrap(),
        );
        female_ds
            .insert(Element::new(TAG_PATIENT_SEX, Vr::Cs, Value::Str("F".to_string())).unwrap());
        let female = adapter.map_patient(&female_ds).expect("female");
        assert_eq!(female.gender.as_deref(), Some("female"));
    }

    #[test]
    fn audit_callback_emits_events() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let adapter = FhirAdapter::with_audit("https://fhir.example.com", Some(audit));
        let ds = patient_dataset();
        adapter.map_patient(&ds).expect("patient");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|f| f.key == "operation"
            && matches!(&f.value, AuditValue::Plain(v) if v == "map_patient")));
    }

    #[test]
    fn roundtrip_json_patient() {
        let adapter = FhirAdapter::new("https://fhir.example.com");
        let ds = patient_dataset();
        let patient = adapter.map_patient(&ds).expect("patient");
        let json = adapter.to_json_pretty(&patient).expect("json pretty");

        // Verify it can be deserialized back
        let restored: FhirPatient = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, patient.id);
        assert_eq!(restored.name.len(), patient.name.len());
    }
}
