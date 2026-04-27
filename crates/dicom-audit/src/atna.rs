//! IHE ATNA (Audit Trail and Node Authentication) profile export.
//!
//! Implements export of audit records in IHE ATNA profile format per
//! RFC 3881 (XML) and DICOM SUP 95. ATNA is required for IHE
//! integration profiles and regulatory compliance.

use crate::{AuditEventKind, AuditRecord};

/// IHE ATNA event type codes per RFC 3881.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtnaEventType {
    /// Application activity event (application start/stop).
    ApplicationActivity,
    /// Security alert event (authentication failure, break-glass).
    SecurityAlert,
    /// Patient record event (query, retrieve, create, update, delete).
    PatientRecord,
    /// Health service event (procedure, treatment).
    HealthServiceEvent,
    /// Audit log used event (audit log accessed).
    AuditLogUsed,
}

impl AtnaEventType {
    /// Return the RFC 3881 event code.
    pub fn code(self) -> &'static str {
        match self {
            AtnaEventType::ApplicationActivity => "110100",
            AtnaEventType::SecurityAlert => "110113",
            AtnaEventType::PatientRecord => "110110",
            AtnaEventType::HealthServiceEvent => "110112",
            AtnaEventType::AuditLogUsed => "110101",
        }
    }

    /// Return the RFC 3881 event code system.
    pub fn code_system(self) -> &'static str {
        "DCM"
    }

    /// Return the display name for this event type.
    pub fn display(self) -> &'static str {
        match self {
            AtnaEventType::ApplicationActivity => "Application Activity",
            AtnaEventType::SecurityAlert => "Security Alert",
            AtnaEventType::PatientRecord => "Patient Record",
            AtnaEventType::HealthServiceEvent => "Health Service Event",
            AtnaEventType::AuditLogUsed => "Audit Log Used",
        }
    }

    /// Map an AuditEventKind to an ATNA event type.
    pub fn from_audit_kind(kind: AuditEventKind) -> Self {
        match kind {
            AuditEventKind::AuthzDecision => AtnaEventType::SecurityAlert,
            AuditEventKind::AuthnFailure => AtnaEventType::SecurityAlert,
            AuditEventKind::ServiceEvent => AtnaEventType::ApplicationActivity,
        }
    }
}

/// ATNA event action codes per RFC 3881.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtnaEventAction {
    /// Create.
    Create,
    /// Read.
    Read,
    /// Update.
    Update,
    /// Delete.
    Delete,
    /// Execute.
    Execute,
}

impl AtnaEventAction {
    /// Return the RFC 3881 action code.
    pub fn code(self) -> &'static str {
        match self {
            AtnaEventAction::Create => "C",
            AtnaEventAction::Read => "R",
            AtnaEventAction::Update => "U",
            AtnaEventAction::Delete => "D",
            AtnaEventAction::Execute => "E",
        }
    }
}

/// ATNA participant information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtnaParticipant {
    /// User identifier.
    pub user_id: String,
    /// Alternative user identifier (e.g., username vs. DN).
    pub alt_user_id: Option<String>,
    /// Human-readable user name.
    pub user_name: Option<String>,
    /// Network access point (IP address or hostname).
    pub network_access_point: Option<String>,
    /// Whether this participant is the requestor.
    pub is_requestor: bool,
}

/// ATNA object role per RFC 3881.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtnaObjectRole {
    /// Patient.
    Patient,
    /// Person.
    Person,
    /// System Object.
    SystemObject,
    /// Organization.
    Organization,
    /// Other.
    Other,
}

impl AtnaObjectRole {
    /// Return the RFC 3881 role code.
    pub fn code(self) -> &'static str {
        match self {
            AtnaObjectRole::Patient => "1",
            AtnaObjectRole::Person => "2",
            AtnaObjectRole::SystemObject => "4",
            AtnaObjectRole::Organization => "5",
            AtnaObjectRole::Other => "7",
        }
    }
}

/// IHE ATNA exporter for converting audit records to RFC 3881 XML format.
#[derive(Debug, Clone)]
pub struct AtnaExporter;

impl AtnaExporter {
    /// Export an audit record as IHE ATNA RFC 3881 XML message.
    ///
    /// Produces a well-formed XML document conforming to the IHE ATNA
    /// profile using RFC 3881 schema elements:
    /// - `EventIdentification` with event type, action, and timestamp
    /// - `ActiveParticipant` for the requesting user
    /// - `ParticipantObjectIdentification` for the affected resource
    pub fn export_as_atna_xml(record: &AuditRecord) -> String {
        let event_type = AtnaEventType::from_audit_kind(record.kind);
        let event_action = Self::infer_event_action(record);

        let timestamp = Self::extract_field(record, "timestamp")
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let outcome = Self::extract_field(record, "outcome")
            .unwrap_or_else(|| "0".to_string());

        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<AuditMessage>\n");

        // EventIdentification
        xml.push_str("  <EventIdentification ");
        xml.push_str("EventActionCode=\"");
        xml.push_str(event_action.code());
        xml.push_str("\" ");
        xml.push_str("EventDateTime=\"");
        xml.push_str(&timestamp);
        xml.push_str("\" ");
        xml.push_str("EventOutcomeIndicator=\"");
        xml.push_str(&outcome);
        xml.push_str("\">\n");
        xml.push_str("    <EventID code=\"");
        xml.push_str(event_type.code());
        xml.push_str("\" codeSystemName=\"");
        xml.push_str(event_type.code_system());
        xml.push_str("\" displayName=\"");
        xml.push_str(event_type.display());
        xml.push_str("\" />\n");
        xml.push_str("  </EventIdentification>\n");

        // ActiveParticipant
        let user_id = Self::extract_field(record, "principal")
            .or_else(|| Self::extract_field(record, "subject"))
            .unwrap_or_else(|| "anonymous".to_string());
        let role_id = Self::extract_field(record, "role")
            .unwrap_or_else(|| "unknown".to_string());

        xml.push_str("  <ActiveParticipant UserID=\"");
        xml.push_str(&xml_escape(&user_id));
        xml.push_str("\" UserIsRequestor=\"true\">\n");
        xml.push_str("    <RoleIDCode code=\"");
        xml.push_str(&xml_escape(&role_id));
        xml.push_str("\" codeSystemName=\"DICOM\" displayName=\"");
        xml.push_str(&xml_escape(&role_id));
        xml.push_str("\" />\n");
        xml.push_str("  </ActiveParticipant>\n");

        // ParticipantObjectIdentification
        let resource = Self::extract_field(record, "resource")
            .or_else(|| Self::extract_field(record, "study_uid"))
            .unwrap_or_else(|| "unknown".to_string());
        let object_type = Self::extract_field(record, "object_type")
            .unwrap_or_else(|| "2".to_string()); // Default: System Object

        xml.push_str("  <ParticipantObjectIdentification ");
        xml.push_str("ParticipantObjectID=\"");
        xml.push_str(&xml_escape(&resource));
        xml.push_str("\" ");
        xml.push_str("ParticipantObjectTypeCode=\"");
        xml.push_str(&object_type);
        xml.push_str("\">\n");
        xml.push_str("    <ParticipantObjectIDTypeCode code=\"");
        xml.push_str(event_type.code());
        xml.push_str("\" codeSystemName=\"DCM\" />\n");
        xml.push_str("    <ParticipantObjectName>");
        xml.push_str(&xml_escape(&resource));
        xml.push_str("</ParticipantObjectName>\n");
        xml.push_str("  </ParticipantObjectIdentification>\n");

        xml.push_str("</AuditMessage>");
        xml
    }

    /// Export an audit record as IHE ATNA DICOM SUP 95 binary message.
    ///
    /// Returns a simplified binary representation suitable for transmission
    /// to a Syslog collector following IHE ATNA profile. The format is:
    /// header length (4 bytes BE) + XML payload bytes.
    pub fn export_as_atna_dicom(record: &AuditRecord) -> Vec<u8> {
        let xml = Self::export_as_atna_xml(record);
        let xml_bytes = xml.as_bytes();
        let len = u32::try_from(xml_bytes.len()).unwrap_or(u32::MAX);
        let mut result = Vec::with_capacity(4 + xml_bytes.len());
        result.extend_from_slice(&len.to_be_bytes());
        result.extend_from_slice(xml_bytes);
        result
    }

    /// Batch export multiple records as ATNA XML messages.
    pub fn export_batch(records: &[AuditRecord]) -> Vec<String> {
        records.iter().map(|r| Self::export_as_atna_xml(r)).collect()
    }

    /// Infer the ATNA event action from the audit record kind and fields.
    fn infer_event_action(record: &AuditRecord) -> AtnaEventAction {
        let action = Self::extract_field(record, "action");
        match action.as_deref() {
            Some("create") | Some("store") => AtnaEventAction::Create,
            Some("read") | Some("query") | Some("retrieve") => AtnaEventAction::Read,
            Some("update") | Some("modify") => AtnaEventAction::Update,
            Some("delete") => AtnaEventAction::Delete,
            _ => AtnaEventAction::Execute,
        }
    }

    /// Extract a field value from an audit record by key.
    fn extract_field(record: &AuditRecord, key: &str) -> Option<String> {
        record
            .fields
            .iter()
            .find(|f| f.key == key)
            .map(|f| f.value.clone())
    }
}

/// Escape special XML characters in a string.
fn xml_escape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            _ => result.push(c),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atna_event_type_codes_are_valid() {
        assert_eq!(AtnaEventType::ApplicationActivity.code(), "110100");
        assert_eq!(AtnaEventType::SecurityAlert.code(), "110113");
        assert_eq!(AtnaEventType::PatientRecord.code(), "110110");
        assert_eq!(AtnaEventType::HealthServiceEvent.code(), "110112");
        assert_eq!(AtnaEventType::AuditLogUsed.code(), "110101");
    }

    #[test]
    fn atna_event_type_from_audit_kind() {
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::AuthzDecision),
            AtnaEventType::SecurityAlert
        );
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::AuthnFailure),
            AtnaEventType::SecurityAlert
        );
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::ServiceEvent),
            AtnaEventType::ApplicationActivity
        );
    }

    #[test]
    fn atna_xml_contains_required_elements() {
        use crate::{AuditFieldRecord, AuditEventKind};

        let record = AuditRecord {
            sequence: 1,
            kind: AuditEventKind::AuthzDecision,
            fields: vec![
                AuditFieldRecord { key: "principal", value: "dr.smith".to_string() },
                AuditFieldRecord { key: "resource", value: "study-1.2.3".to_string() },
                AuditFieldRecord { key: "role", value: "Radiologist".to_string() },
                AuditFieldRecord { key: "outcome", value: "0".to_string() },
                AuditFieldRecord { key: "timestamp", value: "2026-03-01T12:00:00Z".to_string() },
            ],
            integrity_hash: "abc123".to_string(),
        };

        let xml = AtnaExporter::export_as_atna_xml(&record);
        assert!(xml.contains("<AuditMessage>"), "XML must contain AuditMessage root element");
        assert!(xml.contains("<EventIdentification"), "XML must contain EventIdentification");
        assert!(xml.contains("EventActionCode="), "XML must contain EventActionCode");
        assert!(xml.contains("EventDateTime="), "XML must contain EventDateTime");
        assert!(xml.contains("<EventID"), "XML must contain EventID");
        assert!(xml.contains("<ActiveParticipant"), "XML must contain ActiveParticipant");
        assert!(xml.contains("UserIsRequestor=\"true\""), "ActiveParticipant must be requestor");
        assert!(xml.contains("<ParticipantObjectIdentification"), "XML must contain ParticipantObjectIdentification");
        assert!(xml.contains("dr.smith"), "XML must contain principal user ID");
    }

    #[test]
    fn atna_xml_escapes_special_characters() {
        use crate::{AuditFieldRecord, AuditEventKind};

        let record = AuditRecord {
            sequence: 1,
            kind: AuditEventKind::AuthzDecision,
            fields: vec![
                AuditFieldRecord { key: "principal", value: "user<>\"&'".to_string() },
                AuditFieldRecord { key: "resource", value: "test".to_string() },
            ],
            integrity_hash: "hash".to_string(),
        };

        let xml = AtnaExporter::export_as_atna_xml(&record);
        assert!(xml.contains("&lt;&gt;&quot;&amp;&apos;"), "XML must escape special characters");
        assert!(!xml.contains("user<>\"&'"), "XML must not contain unescaped characters");
    }

    #[test]
    fn atna_dicom_export_has_header() {
        use crate::{AuditFieldRecord, AuditEventKind};

        let record = AuditRecord {
            sequence: 1,
            kind: AuditEventKind::ServiceEvent,
            fields: vec![
                AuditFieldRecord { key: "subject", value: "test".to_string() },
            ],
            integrity_hash: "hash".to_string(),
        };

        let binary = AtnaExporter::export_as_atna_dicom(&record);
        assert!(binary.len() > 4, "binary output must have header + payload");

        // Read the length header (4 bytes BE)
        let header_len = u32::from_be_bytes([binary[0], binary[1], binary[2], binary[3]]);
        assert_eq!(
            header_len as usize,
            binary.len() - 4,
            "header length must match payload size"
        );
    }

    #[test]
    fn atna_batch_export() {
        use crate::{AuditFieldRecord, AuditEventKind};

        let records = vec![
            AuditRecord {
                sequence: 1,
                kind: AuditEventKind::ServiceEvent,
                fields: vec![AuditFieldRecord { key: "subject", value: "a".to_string() }],
                integrity_hash: "h1".to_string(),
            },
            AuditRecord {
                sequence: 2,
                kind: AuditEventKind::AuthzDecision,
                fields: vec![AuditFieldRecord { key: "subject", value: "b".to_string() }],
                integrity_hash: "h2".to_string(),
            },
        ];

        let batch = AtnaExporter::export_batch(&records);
        assert_eq!(batch.len(), 2, "batch must export all records");
        for xml in &batch {
            assert!(xml.contains("<AuditMessage>"), "each export must be valid XML");
        }
    }
}
