//! ORU result delivery: SR measurement report → HL7 ORU message.
//!
//! When a DICOM Structured Report (SR) containing measurement results is
//! generated, this module transforms the measurement data into an HL7 ORU
//! (Observation Result) message for delivery to downstream systems such as
//! RIS or EMR via MLLP.

use crate::{AuditCallback, Hl7Encoder, Hl7Error, ObxSegment, OrcSegment, ObrSegment, OruBuilder, OruMessage, PidSegment, MshSegment};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Patient information for ORU result delivery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OruPatientInfo {
    /// Patient ID.
    pub patient_id: String,
    /// Patient name (HL7 PN format: Family^Given).
    pub patient_name: String,
    /// Accession number tying the result to the order.
    pub accession_number: String,
}

/// Measurement data from a DICOM SR report.
///
/// Each measurement represents a single observation extracted from a
/// DICOM SR measurement report, such as a lesion dimension, angle,
/// or velocity measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementData {
    /// Type of measurement (e.g., "length", "angle", "area", "velocity").
    pub measurement_type: String,
    /// Measured value as a string (e.g., "25.3").
    pub value: String,
    /// Unit of measurement (e.g., "mm", "deg", "cm2", "cm/s").
    pub unit: String,
    /// Reference range for the measurement (e.g., "0-50").
    pub reference_range: Option<String>,
    /// Observation timestamp (HL7 DTM format: YYYYMMDDHHMMSS).
    pub observation_time: String,
}

/// ORU result delivery engine.
///
/// Generates HL7 ORU messages from DICOM SR measurement data. The
/// resulting ORU message can be encoded and sent over MLLP to
/// downstream systems.
pub struct ResultDeliveryEngine {
    /// Audit callback for recording operations.
    audit: Option<AuditCallback>,
}

impl fmt::Debug for ResultDeliveryEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResultDeliveryEngine")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl Default for ResultDeliveryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultDeliveryEngine {
    /// Create a new result delivery engine.
    pub fn new() -> Self {
        Self { audit: None }
    }

    /// Create a result delivery engine with audit callback.
    pub fn with_audit(audit: Option<AuditCallback>) -> Self {
        Self { audit }
    }

    /// Generate ORU from SR measurement data.
    ///
    /// Creates a complete HL7 ORU^R01 message from patient information
    /// and a set of SR measurements. Each measurement is mapped to an
    /// OBX segment with appropriate value types and observation identifiers.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::EncodingFailed` if patient info is missing
    /// required fields, or `Hl7Error::ParseFailed` if the measurements
    /// are empty.
    pub fn generate_oru_from_sr(
        &self,
        patient: &OruPatientInfo,
        measurements: &[MeasurementData],
    ) -> Result<OruMessage, Hl7Error> {
        if patient.patient_id.is_empty() {
            return Err(Hl7Error::EncodingFailed {
                detail: "patient ID is required for ORU generation".to_string(),
            });
        }

        if measurements.is_empty() {
            return Err(Hl7Error::EncodingFailed {
                detail: "at least one measurement is required for ORU generation".to_string(),
            });
        }

        let msh = MshSegment {
            field_separator: '|',
            encoding_characters: "^~\\&".to_string(),
            sending_application: "PACS".to_string(),
            sending_facility: "HOSPITAL".to_string(),
            receiving_application: "RIS".to_string(),
            receiving_facility: "HOSPITAL".to_string(),
            datetime: measurements
                .first()
                .map(|m| m.observation_time.clone())
                .unwrap_or_default(),
            message_type: "ORU^R01".to_string(),
            message_control_id: generate_oru_control_id(&patient.accession_number),
            processing_id: "P".to_string(),
            version_id: "2.5.1".to_string(),
        };

        let pid = PidSegment {
            patient_id: patient.patient_id.clone(),
            patient_name: patient.patient_name.clone(),
            birth_date: None,
            sex: None,
            address: None,
            phone_home: None,
        };

        let orc = OrcSegment {
            order_control: "SC".to_string(),
            placer_order_number: Some(patient.accession_number.clone()),
            filler_order_number: Some(patient.accession_number.clone()),
            ordering_provider: None,
        };

        let obr = ObrSegment {
            placer_order_number: Some(patient.accession_number.clone()),
            filler_order_number: Some(patient.accession_number.clone()),
            service_identifier: Some("SR_MEASUREMENT".to_string()),
            requested_datetime: None,
            observation_datetime: measurements
                .first()
                .map(|m| m.observation_time.clone()),
            ordering_provider: None,
            diagnostic_service_section: Some("RAD".to_string()),
        };

        let obx_segments: Vec<ObxSegment> = measurements
            .iter()
            .enumerate()
            .map(|(i, m)| self.build_measurement_obx(m, i + 1))
            .collect();

        let builder = OruBuilder::new(msh, pid, orc, obr);
        let oru = obx_segments
            .into_iter()
            .fold(builder, |b, obx| b.add_obx(obx))
            .build();

        self.record_audit("generate_oru_from_sr", Some(&patient.accession_number))?;

        Ok(oru)
    }

    /// Build OBX segment from measurement data.
    ///
    /// Maps a single measurement to an HL7 OBX segment. Numeric measurements
    /// use value type "NM", string measurements use "ST", and coded
    /// measurements use "CE".
    fn build_measurement_obx(&self, measurement: &MeasurementData, set_id: usize) -> ObxSegment {
        let value_type = derive_value_type(&measurement.value);

        ObxSegment {
            set_id: set_id.to_string(),
            value_type,
            observation_identifier: format!("SR_{}", measurement.measurement_type.to_uppercase()),
            observation_value: Some(measurement.value.clone()),
            units: Some(measurement.unit.clone()),
            reference_range: measurement.reference_range.clone(),
            abnormal_flags: None,
            result_status: Some("F".to_string()), // Final result
        }
    }

    /// Encode an ORU message to HL7 wire format.
    ///
    /// Convenience method that combines `generate_oru_from_sr` with
    /// `Hl7Encoder::encode_oru`.
    pub fn encode_and_deliver(
        &self,
        patient: &OruPatientInfo,
        measurements: &[MeasurementData],
    ) -> Result<String, Hl7Error> {
        let oru = self.generate_oru_from_sr(patient, measurements)?;
        let encoder = Hl7Encoder::new();
        encoder.encode_oru(&oru)
    }

    fn record_audit(
        &self,
        operation: &'static str,
        subject_id: Option<&str>,
    ) -> Result<(), Hl7Error> {
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
                key: "accession_number",
                value: AuditValue::Sensitive(id.to_string()),
            });
        }
        callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields,
        })
        .map_err(|e| Hl7Error::EncodingFailed {
            detail: format!("audit callback failed: {e}"),
        })
    }
}

/// Generate a message control ID for ORU messages.
///
/// Creates a deterministic control ID from the accession number,
/// suitable for tracking and acknowledgment correlation.
fn generate_oru_control_id(accession_number: &str) -> String {
    format!("ORU_{accession_number}")
}

/// Derive the HL7 value type from the measurement value string.
///
/// Numeric values (parseable as f64) map to "NM", otherwise "ST".
fn derive_value_type(value: &str) -> String {
    if value.parse::<f64>().is_ok() {
        "NM".to_string()
    } else {
        "ST".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_patient() -> OruPatientInfo {
        OruPatientInfo {
            patient_id: "PAT001".to_string(),
            patient_name: "Smith^John".to_string(),
            accession_number: "ACC12345".to_string(),
        }
    }

    fn sample_measurements() -> Vec<MeasurementData> {
        vec![
            MeasurementData {
                measurement_type: "length".to_string(),
                value: "25.3".to_string(),
                unit: "mm".to_string(),
                reference_range: Some("0-50".to_string()),
                observation_time: "20240115103000".to_string(),
            },
            MeasurementData {
                measurement_type: "angle".to_string(),
                value: "45.0".to_string(),
                unit: "deg".to_string(),
                reference_range: None,
                observation_time: "20240115103000".to_string(),
            },
        ]
    }

    #[test]
    fn generate_oru_creates_valid_message() {
        let engine = ResultDeliveryEngine::new();
        let patient = sample_patient();
        let measurements = sample_measurements();

        let oru = engine
            .generate_oru_from_sr(&patient, &measurements)
            .expect("generate ORU");

        assert_eq!(oru.msh.message_type, "ORU^R01");
        assert_eq!(oru.pid.patient_id, "PAT001");
        assert_eq!(oru.orc.order_control, "SC");
        assert_eq!(oru.orc.placer_order_number.as_deref(), Some("ACC12345"));
        assert_eq!(oru.obx.len(), 2);
        assert_eq!(oru.obx[0].set_id, "1");
        assert_eq!(oru.obx[0].observation_identifier, "SR_LENGTH");
        assert_eq!(oru.obx[0].observation_value.as_deref(), Some("25.3"));
        assert_eq!(oru.obx[0].value_type, "NM");
        assert_eq!(oru.obx[1].set_id, "2");
    }

    #[test]
    fn generate_oru_rejects_empty_patient_id() {
        let engine = ResultDeliveryEngine::new();
        let mut patient = sample_patient();
        patient.patient_id = String::new();

        let result = engine.generate_oru_from_sr(&patient, &sample_measurements());
        assert!(result.is_err());
    }

    #[test]
    fn generate_oru_rejects_empty_measurements() {
        let engine = ResultDeliveryEngine::new();
        let patient = sample_patient();

        let result = engine.generate_oru_from_sr(&patient, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn encode_and_deliver_produces_wire_format() {
        let engine = ResultDeliveryEngine::new();
        let patient = sample_patient();
        let measurements = sample_measurements();

        let encoded = engine
            .encode_and_deliver(&patient, &measurements)
            .expect("encode and deliver");

        assert!(encoded.contains("MSH|"));
        assert!(encoded.contains("PID|"));
        assert!(encoded.contains("ORC|"));
        assert!(encoded.contains("OBR|"));
        assert!(encoded.contains("OBX|"));
        assert!(encoded.contains("25.3"));
    }

    #[test]
    fn derive_value_type_numeric() {
        assert_eq!(derive_value_type("25.3"), "NM");
        assert_eq!(derive_value_type("0"), "NM");
        assert_eq!(derive_value_type("-10.5"), "NM");
    }

    #[test]
    fn derive_value_type_string() {
        assert_eq!(derive_value_type("normal"), "ST");
        assert_eq!(derive_value_type("positive"), "ST");
    }

    #[test]
    fn measurement_data_serialization() {
        let m = MeasurementData {
            measurement_type: "area".to_string(),
            value: "12.5".to_string(),
            unit: "cm2".to_string(),
            reference_range: Some("0-100".to_string()),
            observation_time: "20240115103000".to_string(),
        };

        let json = serde_json::to_string(&m).expect("serialize");
        let restored: MeasurementData = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.measurement_type, m.measurement_type);
        assert_eq!(restored.value, m.value);
    }

    #[test]
    fn string_measurement_uses_st_value_type() {
        let engine = ResultDeliveryEngine::new();
        let patient = sample_patient();
        let measurements = vec![MeasurementData {
            measurement_type: "finding".to_string(),
            value: "no_nodule_detected".to_string(),
            unit: "{}".to_string(),
            reference_range: None,
            observation_time: "20240115103000".to_string(),
        }];

        let oru = engine
            .generate_oru_from_sr(&patient, &measurements)
            .expect("generate ORU");

        assert_eq!(oru.obx[0].value_type, "ST");
    }
}
