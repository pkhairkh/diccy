//! Instance Availability Notification (IAN) types.

/// Instance Availability Notification (IAN) payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IanNotification {
    /// Stable event id.
    pub event_id: String,
    /// Study Instance UID.
    pub study_instance_uid: String,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// Outcome status code.
    pub status_code: u16,
}
