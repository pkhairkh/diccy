//! Session policy and status bounded-context module.
//!
//! Contains `SessionPolicy`, `SessionStatus`, session directory, role management,
//! and session state machine methods.

use crate::SessionState;
use dicom_core::{Error, ErrorKind, Result};
use std::collections::BTreeMap;

/// Deterministic session policy for timeout and lock controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    /// Inactivity timeout in seconds.
    pub inactivity_timeout_secs: u64,
    /// Warning window before timeout in seconds.
    pub warning_window_secs: u64,
    /// Authentication failures that trigger lock.
    pub max_failures: u32,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            inactivity_timeout_secs: 900,
            warning_window_secs: 60,
            max_failures: 5,
        }
    }
}

/// Session status tracked for deterministic lock transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    /// Last activity timestamp (seconds since epoch from caller clock).
    last_activity_epoch_secs: u64,
    /// Number of consecutive authentication failures.
    failed_attempts: u32,
    /// Explicit lock flag set by failure policy.
    locked: bool,
}

impl SessionStatus {
    /// Create a new active session status.
    pub fn new(now_epoch_secs: u64) -> Self {
        Self {
            last_activity_epoch_secs: now_epoch_secs,
            failed_attempts: 0,
            locked: false,
        }
    }

    /// Return last activity timestamp in seconds since epoch.
    pub fn last_activity_epoch_secs(&self) -> u64 {
        self.last_activity_epoch_secs
    }

    /// Return number of consecutive authentication failures.
    pub fn failed_attempts(&self) -> u32 {
        self.failed_attempts
    }

    /// Return whether the session is currently locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Record user activity.
    pub fn touch(&mut self, now_epoch_secs: u64) {
        self.last_activity_epoch_secs = now_epoch_secs;
    }

    /// Register an authentication failure and return resulting session state.
    pub fn register_failure(&mut self, now_epoch_secs: u64, policy: SessionPolicy) -> SessionState {
        self.last_activity_epoch_secs = now_epoch_secs;
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        if self.failed_attempts >= policy.max_failures.max(1) {
            self.locked = true;
        }
        self.state_at(now_epoch_secs, policy)
    }

    /// Clear failure counter after successful re-authentication.
    pub fn clear_failures(&mut self) {
        self.failed_attempts = 0;
    }

    /// Unlock the session after successful re-authentication.
    pub fn unlock(&mut self, now_epoch_secs: u64) {
        self.locked = false;
        self.failed_attempts = 0;
        self.last_activity_epoch_secs = now_epoch_secs;
    }

    /// Explicitly lock the session regardless of failure count.
    pub fn lock(&mut self) {
        self.locked = true;
    }

    /// Record a failed authentication attempt and return the new failure count.
    pub fn record_failed_attempt(&mut self) -> u32 {
        self.failed_attempts = self.failed_attempts.saturating_add(1);
        self.failed_attempts
    }

    /// Evaluate session state at a deterministic timestamp.
    pub fn state_at(&self, now_epoch_secs: u64, policy: SessionPolicy) -> SessionState {
        if self.locked || self.failed_attempts >= policy.max_failures.max(1) {
            return SessionState::Locked;
        }
        let inactivity = now_epoch_secs.saturating_sub(self.last_activity_epoch_secs);
        let timeout = policy.inactivity_timeout_secs.max(1);
        if inactivity >= timeout {
            return SessionState::Locked;
        }
        let warning_threshold = timeout.saturating_sub(policy.warning_window_secs);
        if inactivity >= warning_threshold && policy.warning_window_secs > 0 {
            SessionState::Warning
        } else {
            SessionState::Active
        }
    }
}

/// Force session re-authentication for suspicious session activity.
pub fn force_reauthentication_for_suspicious_session(session: &mut SessionStatus) -> SessionState {
    session.locked = true;
    SessionState::Locked
}

/// Determine whether a session state requires operator re-authentication.
pub fn session_requires_reauthentication(state: SessionState) -> bool {
    matches!(state, SessionState::Locked)
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

fn is_shared_identity(principal: &str) -> bool {
    let lowered = principal.to_ascii_lowercase();
    lowered.contains("shared")
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
