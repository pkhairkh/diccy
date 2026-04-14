#![deny(missing_docs)]

//! DIMSE command parsing for Verification (C-ECHO), Storage (C-STORE), and
//! feature-gated Query/Retrieve services.

use dicom_core::{validate_uid_strict, Error, ErrorKind, Result, Tag};
use dicom_net::Pdv;
use std::collections::BTreeSet;

const SOP_CLASS_VERIFICATION: &str = "1.2.840.10008.1.1";
const TAG_UID: Tag = Tag(0x0000, 0x0002);

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
        bytes.len() as u64,
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
        let length = cursor.read_u32_le()? as usize;
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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
    write_ul(&mut out, 0x0000, body.len() as u32);
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

fn write_ul(out: &mut Vec<u8>, element: u16, value: u32) {
    write_tag(out, 0x0000, element);
    out.extend_from_slice(&4u32.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_us(out: &mut Vec<u8>, element: u16, value: u16) {
    write_tag(out, 0x0000, element);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_ui(out: &mut Vec<u8>, element: u16, uid: &str) {
    write_tag(out, 0x0000, element);
    let mut bytes = uid.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
}

#[cfg(feature = "dimse-c-move")]
fn write_ae(out: &mut Vec<u8>, element: u16, ae: &str) {
    write_tag(out, 0x0000, element);
    let mut bytes = ae.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(b' ');
    }
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
}

fn write_tag(out: &mut Vec<u8>, group: u16, element: u16) {
    out.extend_from_slice(&group.to_le_bytes());
    out.extend_from_slice(&element.to_le_bytes());
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-dimse".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn unsupported_sop(uid: String) -> Box<Error> {
    Error::from_kind(
        ErrorKind::UnsupportedSopClass { sop_class_uid: uid },
        "unsupported SOP class",
    )
    .into()
}

fn enforce_limit(limit_name: &'static str, observed: u64, allowed: u64) -> Result<()> {
    if observed > allowed {
        return Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name,
                observed,
                allowed,
            },
            "limit exceeded",
        )
        .into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::ErrorKind;

    #[test]
    fn parse_c_echo_request() {
        // REQ-DIMSE-300: Verification SOP C-ECHO requests must parse deterministically.
        let bytes = build_c_echo_request(7);
        let msg = parse_command_set(&bytes, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CEchoRq { message_id, .. } => {
                assert_eq!(message_id, 7);
            }
            _ => panic!("expected C-ECHO request"),
        }
    }

    #[test]
    fn parse_rejects_unsupported_sop() {
        // REQ-DIMSE-301: Unsupported SOP classes fail closed.
        // REQ-DIMSE-305: Unsupported SOP class UIDs map to UnsupportedSopClass.
        let mut bytes = build_c_echo_request(1);
        let idx = bytes
            .windows(SOP_CLASS_VERIFICATION.len())
            .position(|w| w == SOP_CLASS_VERIFICATION.as_bytes())
            .expect("uid position");
        bytes[idx] = b'9';
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::UnsupportedSopClass { .. }));
    }

    #[test]
    fn decode_error_stage_is_dicom_dimse() {
        // REQ-DIMSE-305: DIMSE command parse failures surface DecodeError with dicom-dimse stage.
        let err = parse_command_set(&[], &DimseLimits::default()).expect_err("error");
        match err.kind {
            ErrorKind::DecodeError { stage, .. } => {
                assert_eq!(stage, "dicom-dimse");
            }
            _ => panic!("expected decode error"),
        }
    }

    #[test]
    fn command_length_limit_enforced() {
        // REQ-SEC-401: DIMSE command limits are enforced.
        let bytes = build_c_echo_request(1);
        let limits = DimseLimits {
            max_command_bytes: 8,
        };
        let err = parse_command_set(&bytes, &limits).expect_err("error");
        match err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "max_command_bytes");
            }
            _ => panic!("expected limit exceeded"),
        }
    }

    #[test]
    fn parse_c_store_request() {
        // REQ-DIMSE-302: C-STORE requests must parse deterministically.
        let bytes = build_c_store_request(9, "1.2.840.10008.5.1.4.1.1.2", "1.2.3.4.5", 0);
        let msg = parse_command_set(&bytes, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CStoreRq {
                message_id,
                sop_instance_uid,
                ..
            } => {
                assert_eq!(message_id, 9);
                assert_eq!(sop_instance_uid, "1.2.3.4.5");
            }
            _ => panic!("expected C-STORE request"),
        }
    }

    #[cfg(not(feature = "dimse-c-find"))]
    #[test]
    fn parse_rejects_unsupported_command_field() {
        // REQ-DIMSE-304: Unsupported DIMSE commands must fail closed.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.1.1");
        write_us(&mut body, 0x0100, 0x0020);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(&mut out, 0x0000, body.len() as u32);
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[cfg(not(feature = "dimse-c-move"))]
    #[test]
    fn parse_rejects_unsupported_c_move_command_field() {
        // REQ-DIMSE-304: Deferred C-MOVE commands must fail closed when feature is disabled.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.2.2");
        write_us(&mut body, 0x0100, 0x0021);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(&mut out, 0x0000, body.len() as u32);
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[cfg(not(feature = "dimse-c-get"))]
    #[test]
    fn parse_rejects_unsupported_c_get_command_field() {
        // REQ-DIMSE-304: Deferred C-GET commands must fail closed when feature is disabled.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, "1.2.840.10008.5.1.4.1.2.2.3");
        write_us(&mut body, 0x0100, 0x0010);
        write_us(&mut body, 0x0110, 1);
        write_us(&mut body, 0x0800, 0x0000);

        let mut out = Vec::new();
        write_ul(&mut out, 0x0000, body.len() as u32);
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_unknown_command_element() {
        // REQ-DIMSE-304: Unsupported command elements fail closed.
        let mut bytes = build_c_echo_request(1);
        let mut unknown = Vec::new();
        write_us(&mut unknown, 0x9999, 1);
        bytes.extend_from_slice(&unknown);
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_duplicate_command_elements() {
        // REQ-DIMSE-304: Duplicate command elements fail closed.
        let mut bytes = build_c_echo_request(1);
        let mut duplicate = Vec::new();
        write_us(&mut duplicate, 0x0110, 2);
        bytes.extend_from_slice(&duplicate);
        let err = parse_command_set(&bytes, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_rejects_out_of_envelope_command_field() {
        // REQ-DIMSE-301/304: out-of-envelope DIMSE command fields must fail closed.
        let mut body = Vec::new();
        write_ui(&mut body, 0x0002, SOP_CLASS_VERIFICATION);
        write_us(&mut body, 0x0100, 0x7FFF);
        write_us(&mut body, 0x0110, 1);

        let mut out = Vec::new();
        write_ul(&mut out, 0x0000, body.len() as u32);
        out.extend_from_slice(&body);

        let err = parse_command_set(&out, &DimseLimits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[cfg(feature = "dimse-c-find")]
    #[test]
    fn parse_c_find_request_response() {
        // REQ-DIMSE-310: C-FIND command sets must parse deterministically when enabled.
        let request = build_c_find_request(5, "1.2.840.10008.5.1.4.1.2.1.1", 0);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CFindRq {
                message_id,
                sop_class_uid,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 5);
                assert_eq!(sop_class_uid, "1.2.840.10008.5.1.4.1.2.1.1");
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-FIND request"),
        }

        let response = build_c_find_response(5, 0x0000, "1.2.840.10008.5.1.4.1.2.1.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CFindRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 5);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-FIND response"),
        }
    }

    #[cfg(feature = "dimse-c-move")]
    #[test]
    fn parse_c_move_request_response() {
        // REQ-DIMSE-320: C-MOVE command sets must parse deterministically when enabled.
        let request = build_c_move_request(7, "1.2.840.10008.5.1.4.1.2.2.1", "DEST_AE", 1);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CMoveRq {
                message_id,
                move_destination,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 7);
                assert_eq!(move_destination, "DEST_AE");
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-MOVE request"),
        }

        let response = build_c_move_response(7, 0x0000, "1.2.840.10008.5.1.4.1.2.2.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CMoveRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 7);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-MOVE response"),
        }
    }

    #[cfg(feature = "dimse-c-get")]
    #[test]
    fn parse_c_get_request_response() {
        // REQ-DIMSE-330: C-GET command sets must parse deterministically when enabled.
        let request = build_c_get_request(9, "1.2.840.10008.5.1.4.1.2.3.1", 2);
        let msg = parse_command_set(&request, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CGetRq {
                message_id,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id, 9);
                assert_eq!(command_data_set_type, 0x0000);
            }
            _ => panic!("expected C-GET request"),
        }

        let response = build_c_get_response(9, 0x0000, "1.2.840.10008.5.1.4.1.2.3.1", 0x0101);
        let msg = parse_command_set(&response, &DimseLimits::default()).expect("parse");
        match msg {
            DimseMessage::CGetRsp {
                message_id_responded_to,
                status,
                command_data_set_type,
                ..
            } => {
                assert_eq!(message_id_responded_to, 9);
                assert_eq!(status, 0x0000);
                assert_eq!(command_data_set_type, 0x0101);
            }
            _ => panic!("expected C-GET response"),
        }
    }
}
