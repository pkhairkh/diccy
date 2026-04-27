#![deny(missing_docs)]
#![deny(clippy::cast_possible_truncation)]

//! DIMSE command parsing for Verification (C-ECHO), Storage (C-STORE), and
//! feature-gated Query/Retrieve services.

use dicom_core::{validate_uid_strict, Error, ErrorKind, Result, Tag};
use dicom_net::Pdv;
use std::collections::BTreeSet;
use std::fmt;

/// SOP Class UID for Verification.
pub const SOP_CLASS_VERIFICATION: &str = "1.2.840.10008.1.1";
const TAG_UID: Tag = Tag(0x0000, 0x0002);

// ===========================================================================
// S10-T1: Domain Enums for DIMSE
// ===========================================================================

/// DIMSE status codes with named variants for common DICOM status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimseStatus {
    /// Success (0x0000).
    Success,
    /// Warning - coerced to SOP instance (0xB000).
    WarningCoerced,
    /// Warning - elements discarded (0xB007).
    WarningDiscarded,
    /// Failure - refused (0x0122).
    Refused,
    /// Failure - cannot understand (0x0121).
    CannotUnderstand,
    /// Failure - no such object instance (0x0112).
    NoSuchObjectInstance,
    /// Failure - class not supported (0x0124).
    ClassNotSupported,
    /// Failure - dataset does not match SOP class (0x0110).
    DatasetMismatch,
    /// Cancel (0xFE00).
    Cancel,
    /// Pending (0xFF00).
    Pending,
    /// Pending with warnings (0xFF01).
    PendingWarning,
    /// Unknown/raw status value.
    Other(u16),
}

impl DimseStatus {
    /// Create a DimseStatus from a raw u16 status code.
    pub fn from_u16(raw: u16) -> Self {
        match raw {
            0x0000 => DimseStatus::Success,
            0xB000 => DimseStatus::WarningCoerced,
            0xB007 => DimseStatus::WarningDiscarded,
            0x0122 => DimseStatus::Refused,
            0x0121 => DimseStatus::CannotUnderstand,
            0x0112 => DimseStatus::NoSuchObjectInstance,
            0x0124 => DimseStatus::ClassNotSupported,
            0x0110 => DimseStatus::DatasetMismatch,
            0xFE00 => DimseStatus::Cancel,
            0xFF00 => DimseStatus::Pending,
            0xFF01 => DimseStatus::PendingWarning,
            _ => DimseStatus::Other(raw),
        }
    }

    /// Convert the status to its raw u16 value.
    pub fn as_u16(self) -> u16 {
        match self {
            DimseStatus::Success => 0x0000,
            DimseStatus::WarningCoerced => 0xB000,
            DimseStatus::WarningDiscarded => 0xB007,
            DimseStatus::Refused => 0x0122,
            DimseStatus::CannotUnderstand => 0x0121,
            DimseStatus::NoSuchObjectInstance => 0x0112,
            DimseStatus::ClassNotSupported => 0x0124,
            DimseStatus::DatasetMismatch => 0x0110,
            DimseStatus::Cancel => 0xFE00,
            DimseStatus::Pending => 0xFF00,
            DimseStatus::PendingWarning => 0xFF01,
            DimseStatus::Other(raw) => raw,
        }
    }

    /// Return true if this is a success status.
    pub fn is_success(self) -> bool {
        matches!(self, DimseStatus::Success)
    }

    /// Return true if this is a warning status.
    pub fn is_warning(self) -> bool {
        matches!(
            self,
            DimseStatus::WarningCoerced
                | DimseStatus::WarningDiscarded
                | DimseStatus::PendingWarning
        )
    }

    /// Return true if this is a failure status.
    pub fn is_failure(self) -> bool {
        matches!(
            self,
            DimseStatus::Refused
                | DimseStatus::CannotUnderstand
                | DimseStatus::NoSuchObjectInstance
                | DimseStatus::ClassNotSupported
                | DimseStatus::DatasetMismatch
        )
    }

    /// Return true if this is a pending status.
    pub fn is_pending(self) -> bool {
        matches!(self, DimseStatus::Pending | DimseStatus::PendingWarning)
    }
}

impl fmt::Display for DimseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DimseStatus::Success => write!(f, "Success(0x0000)"),
            DimseStatus::WarningCoerced => write!(f, "Warning-Coerced(0xB000)"),
            DimseStatus::WarningDiscarded => write!(f, "Warning-Discarded(0xB007)"),
            DimseStatus::Refused => write!(f, "Refused(0x0122)"),
            DimseStatus::CannotUnderstand => write!(f, "CannotUnderstand(0x0121)"),
            DimseStatus::NoSuchObjectInstance => write!(f, "NoSuchObjectInstance(0x0112)"),
            DimseStatus::ClassNotSupported => write!(f, "ClassNotSupported(0x0124)"),
            DimseStatus::DatasetMismatch => write!(f, "DatasetMismatch(0x0110)"),
            DimseStatus::Cancel => write!(f, "Cancel(0xFE00)"),
            DimseStatus::Pending => write!(f, "Pending(0xFF00)"),
            DimseStatus::PendingWarning => write!(f, "PendingWarning(0xFF01)"),
            DimseStatus::Other(raw) => write!(f, "Other(0x{raw:04X})"),
        }
    }
}

/// DIMSE message priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    /// Low priority (0x0002).
    Low,
    /// Medium priority (0x0000, default).
    Medium,
    /// High priority (0x0001).
    High,
}

impl Priority {
    /// Convert to the wire format u16 value.
    pub fn as_u16(self) -> u16 {
        match self {
            Priority::Low => 0x0002,
            Priority::Medium => 0x0000,
            Priority::High => 0x0001,
        }
    }

    /// Convert from a wire format u16 value.
    pub fn from_u16(raw: u16) -> Self {
        match raw {
            0x0002 => Priority::Low,
            0x0001 => Priority::High,
            _ => Priority::Medium,
        }
    }
}

impl Default for Priority {
    fn default() -> Self {
        Priority::Medium
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "Low(0x0002)"),
            Priority::Medium => write!(f, "Medium(0x0000)"),
            Priority::High => write!(f, "High(0x0001)"),
        }
    }
}

/// DIMSE parsing limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseLimits {
    /// Max command set bytes.
    pub max_command_bytes: u64,
}

impl Default for DimseLimits {
    fn default() -> Self {
        Self {
            max_command_bytes: 64 * 1024,
        }
    }
}

/// DIMSE messages supported by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DimseMessage {
    /// C-ECHO request.
    CEchoRq {
        /// Message ID.
        message_id: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
    },
    /// C-ECHO response.
    CEchoRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
    },
    /// C-STORE request.
    CStoreRq {
        /// Message ID.
        message_id: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Affected SOP Instance UID.
        sop_instance_uid: String,
        /// Priority value.
        priority: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-STORE response.
    CStoreRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Affected SOP Instance UID.
        sop_instance_uid: String,
    },
    /// N-ACTION request.
    NActionRq {
        /// Message ID.
        message_id: u16,
        /// Requested SOP Class UID.
        sop_class_uid: String,
        /// Requested SOP Instance UID.
        sop_instance_uid: String,
        /// Action Type ID.
        action_type_id: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// N-ACTION response.
    NActionRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Affected SOP Instance UID.
        sop_instance_uid: String,
        /// Action Type ID.
        action_type_id: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-FIND request (feature-gated).
    #[cfg(feature = "dimse-c-find")]
    CFindRq {
        /// Message ID.
        message_id: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Priority value.
        priority: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-FIND response (feature-gated).
    #[cfg(feature = "dimse-c-find")]
    CFindRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-MOVE request (feature-gated).
    #[cfg(feature = "dimse-c-move")]
    CMoveRq {
        /// Message ID.
        message_id: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Move Destination AE title.
        move_destination: String,
        /// Priority value.
        priority: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-MOVE response (feature-gated).
    #[cfg(feature = "dimse-c-move")]
    CMoveRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-GET request (feature-gated).
    #[cfg(feature = "dimse-c-get")]
    CGetRq {
        /// Message ID.
        message_id: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Priority value.
        priority: u16,
        /// Command data set type.
        command_data_set_type: u16,
    },
    /// C-GET response (feature-gated).
    #[cfg(feature = "dimse-c-get")]
    CGetRsp {
        /// Message ID responded to.
        message_id_responded_to: u16,
        /// Status code.
        status: u16,
        /// Affected SOP Class UID.
        sop_class_uid: String,
        /// Command data set type.
        command_data_set_type: u16,
    },
}

/// Parse a command PDV into a DIMSE message.
pub fn parse_command_pdv(pdv: &Pdv, limits: &DimseLimits) -> Result<DimseMessage> {
    if !pdv.is_command() {
        return Err(decode_error("PDV is not a command fragment"));
    }
    if !pdv.is_last() {
        return Err(decode_error("command PDV must be last fragment"));
    }
    parse_command_set(&pdv.data, limits)
}

/// Parse a raw command set (implicit VR little endian).
pub fn parse_command_set(bytes: &[u8], limits: &DimseLimits) -> Result<DimseMessage> {
    enforce_limit(
        "max_command_bytes",
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        limits.max_command_bytes,
    )?;
    let mut cursor = Cursor::new(bytes);
    let mut command_field = None;
    let mut message_id = None;
    let mut message_id_responded_to = None;
    let mut status = None;
    let mut priority = None;
    let mut sop_class_uid = None;
    let mut sop_instance_uid = None;
    let mut command_data_set_type = None;
    let mut action_type_id = None;
    let mut seen_elements = BTreeSet::new();
    #[cfg(feature = "dimse-c-move")]
    let mut move_destination = None;

    while cursor.remaining() > 0 {
        if cursor.remaining() < 8 {
            return Err(decode_error("truncated command element header"));
        }
        let group = cursor.read_u16_le()?;
        let element = cursor.read_u16_le()?;
        let length = usize::try_from(cursor.read_u32_le()?)
            .map_err(|_| decode_error("command element length exceeds usize"))?;
        if cursor.remaining() < length {
            return Err(decode_error("command element length exceeds buffer"));
        }
        let value = cursor.take(length)?;
        if group != 0x0000 {
            return Err(decode_error("non-command group tag in command set"));
        }
        if !seen_elements.insert(element) {
            return Err(decode_error(format!(
                "duplicate command element (0000,{element:04X})"
            )));
        }
        match element {
            0x0000 => {
                let _ = parse_u32(value)?;
            }
            0x0002 => {
                let uid = parse_uid(value)?;
                sop_class_uid = Some(uid);
            }
            0x0100 => {
                command_field = Some(parse_u16(value)?);
            }
            0x0110 => {
                message_id = Some(parse_u16(value)?);
            }
            0x0120 => {
                message_id_responded_to = Some(parse_u16(value)?);
            }
            0x0800 => {
                command_data_set_type = Some(parse_u16(value)?);
            }
            0x0900 => {
                status = Some(parse_u16(value)?);
            }
            0x0700 => {
                priority = Some(parse_u16(value)?);
            }
            #[cfg(feature = "dimse-c-move")]
            0x0600 => {
                move_destination = Some(parse_ae_title(value)?);
            }
            0x1000 => {
                let uid = parse_uid(value)?;
                sop_instance_uid = Some(uid);
            }
            0x1008 => {
                action_type_id = Some(parse_u16(value)?);
            }
            _ => {
                return Err(decode_error(format!(
                    "unsupported command element (0000,{element:04X})"
                )));
            }
        }
    }

    let command_field = command_field.ok_or_else(|| decode_error("missing command field"))?;
    let sop_class_uid =
        sop_class_uid.ok_or_else(|| decode_error("missing affected SOP class UID"))?;

    match command_field {
        0x0030 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type != 0x0101 {
                return Err(decode_error("C-ECHO must have no data set"));
            }
            if sop_class_uid != SOP_CLASS_VERIFICATION {
                return Err(unsupported_sop(sop_class_uid));
            }
            Ok(DimseMessage::CEchoRq {
                message_id,
                sop_class_uid,
            })
        }
        0x8030 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            if sop_class_uid != SOP_CLASS_VERIFICATION {
                return Err(unsupported_sop(sop_class_uid));
            }
            Ok(DimseMessage::CEchoRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
            })
        }
        0x0001 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let sop_instance_uid =
                sop_instance_uid.ok_or_else(|| decode_error("missing SOP instance UID"))?;
            let priority = priority.unwrap_or(0);
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type == 0x0101 {
                return Err(decode_error("C-STORE requires a data set"));
            }
            Ok(DimseMessage::CStoreRq {
                message_id,
                sop_class_uid,
                sop_instance_uid,
                priority,
                command_data_set_type,
            })
        }
        0x8001 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            let sop_instance_uid =
                sop_instance_uid.ok_or_else(|| decode_error("missing SOP instance UID"))?;
            Ok(DimseMessage::CStoreRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
                sop_instance_uid,
            })
        }
        0x0130 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let sop_instance_uid =
                sop_instance_uid.ok_or_else(|| decode_error("missing SOP instance UID"))?;
            let action_type_id =
                action_type_id.ok_or_else(|| decode_error("missing action type id"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type == 0x0101 {
                return Err(decode_error("N-ACTION requires a data set"));
            }
            Ok(DimseMessage::NActionRq {
                message_id,
                sop_class_uid,
                sop_instance_uid,
                action_type_id,
                command_data_set_type,
            })
        }
        0x8130 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            let sop_instance_uid =
                sop_instance_uid.ok_or_else(|| decode_error("missing SOP instance UID"))?;
            let action_type_id =
                action_type_id.ok_or_else(|| decode_error("missing action type id"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            Ok(DimseMessage::NActionRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
                sop_instance_uid,
                action_type_id,
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-find")]
        0x0020 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type == 0x0101 {
                return Err(decode_error("C-FIND requires a data set"));
            }
            Ok(DimseMessage::CFindRq {
                message_id,
                sop_class_uid,
                priority: priority.unwrap_or(0),
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-find")]
        0x8020 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            Ok(DimseMessage::CFindRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-move")]
        0x0021 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type == 0x0101 {
                return Err(decode_error("C-MOVE requires a data set"));
            }
            let move_destination =
                move_destination.ok_or_else(|| decode_error("missing move destination"))?;
            Ok(DimseMessage::CMoveRq {
                message_id,
                sop_class_uid,
                move_destination,
                priority: priority.unwrap_or(0),
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-move")]
        0x8021 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            Ok(DimseMessage::CMoveRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-get")]
        0x0010 => {
            let message_id = message_id.ok_or_else(|| decode_error("missing message ID"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            if command_data_set_type == 0x0101 {
                return Err(decode_error("C-GET requires a data set"));
            }
            Ok(DimseMessage::CGetRq {
                message_id,
                sop_class_uid,
                priority: priority.unwrap_or(0),
                command_data_set_type,
            })
        }
        #[cfg(feature = "dimse-c-get")]
        0x8010 => {
            let message_id_responded_to = message_id_responded_to
                .ok_or_else(|| decode_error("missing message ID responded to"))?;
            let status = status.ok_or_else(|| decode_error("missing status"))?;
            let command_data_set_type = command_data_set_type
                .ok_or_else(|| decode_error("missing command data set type"))?;
            Ok(DimseMessage::CGetRsp {
                message_id_responded_to,
                status,
                sop_class_uid,
                command_data_set_type,
            })
        }
        _ => Err(decode_error("unsupported command field")),
    }
}

/// Build a C-ECHO response command set.
pub fn build_c_echo_response(message_id_responded_to: u16, status: u16) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, SOP_CLASS_VERIFICATION);
    write_us(&mut body, 0x0100, 0x8030);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, 0x0101);
    write_us(&mut body, 0x0900, status);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-ECHO request command set.
pub fn build_c_echo_request(message_id: u16) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, SOP_CLASS_VERIFICATION);
    write_us(&mut body, 0x0100, 0x0030);
    write_us(&mut body, 0x0110, message_id);
    write_us(&mut body, 0x0800, 0x0101);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-STORE response command set.
pub fn build_c_store_response(
    message_id_responded_to: u16,
    status: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x8001);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, 0x0101);
    write_us(&mut body, 0x0900, status);
    write_ui(&mut body, 0x1000, sop_instance_uid);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-STORE request command set.
pub fn build_c_store_request(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    priority: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x0001);
    write_us(&mut body, 0x0110, message_id);
    write_us(&mut body, 0x0700, priority);
    write_us(&mut body, 0x0800, 0x0000);
    write_ui(&mut body, 0x1000, sop_instance_uid);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build an N-ACTION request command set.
pub fn build_n_action_request(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    action_type_id: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x0130);
    write_us(&mut body, 0x0110, message_id);
    write_us(&mut body, 0x0800, 0x0000);
    write_ui(&mut body, 0x1000, sop_instance_uid);
    write_us(&mut body, 0x1008, action_type_id);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build an N-ACTION response command set.
pub fn build_n_action_response(
    message_id_responded_to: u16,
    status: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    action_type_id: u16,
    command_data_set_type: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x8130);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, command_data_set_type);
    write_us(&mut body, 0x0900, status);
    write_ui(&mut body, 0x1000, sop_instance_uid);
    write_us(&mut body, 0x1008, action_type_id);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-FIND request command set.
#[cfg(feature = "dimse-c-find")]
pub fn build_c_find_request(message_id: u16, sop_class_uid: &str, priority: u16) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x0020);
    write_us(&mut body, 0x0110, message_id);
    write_us(&mut body, 0x0700, priority);
    write_us(&mut body, 0x0800, 0x0000);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-FIND response command set.
#[cfg(feature = "dimse-c-find")]
pub fn build_c_find_response(
    message_id_responded_to: u16,
    status: u16,
    sop_class_uid: &str,
    command_data_set_type: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x8020);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, command_data_set_type);
    write_us(&mut body, 0x0900, status);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-MOVE request command set.
#[cfg(feature = "dimse-c-move")]
pub fn build_c_move_request(
    message_id: u16,
    sop_class_uid: &str,
    move_destination: &str,
    priority: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x0021);
    write_us(&mut body, 0x0110, message_id);
    write_ae(&mut body, 0x0600, move_destination);
    write_us(&mut body, 0x0700, priority);
    write_us(&mut body, 0x0800, 0x0000);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-MOVE response command set.
#[cfg(feature = "dimse-c-move")]
pub fn build_c_move_response(
    message_id_responded_to: u16,
    status: u16,
    sop_class_uid: &str,
    command_data_set_type: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x8021);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, command_data_set_type);
    write_us(&mut body, 0x0900, status);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-GET request command set.
#[cfg(feature = "dimse-c-get")]
pub fn build_c_get_request(message_id: u16, sop_class_uid: &str, priority: u16) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x0010);
    write_us(&mut body, 0x0110, message_id);
    write_us(&mut body, 0x0700, priority);
    write_us(&mut body, 0x0800, 0x0000);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

/// Build a C-GET response command set.
#[cfg(feature = "dimse-c-get")]
pub fn build_c_get_response(
    message_id_responded_to: u16,
    status: u16,
    sop_class_uid: &str,
    command_data_set_type: u16,
) -> Vec<u8> {
    let mut body = Vec::new();
    write_ui(&mut body, 0x0002, sop_class_uid);
    write_us(&mut body, 0x0100, 0x8010);
    write_us(&mut body, 0x0120, message_id_responded_to);
    write_us(&mut body, 0x0800, command_data_set_type);
    write_us(&mut body, 0x0900, status);

    let mut out = Vec::new();
    write_ul(
        &mut out,
        0x0000,
        u32::try_from(body.len())
            .map_err(|_| "command set length exceeds u32".to_string())
            .unwrap_or(u32::MAX),
    );
    out.extend_from_slice(&body);
    out
}

fn parse_uid(raw: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(raw).map_err(|_| decode_error("UID not UTF-8"))?;
    let trimmed = text.trim_end_matches('\0').trim_end_matches(' ');
    validate_uid_strict(TAG_UID, trimmed)?;
    Ok(trimmed.to_string())
}

#[cfg(feature = "dimse-c-move")]
fn parse_ae_title(raw: &[u8]) -> Result<String> {
    if raw.len() > 16 {
        return Err(decode_error("AE title length exceeds 16 bytes"));
    }
    if raw.iter().any(|&b| !(0x20..=0x7e).contains(&b)) {
        return Err(decode_error("AE title contains non-ASCII characters"));
    }
    let text = std::str::from_utf8(raw).map_err(|_| decode_error("AE title is not valid UTF-8"))?;
    let trimmed = text.trim_end_matches('\0').trim_end_matches(' ');
    if trimmed.is_empty() {
        return Err(decode_error("AE title must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn parse_u16(raw: &[u8]) -> Result<u16> {
    if raw.len() != 2 {
        return Err(decode_error("expected 2-byte value"));
    }
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn parse_u32(raw: &[u8]) -> Result<u32> {
    if raw.len() != 4 {
        return Err(decode_error("expected 4-byte value"));
    }
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

/// Write an UL (unsigned long) command element to the output buffer.
pub fn write_ul(out: &mut Vec<u8>, element: u16, value: u32) {
    write_tag(out, 0x0000, element);
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

/// Write a US (unsigned short) command element to the output buffer.
pub fn write_us(out: &mut Vec<u8>, element: u16, value: u16) {
    write_tag(out, 0x0000, element);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

/// Write a UI (UID) command element to the output buffer.
pub fn write_ui(out: &mut Vec<u8>, element: u16, uid: &str) {
    write_tag(out, 0x0000, element);
    let mut bytes = uid.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&bytes);
}

#[cfg(feature = "dimse-c-move")]
fn write_ae(out: &mut Vec<u8>, element: u16, ae: &str) {
    write_tag(out, 0x0000, element);
    let mut bytes = ae.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(b' ');
    }
    out.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&bytes);
}

/// Write a DICOM tag (group, element) to the output buffer.
pub fn write_tag(out: &mut Vec<u8>, group: u16, element: u16) {
    out.extend_from_slice(&group.to_le_bytes());
    out.extend_from_slice(&element.to_le_bytes());
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    dicom_util::decode_error("dicom-dimse", &detail.into())
}

fn unsupported_sop(uid: String) -> Box<Error> {
    Error::from_kind(
        ErrorKind::UnsupportedSopClass { sop_class_uid: uid },
        "unsupported SOP class",
    )
    .into()
}

fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    dicom_util::enforce_limit(limit_name, observed, allowed)
}

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if self.remaining() < len {
            return Err(decode_error("truncated buffer"));
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.buf[start..start + len])
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}
