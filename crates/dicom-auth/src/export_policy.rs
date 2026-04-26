//! Export policy bounded-context module.
//!
//! Contains `ClipboardPolicy`, `RemovableMediaPolicy`, and `WorkspacePrivacyMode`.

use dicom_core::{Error, ErrorKind, Result};

/// Backward-compatible alias for [`ClipboardPolicy`].
pub type ClipboardPolicyDecision = ClipboardPolicy;

/// Backward-compatible alias for [`RemovableMediaPolicy`].
pub type RemovableMediaExportRequest = RemovableMediaPolicy;

/// Clipboard/copy policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPolicy {
    /// Copy/export allow state.
    pub allowed: bool,
    /// Audit requirement state.
    pub audited: bool,
    /// Structured rationale.
    pub rationale: String,
}

/// Evaluate clipboard and copy-to-export policy decisions.
pub fn evaluate_clipboard_policy(allowed_by_policy: bool) -> ClipboardPolicy {
    if allowed_by_policy {
        ClipboardPolicy {
            allowed: true,
            audited: true,
            rationale: "policy allows clipboard/export action".to_string(),
        }
    } else {
        ClipboardPolicy {
            allowed: false,
            audited: true,
            rationale: "clipboard/export action denied by policy".to_string(),
        }
    }
}

/// Removable media export authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovableMediaPolicy {
    /// Requesting identity.
    pub requested_by: String,
    /// Approver identity.
    pub approved_by: String,
    /// Policy authorization code.
    pub policy_authorization_code: String,
}

/// Validate removable-media export authorization controls.
pub fn validate_removable_media_export(request: &RemovableMediaPolicy) -> Result<()> {
    if request.requested_by.trim().is_empty()
        || request.approved_by.trim().is_empty()
        || request.policy_authorization_code.trim().is_empty()
    {
        return Err(hi_decode_error(
            "removable-media export requires explicit policy-compliant authorization",
        ));
    }
    Ok(())
}

/// Privacy-mode state for non-clinical workspaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePrivacyMode {
    /// True when running in clinical interpretation mode.
    pub clinical_mode: bool,
    /// True when identifiers are masked.
    pub mask_identifiers: bool,
}

/// Determine default identifier masking behavior for workspace mode.
pub fn workspace_privacy_mode(clinical_mode: bool) -> WorkspacePrivacyMode {
    WorkspacePrivacyMode {
        clinical_mode,
        mask_identifiers: !clinical_mode,
    }
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
