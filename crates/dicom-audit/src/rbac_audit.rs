//! RBAC permission check audit events.
//!
//! Generates audit events for all RBAC permission check outcomes (both
//! allowed and denied), which is required for regulatory compliance under
//! IEC 62304 and IHE ATNA profiles.

use crate::{AuditEvent, AuditEventKind, AuditField, AuditValue};

/// RBAC access decision outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbacDecision {
    /// Access was allowed.
    Allowed,
    /// Access was denied.
    Denied,
}

impl RbacDecision {
    /// Return the decision as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            RbacDecision::Allowed => "Allowed",
            RbacDecision::Denied => "Denied",
        }
    }
}

/// RBAC permission check audit event details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbacAuditEvent {
    /// ISO 8601 timestamp of the access check.
    pub timestamp: String,
    /// Principal (user or service account) that requested access.
    pub principal: String,
    /// Role assigned to the principal.
    pub role: String,
    /// Action that was attempted.
    pub action: String,
    /// Resource that was accessed.
    pub resource: String,
    /// Access decision outcome.
    pub decision: RbacDecision,
    /// Optional reason for the decision (especially for denials).
    pub reason: Option<String>,
}

/// Generate an RBAC audit event for a permission check outcome.
///
/// Creates a structured audit event that records the principal, role,
/// action, resource, and decision (allowed or denied). Denied decisions
/// are always recorded; allowed decisions are recorded for completeness
/// per IHE ATNA requirements.
///
/// # Arguments
///
/// * `principal` - The user or service account requesting access.
/// * `role` - The role assigned to the principal.
/// * `action` - The action being attempted (e.g., "ReadStudy").
/// * `resource` - The resource being accessed (e.g., study UID).
/// * `decision` - Whether access was allowed or denied.
///
/// # Returns
///
/// An [`AuditEvent`] ready for recording in an [`AuditLog`](crate::AuditLog).
pub fn audit_rbac_check(
    principal: &str,
    role: &str,
    action: &str,
    resource: &str,
    decision: RbacDecision,
) -> AuditEvent {
    let mut fields = vec![
        AuditField {
            key: "event_type",
            value: AuditValue::Plain("rbac_permission_check".to_string()),
        },
        AuditField {
            key: "principal",
            value: AuditValue::Sensitive(principal.to_string()),
        },
        AuditField {
            key: "role",
            value: AuditValue::Plain(role.to_string()),
        },
        AuditField {
            key: "action",
            value: AuditValue::Plain(action.to_string()),
        },
        AuditField {
            key: "resource",
            value: AuditValue::Sensitive(resource.to_string()),
        },
        AuditField {
            key: "decision",
            value: AuditValue::Plain(decision.as_str().to_string()),
        },
    ];

    // Add reason for denied decisions
    if decision == RbacDecision::Denied {
        fields.push(AuditField {
            key: "reason",
            value: AuditValue::Plain(format!(
                "role '{}' does not have '{}' permission",
                role, action
            )),
        });
    }

    AuditEvent {
        kind: AuditEventKind::AuthzDecision,
        fields,
    }
}

/// Generate an RBAC audit event with an explicit reason.
///
/// Like [`audit_rbac_check`] but allows specifying a custom reason
/// for the decision, useful for break-glass overrides or policy
/// violations.
pub fn audit_rbac_check_with_reason(
    principal: &str,
    role: &str,
    action: &str,
    resource: &str,
    decision: RbacDecision,
    reason: &str,
) -> AuditEvent {
    let mut event = audit_rbac_check(principal, role, action, resource, decision);
    event.fields.push(AuditField {
        key: "reason",
        value: AuditValue::Plain(reason.to_string()),
    });
    event
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rbac_audit_allowed_event() {
        let event = audit_rbac_check(
            "dr.smith",
            "Radiologist",
            "ReadStudy",
            "1.2.840.113619.2.55.3",
            RbacDecision::Allowed,
        );

        assert_eq!(event.kind, AuditEventKind::AuthzDecision);

        // Find the decision field
        let decision_field = event
            .fields
            .iter()
            .find(|f| f.key == "decision")
            .expect("must have decision field");
        assert_eq!(decision_field.value, AuditValue::Plain("Allowed".to_string()));

        // Allowed events should not have a reason
        let reason_field = event.fields.iter().find(|f| f.key == "reason");
        assert!(reason_field.is_none(), "allowed events should not auto-generate reason");
    }

    #[test]
    fn rbac_audit_denied_event() {
        let event = audit_rbac_check(
            "nurse.jones",
            "Technologist",
            "DeleteStudy",
            "1.2.840.113619.2.55.3",
            RbacDecision::Denied,
        );

        assert_eq!(event.kind, AuditEventKind::AuthzDecision);

        let decision_field = event
            .fields
            .iter()
            .find(|f| f.key == "decision")
            .expect("must have decision field");
        assert_eq!(decision_field.value, AuditValue::Plain("Denied".to_string()));

        // Denied events should have a reason
        let reason_field = event.fields.iter().find(|f| f.key == "reason");
        assert!(reason_field.is_some(), "denied events must have a reason");
    }

    #[test]
    fn rbac_audit_event_has_required_fields() {
        let event = audit_rbac_check(
            "user",
            "Role",
            "Action",
            "Resource",
            RbacDecision::Allowed,
        );

        let keys: Vec<&str> = event.fields.iter().map(|f| f.key).collect();
        assert!(keys.contains(&"event_type"), "must have event_type");
        assert!(keys.contains(&"principal"), "must have principal");
        assert!(keys.contains(&"role"), "must have role");
        assert!(keys.contains(&"action"), "must have action");
        assert!(keys.contains(&"resource"), "must have resource");
        assert!(keys.contains(&"decision"), "must have decision");
    }

    #[test]
    fn rbac_audit_principal_and_resource_are_sensitive() {
        let event = audit_rbac_check(
            "dr.smith",
            "Radiologist",
            "ReadStudy",
            "1.2.840.113619",
            RbacDecision::Allowed,
        );

        let principal_field = event.fields.iter().find(|f| f.key == "principal").unwrap();
        assert!(
            matches!(principal_field.value, AuditValue::Sensitive(_)),
            "principal must be marked Sensitive"
        );

        let resource_field = event.fields.iter().find(|f| f.key == "resource").unwrap();
        assert!(
            matches!(resource_field.value, AuditValue::Sensitive(_)),
            "resource must be marked Sensitive"
        );
    }

    #[test]
    fn rbac_audit_with_custom_reason() {
        let event = audit_rbac_check_with_reason(
            "dr.emergency",
            "Radiologist",
            "DeleteStudy",
            "1.2.840.113619",
            RbacDecision::Allowed,
            "break-glass override: emergency procedure",
        );

        let reason_field = event.fields.iter().find(|f| f.key == "reason");
        assert!(reason_field.is_some(), "must have reason field");
        if let Some(field) = reason_field {
            match &field.value {
                AuditValue::Plain(reason) => {
                    assert!(reason.contains("break-glass"), "reason must contain break-glass");
                }
                _ => panic!("reason must be Plain value"),
            }
        }
    }

    #[test]
    fn rbac_decision_as_str() {
        assert_eq!(RbacDecision::Allowed.as_str(), "Allowed");
        assert_eq!(RbacDecision::Denied.as_str(), "Denied");
    }

    #[test]
    fn rbac_audit_event_validates() {
        let event = audit_rbac_check(
            "user",
            "Role",
            "Action",
            "Resource",
            RbacDecision::Allowed,
        );
        assert!(event.validate().is_ok(), "RBAC audit event must pass validation");
    }
}
