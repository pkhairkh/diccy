//! MLLP server for receiving HL7 messages.
//!
//! Implements an MLLP (Minimum Lower Layer Protocol) server that can
//! receive incoming HL7 messages over TCP. The server uses a handler
//! trait pattern to route messages to the appropriate processing engine
//! (order workflow, result delivery, or patient reconciliation).

use crate::{AckBuilder, Hl7Error, Hl7Parser, MllpFramer};
use crate::order_workflow::OrderWorkflowEngine;
use crate::patient_recon::PatientReconciliationEngine;
use crate::result_delivery::ResultDeliveryEngine;
use std::fmt;
use std::sync::Arc;

/// Handler trait for processing incoming HL7 messages.
///
/// Implementations receive the raw HL7 message string and return
/// either an ACK response string or an error. The MLLP server
/// sends the response back to the client via MLLP framing.
pub trait Hl7MessageHandler: Send + Sync {
    /// Handle an incoming HL7 message.
    ///
    /// The `raw_message` parameter contains the unframed HL7 message
    /// string (without MLLP SB/EB markers). The return value should
    /// be a valid HL7 ACK message string.
    fn handle_message(&self, raw_message: &str) -> Result<String, Hl7Error>;
}

/// MLLP server for receiving HL7 messages.
///
/// Listens on a TCP port for MLLP-framed HL7 messages, unframes them,
/// dispatches to the registered handler, and sends the response back
/// as an MLLP-framed ACK.
///
/// # Note
///
/// The current implementation provides the server structure and message
/// processing pipeline. Actual TCP listener functionality would be
/// implemented using async runtime (tokio) in a production deployment.
pub struct MllpServer {
    /// Address to bind to (e.g., "127.0.0.1:2575").
    bind_address: String,
    /// Message handler.
    handler: Arc<dyn Hl7MessageHandler + Send + Sync>,
}

impl fmt::Debug for MllpServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MllpServer")
            .field("bind_address", &self.bind_address)
            .finish()
    }
}

impl MllpServer {
    /// Create a new MLLP server.
    ///
    /// The server will listen on the given bind address and dispatch
    /// incoming messages to the provided handler.
    pub fn new(bind_address: &str, handler: Arc<dyn Hl7MessageHandler + Send + Sync>) -> Self {
        Self {
            bind_address: bind_address.to_string(),
            handler,
        }
    }

    /// Start the MLLP server.
    ///
    /// # Note
    ///
    /// In the current implementation, this is a stub that validates
    /// the server configuration. A full implementation would start
    /// a TCP listener on the bind address.
    ///
    /// # Errors
    ///
    /// Returns `Hl7Error::ParseFailed` if the bind address is empty.
    pub fn start(&self) -> Result<(), Hl7Error> {
        if self.bind_address.is_empty() {
            return Err(Hl7Error::ParseFailed {
                detail: "MLLP server bind address must not be empty".to_string(),
            });
        }
        // Stub: actual TCP listener would be here in production
        Ok(())
    }

    /// Process a single MLLP-framed message.
    ///
    /// Unframes the raw bytes, passes the message to the handler,
    /// and returns the MLLP-framed response. This is the core
    /// processing loop for each connection.
    pub fn process_mllp_frame(&self, framed_data: &[u8]) -> Result<Vec<u8>, Hl7Error> {
        let raw_message = MllpFramer::unframe(framed_data)?;
        let response = self.handler.handle_message(&raw_message)?;
        Ok(MllpFramer::frame(&response))
    }

    /// Return the bind address.
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }
}

/// Composite handler that routes HL7 messages to the appropriate engine.
///
/// Examines the MSH-9 (message type) field of incoming messages and
/// dispatches them to the correct processing engine:
/// - ORM messages → OrderWorkflowEngine
/// - ORU messages → ResultDeliveryEngine (acknowledgment only)
/// - ADT messages → PatientReconciliationEngine
pub struct Hl7MessageRouter {
    /// Order workflow engine for ORM messages.
    pub order_engine: OrderWorkflowEngine,
    /// Result delivery engine for ORU messages.
    pub result_engine: ResultDeliveryEngine,
    /// Patient reconciliation engine for ADT messages.
    pub recon_engine: PatientReconciliationEngine,
}

impl fmt::Debug for Hl7MessageRouter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Hl7MessageRouter").finish()
    }
}

impl Hl7MessageRouter {
    /// Create a new message router with default engines.
    pub fn new() -> Self {
        Self {
            order_engine: OrderWorkflowEngine::new(),
            result_engine: ResultDeliveryEngine::new(),
            recon_engine: PatientReconciliationEngine::new(),
        }
    }

    /// Create a message router with custom engines.
    pub fn with_engines(
        order_engine: OrderWorkflowEngine,
        result_engine: ResultDeliveryEngine,
        recon_engine: PatientReconciliationEngine,
    ) -> Self {
        Self {
            order_engine,
            result_engine,
            recon_engine,
        }
    }

    /// Determine the message category from MSH-9.
    fn classify_message(raw_message: &str) -> MessageCategory {
        let parser = Hl7Parser::new();
        let segments = parser.parse_segments(raw_message);

        let msh = segments.iter().find(|s| s.segment_id() == "MSH");
        let Some(msh) = msh else {
            return MessageCategory::Unknown;
        };

        // MSH-9 is at field index 8 after split
        let msg_type = msh.field_str(8).unwrap_or("");

        if msg_type.starts_with("ORM") {
            MessageCategory::Orm
        } else if msg_type.starts_with("ORU") {
            MessageCategory::Oru
        } else if msg_type.starts_with("ADT") {
            MessageCategory::Adt
        } else {
            MessageCategory::Unknown
        }
    }
}

impl Default for Hl7MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Message category for routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessageCategory {
    /// ORM — Order message.
    Orm,
    /// ORU — Observation result message.
    Oru,
    /// ADT — Admit/Discharge/Transfer message.
    Adt,
    /// Unknown or unsupported message type.
    Unknown,
}

impl Hl7MessageHandler for Hl7MessageRouter {
    fn handle_message(&self, raw_message: &str) -> Result<String, Hl7Error> {
        let parser = Hl7Parser::new();

        // Extract control ID for ACK
        let segments = parser.parse_segments(raw_message);
        let control_id = segments
            .iter()
            .find(|s| s.segment_id() == "MSH")
            .and_then(|s| s.field_str(9))
            .unwrap_or("UNKNOWN");

        let category = Self::classify_message(raw_message);

        match category {
            MessageCategory::Orm => {
                let orm = parser.parse_orm(raw_message)?;
                let mwl = self.order_engine.process_orm_order(&orm)?;
                Ok(AckBuilder::build_ack(
                    "PACS",
                    "HOSPITAL",
                    control_id,
                    "AA",
                    &format!("MWL entry created: {}", mwl.accession_number),
                ))
            }
            MessageCategory::Adt => {
                let adt = parser.parse_adt(raw_message)?;
                let result = self.recon_engine.process_adt(&adt)?;
                let description = match &result {
                    crate::patient_recon::AdtReconciliationResult::Admit(r) => {
                        format!("Patient admitted: {}", r.patient_id)
                    }
                    crate::patient_recon::AdtReconciliationResult::Update(u) => {
                        format!("Patient updated: {} ({} fields)", u.patient_id, u.updated_fields.len())
                    }
                    crate::patient_recon::AdtReconciliationResult::Merge(m) => {
                        format!("Patient merged: {} -> {}", m.old_id, m.new_id)
                    }
                    crate::patient_recon::AdtReconciliationResult::NoChange { patient_id, event_type } => {
                        format!("No identity change: {} ({})", patient_id, event_type)
                    }
                };
                Ok(AckBuilder::build_ack(
                    "PACS",
                    "HOSPITAL",
                    control_id,
                    "AA",
                    &description,
                ))
            }
            MessageCategory::Oru => {
                // ORU received — typically just acknowledge
                Ok(AckBuilder::build_ack(
                    "PACS",
                    "HOSPITAL",
                    control_id,
                    "AA",
                    "ORU received",
                ))
            }
            MessageCategory::Unknown => {
                Ok(AckBuilder::build_ack(
                    "PACS",
                    "HOSPITAL",
                    control_id,
                    "AR",
                    "Unsupported message type",
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mllp_server_new() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("127.0.0.1:2575", router);
        assert_eq!(server.bind_address(), "127.0.0.1:2575");
    }

    #[test]
    fn mllp_server_start_validates_address() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("", router);
        let result = server.start();
        assert!(result.is_err());
    }

    #[test]
    fn mllp_server_start_accepts_valid_address() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("127.0.0.1:2575", router);
        let result = server.start();
        assert!(result.is_ok());
    }

    #[test]
    fn process_mllp_frame_orm_message() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("127.0.0.1:2575", router);

        let orm_message = "MSH|^~\\&|RIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M\rORC|NW|ORD001|ACC12345||||||Dr^Smith\rOBR||ORD001|ACC12345|CT_CHEST||20240115100000||||Dr^Smith||||||RAD";
        let framed = MllpFramer::frame(orm_message);

        let response_frame = server.process_mllp_frame(&framed).expect("process frame");
        let response = MllpFramer::unframe(&response_frame).expect("unframe response");

        assert!(response.contains("MSH|"));
        assert!(response.contains("ACK"));
        assert!(response.contains("AA"));
        assert!(response.contains("ACC12345"));
    }

    #[test]
    fn process_mllp_frame_adt_message() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("127.0.0.1:2575", router);

        let adt_message = "MSH|^~\\&|HIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ADT^A01|MSG002|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M";
        let framed = MllpFramer::frame(adt_message);

        let response_frame = server.process_mllp_frame(&framed).expect("process frame");
        let response = MllpFramer::unframe(&response_frame).expect("unframe response");

        assert!(response.contains("AA"));
    }

    #[test]
    fn process_mllp_frame_unknown_message() {
        let router = Arc::new(Hl7MessageRouter::new());
        let server = MllpServer::new("127.0.0.1:2575", router);

        let unknown_message = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||SIU^S12|MSG003|P|2.5.1\rPID|||PAT001||Smith";
        let framed = MllpFramer::frame(unknown_message);

        let response_frame = server.process_mllp_frame(&framed).expect("process frame");
        let response = MllpFramer::unframe(&response_frame).expect("unframe response");

        assert!(response.contains("AR"));
    }

    #[test]
    fn classify_message_orm() {
        let msg = "MSH|^~\\&|RIS|HOSP|PACS|HOSP|20240115||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001";
        assert_eq!(Hl7MessageRouter::classify_message(msg), MessageCategory::Orm);
    }

    #[test]
    fn classify_message_adt() {
        let msg = "MSH|^~\\&|HIS|HOSP|PACS|HOSP|20240115||ADT^A01|MSG001|P|2.5.1\rPID|||PAT001";
        assert_eq!(Hl7MessageRouter::classify_message(msg), MessageCategory::Adt);
    }

    #[test]
    fn classify_message_oru() {
        let msg = "MSH|^~\\&|PACS|HOSP|RIS|HOSP|20240115||ORU^R01|MSG001|P|2.5.1\rPID|||PAT001";
        assert_eq!(Hl7MessageRouter::classify_message(msg), MessageCategory::Oru);
    }

    #[test]
    fn classify_message_unknown() {
        let msg = "MSH|^~\\&|APP|FAC|APP|FAC|20240115||SIU^S12|MSG001|P|2.5.1\rPID|||PAT001";
        assert_eq!(Hl7MessageRouter::classify_message(msg), MessageCategory::Unknown);
    }

    #[test]
    fn router_handler_orm_creates_mwl_entry() {
        let router = Hl7MessageRouter::new();
        let msg = "MSH|^~\\&|RIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ORM^O01|MSG001|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M\rORC|NW|ORD001|ACC12345||||||Dr^Smith\rOBR||ORD001|ACC12345|CT_CHEST||20240115100000||||Dr^Smith||||||RAD";

        let response = router.handle_message(msg).expect("handle ORM");
        assert!(response.contains("AA"));
        assert!(response.contains("MWL entry created"));
    }

    #[test]
    fn router_handler_adt_admit() {
        let router = Hl7MessageRouter::new();
        let msg = "MSH|^~\\&|HIS|HOSPITAL|PACS|HOSPITAL|20240115120000||ADT^A01|MSG002|P|2.5.1\rPID|||PAT001||Smith^John||19800101|M";

        let response = router.handle_message(msg).expect("handle ADT");
        assert!(response.contains("AA"));
        assert!(response.contains("Patient admitted"));
    }
}
