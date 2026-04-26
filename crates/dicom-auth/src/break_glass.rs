//! Break-glass authorization bounded-context module.
//!
//! Contains `BreakGlassPolicy`, `BreakGlassRequest`, `BreakGlassGrant`, `BreakGlassOutcome`,
//! and secure-default override types.

use dicom_core::{Error, ErrorKind, Result};

/// Policy configuration for break-glass access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlassPolicy {
    /// Maximum duration in seconds for a break-glass grant.
    pub max_duration_secs: u64,
    /// Whether post-event audit review is required.
    pub post_event_audit_required: bool,
}

impl Default for BreakGlassPolicy {
    fn default() -> Self {
        Self {
            max_duration_secs: 3600,
            post_event_audit_required: true,
        }
    }
}

/// Break-glass activation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlassRequest {
    /// Requesting user identity.
    pub principal: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Scope identifier for temporary elevated access.
    pub scope: String,
    /// Approver identity.
    pub approved_by: String,
    /// Current timestamp.
    pub now_epoch_secs: u64,
    /// Requested expiry timestamp.
    pub expires_epoch_secs: u64,
}

/// Activated break-glass grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakGlassGrant {
    /// Requesting user identity.
    pub principal: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Scope identifier.
    pub scope: String,
    /// Approver identity.
    pub approved_by: String,
    /// Grant activation time.
    pub activated_epoch_secs: u64,
    /// Grant expiry time.
    pub expires_epoch_secs: u64,
    /// Post-event audit review requirement.
    pub post_event_audit_review_required: bool,
}

/// Outcome of a break-glass request evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakGlassOutcome {
    /// Break-glass access granted.
    Granted,
    /// Break-glass access denied.
    Denied,
}

/// Activate break-glass access when request metadata is complete and bounded.
pub fn activate_break_glass_access(
    request: &BreakGlassRequest,
    max_duration_secs: u64,
) -> Result<BreakGlassGrant> {
    if request.principal.trim().is_empty()
        || request.reason_code.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.approved_by.trim().is_empty()
    {
        return Err(hi_decode_error(
            "break-glass activation requires principal, reason, scope, and approver",
        ));
    }
    if request.expires_epoch_secs <= request.now_epoch_secs {
        return Err(hi_decode_error(
            "break-glass expiry must be after activation time",
        ));
    }
    if request.expires_epoch_secs - request.now_epoch_secs > max_duration_secs.max(1) {
        return Err(hi_decode_error("break-glass duration exceeds policy bound"));
    }
    Ok(BreakGlassGrant {
        principal: request.principal.clone(),
        reason_code: request.reason_code.clone(),
        scope: request.scope.clone(),
        approved_by: request.approved_by.clone(),
        activated_epoch_secs: request.now_epoch_secs,
        expires_epoch_secs: request.expires_epoch_secs,
        post_event_audit_review_required: true,
    })
}

/// Time-bounded override request that relaxes secure defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecureDefaultOverrideRequest {
    /// Requesting operator identity.
    pub requested_by: String,
    /// Approver identity.
    pub approved_by: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Current timestamp.
    pub now_epoch_secs: u64,
    /// Requested expiry timestamp.
    pub expires_epoch_secs: u64,
}

/// Activated secure-default override details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSecureDefaultOverride {
    /// Requesting operator identity.
    pub requested_by: String,
    /// Approver identity.
    pub approved_by: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Start timestamp.
    pub activated_epoch_secs: u64,
    /// Expiry timestamp.
    pub expires_epoch_secs: u64,
}

/// Activate an override when approval and bounded expiry constraints are met.
pub fn activate_secure_default_override(
    request: &SecureDefaultOverrideRequest,
    max_duration_secs: u64,
) -> Result<ActiveSecureDefaultOverride> {
    if request.requested_by.trim().is_empty()
        || request.approved_by.trim().is_empty()
        || request.reason_code.trim().is_empty()
    {
        return Err(hi_decode_error(
            "secure-default override requires requestor, approver, and reason code",
        ));
    }
    if request.expires_epoch_secs <= request.now_epoch_secs {
        return Err(hi_decode_error(
            "secure-default override expiry must be after activation time",
        ));
    }
    let duration = request.expires_epoch_secs - request.now_epoch_secs;
    if duration > max_duration_secs.max(1) {
        return Err(hi_decode_error(
            "secure-default override duration exceeds policy bound",
        ));
    }
    Ok(ActiveSecureDefaultOverride {
        requested_by: request.requested_by.clone(),
        approved_by: request.approved_by.clone(),
        reason_code: request.reason_code.clone(),
        activated_epoch_secs: request.now_epoch_secs,
        expires_epoch_secs: request.expires_epoch_secs,
    })
}

fn hi_decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-auth".to_string(),
            detail: detail.into(),
        },
        "human-interface policy error",
    )
    .into()
}
