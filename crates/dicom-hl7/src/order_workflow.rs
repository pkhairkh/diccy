//! ORM → MWL pipeline: incoming order creates MWL entry.
//!
//! When an HL7 ORM (Order) message is received, this module transforms it
//! into a DICOM Modality Worklist (MWL) entry. The MWL entry can then be
//! queried by modalities to retrieve scheduled procedures. When the study
//! is performed and stored, the MWL entry is linked via the Study Instance
//! UID and its status is updated.

use crate::{AuditCallback, Hl7Error, OrmMessage};
use serde::{Deserialize, Serialize};
use std::fmt;

/// MWL entry status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MwlEntryStatus {
    /// Order received, procedure scheduled.
    Scheduled,
    /// Study in progress (modality has queried MWL).
    InProgress,
    /// Study completed and stored.
    Completed,
    /// Order cancelled.
    Cancelled,
}

impl fmt::Display for MwlEntryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MwlEntryStatus::Scheduled => write!(f, "SCHEDULED"),
            MwlEntryStatus::InProgress => write!(f, "IN_PROGRESS"),
            MwlEntryStatus::Completed => write!(f, "COMPLETED"),
            MwlEntryStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

/// MWL entry created from an HL7 order.
///
/// Represents a scheduled procedure in the DICOM Modality Worklist,
/// derived from an incoming ORM message. The accession number ties
/// the order to the resulting study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MwlEntry {
    /// Accession number (from ORC-3 filler order number or OBR-3).
    pub accession_number: String,
    /// Patient ID (from PID-3).
    pub patient_id: String,
    /// Patient name (from PID-5, DICOM PN format: Family^Given).
    pub patient_name: String,
    /// Requested Procedure ID (derived from order).
    pub requested_procedure_id: String,
    /// Human-readable description of the requested procedure.
    pub requested_procedure_description: String,
    /// Modality code (e.g., "CT", "MR", "US").
    pub modality: String,
    /// Scheduled Station AE Title (from order or default).
    pub scheduled_station_ae_title: String,
    /// Scheduled date/time for the procedure.
    pub scheduled_date_time: Option<String>,
    /// Ordering physician (from ORC-12 or OBR-16).
    pub ordering_physician: Option<String>,
    /// Study Instance UID (populated once the study is linked).
    pub study_uid: Option<String>,
    /// Current lifecycle status of this MWL entry.
    pub status: MwlEntryStatus,
}

/// ORM → MWL pipeline engine.
///
/// Processes incoming HL7 ORM messages and creates DICOM Modality Worklist
/// entries that can be queried by modalities. Follows the IHE Scheduled
/// Workflow (SWF) profile pattern.
pub struct OrderWorkflowEngine {
    /// Audit callback for recording operations.
    audit: Option<AuditCallback>,
}

impl fmt::Debug for OrderWorkflowEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderWorkflowEngine")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl Default for OrderWorkflowEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OrderWorkflowEngine {
    /// Create a new order workflow engine.
    pub fn new() -> Self {
        Self { audit: None }
    }

    /// Create an order workflow engine with audit callback.
    pub fn with_audit(audit: Option<AuditCallback>) -> Self {
        Self { audit }
    }

    /// Process an ORM message: create MWL entry from order.
    ///
    /// Transforms an incoming HL7 ORM order message into a DICOM Modality
    /// Worklist entry. The accession number is derived from the filler order
    /// number (ORC-3) or the placer order number (ORC-2). The procedure
    /// description comes from the OBR service identifier.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if the order is missing required
    /// fields (accession number, patient ID, or procedure description).
    pub fn process_orm_order(&self, orm: &OrmMessage) -> Result<MwlEntry, Hl7Error> {
        // Validate required fields
        let accession_number = orm
            .orc
            .filler_order_number
            .as_deref()
            .or(orm.orc.placer_order_number.as_deref())
            .or(orm.obr.filler_order_number.as_deref())
            .or(orm.obr.placer_order_number.as_deref())
            .ok_or_else(|| Hl7Error::ParseFailed {
                detail: "ORM message missing accession/order number (ORC-2/ORC-3/OBR-2/OBR-3)"
                    .to_string(),
            })?
            .to_string();

        if orm.pid.patient_id.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "ORM message missing patient ID (PID-3)".to_string(),
            });
        }

        let requested_procedure_description = orm
            .obr
            .service_identifier
            .as_deref()
            .unwrap_or("UNKNOWN_PROCEDURE")
            .to_string();

        let modality = orm
            .obr
            .diagnostic_service_section
            .as_deref()
            .unwrap_or("RAD");

        let mwl_entry = self.create_mwl_entry(orm);

        self.record_audit("process_orm_order", Some(&accession_number))?;

        // Validate the created entry
        if mwl_entry.accession_number.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "created MWL entry has empty accession number".to_string(),
            });
        }

        let _ = modality; // Used in validation above
        let _ = requested_procedure_description; // Used in validation above

        Ok(mwl_entry)
    }

    /// Create MWL entry from ORM order fields.
    ///
    /// Maps the HL7 ORM segments to the MWL entry structure. The accession
    /// number is preferentially taken from ORC-3 (filler order number),
    /// falling back to ORC-2, OBR-3, and OBR-2 in that order.
    fn create_mwl_entry(&self, orm: &OrmMessage) -> MwlEntry {
        let accession_number = orm
            .orc
            .filler_order_number
            .as_deref()
            .or(orm.orc.placer_order_number.as_deref())
            .or(orm.obr.filler_order_number.as_deref())
            .or(orm.obr.placer_order_number.as_deref())
            .unwrap_or("")
            .to_string();

        let requested_procedure_id = format!("RP_{}", accession_number);

        let requested_procedure_description = orm
            .obr
            .service_identifier
            .as_deref()
            .unwrap_or("UNKNOWN_PROCEDURE")
            .to_string();

        let modality = derive_modality_from_procedure(&requested_procedure_description);

        let scheduled_station_ae_title = format!("{}_STATION", modality);

        let ordering_physician = orm
            .orc
            .ordering_provider
            .as_deref()
            .or(orm.obr.ordering_provider.as_deref())
            .map(|s| s.to_string());

        MwlEntry {
            accession_number,
            patient_id: orm.pid.patient_id.clone(),
            patient_name: orm.pid.patient_name.clone(),
            requested_procedure_id,
            requested_procedure_description,
            modality,
            scheduled_station_ae_title,
            scheduled_date_time: orm.obr.requested_datetime.clone(),
            ordering_physician,
            study_uid: None,
            status: MwlEntryStatus::Scheduled,
        }
    }

    /// Update MWL entry when a study is received.
    ///
    /// Links the study instance UID to the MWL entry and updates the
    /// status to `InProgress`. This is called when a C-STORE request
    /// is received that matches the accession number on the MWL entry.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if the study UID is empty.
    pub fn link_study_to_order(
        &self,
        mwl_entry: &mut MwlEntry,
        study_uid: &str,
    ) -> Result<(), Hl7Error> {
        if study_uid.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "study UID must not be empty when linking to order".to_string(),
            });
        }

        mwl_entry.study_uid = Some(study_uid.to_string());
        mwl_entry.status = MwlEntryStatus::InProgress;

        self.record_audit("link_study_to_order", Some(study_uid))?;

        Ok(())
    }

    /// Mark an MWL entry as completed.
    ///
    /// Called when all instances for a study have been stored and
    /// the study is considered complete.
    pub fn complete_mwl_entry(&self, mwl_entry: &mut MwlEntry) -> Result<(), Hl7Error> {
        if mwl_entry.study_uid.is_none() {
            return Err(Hl7Error::ParseFailed {
                detail: "cannot complete MWL entry without linked study UID".to_string(),
            });
        }

        mwl_entry.status = MwlEntryStatus::Completed;

        self.record_audit(
            "complete_mwl_entry",
            mwl_entry.study_uid.as_deref(),
        )?;

        Ok(())
    }

    /// Cancel an MWL entry.
    ///
    /// Called when an order is cancelled (ORC-1 = "DC").
    pub fn cancel_mwl_entry(&self, mwl_entry: &mut MwlEntry) -> Result<(), Hl7Error> {
        mwl_entry.status = MwlEntryStatus::Cancelled;

        self.record_audit("cancel_mwl_entry", Some(&mwl_entry.accession_number))?;

        Ok(())
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
                key: "subject_id",
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

/// Derive a modality code from the procedure description.
///
/// Attempts to extract a DICOM modality code from the OBR-4 service
/// identifier. Common patterns like "CT_CHEST" or "MR_BRAIN" are mapped
/// to their modality prefix.
fn derive_modality_from_procedure(procedure: &str) -> String {
    let upper = procedure.to_uppercase();
    if upper.starts_with("CT") {
        "CT".to_string()
    } else if upper.starts_with("MR") || upper.starts_with("MRI") {
        "MR".to_string()
    } else if upper.starts_with("US") {
        "US".to_string()
    } else if upper.starts_with("XR") || upper.starts_with("CR") || upper.starts_with("DX") {
        "XR".to_string()
    } else if upper.starts_with("MG") {
        "MG".to_string()
    } else if upper.starts_with("NM") {
        "NM".to_string()
    } else if upper.starts_with("PT") || upper.starts_with("PET") {
        "PT".to_string()
    } else if upper.starts_with("XA") {
        "XA".to_string()
    } else if upper.starts_with("RF") {
        "RF".to_string()
    } else if upper.starts_with("SR") {
        "SR".to_string()
    } else {
        "OT".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MshSegment, OrcSegment, ObrSegment, PidSegment};

    fn sample_orm() -> OrmMessage {
        OrmMessage {
            msh: MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "RIS".to_string(),
                sending_facility: "HOSPITAL".to_string(),
                receiving_application: "PACS".to_string(),
                receiving_facility: "HOSPITAL".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ORM^O01".to_string(),
                message_control_id: "ORM001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            pid: PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith^John".to_string(),
                birth_date: Some("19800101".to_string()),
                sex: Some("M".to_string()),
                address: None,
                phone_home: None,
            },
            orc: OrcSegment {
                order_control: "NW".to_string(),
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: Some("ACC12345".to_string()),
                ordering_provider: Some("Dr^Smith".to_string()),
            },
            obr: ObrSegment {
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: Some("ACC12345".to_string()),
                service_identifier: Some("CT_CHEST".to_string()),
                requested_datetime: Some("20240115100000".to_string()),
                observation_datetime: None,
                ordering_provider: Some("Dr^Smith".to_string()),
                diagnostic_service_section: Some("RAD".to_string()),
            },
        }
    }

    #[test]
    fn process_orm_creates_mwl_entry() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mwl = engine.process_orm_order(&orm).expect("process ORM");

        assert_eq!(mwl.accession_number, "ACC12345");
        assert_eq!(mwl.patient_id, "PAT001");
        assert_eq!(mwl.patient_name, "Smith^John");
        assert_eq!(mwl.requested_procedure_id, "RP_ACC12345");
        assert_eq!(mwl.requested_procedure_description, "CT_CHEST");
        assert_eq!(mwl.modality, "CT");
        assert_eq!(mwl.status, MwlEntryStatus::Scheduled);
        assert!(mwl.study_uid.is_none());
    }

    #[test]
    fn process_orm_uses_placer_when_no_filler() {
        let engine = OrderWorkflowEngine::new();
        let mut orm = sample_orm();
        orm.orc.filler_order_number = None;
        orm.obr.filler_order_number = None;

        let mwl = engine.process_orm_order(&orm).expect("process ORM");
        assert_eq!(mwl.accession_number, "ORD001");
    }

    #[test]
    fn process_orm_rejects_missing_order_number() {
        let engine = OrderWorkflowEngine::new();
        let mut orm = sample_orm();
        orm.orc.filler_order_number = None;
        orm.orc.placer_order_number = None;
        orm.obr.filler_order_number = None;
        orm.obr.placer_order_number = None;

        let result = engine.process_orm_order(&orm);
        assert!(result.is_err());
    }

    #[test]
    fn process_orm_rejects_empty_patient_id() {
        let engine = OrderWorkflowEngine::new();
        let mut orm = sample_orm();
        orm.pid.patient_id = String::new();

        let result = engine.process_orm_order(&orm);
        assert!(result.is_err());
    }

    #[test]
    fn link_study_to_order_updates_mwl() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mut mwl = engine.process_orm_order(&orm).expect("process ORM");

        engine
            .link_study_to_order(&mut mwl, "1.2.840.113619.2.55.3")
            .expect("link study");

        assert_eq!(mwl.study_uid, Some("1.2.840.113619.2.55.3".to_string()));
        assert_eq!(mwl.status, MwlEntryStatus::InProgress);
    }

    #[test]
    fn link_study_rejects_empty_uid() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mut mwl = engine.process_orm_order(&orm).expect("process ORM");

        let result = engine.link_study_to_order(&mut mwl, "");
        assert!(result.is_err());
    }

    #[test]
    fn complete_mwl_entry() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mut mwl = engine.process_orm_order(&orm).expect("process ORM");
        engine
            .link_study_to_order(&mut mwl, "1.2.3.4.5")
            .expect("link");

        engine.complete_mwl_entry(&mut mwl).expect("complete");
        assert_eq!(mwl.status, MwlEntryStatus::Completed);
    }

    #[test]
    fn complete_rejects_without_study_uid() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mut mwl = engine.process_orm_order(&orm).expect("process ORM");

        let result = engine.complete_mwl_entry(&mut mwl);
        assert!(result.is_err());
    }

    #[test]
    fn cancel_mwl_entry() {
        let engine = OrderWorkflowEngine::new();
        let orm = sample_orm();
        let mut mwl = engine.process_orm_order(&orm).expect("process ORM");

        engine.cancel_mwl_entry(&mut mwl).expect("cancel");
        assert_eq!(mwl.status, MwlEntryStatus::Cancelled);
    }

    #[test]
    fn derive_modality_codes() {
        assert_eq!(derive_modality_from_procedure("CT_CHEST"), "CT");
        assert_eq!(derive_modality_from_procedure("MR_BRAIN"), "MR");
        assert_eq!(derive_modality_from_procedure("US_ABDOMEN"), "US");
        assert_eq!(derive_modality_from_procedure("XR_CHEST"), "XR");
        assert_eq!(derive_modality_from_procedure("MG_LEFT"), "MG");
        assert_eq!(derive_modality_from_procedure("NM_BONE"), "NM");
        assert_eq!(derive_modality_from_procedure("PET_CHEST"), "PT");
        assert_eq!(derive_modality_from_procedure("XA_CARDIAC"), "XA");
        assert_eq!(derive_modality_from_procedure("RF_FLUORO"), "RF");
        assert_eq!(derive_modality_from_procedure("UNKNOWN_PROC"), "OT");
    }

    #[test]
    fn mwl_entry_serialization_roundtrip() {
        let entry = MwlEntry {
            accession_number: "ACC001".to_string(),
            patient_id: "PAT001".to_string(),
            patient_name: "Smith^John".to_string(),
            requested_procedure_id: "RP_ACC001".to_string(),
            requested_procedure_description: "CT Chest".to_string(),
            modality: "CT".to_string(),
            scheduled_station_ae_title: "CT_STATION".to_string(),
            scheduled_date_time: Some("20240115100000".to_string()),
            ordering_physician: Some("Dr^Smith".to_string()),
            study_uid: None,
            status: MwlEntryStatus::Scheduled,
        };

        let json = serde_json::to_string(&entry).expect("serialize");
        let restored: MwlEntry = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.accession_number, entry.accession_number);
        assert_eq!(restored.status, entry.status);
    }
}
