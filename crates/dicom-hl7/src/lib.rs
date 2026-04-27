#![deny(missing_docs)]

//! HL7 v2 adapter for DICOM-HL7 interoperability.
//!
//! Provides ADT message parsing (A01/A02/A03/A08) for patient sync,
//! ORM message parsing for order entry, ORU message generation for
//! result delivery, and MLLP (Minimum Lower Layer Protocol) transport.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::Result;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// --- HL7 v2 Segment Field Separator ---

const HL7_FIELD_SEP: char = '|';
#[allow(dead_code)]
const HL7_COMPONENT_SEP: char = '^';
#[allow(dead_code)]
const HL7_REPEAT_SEP: char = '~';
#[allow(dead_code)]
const HL7_ESCAPE_CHAR: char = '\\';
#[allow(dead_code)]
const HL7_SUBCOMPONENT_SEP: char = '&';

// --- HL7 v2 Message Types ---

/// HL7 v2 ADT message types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AdtMessageType {
    /// A01 — Admit/Visit Notification.
    A01,
    /// A02 — Transfer a Patient.
    A02,
    /// A03 — Discharge/End Visit.
    A03,
    /// A08 — Update an Admission.
    A08,
}

impl fmt::Display for AdtMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AdtMessageType::A01 => write!(f, "A01"),
            AdtMessageType::A02 => write!(f, "A02"),
            AdtMessageType::A03 => write!(f, "A03"),
            AdtMessageType::A08 => write!(f, "A08"),
        }
    }
}

/// HL7 v2 ORM message types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OrmMessageType {
    /// O01 — Order Message.
    O01,
}

impl fmt::Display for OrmMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrmMessageType::O01 => write!(f, "O01"),
        }
    }
}

/// HL7 v2 ORU message types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OruMessageType {
    /// R01 — Observation Result.
    R01,
}

impl fmt::Display for OruMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OruMessageType::R01 => write!(f, "R01"),
        }
    }
}

// --- Parsed HL7 v2 Message Structures ---

/// Parsed HL7 v2 MSH (Message Header) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MshSegment {
    /// Field separator (usually '|').
    pub field_separator: char,
    /// Encoding characters (usually '^~\\&').
    pub encoding_characters: String,
    /// Sending application.
    pub sending_application: String,
    /// Sending facility.
    pub sending_facility: String,
    /// Receiving application.
    pub receiving_application: String,
    /// Receiving facility.
    pub receiving_facility: String,
    /// Date/Time of message.
    pub datetime: String,
    /// Message type (e.g., "ADT^A01").
    pub message_type: String,
    /// Message Control ID.
    pub message_control_id: String,
    /// Processing ID (P=Production, T=Training, D=Debugging).
    pub processing_id: String,
    /// Version ID (e.g., "2.5.1").
    pub version_id: String,
}

/// Parsed HL7 v2 PID (Patient Identification) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PidSegment {
    /// Patient ID (external).
    pub patient_id: String,
    /// Patient Name (Family^Given^Middle).
    pub patient_name: String,
    /// Date of Birth (YYYYMMDD).
    pub birth_date: Option<String>,
    /// Administrative Sex (M/F/O/U).
    pub sex: Option<String>,
    /// Patient Address.
    pub address: Option<String>,
    /// Phone Number Home.
    pub phone_home: Option<String>,
}

/// Parsed HL7 v2 PV1 (Patient Visit) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pv1Segment {
    /// Patient Class (I=Inpatient, O=Outpatient, E=Emergency).
    pub patient_class: Option<String>,
    /// Assigned Patient Location.
    pub assigned_location: Option<String>,
    /// Admission Type.
    pub admission_type: Option<String>,
    /// Attending Physician.
    pub attending_physician: Option<String>,
    /// Referring Physician.
    pub referring_physician: Option<String>,
}

/// Parsed HL7 v2 ORC (Common Order) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrcSegment {
    /// Order Control (NW=New, SC=Scheduled, DC=Discontinue).
    pub order_control: String,
    /// Placer Order Number.
    pub placer_order_number: Option<String>,
    /// Filler Order Number.
    pub filler_order_number: Option<String>,
    /// Ordering Provider.
    pub ordering_provider: Option<String>,
}

/// Parsed HL7 v2 OBR (Observation Request) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObrSegment {
    /// Placer Order Number.
    pub placer_order_number: Option<String>,
    /// Filler Order Number.
    pub filler_order_number: Option<String>,
    /// Universal Service Identifier (procedure code).
    pub service_identifier: Option<String>,
    /// Requested Date/Time.
    pub requested_datetime: Option<String>,
    /// Observation Date/Time.
    pub observation_datetime: Option<String>,
    /// Ordering Provider.
    pub ordering_provider: Option<String>,
    /// Diagnostic Serv Sect ID (e.g., "RAD").
    pub diagnostic_service_section: Option<String>,
}

/// Parsed HL7 v2 OBX (Observation/Result) segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObxSegment {
    /// Set ID.
    pub set_id: String,
    /// Value Type (ST=String, NM=Numeric, CE=Coded Entry).
    pub value_type: String,
    /// Observation Identifier.
    pub observation_identifier: String,
    /// Observation Value.
    pub observation_value: Option<String>,
    /// Units.
    pub units: Option<String>,
    /// Reference Range.
    pub reference_range: Option<String>,
    /// Abnormal Flags.
    pub abnormal_flags: Option<String>,
    /// Observation Result Status (F=Final, P=Preliminary, C=Corrected).
    pub result_status: Option<String>,
}

/// A complete parsed HL7 v2 ADT message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdtMessage {
    /// Message header.
    pub msh: MshSegment,
    /// Patient identification.
    pub pid: PidSegment,
    /// Patient visit.
    pub pv1: Option<Pv1Segment>,
    /// ADT event type.
    pub event_type: AdtMessageType,
}

/// A complete parsed HL7 v2 ORM message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrmMessage {
    /// Message header.
    pub msh: MshSegment,
    /// Patient identification.
    pub pid: PidSegment,
    /// Common order.
    pub orc: OrcSegment,
    /// Observation request.
    pub obr: ObrSegment,
}

/// A complete HL7 v2 ORU message (for generation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OruMessage {
    /// Message header.
    pub msh: MshSegment,
    /// Patient identification.
    pub pid: PidSegment,
    /// Common order.
    pub orc: OrcSegment,
    /// Observation request.
    pub obr: ObrSegment,
    /// Observation results.
    pub obx: Vec<ObxSegment>,
}

// --- Error Type ---

/// HL7 adapter error type.
#[derive(Debug, Clone, PartialEq)]
pub enum Hl7Error {
    /// Message parsing failed.
    ParseFailed {
        /// Failure detail.
        detail: String,
    },
    /// Required segment is missing.
    MissingSegment {
        /// Segment identifier (e.g., "MSH", "PID").
        segment: String,
    },
    /// Field index out of range.
    FieldOutOfRange {
        /// Segment identifier.
        segment: String,
        /// Field index.
        index: usize,
    },
    /// Invalid message type.
    InvalidMessageType {
        /// Message type string.
        message_type: String,
    },
    /// Encoding failure.
    EncodingFailed {
        /// Failure detail.
        detail: String,
    },
}

impl fmt::Display for Hl7Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hl7Error::ParseFailed { detail } => write!(f, "HL7 parse failed: {detail}"),
            Hl7Error::MissingSegment { segment } => {
                write!(f, "missing required HL7 segment: {segment}")
            }
            Hl7Error::FieldOutOfRange { segment, index } => {
                write!(f, "field index {index} out of range in segment {segment}")
            }
            Hl7Error::InvalidMessageType { message_type } => {
                write!(f, "invalid HL7 message type: {message_type}")
            }
            Hl7Error::EncodingFailed { detail } => {
                write!(f, "HL7 encoding failed: {detail}")
            }
        }
    }
}

impl std::error::Error for Hl7Error {}

/// Audit callback for HL7 adapter operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

// --- HL7 v2 Parser ---

/// HL7 v2 message parser.
pub struct Hl7Parser {
    /// Audit callback.
    audit: Option<AuditCallback>,
}

impl Default for Hl7Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Hl7Parser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hl7Parser")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl Hl7Parser {
    /// Create a new HL7 parser.
    pub fn new() -> Self {
        Self { audit: None }
    }

    /// Create an HL7 parser with audit callback.
    pub fn with_audit(audit: Option<AuditCallback>) -> Self {
        Self { audit }
    }

    /// Parse an HL7 v2 message string into its constituent segments.
    pub fn parse_segments(&self, message: &str) -> Vec<Hl7Segment> {
        message
            .split('\r')
            .filter(|s| !s.is_empty())
            .map(|s| Hl7Segment {
                raw: s.to_string(),
                fields: s.split(HL7_FIELD_SEP).map(String::from).collect(),
            })
            .collect()
    }

    /// Parse an ADT message from an HL7 v2 message string.
    pub fn parse_adt(&self, message: &str) -> std::result::Result<AdtMessage, Hl7Error> {
        let segments = self.parse_segments(message);

        let msh = self.parse_msh(&segments)?;
        let pid = self.parse_pid(&segments)?;
        let pv1 = self.parse_pv1(&segments).ok();

        let event_type = self.extract_adt_event_type(&msh.message_type)?;

        self.record_audit("parse_adt", Some(&msh.message_control_id))?;

        Ok(AdtMessage {
            msh,
            pid,
            pv1,
            event_type,
        })
    }

    /// Parse an ORM message from an HL7 v2 message string.
    pub fn parse_orm(&self, message: &str) -> std::result::Result<OrmMessage, Hl7Error> {
        let segments = self.parse_segments(message);

        let msh = self.parse_msh(&segments)?;
        let pid = self.parse_pid(&segments)?;
        let orc = self.parse_orc(&segments)?;
        let obr = self.parse_obr(&segments)?;

        self.record_audit("parse_orm", Some(&msh.message_control_id))?;

        Ok(OrmMessage { msh, pid, orc, obr })
    }

    fn parse_msh(&self, segments: &[Hl7Segment]) -> std::result::Result<MshSegment, Hl7Error> {
        let seg = segments
            .iter()
            .find(|s| s.segment_id() == "MSH")
            .ok_or_else(|| Hl7Error::MissingSegment {
                segment: "MSH".to_string(),
            })?;

        Ok(MshSegment {
            field_separator: seg.field_char(0).unwrap_or(HL7_FIELD_SEP),
            encoding_characters: seg.field_str(1).unwrap_or("^~\\&").to_string(),
            sending_application: seg.field_str(2).unwrap_or("").to_string(),
            sending_facility: seg.field_str(3).unwrap_or("").to_string(),
            receiving_application: seg.field_str(4).unwrap_or("").to_string(),
            receiving_facility: seg.field_str(5).unwrap_or("").to_string(),
            datetime: seg.field_str(6).unwrap_or("").to_string(),
            // MSH-9 (Message Type) is at field index 8 after split
            message_type: seg.field_str(8).unwrap_or("").to_string(),
            // MSH-10 (Message Control ID) is at field index 9 after split
            message_control_id: seg.field_str(9).unwrap_or("").to_string(),
            // MSH-11 (Processing ID) is at field index 10 after split
            processing_id: seg.field_str(10).unwrap_or("").to_string(),
            // MSH-12 (Version ID) is at field index 11 after split
            version_id: seg.field_str(11).unwrap_or("2.5.1").to_string(),
        })
    }

    fn parse_pid(&self, segments: &[Hl7Segment]) -> std::result::Result<PidSegment, Hl7Error> {
        let seg = segments
            .iter()
            .find(|s| s.segment_id() == "PID")
            .ok_or_else(|| Hl7Error::MissingSegment {
                segment: "PID".to_string(),
            })?;

        Ok(PidSegment {
            patient_id: seg.field_str(3).unwrap_or("").to_string(),
            patient_name: seg.field_str(5).unwrap_or("").to_string(),
            birth_date: seg.field_str(7).map(|s| s.to_string()),
            sex: seg.field_str(8).map(|s| s.to_string()),
            address: seg.field_str(11).map(|s| s.to_string()),
            phone_home: seg.field_str(13).map(|s| s.to_string()),
        })
    }

    fn parse_pv1(&self, segments: &[Hl7Segment]) -> std::result::Result<Pv1Segment, Hl7Error> {
        let seg = segments
            .iter()
            .find(|s| s.segment_id() == "PV1")
            .ok_or_else(|| Hl7Error::MissingSegment {
                segment: "PV1".to_string(),
            })?;

        Ok(Pv1Segment {
            patient_class: seg.field_str(1).map(|s| s.to_string()),
            assigned_location: seg.field_str(2).map(|s| s.to_string()),
            admission_type: seg.field_str(3).map(|s| s.to_string()),
            attending_physician: seg.field_str(6).map(|s| s.to_string()),
            referring_physician: seg.field_str(7).map(|s| s.to_string()),
        })
    }

    fn parse_orc(&self, segments: &[Hl7Segment]) -> std::result::Result<OrcSegment, Hl7Error> {
        let seg = segments
            .iter()
            .find(|s| s.segment_id() == "ORC")
            .ok_or_else(|| Hl7Error::MissingSegment {
                segment: "ORC".to_string(),
            })?;

        Ok(OrcSegment {
            order_control: seg.field_str(1).unwrap_or("NW").to_string(),
            placer_order_number: seg.field_str(2).map(|s| s.to_string()),
            filler_order_number: seg.field_str(3).map(|s| s.to_string()),
            ordering_provider: seg.field_str(12).map(|s| s.to_string()),
        })
    }

    fn parse_obr(&self, segments: &[Hl7Segment]) -> std::result::Result<ObrSegment, Hl7Error> {
        let seg = segments
            .iter()
            .find(|s| s.segment_id() == "OBR")
            .ok_or_else(|| Hl7Error::MissingSegment {
                segment: "OBR".to_string(),
            })?;

        Ok(ObrSegment {
            placer_order_number: seg.field_str(2).map(|s| s.to_string()),
            filler_order_number: seg.field_str(3).map(|s| s.to_string()),
            service_identifier: seg.field_str(4).map(|s| s.to_string()),
            requested_datetime: seg.field_str(6).map(|s| s.to_string()),
            observation_datetime: seg.field_str(7).map(|s| s.to_string()),
            ordering_provider: seg.field_str(10).map(|s| s.to_string()),
            diagnostic_service_section: seg.field_str(16).map(|s| s.to_string()),
        })
    }

    fn extract_adt_event_type(
        &self,
        message_type: &str,
    ) -> std::result::Result<AdtMessageType, Hl7Error> {
        // Message type may be "ADT^A01" or just "A01"
        let trigger = if message_type.contains('^') {
            message_type.split('^').nth(1).unwrap_or(message_type)
        } else {
            message_type
        };

        match trigger {
            "A01" => Ok(AdtMessageType::A01),
            "A02" => Ok(AdtMessageType::A02),
            "A03" => Ok(AdtMessageType::A03),
            "A08" => Ok(AdtMessageType::A08),
            _ => Err(Hl7Error::InvalidMessageType {
                message_type: message_type.to_string(),
            }),
        }
    }

    fn record_audit(
        &self,
        operation: &'static str,
        subject_id: Option<&str>,
    ) -> std::result::Result<(), Hl7Error> {
        let Some(callback) = &self.audit else {
            return Ok(());
        };
        let mut fields = vec![AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        }];
        if let Some(id) = subject_id {
            fields.push(AuditField {
                key: "message_control_id",
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

// --- HL7 v2 Message Builder ---

/// HL7 v2 ORU message builder.
#[derive(Debug, Clone)]
pub struct OruBuilder {
    msh: MshSegment,
    pid: PidSegment,
    orc: OrcSegment,
    obr: ObrSegment,
    obx: Vec<ObxSegment>,
}

impl OruBuilder {
    /// Create a new ORU builder with required segments.
    pub fn new(msh: MshSegment, pid: PidSegment, orc: OrcSegment, obr: ObrSegment) -> Self {
        Self {
            msh,
            pid,
            orc,
            obr,
            obx: Vec::new(),
        }
    }

    /// Add an observation result.
    pub fn add_obx(mut self, obx: ObxSegment) -> Self {
        self.obx.push(obx);
        self
    }

    /// Build the ORU message.
    pub fn build(self) -> OruMessage {
        OruMessage {
            msh: self.msh,
            pid: self.pid,
            orc: self.orc,
            obr: self.obr,
            obx: self.obx,
        }
    }
}

// --- HL7 v2 Encoder ---

/// HL7 v2 message encoder.
pub struct Hl7Encoder {
    /// Audit callback.
    audit: Option<AuditCallback>,
}

impl Default for Hl7Encoder {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for Hl7Encoder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hl7Encoder")
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

impl Hl7Encoder {
    /// Create a new HL7 encoder.
    pub fn new() -> Self {
        Self { audit: None }
    }

    /// Create an HL7 encoder with audit callback.
    pub fn with_audit(audit: Option<AuditCallback>) -> Self {
        Self { audit }
    }

    /// Encode an ORU message to an HL7 v2 wire format string.
    pub fn encode_oru(&self, message: &OruMessage) -> std::result::Result<String, Hl7Error> {
        let mut segments = Vec::new();

        // MSH segment
        segments.push(format!(
            "MSH|{enc}|{sa}|{sf}|{ra}|{rf}|{dt}|ADT^ORU^R01|{ctrl}|{pid}|{ver}",
            enc = message.msh.encoding_characters,
            sa = message.msh.sending_application,
            sf = message.msh.sending_facility,
            ra = message.msh.receiving_application,
            rf = message.msh.receiving_facility,
            dt = message.msh.datetime,
            ctrl = message.msh.message_control_id,
            pid = message.msh.processing_id,
            ver = message.msh.version_id,
        ));

        // PID segment
        let mut pid_fields = vec![
            "PID".to_string(),
            String::new(), // Set ID
            String::new(), // Patient ID (internal)
            message.pid.patient_id.clone(),
        ];
        pid_fields.push(message.pid.patient_name.clone());
        if let Some(bd) = &message.pid.birth_date {
            pid_fields.push(bd.clone());
        }
        if let Some(sex) = &message.pid.sex {
            pid_fields.push(sex.clone());
        }
        segments.push(pid_fields.join("|"));

        // ORC segment
        segments.push(format!(
            "ORC|{oc}|{po}|{fo}||||||{op}",
            oc = message.orc.order_control,
            po = message.orc.placer_order_number.as_deref().unwrap_or(""),
            fo = message.orc.filler_order_number.as_deref().unwrap_or(""),
            op = message.orc.ordering_provider.as_deref().unwrap_or(""),
        ));

        // OBR segment
        segments.push(format!(
            "OBR||{po}|{fo}|{si}||{odt}||||{op}||||||{dss}",
            po = message.obr.placer_order_number.as_deref().unwrap_or(""),
            fo = message.obr.filler_order_number.as_deref().unwrap_or(""),
            si = message.obr.service_identifier.as_deref().unwrap_or(""),
            odt = message.obr.observation_datetime.as_deref().unwrap_or(""),
            op = message.obr.ordering_provider.as_deref().unwrap_or(""),
            dss = message
                .obr
                .diagnostic_service_section
                .as_deref()
                .unwrap_or(""),
        ));

        // OBX segments
        for obx in &message.obx {
            segments.push(format!(
                "OBX|{sid}|{vt}|{oi}|{ov}|{units}|{ref}|{flags}|{status}",
                sid = obx.set_id,
                vt = obx.value_type,
                oi = obx.observation_identifier,
                ov = obx.observation_value.as_deref().unwrap_or(""),
                units = obx.units.as_deref().unwrap_or(""),
                ref = obx.reference_range.as_deref().unwrap_or(""),
                flags = obx.abnormal_flags.as_deref().unwrap_or(""),
                status = obx.result_status.as_deref().unwrap_or(""),
            ));
        }

        let encoded = segments.join("\r");

        self.record_audit("encode_oru", Some(&message.msh.message_control_id))?;

        Ok(encoded)
    }

    fn record_audit(
        &self,
        operation: &'static str,
        subject_id: Option<&str>,
    ) -> std::result::Result<(), Hl7Error> {
        let Some(callback) = &self.audit else {
            return Ok(());
        };
        let mut fields = vec![AuditField {
            key: "operation",
            value: AuditValue::Plain(operation.to_string()),
        }];
        if let Some(id) = subject_id {
            fields.push(AuditField {
                key: "message_control_id",
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

// --- MLLP Transport ---

/// MLLP (Minimum Lower Layer Protocol) framing.
///
/// MLLP wraps HL7 messages with SB (0x0B) start block and EB (0x1C) end block
/// characters, with a trailing CR (0x0D).
pub struct MllpFramer;

impl MllpFramer {
    /// MLLP Start Block character.
    pub const SB: u8 = 0x0B;
    /// MLLP End Block character.
    pub const EB: u8 = 0x1C;
    /// MLLP Carriage Return.
    pub const CR: u8 = 0x0D;

    /// Frame an HL7 message for MLLP transport.
    pub fn frame(message: &str) -> Vec<u8> {
        let mut framed = Vec::new();
        framed.push(Self::SB);
        framed.extend_from_slice(message.as_bytes());
        framed.push(Self::EB);
        framed.push(Self::CR);
        framed
    }

    /// Unframe an MLLP-framed message, extracting the HL7 content.
    pub fn unframe(data: &[u8]) -> std::result::Result<String, Hl7Error> {
        if data.len() < 3 {
            return Err(Hl7Error::ParseFailed {
                detail: "MLLP frame too short".to_string(),
            });
        }
        if data[0] != Self::SB {
            return Err(Hl7Error::ParseFailed {
                detail: "MLLP frame missing start block".to_string(),
            });
        }
        // Find end block
        let eb_pos =
            data.iter()
                .position(|&b| b == Self::EB)
                .ok_or_else(|| Hl7Error::ParseFailed {
                    detail: "MLLP frame missing end block".to_string(),
                })?;

        let message_bytes = &data[1..eb_pos];
        String::from_utf8(message_bytes.to_vec()).map_err(|e| Hl7Error::ParseFailed {
            detail: format!("MLLP payload is not valid UTF-8: {e}"),
        })
    }
}

// --- Internal Segment Helper ---

/// Parsed HL7 segment with field access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hl7Segment {
    /// Raw segment string.
    raw: String,
    /// Parsed fields.
    fields: Vec<String>,
}

impl Hl7Segment {
    /// Return the segment identifier (e.g., "MSH", "PID").
    pub fn segment_id(&self) -> &str {
        self.fields.first().map(|s| s.as_str()).unwrap_or("")
    }

    /// Get a field value by index (1-based, MSH field 0 is the field separator).
    pub fn field_str(&self, index: usize) -> Option<&str> {
        self.fields.get(index).map(|s| s.as_str())
    }

    /// Get a single character from a field.
    pub fn field_char(&self, index: usize) -> Option<char> {
        self.field_str(index).and_then(|s| s.chars().next())
    }
}

// --- HL7 v2 Acknowledgment ---

/// HL7 v2 ACK message builder.
pub struct AckBuilder;

impl AckBuilder {
    /// Build an acknowledgment message for a received message.
    pub fn build_ack(
        receiving_app: &str,
        receiving_facility: &str,
        original_control_id: &str,
        ack_code: &str,
        text_message: &str,
    ) -> String {
        let _timestamp = "20240115120000"; // Would be real timestamp in production
        format!(
            "MSH|^~\\&|{ra}|{rf}|||||ACK|{ctrl}|P|2.5.1\rMSA|{code}|{orig_ctrl}|{text}",
            ra = receiving_app,
            rf = receiving_facility,
            ctrl = format!("ACK_{original_control_id}"),
            code = ack_code,
            orig_ctrl = original_control_id,
            text = text_message,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_adt_a01() -> String {
        "MSH|^~\\&|SEND_APP|SEND_FAC|RECV_APP|RECV_FAC|20240115120000||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John^M||19800101|M".to_string()
    }

    fn sample_orm_o01() -> String {
        "MSH|^~\\&|SEND_APP|SEND_FAC|RECV_APP|RECV_FAC|20240115120000||ORM^O01|MSG002|P|2.5.1\rPID|||PAT001||Smith^John^M||19800101|M\rORC|NW|ORD001||||||Smith^Dr\rOBR||ORD001||CT_CHEST||20240115100000||||Smith^Dr||||||RAD".to_string()
    }

    #[test]
    fn parse_adt_a01() {
        let parser = Hl7Parser::new();
        let msg = parser.parse_adt(&sample_adt_a01()).expect("parse ADT");

        assert_eq!(msg.msh.sending_application, "SEND_APP");
        assert_eq!(msg.msh.sending_facility, "SEND_FAC");
        assert_eq!(msg.msh.receiving_application, "RECV_APP");
        assert_eq!(msg.msh.message_type, "ADT^A01");
        assert_eq!(msg.msh.message_control_id, "MSG001");
        assert_eq!(msg.msh.processing_id, "P");
        assert_eq!(msg.msh.version_id, "2.5.1");

        assert_eq!(msg.pid.patient_id, "PAT001");
        assert_eq!(msg.pid.patient_name, "Smith^John^M");
        assert_eq!(msg.pid.birth_date.as_deref(), Some("19800101"));
        assert_eq!(msg.pid.sex.as_deref(), Some("M"));

        assert_eq!(msg.event_type, AdtMessageType::A01);
    }

    #[test]
    fn parse_adt_missing_msh_fails() {
        let parser = Hl7Parser::new();
        let msg = "PID|||PAT001||Smith".to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "MSH"));
    }

    #[test]
    fn parse_adt_missing_pid_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1".to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "PID"));
    }

    #[test]
    fn parse_adt_invalid_event_type() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A99|MSG001|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg);
        assert!(matches!(result, Err(Hl7Error::InvalidMessageType { .. })));
    }

    #[test]
    fn parse_adt_a02() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A02|MSG002|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A02");
        assert_eq!(result.event_type, AdtMessageType::A02);
    }

    #[test]
    fn parse_adt_a03() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A03|MSG003|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A03");
        assert_eq!(result.event_type, AdtMessageType::A03);
    }

    #[test]
    fn parse_adt_a08() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A08|MSG008|P|2.5.1\rPID|||PAT001||Smith"
            .to_string();
        let result = parser.parse_adt(&msg).expect("parse A08");
        assert_eq!(result.event_type, AdtMessageType::A08);
    }

    #[test]
    fn parse_orm_o01() {
        let parser = Hl7Parser::new();
        let msg = parser.parse_orm(&sample_orm_o01()).expect("parse ORM");

        assert_eq!(msg.msh.message_type, "ORM^O01");
        assert_eq!(msg.pid.patient_id, "PAT001");
        assert_eq!(msg.orc.order_control, "NW");
        assert_eq!(msg.orc.placer_order_number.as_deref(), Some("ORD001"));
        assert_eq!(msg.obr.service_identifier.as_deref(), Some("CT_CHEST"));
        assert_eq!(msg.obr.diagnostic_service_section.as_deref(), Some("RAD"));
    }

    #[test]
    fn parse_orm_missing_orc_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith\rOBR||ORD001||CT_CHEST".to_string();
        let result = parser.parse_orm(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "ORC"));
    }

    #[test]
    fn parse_orm_missing_obr_fails() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith\rORC|NW|ORD001".to_string();
        let result = parser.parse_orm(&msg);
        assert!(matches!(result, Err(Hl7Error::MissingSegment { segment }) if segment == "OBR"));
    }

    #[test]
    fn encode_oru_message() {
        let encoder = Hl7Encoder::new();
        let oru = OruBuilder::new(
            MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "PACS".to_string(),
                sending_facility: "HOSPITAL".to_string(),
                receiving_application: "RIS".to_string(),
                receiving_facility: "HOSPITAL".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ORU^R01".to_string(),
                message_control_id: "ORU001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith^John".to_string(),
                birth_date: Some("19800101".to_string()),
                sex: Some("M".to_string()),
                address: None,
                phone_home: None,
            },
            OrcSegment {
                order_control: "NW".to_string(),
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: None,
                ordering_provider: None,
            },
            ObrSegment {
                placer_order_number: Some("ORD001".to_string()),
                filler_order_number: None,
                service_identifier: Some("CT_CHEST".to_string()),
                requested_datetime: None,
                observation_datetime: Some("20240115100000".to_string()),
                ordering_provider: None,
                diagnostic_service_section: Some("RAD".to_string()),
            },
        )
        .add_obx(ObxSegment {
            set_id: "1".to_string(),
            value_type: "NM".to_string(),
            observation_identifier: "FINDING_001".to_string(),
            observation_value: Some("25.3".to_string()),
            units: Some("mm".to_string()),
            reference_range: None,
            abnormal_flags: None,
            result_status: Some("F".to_string()),
        })
        .build();

        let encoded = encoder.encode_oru(&oru).expect("encode");
        assert!(encoded.contains("MSH|"));
        assert!(encoded.contains("PID|"));
        assert!(encoded.contains("ORC|"));
        assert!(encoded.contains("OBR|"));
        assert!(encoded.contains("OBX|"));
        assert!(encoded.contains("25.3"));
    }

    #[test]
    fn mllp_frame_unframe_roundtrip() {
        let message = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001";
        let framed = MllpFramer::frame(message);
        assert_eq!(framed[0], MllpFramer::SB);
        assert_eq!(*framed.last().unwrap(), MllpFramer::CR);

        let unframed = MllpFramer::unframe(&framed).expect("unframe");
        assert_eq!(unframed, message);
    }

    #[test]
    fn mllp_unframe_rejects_too_short() {
        let data = vec![0x0B, 0x1C];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn mllp_unframe_rejects_missing_start() {
        let data = vec![0x00, b'M', b'S', b'H', 0x1C, 0x0D];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn mllp_unframe_rejects_missing_end() {
        let data = vec![0x0B, b'M', b'S', b'H', 0x0D];
        let result = MllpFramer::unframe(&data);
        assert!(matches!(result, Err(Hl7Error::ParseFailed { .. })));
    }

    #[test]
    fn ack_builder_produces_valid_ack() {
        let ack = AckBuilder::build_ack("RECV_APP", "RECV_FAC", "MSG001", "AA", "Message accepted");
        assert!(ack.starts_with("MSH|"));
        assert!(ack.contains("ACK"));
        assert!(ack.contains("MSA|AA|MSG001"));
    }

    #[test]
    fn adt_with_pv1_segment() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M\rPV1|I|ICU^101^A||||Smith^Dr".to_string();
        let result = parser.parse_adt(&msg).expect("parse with PV1");
        assert!(result.pv1.is_some());
        let pv1 = result.pv1.unwrap();
        assert_eq!(pv1.patient_class.as_deref(), Some("I"));
        assert_eq!(pv1.assigned_location.as_deref(), Some("ICU^101^A"));
    }

    #[test]
    fn audit_callback_emits_on_parse() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let parser = Hl7Parser::with_audit(Some(audit));
        parser.parse_adt(&sample_adt_a01()).expect("parse");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
        assert!(events[0].fields.iter().any(|f| f.key == "operation"
            && matches!(&f.value, AuditValue::Plain(v) if v == "parse_adt")));
    }

    #[test]
    fn audit_callback_emits_on_encode() {
        use std::sync::Mutex;

        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            events_handle.lock().expect("lock").push(event);
            Ok(())
        });

        let encoder = Hl7Encoder::with_audit(Some(audit));
        let oru = OruBuilder::new(
            MshSegment {
                field_separator: '|',
                encoding_characters: "^~\\&".to_string(),
                sending_application: "PACS".to_string(),
                sending_facility: "HOSP".to_string(),
                receiving_application: "RIS".to_string(),
                receiving_facility: "HOSP".to_string(),
                datetime: "20240115120000".to_string(),
                message_type: "ORU^R01".to_string(),
                message_control_id: "ORU001".to_string(),
                processing_id: "P".to_string(),
                version_id: "2.5.1".to_string(),
            },
            PidSegment {
                patient_id: "PAT001".to_string(),
                patient_name: "Smith".to_string(),
                birth_date: None,
                sex: None,
                address: None,
                phone_home: None,
            },
            OrcSegment {
                order_control: "NW".to_string(),
                placer_order_number: None,
                filler_order_number: None,
                ordering_provider: None,
            },
            ObrSegment {
                placer_order_number: None,
                filler_order_number: None,
                service_identifier: None,
                requested_datetime: None,
                observation_datetime: None,
                ordering_provider: None,
                diagnostic_service_section: None,
            },
        )
        .build();

        encoder.encode_oru(&oru).expect("encode");

        let events = events.lock().expect("lock");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn parse_segments_splits_correctly() {
        let parser = Hl7Parser::new();
        let msg = "MSH|^~\\&|APP\rPID|||PAT001";
        let segments = parser.parse_segments(msg);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].segment_id(), "MSH");
        assert_eq!(segments[1].segment_id(), "PID");
    }

    #[test]
    fn field_access_works() {
        let parser = Hl7Parser::new();
        let segments = parser.parse_segments("MSH|^~\\&|APP|FAC");
        let msh = &segments[0];
        assert_eq!(msh.segment_id(), "MSH");
        assert_eq!(msh.field_str(2), Some("APP"));
        assert_eq!(msh.field_str(3), Some("FAC"));
    }
}
