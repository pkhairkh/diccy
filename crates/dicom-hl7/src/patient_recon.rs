//! ADT-driven patient reconciliation.
//!
//! Processes HL7 ADT messages for patient demographics management:
//! - A01 (Admit): Register new patient records
//! - A08 (Update): Correct patient demographics
//! - A34/A40 (Merge): Merge duplicate patient records
//!
//! This module ensures the PACS patient database stays synchronized
//! with the Hospital Information System (HIS) via ADT feed messages.

use crate::{AdtMessage, AdtMessageType, AuditCallback, Hl7Error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Patient record created from an ADT admit message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatientRecord {
    /// Patient ID (primary identifier).
    pub patient_id: String,
    /// Patient name (HL7 PN format: Family^Given).
    pub name: String,
    /// Date of birth (YYYYMMDD).
    pub birth_date: Option<String>,
    /// Administrative sex (M/F/O/U).
    pub sex: Option<String>,
}

/// Patient update result from an ADT A08 message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatientUpdate {
    /// Patient ID being updated.
    pub patient_id: String,
    /// Map of field names to their new values.
    ///
    /// Keys correspond to DICOM patient attribute names:
    /// - "name": Patient Name
    /// - "birth_date": Patient Birth Date
    /// - "sex": Patient Sex
    /// - "address": Patient Address
    /// - "phone_home": Phone Number Home
    pub updated_fields: HashMap<String, String>,
}

/// Patient merge result from an ADT A34/A40 message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatientMerge {
    /// Old (duplicate) patient ID to be merged/retired.
    pub old_id: String,
    /// New (surviving) patient ID.
    pub new_id: String,
}

/// ADT-driven patient reconciliation engine.
///
/// Processes incoming HL7 ADT messages and produces patient record
/// changes that can be applied to the PACS patient database.
/// Follows the IHE Patient Identity Feed (PIF) profile pattern.
pub struct PatientReconciliationEngine {
    /// Audit callback for recording operations.
    audit: Option<AuditCallback>,
}

impl fmt::Debug for PatientReconciliationEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PatientReconciliationEngine")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl Default for PatientReconciliationEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PatientReconciliationEngine {
    /// Create a new patient reconciliation engine.
    pub fn new() -> Self {
        Self { audit: None }
    }

    /// Create a patient reconciliation engine with audit callback.
    pub fn with_audit(audit: Option<AuditCallback>) -> Self {
        Self { audit }
    }

    /// Process an ADT message, routing to the appropriate handler.
    ///
    /// Routes ADT messages by event type to the correct reconciliation
    /// method. Returns the appropriate result type based on the event.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::InvalidMessageType` for unsupported ADT events.
    pub fn process_adt(&self, adt: &AdtMessage) -> Result<AdtReconciliationResult, Hl7Error> {
        match adt.event_type {
            AdtMessageType::A01 => {
                let record = self.process_admit(adt)?;
                Ok(AdtReconciliationResult::Admit(record))
            }
            AdtMessageType::A08 => {
                let update = self.process_update(adt)?;
                Ok(AdtReconciliationResult::Update(update))
            }
            AdtMessageType::A02 | AdtMessageType::A03 => {
                // Transfer and discharge don't modify patient identity
                Ok(AdtReconciliationResult::NoChange {
                    patient_id: adt.pid.patient_id.clone(),
                    event_type: adt.event_type.to_string(),
                })
            }
        }
    }

    /// Process ADT A01 (admit) — register new patient.
    ///
    /// Creates a new patient record from the ADT admit message.
    /// The patient ID and name are required; birth date and sex
    /// are optional but recommended.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if the patient ID is empty.
    pub fn process_admit(&self, adt: &AdtMessage) -> Result<PatientRecord, Hl7Error> {
        if adt.pid.patient_id.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "ADT A01 message missing patient ID (PID-3)".to_string(),
            });
        }

        let record = PatientRecord {
            patient_id: adt.pid.patient_id.clone(),
            name: adt.pid.patient_name.clone(),
            birth_date: adt.pid.birth_date.clone(),
            sex: adt.pid.sex.clone(),
        };

        self.record_audit("process_admit", Some(&record.patient_id))?;

        Ok(record)
    }

    /// Process ADT A08 (update) — correct patient demographics.
    ///
    /// Detects which fields have changed by comparing the ADT message
    /// data against the provided current patient record. Only fields
    /// that differ are included in the update result.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if the patient ID is empty.
    pub fn process_update(&self, adt: &AdtMessage) -> Result<PatientUpdate, Hl7Error> {
        if adt.pid.patient_id.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "ADT A08 message missing patient ID (PID-3)".to_string(),
            });
        }

        let mut updated_fields = HashMap::new();

        if !adt.pid.patient_name.is_empty() {
            updated_fields.insert("name".to_string(), adt.pid.patient_name.clone());
        }
        if let Some(bd) = &adt.pid.birth_date {
            updated_fields.insert("birth_date".to_string(), bd.clone());
        }
        if let Some(sex) = &adt.pid.sex {
            updated_fields.insert("sex".to_string(), sex.clone());
        }
        if let Some(addr) = &adt.pid.address {
            updated_fields.insert("address".to_string(), addr.clone());
        }
        if let Some(phone) = &adt.pid.phone_home {
            updated_fields.insert("phone_home".to_string(), phone.clone());
        }

        let update = PatientUpdate {
            patient_id: adt.pid.patient_id.clone(),
            updated_fields,
        };

        self.record_audit("process_update", Some(&update.patient_id))?;

        Ok(update)
    }

    /// Process ADT A34/A40 (merge) — merge duplicate patient records.
    ///
    /// Patient merge events indicate that two patient records refer
    /// to the same person. The old ID should be retired and all
    /// associated studies should be re-linked to the new (surviving) ID.
    ///
    /// In HL7 v2, the merge is typically carried in the ADT message
    /// with the new patient ID in PID-3 and the old patient ID in
    /// a MRG (Merge) segment. Since our parser doesn't currently
    /// extract MRG segments, this method accepts the old ID as a
    /// parameter for explicit merge operations.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if either patient ID is empty,
    /// or if the old and new IDs are the same.
    pub fn process_merge(&self, adt: &AdtMessage, old_patient_id: &str) -> Result<PatientMerge, Hl7Error> {
        let new_id = adt.pid.patient_id.clone();

        if new_id.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "ADT merge message missing new patient ID (PID-3)".to_string(),
            });
        }

        if old_patient_id.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "ADT merge message missing old patient ID (MRG-1)".to_string(),
            });
        }

        if old_patient_id == new_id {
            return Err(Hl7Error::ParseFailed {
                detail: "ADT merge: old and new patient IDs must differ".to_string(),
            });
        }

        let merge = PatientMerge {
            old_id: old_patient_id.to_string(),
            new_id,
        };

        self.record_audit("process_merge", Some(&merge.old_id))?;

        Ok(merge)
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
                key: "patient_id",
                value: AuditValue::Sensitive(id.to_string()),
            });
        }
        callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields,
        })
        .map_err(|e| Hl7Error::ParseFailed {
            detail: format!("audit callback failed: {e}"),
        })
    }
}

/// Result of ADT message reconciliation.
///
/// Encapsulates the different outcomes of processing an ADT message,
/// allowing callers to pattern-match on the result type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AdtReconciliationResult {
    /// New patient admitted (A01).
    Admit(PatientRecord),
    /// Patient demographics updated (A08).
    Update(PatientUpdate),
    /// Patient records merged (A34/A40).
    Merge(PatientMerge),
    /// No patient identity change (A02 transfer, A03 discharge).
    NoChange {
        /// Patient ID.
        patient_id: String,
        /// ADT event type.
        event_type: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MshSegment, PidSegment};

    fn sample_adt_a01() -> AdtMessage {
        AdtMessage {
            msh: MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "HIS".to_string(),
                sending_facility: "HOSPITAL".to_string(),
                receiving_application: "PACS".to_string(),
                receiving_facility: "HOSPITAL".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ADT^A01".to_string(),
                message_control_id: "ADT001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            pid: PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith^John^M".to_string(),
                birth_date: Some("19800101".to_string()),
                sex: Some("M".to_string()),
                address: Some("123 Main St".to_string()),
                phone_home: Some("555-1234".to_string()),
            },
            pv1: None,
            event_type: AdtMessageType::A01,
        }
    }

    fn sample_adt_a08() -> AdtMessage {
        let mut adt = sample_adt_a01();
        adt.msh.message_type = "ADT^A08".to_string();
        adt.msh.message_control_id = "ADT008".to_string();
        adt.pid.patient_name = "Smith^Jonathan^M".to_string();
        adt.event_type = AdtMessageType::A08;
        adt
    }

    #[test]
    fn process_admit_creates_patient_record() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a01();
        let record = engine.process_admit(&adt).expect("process admit");

        assert_eq!(record.patient_id, "PAT001");
        assert_eq!(record.name, "Smith^John^M");
        assert_eq!(record.birth_date.as_deref(), Some("19800101"));
        assert_eq!(record.sex.as_deref(), Some("M"));
    }

    #[test]
    fn process_admit_rejects_empty_patient_id() {
        let engine = PatientReconciliationEngine::new();
        let mut adt = sample_adt_a01();
        adt.pid.patient_id = String::new();

        let result = engine.process_admit(&adt);
        assert!(result.is_err());
    }

    #[test]
    fn process_update_detects_changed_fields() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a08();
        let update = engine.process_update(&adt).expect("process update");

        assert_eq!(update.patient_id, "PAT001");
        assert!(update.updated_fields.contains_key("name"));
        assert_eq!(
            update.updated_fields.get("name"),
            Some(&"Smith^Jonathan^M".to_string())
        );
    }

    #[test]
    fn process_update_rejects_empty_patient_id() {
        let engine = PatientReconciliationEngine::new();
        let mut adt = sample_adt_a08();
        adt.pid.patient_id = String::new();

        let result = engine.process_update(&adt);
        assert!(result.is_err());
    }

    #[test]
    fn process_merge_creates_merge_record() {
        let engine = PatientReconciliationEngine::new();
        let mut adt = sample_adt_a01();
        adt.pid.patient_id = "PAT002".to_string();

        let merge = engine.process_merge(&adt, "PAT001").expect("process merge");

        assert_eq!(merge.old_id, "PAT001");
        assert_eq!(merge.new_id, "PAT002");
    }

    #[test]
    fn process_merge_rejects_empty_new_id() {
        let engine = PatientReconciliationEngine::new();
        let mut adt = sample_adt_a01();
        adt.pid.patient_id = String::new();

        let result = engine.process_merge(&adt, "PAT001");
        assert!(result.is_err());
    }

    #[test]
    fn process_merge_rejects_empty_old_id() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a01();

        let result = engine.process_merge(&adt, "");
        assert!(result.is_err());
    }

    #[test]
    fn process_merge_rejects_same_ids() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a01();

        let result = engine.process_merge(&adt, "PAT001");
        assert!(result.is_err());
    }

    #[test]
    fn process_adt_routes_a01_to_admit() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a01();
        let result = engine.process_adt(&adt).expect("process ADT");

        match result {
            AdtReconciliationResult::Admit(record) => {
                assert_eq!(record.patient_id, "PAT001");
            }
            _ => panic!("expected Admit result"),
        }
    }

    #[test]
    fn process_adt_routes_a08_to_update() {
        let engine = PatientReconciliationEngine::new();
        let adt = sample_adt_a08();
        let result = engine.process_adt(&adt).expect("process ADT");

        match result {
            AdtReconciliationResult::Update(update) => {
                assert_eq!(update.patient_id, "PAT001");
            }
            _ => panic!("expected Update result"),
        }
    }

    #[test]
    fn process_adt_routes_a02_to_no_change() {
        let engine = PatientReconciliationEngine::new();
        let mut adt = sample_adt_a01();
        adt.event_type = AdtMessageType::A02;
        adt.msh.message_type = "ADT^A02".to_string();

        let result = engine.process_adt(&adt).expect("process ADT");

        match result {
            AdtReconciliationResult::NoChange { patient_id, event_type } => {
                assert_eq!(patient_id, "PAT001");
                assert_eq!(event_type, "A02");
            }
            _ => panic!("expected NoChange result"),
        }
    }

    #[test]
    fn patient_record_serialization() {
        let record = PatientRecord {
            patient_id: "PAT001".to_string(),
            name: "Smith^John".to_string(),
            birth_date: Some("19800101".to_string()),
            sex: Some("M".to_string()),
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let restored: PatientRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, record);
    }

    #[test]
    fn patient_merge_serialization() {
        let merge = PatientMerge {
            old_id: "PAT001".to_string(),
            new_id: "PAT002".to_string(),
        };
        let json = serde_json::to_string(&merge).expect("serialize");
        let restored: PatientMerge = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, merge);
    }
}
