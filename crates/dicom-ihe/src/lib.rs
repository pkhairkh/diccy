#![deny(missing_docs)]

//! IHE (Integrating the Healthcare Enterprise) integration profiles.
//!
//! Provides:
//! - **S8-T3**: IHE Scheduled Workflow (SWF), Patient Information Reconciliation (PIR),
//!   Access to Radiology Information (ARI), Cross-enterprise Document Sharing (XDS-I.b),
//!   and AI Results (AIR) profile implementations.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S8-T3: IHE Integration Profiles
// ===========================================================================

/// IHE integration profile identifiers.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum IheProfile {
    /// Scheduled Workflow (SWF) — order, procedure, workitem, result.
    Swf,
    /// Patient Information Reconciliation (PIR) — match unidentified patients.
    Pir,
    /// Access to Radiology Information (ARI) — query across enterprises.
    Ari,
    /// Cross-enterprise Document Sharing - Imaging (XDS-I.b).
    XdsIb,
    /// AI Results (AIR) — AI findings exchange profile.
    Air,
}

impl IheProfile {
    /// Return the IHE profile name.
    pub fn name(&self) -> &str {
        match self {
            IheProfile::Swf => "Scheduled Workflow",
            IheProfile::Pir => "Patient Information Reconciliation",
            IheProfile::Ari => "Access to Radiology Information",
            IheProfile::XdsIb => "XDS-I.b",
            IheProfile::Air => "AI Results",
        }
    }

    /// Return the IHE domain for this profile.
    pub fn domain(&self) -> &str {
        match self {
            IheProfile::Swf | IheProfile::Pir | IheProfile::Ari => "Radiology",
            IheProfile::XdsIb => "IT Infrastructure",
            IheProfile::Air => "Radiology",
        }
    }
}

// ===========================================================================
// IHE SWF: Scheduled Workflow
// ===========================================================================

/// Order entry from the HIS/RIS for the SWF profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwfOrder {
    /// Accession number.
    pub accession_number: String,
    /// Ordered procedure code.
    pub procedure_code: String,
    /// Procedure description.
    pub procedure_description: String,
    /// Requesting physician.
    pub requesting_physician: String,
    /// Patient ID.
    pub patient_id: String,
    /// Patient name.
    pub patient_name: String,
    /// Order entry timestamp.
    pub order_timestamp: u64,
    /// Requested procedure priority (STAT, HIGH, ROUTINE, LOW).
    pub priority: String,
}

impl SwfOrder {
    /// Create a new SWF order.
    pub fn new(accession_number: &str, patient_id: &str, procedure_code: &str) -> Self {
        Self {
            accession_number: accession_number.to_string(),
            procedure_code: procedure_code.to_string(),
            procedure_description: String::new(),
            requesting_physician: String::new(),
            patient_id: patient_id.to_string(),
            patient_name: String::new(),
            order_timestamp: 0,
            priority: "ROUTINE".to_string(),
        }
    }
}

/// Workitem (UPS - Unified Procedure Step) in the SWF pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SwfWorkitemState {
    /// Order received, not yet scheduled.
    Scheduled,
    /// Workitem in progress.
    InProgress,
    /// Workitem completed.
    Completed,
    /// Workitem canceled.
    Canceled,
}

/// SWF workitem tracking an order through the radiology workflow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwfWorkitem {
    /// Workitem UID.
    pub workitem_uid: String,
    /// Associated accession number.
    pub accession_number: String,
    /// Current state.
    pub state: SwfWorkitemState,
    /// Assigned performer (technologist/radiologist).
    pub performer: String,
    /// Scheduled start time.
    pub scheduled_start: u64,
    /// Actual start time.
    pub actual_start: Option<u64>,
    /// Actual completion time.
    pub actual_end: Option<u64>,
}

/// Scheduled Workflow engine managing orders through the radiology pipeline.
pub struct SwfEngine {
    /// Pending orders awaiting scheduling.
    orders: BTreeMap<String, SwfOrder>,
    /// Active workitems.
    workitems: BTreeMap<String, SwfWorkitem>,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

/// Audit callback for IHE operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

impl fmt::Debug for SwfEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwfEngine")
            .field("orders", &self.orders.len())
            .field("workitems", &self.workitems.len())
            .finish()
    }
}

impl Default for SwfEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SwfEngine {
    /// Create a new SWF engine.
    pub fn new() -> Self {
        Self {
            orders: BTreeMap::new(),
            workitems: BTreeMap::new(),
            audit: None,
        }
    }

    /// Set the audit callback.
    pub fn set_audit(&mut self, audit: Option<AuditCallback>) {
        self.audit = audit;
    }

    /// Submit a new order to the SWF pipeline.
    pub fn submit_order(&mut self, order: SwfOrder) -> Result<String> {
        if order.accession_number.is_empty() {
            return Err(ihe_error("accession number must not be empty"));
        }
        if order.patient_id.is_empty() {
            return Err(ihe_error("patient ID must not be empty"));
        }

        let accession = order.accession_number.clone();
        self.orders.insert(accession.clone(), order);
        self.record_audit("submit_order", &accession);
        Ok(accession)
    }

    /// Schedule a workitem for an order.
    pub fn schedule_workitem(&mut self, accession_number: &str, performer: &str, scheduled_start: u64) -> Result<String> {
        if !self.orders.contains_key(accession_number) {
            return Err(ihe_error("order not found for accession number"));
        }

        let workitem_uid = format!("1.2.840.113619.6.swf.{}", self.workitems.len());
        let workitem = SwfWorkitem {
            workitem_uid: workitem_uid.clone(),
            accession_number: accession_number.to_string(),
            state: SwfWorkitemState::Scheduled,
            performer: performer.to_string(),
            scheduled_start,
            actual_start: None,
            actual_end: None,
        };

        self.workitems.insert(workitem_uid.clone(), workitem);
        self.record_audit("schedule_workitem", accession_number);
        Ok(workitem_uid)
    }

    /// Start a workitem (transition to InProgress).
    pub fn start_workitem(&mut self, workitem_uid: &str, actual_start: u64) -> Result<()> {
        let workitem = self.workitems.get_mut(workitem_uid)
            .ok_or_else(|| ihe_error("workitem not found"))?;

        if !matches!(workitem.state, SwfWorkitemState::Scheduled) {
            return Err(ihe_error("workitem must be in Scheduled state to start"));
        }

        workitem.state = SwfWorkitemState::InProgress;
        workitem.actual_start = Some(actual_start);
        self.record_audit("start_workitem", workitem_uid);
        Ok(())
    }

    /// Complete a workitem (transition to Completed).
    pub fn complete_workitem(&mut self, workitem_uid: &str, actual_end: u64) -> Result<()> {
        let workitem = self.workitems.get_mut(workitem_uid)
            .ok_or_else(|| ihe_error("workitem not found"))?;

        if !matches!(workitem.state, SwfWorkitemState::InProgress) {
            return Err(ihe_error("workitem must be in InProgress state to complete"));
        }

        workitem.state = SwfWorkitemState::Completed;
        workitem.actual_end = Some(actual_end);
        self.record_audit("complete_workitem", workitem_uid);
        Ok(())
    }

    /// Cancel a workitem.
    pub fn cancel_workitem(&mut self, workitem_uid: &str) -> Result<()> {
        let workitem = self.workitems.get_mut(workitem_uid)
            .ok_or_else(|| ihe_error("workitem not found"))?;

        workitem.state = SwfWorkitemState::Canceled;
        self.record_audit("cancel_workitem", workitem_uid);
        Ok(())
    }

    /// Get a workitem by UID.
    pub fn get_workitem(&self, workitem_uid: &str) -> Option<&SwfWorkitem> {
        self.workitems.get(workitem_uid)
    }

    /// Get the order for an accession number.
    pub fn get_order(&self, accession_number: &str) -> Option<&SwfOrder> {
        self.orders.get(accession_number)
    }

    /// Return all workitems.
    pub fn workitems(&self) -> Vec<&SwfWorkitem> {
        self.workitems.values().collect()
    }

    /// Return all orders.
    pub fn orders(&self) -> Vec<&SwfOrder> {
        self.orders.values().collect()
    }

    fn record_audit(&self, operation: &'static str, subject: &str) {
        let Some(callback) = &self.audit else {
            return;
        };
        let _ = callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields: vec![
                AuditField {
                    key: "profile",
                    value: AuditValue::Plain("SWF".to_string()),
                },
                AuditField {
                    key: "operation",
                    value: AuditValue::Plain(operation.to_string()),
                },
                AuditField {
                    key: "subject",
                    value: AuditValue::Sensitive(subject.to_string()),
                },
            ],
        });
    }
}

// ===========================================================================
// IHE PIR: Patient Information Reconciliation
// ===========================================================================

/// Unidentified patient record for PIR processing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnidentifiedPatient {
    /// Temporary patient ID assigned at the time of study.
    pub temporary_id: String,
    /// Study Instance UID performed on this unidentified patient.
    pub study_uid: String,
    /// Study description.
    pub study_description: String,
    /// Study date.
    pub study_date: String,
    /// Matching confidence score (0.0-1.0).
    pub confidence: f64,
}

/// Reconciled patient identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReconciledPatient {
    /// Confirmed patient ID.
    pub patient_id: String,
    /// Confirmed patient name.
    pub patient_name: String,
    /// Birth date.
    pub birth_date: String,
    /// Sex.
    pub sex: String,
    /// Study UIDs that were reconciled.
    pub reconciled_study_uids: Vec<String>,
}

/// PIR engine for matching unidentified patients.
pub struct PirEngine {
    /// Unidentified patients awaiting reconciliation.
    unidentified: BTreeMap<String, UnidentifiedPatient>,
    /// Reconciled patient records.
    reconciled: Vec<ReconciledPatient>,
}

impl Default for PirEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PirEngine {
    /// Create a new PIR engine.
    pub fn new() -> Self {
        Self {
            unidentified: BTreeMap::new(),
            reconciled: Vec::new(),
        }
    }

    /// Register an unidentified patient.
    pub fn register_unidentified(&mut self, patient: UnidentifiedPatient) -> Result<()> {
        if patient.temporary_id.is_empty() {
            return Err(ihe_error("temporary patient ID must not be empty"));
        }
        self.unidentified.insert(patient.temporary_id.clone(), patient);
        Ok(())
    }

    /// Reconcile an unidentified patient with a known identity.
    pub fn reconcile(&mut self, temporary_id: &str, patient_id: &str, patient_name: &str, birth_date: &str, sex: &str) -> Result<()> {
        let unidentified = self.unidentified.remove(temporary_id)
            .ok_or_else(|| ihe_error("unidentified patient not found"))?;

        let reconciled = ReconciledPatient {
            patient_id: patient_id.to_string(),
            patient_name: patient_name.to_string(),
            birth_date: birth_date.to_string(),
            sex: sex.to_string(),
            reconciled_study_uids: vec![unidentified.study_uid.clone()],
        };

        self.reconciled.push(reconciled);
        Ok(())
    }

    /// Return the number of unidentified patients.
    pub fn unidentified_count(&self) -> usize {
        self.unidentified.len()
    }

    /// Return the number of reconciled patients.
    pub fn reconciled_count(&self) -> usize {
        self.reconciled.len()
    }

    /// Get an unidentified patient by temporary ID.
    pub fn get_unidentified(&self, temporary_id: &str) -> Option<&UnidentifiedPatient> {
        self.unidentified.get(temporary_id)
    }
}

// ===========================================================================
// IHE XDS-I.b: Cross-enterprise Document Sharing
// ===========================================================================

/// XDS document entry metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XdsDocumentEntry {
    /// Unique document entry identifier.
    pub entry_uuid: String,
    /// Document unique ID.
    pub unique_id: String,
    /// Patient ID.
    pub patient_id: String,
    /// Class code (e.g., "DICOM Study").
    pub class_code: String,
    /// Type code.
    pub type_code: String,
    /// Healthcare facility type code.
    pub facility_code: String,
    /// Practice setting code.
    pub practice_setting: String,
    /// Repository unique ID where the document is stored.
    pub repository_uid: String,
    /// Availability status.
    pub available: bool,
}

/// XDS-I.b registry for cross-enterprise document sharing.
pub struct XdsRegistry {
    /// Document entries indexed by entry UUID.
    entries: BTreeMap<String, XdsDocumentEntry>,
    /// Patient ID to document entries index.
    patient_index: BTreeMap<String, Vec<String>>,
}

impl Default for XdsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl XdsRegistry {
    /// Create a new XDS registry.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            patient_index: BTreeMap::new(),
        }
    }

    /// Register a document entry in the XDS registry.
    pub fn register(&mut self, entry: XdsDocumentEntry) -> Result<()> {
        if entry.entry_uuid.is_empty() {
            return Err(ihe_error("entry UUID must not be empty"));
        }
        if entry.patient_id.is_empty() {
            return Err(ihe_error("patient ID must not be empty"));
        }

        let uuid = entry.entry_uuid.clone();
        let patient_id = entry.patient_id.clone();
        self.entries.insert(uuid.clone(), entry);
        self.patient_index.entry(patient_id).or_default().push(uuid);
        Ok(())
    }

    /// Query documents for a patient.
    pub fn query_by_patient(&self, patient_id: &str) -> Vec<&XdsDocumentEntry> {
        self.patient_index
            .get(patient_id)
            .map(|uuids| uuids.iter().filter_map(|u| self.entries.get(u)).collect())
            .unwrap_or_default()
    }

    /// Get a document entry by UUID.
    pub fn get_entry(&self, entry_uuid: &str) -> Option<&XdsDocumentEntry> {
        self.entries.get(entry_uuid)
    }

    /// Return the total number of registered documents.
    pub fn document_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove a document entry (for document retirement).
    pub fn remove(&mut self, entry_uuid: &str) -> Option<XdsDocumentEntry> {
        let entry = self.entries.remove(entry_uuid)?;
        if let Some(uuids) = self.patient_index.get_mut(&entry.patient_id) {
            uuids.retain(|u| u != entry_uuid);
        }
        Some(entry)
    }
}

// ===========================================================================
// IHE AIR: AI Results Profile
// ===========================================================================

/// AIR finding exchange record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AirFinding {
    /// Finding UID.
    pub finding_uid: String,
    /// Study Instance UID.
    pub study_uid: String,
    /// Finding type code (SNOMED CT).
    pub finding_type_code: String,
    /// Finding confidence (0.0-1.0).
    pub confidence: f64,
    /// Algorithm name.
    pub algorithm_name: String,
    /// Algorithm version.
    pub algorithm_version: String,
    /// Algorithm UID.
    pub algorithm_uid: String,
}

/// AIR profile exchange engine.
pub struct AirExchange {
    /// Findings indexed by UID.
    findings: BTreeMap<String, AirFinding>,
    /// Study UID to finding UIDs index.
    study_index: BTreeMap<String, Vec<String>>,
}

impl Default for AirExchange {
    fn default() -> Self {
        Self::new()
    }
}

impl AirExchange {
    /// Create a new AIR exchange engine.
    pub fn new() -> Self {
        Self {
            findings: BTreeMap::new(),
            study_index: BTreeMap::new(),
        }
    }

    /// Submit an AI finding.
    pub fn submit_finding(&mut self, finding: AirFinding) -> Result<()> {
        if finding.finding_uid.is_empty() {
            return Err(ihe_error("finding UID must not be empty"));
        }
        let uid = finding.finding_uid.clone();
        let study_uid = finding.study_uid.clone();
        self.findings.insert(uid.clone(), finding);
        self.study_index.entry(study_uid).or_default().push(uid);
        Ok(())
    }

    /// Query findings for a study.
    pub fn query_by_study(&self, study_uid: &str) -> Vec<&AirFinding> {
        self.study_index
            .get(study_uid)
            .map(|uids| uids.iter().filter_map(|u| self.findings.get(u)).collect())
            .unwrap_or_default()
    }

    /// Return the total number of findings.
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn ihe_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-ihe".to_string(),
            detail: detail.into(),
        },
        "IHE error",
    )
    .into()
}

// ===========================================================================
// Tests: S8-T3 IHE Integration Profiles (minimum 15)
// ===========================================================================

#[cfg(test)]
mod tests_ihe {
    use super::*;

    // --- SWF Tests ---

    #[test]
    fn swf_submit_order() {
        let mut engine = SwfEngine::new();
        let order = SwfOrder::new("ACC-001", "P001", "CT-CHEST");
        let acc = engine.submit_order(order).unwrap();
        assert_eq!(acc, "ACC-001");
        assert_eq!(engine.orders().len(), 1);
    }

    #[test]
    fn swf_reject_empty_accession() {
        let mut engine = SwfEngine::new();
        let order = SwfOrder::new("", "P001", "CT-CHEST");
        assert!(engine.submit_order(order).is_err());
    }

    #[test]
    fn swf_schedule_workitem() {
        let mut engine = SwfEngine::new();
        engine.submit_order(SwfOrder::new("ACC-002", "P002", "MR-BRAIN")).unwrap();
        let uid = engine.schedule_workitem("ACC-002", "Dr. Smith", 1000).unwrap();
        assert!(!uid.is_empty());
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Scheduled));
    }

    #[test]
    fn swf_complete_workflow() {
        let mut engine = SwfEngine::new();
        engine.submit_order(SwfOrder::new("ACC-003", "P003", "CT-ABD")).unwrap();
        let uid = engine.schedule_workitem("ACC-003", "Dr. Jones", 2000).unwrap();

        engine.start_workitem(&uid, 2100).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::InProgress));

        engine.complete_workitem(&uid, 3000).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Completed));
        assert_eq!(wi.actual_end, Some(3000));
    }

    #[test]
    fn swf_cancel_workitem() {
        let mut engine = SwfEngine::new();
        engine.submit_order(SwfOrder::new("ACC-004", "P004", "XR-CHEST")).unwrap();
        let uid = engine.schedule_workitem("ACC-004", "Dr. Lee", 4000).unwrap();

        engine.cancel_workitem(&uid).unwrap();
        let wi = engine.get_workitem(&uid).unwrap();
        assert!(matches!(wi.state, SwfWorkitemState::Canceled));
    }

    #[test]
    fn swf_cannot_start_completed_workitem() {
        let mut engine = SwfEngine::new();
        engine.submit_order(SwfOrder::new("ACC-005", "P005", "US-ABD")).unwrap();
        let uid = engine.schedule_workitem("ACC-005", "Dr. Kim", 5000).unwrap();
        engine.start_workitem(&uid, 5100).unwrap();
        engine.complete_workitem(&uid, 6000).unwrap();

        let result = engine.start_workitem(&uid, 6100);
        assert!(result.is_err());
    }

    // --- PIR Tests ---

    #[test]
    fn pir_register_unidentified() {
        let mut engine = PirEngine::new();
        engine.register_unidentified(UnidentifiedPatient {
            temporary_id: "TEMP-001".to_string(),
            study_uid: "1.2.3.4".to_string(),
            study_description: "CT Chest".to_string(),
            study_date: "2024-01-15".to_string(),
            confidence: 0.0,
        }).unwrap();
        assert_eq!(engine.unidentified_count(), 1);
    }

    #[test]
    fn pir_reconcile_patient() {
        let mut engine = PirEngine::new();
        engine.register_unidentified(UnidentifiedPatient {
            temporary_id: "TEMP-002".to_string(),
            study_uid: "1.2.3.5".to_string(),
            study_description: "MR Brain".to_string(),
            study_date: "2024-02-20".to_string(),
            confidence: 0.0,
        }).unwrap();

        engine.reconcile("TEMP-002", "P12345", "Doe^John", "19800101", "M").unwrap();
        assert_eq!(engine.unidentified_count(), 0);
        assert_eq!(engine.reconciled_count(), 1);
    }

    #[test]
    fn pir_reconcile_unknown_fails() {
        let mut engine = PirEngine::new();
        let result = engine.reconcile("NONEXISTENT", "P12345", "Doe^John", "19800101", "M");
        assert!(result.is_err());
    }

    // --- XDS Tests ---

    #[test]
    fn xds_register_document() {
        let mut registry = XdsRegistry::new();
        registry.register(XdsDocumentEntry {
            entry_uuid: "urn:uuid:12345".to_string(),
            unique_id: "1.2.3.4.5".to_string(),
            patient_id: "P001".to_string(),
            class_code: "DICOM".to_string(),
            type_code: "Imaging".to_string(),
            facility_code: "Hospital".to_string(),
            practice_setting: "Radiology".to_string(),
            repository_uid: "1.2.3.repo".to_string(),
            available: true,
        }).unwrap();
        assert_eq!(registry.document_count(), 1);
    }

    #[test]
    fn xds_query_by_patient() {
        let mut registry = XdsRegistry::new();
        registry.register(XdsDocumentEntry {
            entry_uuid: "urn:uuid:aaa".to_string(),
            unique_id: "1.2.3.4".to_string(),
            patient_id: "P001".to_string(),
            class_code: "DICOM".to_string(),
            type_code: "Imaging".to_string(),
            facility_code: "H".to_string(),
            practice_setting: "R".to_string(),
            repository_uid: "repo".to_string(),
            available: true,
        }).unwrap();
        registry.register(XdsDocumentEntry {
            entry_uuid: "urn:uuid:bbb".to_string(),
            unique_id: "1.2.3.5".to_string(),
            patient_id: "P001".to_string(),
            class_code: "DICOM".to_string(),
            type_code: "Imaging".to_string(),
            facility_code: "H".to_string(),
            practice_setting: "R".to_string(),
            repository_uid: "repo".to_string(),
            available: true,
        }).unwrap();

        let results = registry.query_by_patient("P001");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn xds_remove_document() {
        let mut registry = XdsRegistry::new();
        registry.register(XdsDocumentEntry {
            entry_uuid: "urn:uuid:del".to_string(),
            unique_id: "1.2.3.6".to_string(),
            patient_id: "P002".to_string(),
            class_code: "DICOM".to_string(),
            type_code: "Imaging".to_string(),
            facility_code: "H".to_string(),
            practice_setting: "R".to_string(),
            repository_uid: "repo".to_string(),
            available: true,
        }).unwrap();

        let removed = registry.remove("urn:uuid:del").unwrap();
        assert_eq!(removed.unique_id, "1.2.3.6");
        assert_eq!(registry.document_count(), 0);
    }

    // --- AIR Tests ---

    #[test]
    fn air_submit_finding() {
        let mut exchange = AirExchange::new();
        exchange.submit_finding(AirFinding {
            finding_uid: "1.2.3.f1".to_string(),
            study_uid: "1.2.3.s1".to_string(),
            finding_type_code: "76581006".to_string(),
            confidence: 0.85,
            algorithm_name: "LungNoduleAI".to_string(),
            algorithm_version: "1.0.0".to_string(),
            algorithm_uid: "1.2.3.ai1".to_string(),
        }).unwrap();
        assert_eq!(exchange.finding_count(), 1);
    }

    #[test]
    fn air_query_by_study() {
        let mut exchange = AirExchange::new();
        exchange.submit_finding(AirFinding {
            finding_uid: "1.2.3.f2".to_string(),
            study_uid: "1.2.3.study1".to_string(),
            finding_type_code: "76581006".to_string(),
            confidence: 0.9,
            algorithm_name: "LungAI".to_string(),
            algorithm_version: "2.0".to_string(),
            algorithm_uid: "1.2.3.ai2".to_string(),
        }).unwrap();

        let results = exchange.query_by_study("1.2.3.study1");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].confidence, 0.9);
    }

    // --- Profile metadata tests ---

    #[test]
    fn ihe_profile_names() {
        assert_eq!(IheProfile::Swf.name(), "Scheduled Workflow");
        assert_eq!(IheProfile::Pir.name(), "Patient Information Reconciliation");
        assert_eq!(IheProfile::XdsIb.name(), "XDS-I.b");
        assert_eq!(IheProfile::Air.name(), "AI Results");
    }

    #[test]
    fn ihe_profile_domains() {
        assert_eq!(IheProfile::Swf.domain(), "Radiology");
        assert_eq!(IheProfile::XdsIb.domain(), "IT Infrastructure");
    }
}
