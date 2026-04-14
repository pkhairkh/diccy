//! Human-interface policy primitives for governance, identity, and change control.

use crate::SessionState;
use dicom_core::{Error, ErrorKind, Result};
use std::collections::BTreeMap;

const CLAIM_RESTRICTED_TERMS: [&str; 8] = [
    "diagnostic",
    "diagnose",
    "diagnosis",
    "fda cleared",
    "fda-approved",
    "ce marked",
    "clearance",
    "authorized indication",
];

/// Controlled intended-purpose text used by system metadata views.
pub const APPROVED_INTENDED_PURPOSE_TEXT: &str =
    "RDVF supports deterministic visualization workflows within the declared conformance envelope.";

/// Controlled wording policy for user-visible claim text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlledWordingPolicy {
    /// Approved intended-purpose text.
    pub approved_intended_purpose: String,
}

impl Default for ControlledWordingPolicy {
    fn default() -> Self {
        Self {
            approved_intended_purpose: APPROVED_INTENDED_PURPOSE_TEXT.to_string(),
        }
    }
}

/// Feature profile in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureProfile {
    /// Minimal core profile.
    Core,
    /// Extended workstation profile.
    Workstation,
}

/// Activation state for a feature pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackActivationState {
    /// Pack identifier.
    pub pack_id: String,
    /// Activation flag.
    pub enabled: bool,
}

/// Security posture indicators shown to operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPostureIndicators {
    /// TLS policy label.
    pub tls_policy: String,
    /// Authentication mode label.
    pub auth_mode: String,
    /// Audit mode label.
    pub audit_mode: String,
}

/// Runtime limits currently enforced by policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLimitIndicators {
    /// Maximum accepted input bytes.
    pub max_input_bytes: u64,
    /// Maximum cache entries.
    pub max_cache_entries: u64,
    /// Maximum GPU bytes.
    pub max_gpu_bytes: u64,
    /// Maximum concurrent transport connections.
    pub max_transport_connections: u32,
}

/// Integrity status for policy/configuration checksums.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrityStatus {
    /// Checksum is valid.
    Valid {
        /// Digest string for the validated policy material.
        checksum: String,
    },
    /// Checksum is present but invalid.
    Invalid {
        /// Validation failure rationale.
        reason: String,
    },
    /// Checksum data is missing.
    Missing,
}

/// Workflow availability state for unsupported/out-of-envelope paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedWorkflowState {
    /// Stable workflow identifier.
    pub workflow_id: String,
    /// Visibility flag in the interface.
    pub visible: bool,
    /// Enabled flag for user interaction.
    pub enabled: bool,
    /// Structured rationale.
    pub rationale: String,
}

/// High-risk toggle status with operator-facing risk summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighRiskToggleSummary {
    /// Stable toggle identifier.
    pub toggle_id: String,
    /// Toggle activation state.
    pub enabled: bool,
    /// Risk summary shown before enablement.
    pub risk_summary: String,
}

/// System metadata model rendered by the interface "About/System" view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemMetadataView {
    /// Controlled intended-purpose text.
    pub intended_purpose: String,
    /// Active conformance envelope version.
    pub envelope_version: String,
    /// Application semantic version.
    pub application_version: String,
    /// Build identifier.
    pub build_id: String,
    /// Active profile set.
    pub feature_profile: FeatureProfile,
    /// Feature pack activation states.
    pub pack_activation: Vec<PackActivationState>,
    /// TLS/auth/audit posture indicators.
    pub security_posture: SecurityPostureIndicators,
    /// Deterministic runtime limits.
    pub runtime_limits: RuntimeLimitIndicators,
    /// Policy integrity status.
    pub integrity_status: IntegrityStatus,
    /// Unsupported workflow controls.
    pub unsupported_workflows: Vec<UnsupportedWorkflowState>,
    /// High-risk feature toggle summaries.
    pub high_risk_toggles: Vec<HighRiskToggleSummary>,
}

/// Validate system metadata view content against controlled wording and fail-closed requirements.
pub fn validate_system_metadata_view(
    view: &SystemMetadataView,
    policy: &ControlledWordingPolicy,
) -> Result<()> {
    if view.intended_purpose != policy.approved_intended_purpose {
        return Err(hi_decode_error(
            "intended purpose text does not match approved controlled wording",
        ));
    }
    validate_release_text_input(&view.intended_purpose)?;
    validate_release_text_input(&view.envelope_version)?;
    validate_release_text_input(&view.application_version)?;
    validate_release_text_input(&view.build_id)?;
    validate_release_text_input(&view.security_posture.tls_policy)?;
    validate_release_text_input(&view.security_posture.auth_mode)?;
    validate_release_text_input(&view.security_posture.audit_mode)?;

    if view.envelope_version.trim().is_empty()
        || view.application_version.trim().is_empty()
        || view.build_id.trim().is_empty()
    {
        return Err(hi_decode_error(
            "metadata view must include envelope version, application version, and build identifier",
        ));
    }

    if view.runtime_limits.max_input_bytes == 0
        || view.runtime_limits.max_cache_entries == 0
        || view.runtime_limits.max_gpu_bytes == 0
        || view.runtime_limits.max_transport_connections == 0
    {
        return Err(hi_decode_error(
            "runtime limits must be explicit non-zero values",
        ));
    }

    for workflow in &view.unsupported_workflows {
        if workflow.workflow_id.trim().is_empty() {
            return Err(hi_decode_error(
                "unsupported workflow entries require workflow_id",
            ));
        }
        if workflow.rationale.trim().is_empty() {
            return Err(hi_decode_error(
                "unsupported workflows require structured rationale",
            ));
        }
        if workflow.visible && workflow.enabled {
            return Err(hi_decode_error(
                "unsupported workflows must be hidden or explicitly disabled",
            ));
        }
    }

    for toggle in &view.high_risk_toggles {
        if toggle.toggle_id.trim().is_empty() {
            return Err(hi_decode_error(
                "high-risk toggle entries require toggle_id",
            ));
        }
        if toggle.enabled && toggle.risk_summary.trim().is_empty() {
            return Err(hi_decode_error(
                "high-risk toggles require risk summary before enablement",
            ));
        }
    }

    match &view.integrity_status {
        IntegrityStatus::Valid { checksum } => {
            if checksum.trim().is_empty() {
                return Err(hi_decode_error("valid integrity status requires checksum"));
            }
        }
        IntegrityStatus::Invalid { reason } => {
            if reason.trim().is_empty() {
                return Err(hi_decode_error("invalid integrity status requires reason"));
            }
        }
        IntegrityStatus::Missing => {}
    }

    Ok(())
}

/// Validate user-entered text for claim-surface safety.
pub fn validate_release_text_input(text: &str) -> Result<()> {
    if contains_restricted_claim_terms(text) {
        return Err(hi_decode_error(
            "release/configuration text contains unapproved regulatory claim wording",
        ));
    }
    Ok(())
}

/// Administrative configuration change request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChangeRequest {
    /// Authenticated actor identity.
    pub actor_user_id: String,
    /// Authentication state at request time.
    pub authenticated: bool,
    /// Structured reason code.
    pub reason_code: String,
    /// Configuration key.
    pub key: String,
    /// Previous value.
    pub before: String,
    /// Next value.
    pub after: String,
    /// Timestamp (seconds since epoch).
    pub timestamp_epoch_secs: u64,
}

/// Journal entry for a configuration delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChangeJournalEntry {
    /// Authenticated actor identity.
    pub actor_user_id: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Configuration key.
    pub key: String,
    /// Previous value.
    pub before: String,
    /// Next value.
    pub after: String,
    /// Timestamp (seconds since epoch).
    pub timestamp_epoch_secs: u64,
}

/// Deterministic in-memory configuration journal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigChangeJournal {
    /// Append-only entry list.
    pub entries: Vec<ConfigChangeJournalEntry>,
}

impl ConfigChangeJournal {
    /// Record a configuration change after authentication/reason checks.
    pub fn record_change(
        &mut self,
        request: ConfigChangeRequest,
    ) -> Result<ConfigChangeJournalEntry> {
        if !request.authenticated {
            return Err(hi_decode_error(
                "administrative configuration changes require authenticated identity",
            ));
        }
        if request.actor_user_id.trim().is_empty() || request.reason_code.trim().is_empty() {
            return Err(hi_decode_error(
                "administrative configuration changes require actor identity and reason code",
            ));
        }
        if request.key.trim().is_empty() {
            return Err(hi_decode_error(
                "configuration change key must not be empty",
            ));
        }

        let entry = ConfigChangeJournalEntry {
            actor_user_id: request.actor_user_id,
            reason_code: request.reason_code,
            key: request.key,
            before: request.before,
            after: request.after,
            timestamp_epoch_secs: request.timestamp_epoch_secs,
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }
}

/// Startup readiness checks for fail-closed controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupFailClosedControls {
    /// TLS policy loaded.
    pub tls_policy_loaded: bool,
    /// Auth policy loaded.
    pub auth_policy_loaded: bool,
    /// Audit policy loaded.
    pub audit_policy_loaded: bool,
    /// Claim-surface policy loaded.
    pub claim_surface_policy_loaded: bool,
    /// Integrity checks validated.
    pub integrity_checks_valid: bool,
}

/// Validate startup control readiness and fail closed if required controls are missing.
pub fn validate_startup_fail_closed_controls(controls: StartupFailClosedControls) -> Result<()> {
    if !controls.tls_policy_loaded
        || !controls.auth_policy_loaded
        || !controls.audit_policy_loaded
        || !controls.claim_surface_policy_loaded
        || !controls.integrity_checks_valid
    {
        return Err(hi_decode_error(
            "startup blocked: required fail-closed controls missing or invalid",
        ));
    }
    Ok(())
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

/// Session open request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionOpenRequest {
    /// Session identifier.
    pub session_id: String,
    /// User identity.
    pub principal: String,
    /// Effective role.
    pub role: UserRole,
    /// Security context binding hash.
    pub security_context_hash: String,
    /// Issue timestamp.
    pub issued_epoch_secs: u64,
    /// Expiry timestamp.
    pub expires_epoch_secs: u64,
}

/// User role for high-risk workflow authorization context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserRole {
    /// Read-only viewer role.
    Viewer,
    /// Measurement workflow role.
    MeasuringOperator,
    /// Reporting workflow role.
    Reporter,
    /// Export workflow role.
    Exporter,
    /// Administrative role.
    Administrator,
}

/// Session record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecord {
    /// Session identifier.
    pub session_id: String,
    /// User identity.
    pub principal: String,
    /// Effective role.
    pub effective_role: UserRole,
    /// Security context hash.
    pub security_context_hash: String,
    /// Issue timestamp.
    pub issued_epoch_secs: u64,
    /// Expiry timestamp.
    pub expires_epoch_secs: u64,
    /// Revocation timestamp, if revoked.
    pub revoked_epoch_secs: Option<u64>,
}

/// Reduced session listing entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    /// Session identifier.
    pub session_id: String,
    /// Effective role.
    pub effective_role: UserRole,
    /// Issue timestamp.
    pub issued_epoch_secs: u64,
    /// Expiry timestamp.
    pub expires_epoch_secs: u64,
    /// Revocation flag.
    pub revoked: bool,
}

/// High-risk workflow identity banner model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityBanner {
    /// User identity.
    pub principal: String,
    /// Effective role.
    pub effective_role: UserRole,
}

/// Role transition event for active sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleChangeEvent {
    /// Affected user identity.
    pub principal: String,
    /// Affected session identifier.
    pub session_id: String,
    /// Previous role.
    pub previous_role: UserRole,
    /// Next role.
    pub next_role: UserRole,
    /// Transition timestamp.
    pub changed_epoch_secs: u64,
}

/// Deterministic session directory for identity workflows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionDirectory {
    sessions: BTreeMap<String, SessionRecord>,
}

impl SessionDirectory {
    /// Open a session if identity and token binding prerequisites are valid.
    pub fn open_session(&mut self, request: SessionOpenRequest) -> Result<()> {
        if request.session_id.trim().is_empty() {
            return Err(hi_decode_error("session_id must not be empty"));
        }
        if request.principal.trim().is_empty() || is_shared_identity(&request.principal) {
            return Err(hi_decode_error(
                "shared credentials are not supported; provide unique user identity",
            ));
        }
        if request.security_context_hash.trim().is_empty() {
            return Err(hi_decode_error("security_context_hash must not be empty"));
        }
        if request.expires_epoch_secs <= request.issued_epoch_secs {
            return Err(hi_decode_error("session expiry must be after issue time"));
        }
        if self.sessions.contains_key(&request.session_id) {
            return Err(hi_decode_error("session_id already exists"));
        }
        let record = SessionRecord {
            session_id: request.session_id.clone(),
            principal: request.principal,
            effective_role: request.role,
            security_context_hash: request.security_context_hash,
            issued_epoch_secs: request.issued_epoch_secs,
            expires_epoch_secs: request.expires_epoch_secs,
            revoked_epoch_secs: None,
        };
        self.sessions.insert(request.session_id, record);
        Ok(())
    }

    /// List sessions for the specified identity in deterministic order.
    pub fn list_sessions(&self, principal: &str) -> Vec<SessionSummary> {
        self.sessions
            .values()
            .filter(|record| record.principal == principal)
            .map(|record| SessionSummary {
                session_id: record.session_id.clone(),
                effective_role: record.effective_role,
                issued_epoch_secs: record.issued_epoch_secs,
                expires_epoch_secs: record.expires_epoch_secs,
                revoked: record.revoked_epoch_secs.is_some(),
            })
            .collect()
    }

    /// Revoke a session for the specified identity.
    pub fn revoke_session(
        &mut self,
        principal: &str,
        session_id: &str,
        now_epoch_secs: u64,
    ) -> Result<()> {
        let record = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| hi_decode_error("session not found"))?;
        if record.principal != principal {
            return Err(hi_decode_error(
                "session identity mismatch during revoke operation",
            ));
        }
        record.revoked_epoch_secs = Some(now_epoch_secs);
        Ok(())
    }

    /// Validate a bound session token against identity, context, and expiry.
    pub fn validate_session_token(
        &self,
        principal: &str,
        session_id: &str,
        security_context_hash: &str,
        now_epoch_secs: u64,
    ) -> Result<()> {
        let record = self
            .sessions
            .get(session_id)
            .ok_or_else(|| hi_decode_error("session token not found"))?;
        if record.principal != principal {
            return Err(hi_decode_error("session token principal mismatch"));
        }
        if record.security_context_hash != security_context_hash {
            return Err(hi_decode_error("session token security context mismatch"));
        }
        if record.revoked_epoch_secs.is_some() {
            return Err(hi_decode_error("session token has been revoked"));
        }
        if now_epoch_secs >= record.expires_epoch_secs {
            return Err(hi_decode_error("session token expired"));
        }
        Ok(())
    }

    /// Render identity banner state for high-risk workflow screens.
    pub fn identity_banner(&self, session_id: &str) -> Result<IdentityBanner> {
        let record = self
            .sessions
            .get(session_id)
            .ok_or_else(|| hi_decode_error("session not found"))?;
        Ok(IdentityBanner {
            principal: record.principal.clone(),
            effective_role: record.effective_role,
        })
    }

    /// Apply role changes to all active sessions for a principal.
    pub fn change_role_for_principal(
        &mut self,
        principal: &str,
        next_role: UserRole,
        changed_epoch_secs: u64,
    ) -> Vec<RoleChangeEvent> {
        let mut changed = Vec::new();
        for record in self.sessions.values_mut() {
            if record.principal != principal || record.revoked_epoch_secs.is_some() {
                continue;
            }
            if record.effective_role == next_role {
                continue;
            }
            let previous_role = record.effective_role;
            record.effective_role = next_role;
            changed.push(RoleChangeEvent {
                principal: principal.to_string(),
                session_id: record.session_id.clone(),
                previous_role,
                next_role,
                changed_epoch_secs,
            });
        }
        changed.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        changed
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

/// Operator-facing permission-denied presentation model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDeniedView {
    /// Stable denial code.
    pub code: &'static str,
    /// Plain-language explanation.
    pub message: String,
    /// Deterministic next-step guidance.
    pub next_step: String,
}

/// Build explicit, actionable permission-denied content.
pub fn permission_denied_view(action: &str, required_role: UserRole) -> PermissionDeniedView {
    PermissionDeniedView {
        code: "DVF.HI.AUTH.PERMISSION_DENIED",
        message: format!("The requested action '{action}' is not permitted for your current role."),
        next_step: format!(
            "Re-authenticate with {required_role:?} privileges or request supervised escalation."
        ),
    }
}

/// Result of committing a secret-entry field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretCommitReceipt {
    /// Masked preview of committed value.
    pub masked_preview: String,
    /// Secret length in characters.
    pub length: usize,
}

/// Commit a secret-entry field and clear plaintext content.
pub fn commit_secret_entry(secret: &mut String) -> Result<SecretCommitReceipt> {
    if secret.is_empty() {
        return Err(hi_decode_error("secret value must not be empty"));
    }
    let length = secret.chars().count();
    let masked_preview = "*".repeat(length.min(16));
    secret.clear();
    Ok(SecretCommitReceipt {
        masked_preview,
        length,
    })
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

/// Clipboard/copy policy decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardPolicyDecision {
    /// Copy/export allow state.
    pub allowed: bool,
    /// Audit requirement state.
    pub audited: bool,
    /// Structured rationale.
    pub rationale: String,
}

/// Evaluate clipboard and copy-to-export policy decisions.
pub fn evaluate_clipboard_policy(allowed_by_policy: bool) -> ClipboardPolicyDecision {
    if allowed_by_policy {
        ClipboardPolicyDecision {
            allowed: true,
            audited: true,
            rationale: "policy allows clipboard/export action".to_string(),
        }
    } else {
        ClipboardPolicyDecision {
            allowed: false,
            audited: true,
            rationale: "clipboard/export action denied by policy".to_string(),
        }
    }
}

/// Screenshot/export-to-image policy settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotExportPolicy {
    /// Watermark requirement.
    pub watermark_required: bool,
    /// Identifier-visibility setting.
    pub identifiers_allowed: bool,
}

/// Validate screenshot/export-to-image policy settings.
pub fn validate_screenshot_export_policy(policy: &ScreenshotExportPolicy) -> Result<()> {
    if !policy.watermark_required {
        return Err(hi_decode_error(
            "screenshot/export policy requires watermark enforcement",
        ));
    }
    Ok(())
}

/// Removable-media export authorization request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovableMediaExportRequest {
    /// Requesting identity.
    pub requested_by: String,
    /// Approver identity.
    pub approved_by: String,
    /// Policy authorization code.
    pub policy_authorization_code: String,
}

/// Validate removable-media export authorization controls.
pub fn validate_removable_media_export(request: &RemovableMediaExportRequest) -> Result<()> {
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

/// Force session re-authentication for suspicious session activity.
pub fn force_reauthentication_for_suspicious_session(
    session: &mut crate::SessionStatus,
) -> SessionState {
    session.locked = true;
    SessionState::Locked
}

/// UI string change review record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiStringChangeReview {
    /// Change identifier.
    pub change_id: String,
    /// Claim-surface lint result.
    pub claim_surface_lint_passed: bool,
    /// Controlled wording approver identity.
    pub controlled_wording_approved_by: String,
}

/// Validate a UI string change review record.
pub fn validate_ui_string_change_review(review: &UiStringChangeReview) -> Result<()> {
    if review.change_id.trim().is_empty() {
        return Err(hi_decode_error(
            "UI string change record requires change_id",
        ));
    }
    if !review.claim_surface_lint_passed {
        return Err(hi_decode_error(
            "UI string change review requires claim-surface lint pass",
        ));
    }
    if review.controlled_wording_approved_by.trim().is_empty() {
        return Err(hi_decode_error(
            "UI string change review requires controlled wording approval",
        ));
    }
    Ok(())
}

/// External-impact classification for interface changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceImpactClass {
    /// No externally visible behavior change.
    C0,
    /// Externally visible but low-risk behavior change.
    C1,
    /// High-risk or claim/conformance-affecting behavior change.
    C2,
}

/// Interface change-control record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceChangeControlRecord {
    /// Change identifier.
    pub change_id: String,
    /// External-impact class.
    pub impact_class: InterfaceImpactClass,
    /// Indicates claim boundary impact.
    pub affects_claim_boundary: bool,
    /// Indicates conformance boundary impact.
    pub affects_conformance_boundary: bool,
    /// Evidence/envelope review ticket reference when required.
    pub evidence_review_ticket: Option<String>,
}

/// Validated change-control disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceChangeControlDisposition {
    /// External-impact class.
    pub impact_class: InterfaceImpactClass,
    /// Whether envelope/evidence review is required.
    pub envelope_review_required: bool,
}

/// Validate interface change classification and boundary review requirements.
pub fn validate_interface_change_control_record(
    record: &InterfaceChangeControlRecord,
) -> Result<InterfaceChangeControlDisposition> {
    if record.change_id.trim().is_empty() {
        return Err(hi_decode_error(
            "interface change record requires change_id",
        ));
    }
    let boundary_affected = record.affects_claim_boundary || record.affects_conformance_boundary;
    if boundary_affected {
        let ticket = record.evidence_review_ticket.as_deref().ok_or_else(|| {
            hi_decode_error("boundary-impacting changes require evidence review ticket")
        })?;
        if ticket.trim().is_empty() {
            return Err(hi_decode_error("evidence review ticket must not be empty"));
        }
    }
    Ok(InterfaceChangeControlDisposition {
        impact_class: record.impact_class,
        envelope_review_required: boundary_affected,
    })
}

/// Human-interface requirement revision record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementRevisionRecord {
    /// Requirement identifier.
    pub requirement_id: String,
    /// Version string.
    pub version: String,
    /// Approver signature identifier.
    pub approver_signature: String,
    /// Effective date in `YYYY-MM-DD`.
    pub effective_date: String,
}

/// Validate requirement revision governance metadata.
pub fn validate_requirement_revision_record(record: &RequirementRevisionRecord) -> Result<()> {
    if record.requirement_id.trim().is_empty()
        || record.version.trim().is_empty()
        || record.approver_signature.trim().is_empty()
    {
        return Err(hi_decode_error(
            "requirement revisions require requirement_id, version, and approver signature",
        ));
    }
    if !is_iso_date(&record.effective_date) {
        return Err(hi_decode_error(
            "requirement revisions require effective_date in YYYY-MM-DD format",
        ));
    }
    Ok(())
}

/// Determine whether a session state requires operator re-authentication.
pub fn session_requires_reauthentication(state: SessionState) -> bool {
    matches!(state, SessionState::Locked)
}

fn is_shared_identity(principal: &str) -> bool {
    let lowered = principal.to_ascii_lowercase();
    lowered.contains("shared")
}

fn contains_restricted_claim_terms(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    CLAIM_RESTRICTED_TERMS
        .iter()
        .any(|term| lowered.contains(term))
}

fn is_iso_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    for (idx, byte) in bytes.iter().enumerate() {
        let is_dash = idx == 4 || idx == 7;
        if is_dash {
            if *byte != b'-' {
                return false;
            }
            continue;
        }
        if !byte.is_ascii_digit() {
            return false;
        }
    }
    true
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
