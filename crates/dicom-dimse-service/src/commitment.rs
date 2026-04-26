//! Storage Commitment types and N-ACTION request parsing.

use crate::{decode_error, AssociationInfo};
use dicom_core::{Limits, Result, Tag, Value};
use dicom_io::parse_dataset_bytes;
use dicom_storage::{StorageCommitmentReferencedInstance, StorageCommitmentState};
use std::sync::Arc;

pub(crate) const TAG_TRANSACTION_UID: Tag = Tag(0x0008, 0x1195);
pub(crate) const TAG_REFERENCED_SOP_SEQUENCE: Tag = Tag(0x0008, 0x1199);
pub(crate) const TAG_REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
pub(crate) const TAG_REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

/// Storage Commitment N-ACTION request payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentNActionRequest {
    /// Message ID.
    pub message_id: u16,
    /// Transaction UID for request lifecycle tracking.
    pub transaction_uid: String,
    /// Calling AE title.
    pub calling_ae_title: String,
    /// Called AE title.
    pub called_ae_title: String,
    /// Referenced SOP instances.
    pub referenced_instances: Vec<StorageCommitmentReferencedInstance>,
    /// Association metadata.
    pub association: AssociationInfo,
}

/// Storage Commitment lifecycle event forwarded to workflow/audit sinks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentLifecycleEvent {
    /// Transaction UID.
    pub transaction_uid: String,
    /// Current state after transition.
    pub state: StorageCommitmentState,
    /// Deterministic event code.
    pub event_code: &'static str,
}

/// Callback sink for Storage Commitment lifecycle transitions.
///
/// # S13-T7 — Read-Only Trait Object
///
/// `Fn(StorageCommitmentLifecycleEvent) -> Result<()>` takes `&self`,
/// so `Arc<StorageCommitmentLifecycleSink>` is safe for concurrent
/// read-only invocation.
pub type StorageCommitmentLifecycleSink =
    Arc<dyn Fn(StorageCommitmentLifecycleEvent) -> Result<()> + Send + Sync>;

/// Parse a Storage Commitment N-ACTION request from a DIMSE data set.
pub(crate) fn parse_storage_commitment_n_action_request(
    message_id: u16,
    sop_class_uid: &str,
    sop_instance_uid: &str,
    data_set: &[u8],
    association: &AssociationInfo,
    limits: &Limits,
) -> Result<StorageCommitmentNActionRequest> {
    let action_information =
        parse_dataset_bytes(data_set, &association.transfer_syntax_uid, limits)?;
    let transaction_uid = action_information
        .get_uid_strict(TAG_TRANSACTION_UID, limits)?
        .ok_or_else(|| decode_error("missing Storage Commitment transaction UID"))?
        .to_string();
    let referenced_instances = match action_information.get(TAG_REFERENCED_SOP_SEQUENCE) {
        Some(element) => match element.value() {
            Value::Sequence(items) => {
                if items.is_empty() {
                    vec![StorageCommitmentReferencedInstance {
                        sop_class_uid: sop_class_uid.to_string(),
                        sop_instance_uid: sop_instance_uid.to_string(),
                    }]
                } else {
                    let mut out = Vec::with_capacity(items.len());
                    for item in items {
                        let referenced_sop_class_uid = item
                            .get_uid_strict(TAG_REFERENCED_SOP_CLASS_UID, limits)?
                            .ok_or_else(|| {
                                decode_error(
                                    "missing Referenced SOP Class UID in Storage Commitment sequence item",
                                )
                            })?;
                        let referenced_sop_instance_uid = item
                            .get_uid_strict(TAG_REFERENCED_SOP_INSTANCE_UID, limits)?
                            .ok_or_else(|| {
                                decode_error(
                                    "missing Referenced SOP Instance UID in Storage Commitment sequence item",
                                )
                            })?;
                        out.push(StorageCommitmentReferencedInstance {
                            sop_class_uid: referenced_sop_class_uid.to_string(),
                            sop_instance_uid: referenced_sop_instance_uid.to_string(),
                        });
                    }
                    out
                }
            }
            _ => {
                return Err(decode_error(
                    "Storage Commitment Referenced SOP Sequence must be encoded as SQ",
                ))
            }
        },
        None => vec![StorageCommitmentReferencedInstance {
            sop_class_uid: sop_class_uid.to_string(),
            sop_instance_uid: sop_instance_uid.to_string(),
        }],
    };

    Ok(StorageCommitmentNActionRequest {
        message_id,
        transaction_uid,
        calling_ae_title: association.calling_ae.clone(),
        called_ae_title: association.called_ae.clone(),
        referenced_instances,
        association: association.clone(),
    })
}
