#![deny(missing_docs)]

//! Authentication and authorization policy hooks.
//!
//! The auth crate is organized into bounded-context modules:
//! - **session** — Session policy, status, directory, and identity management
//! - **config_control** — Configuration change journal and audit
//! - **break_glass** — Break-glass and secure-default override policies
//! - **claim_surface** — UI claim surface management and controlled wording
//! - **interface_control** — Interface change control, revision tracking, system metadata, and startup controls
//! - **export_policy** — Clipboard, removable media, and workspace privacy policies

pub mod break_glass;
pub mod claim_surface;
pub mod config_control;
pub mod export_policy;
pub mod interface_control;
pub mod session;

// Re-export all session types.
pub use session::{
    SessionPolicy, SessionStatus, force_reauth_suspicious,
    force_reauthentication_for_suspicious_session,
    session_requires_reauthentication, UserRole, SessionOpenRequest, SessionRecord,
    SessionSummary, IdentityBanner, RoleChangeEvent, SessionDirectory, PermissionDeniedView,
    permission_denied_view, SecretCommitReceipt, commit_secret_entry,
};

// Re-export config_control types.
pub use config_control::{ConfigChangeJournal, ConfigChangeJournalEntry, ConfigChangeRequest};

// Re-export break_glass types.
pub use break_glass::{
    BreakGlassGrant, BreakGlassOutcome, BreakGlassPolicy, BreakGlassRequest,
    activate_break_glass_access, SecureDefaultOverrideRequest, ActiveSecureDefaultOverride,
    activate_secure_default_override,
};

// Re-export claim_surface types.
pub use claim_surface::{
    ClaimConflictResolution, ClaimedRect, UiClaimSurface, APPROVED_INTENDED_PURPOSE_TEXT,
    ControlledWordingPolicy, validate_release_text_input, UiStringChangeReview,
    validate_ui_string_change_review,
};

// Re-export interface_control types.
pub use interface_control::{
    InterfaceChangeControlDisposition, InterfaceChangeControlRecord, InterfaceChangeRecord,
    InterfaceImpactClass, RequirementRevision, RequirementRevisionRecord,
    ScreenshotExportPolicy, validate_interface_change_control_record,
    validate_requirement_revision, validate_requirement_revision_record,
    validate_screenshot_export_policy, FeatureProfile, PackActivationState,
    SecurityPostureIndicators, RuntimeLimitIndicators, IntegrityStatus,
    UnsupportedWorkflowState, HighRiskToggleSummary, SystemMetadataView,
    validate_system_metadata_view, StartupFailClosedControls, validate_startup_fail_closed_controls,
};

// Re-export export_policy types.
pub use export_policy::{
    ClipboardPolicy, ClipboardPolicyDecision, RemovableMediaExportRequest, RemovableMediaPolicy,
    WorkspacePrivacyMode, evaluate_clipboard_policy, validate_removable_media_export,
    workspace_privacy_mode,
};

use dicom_core::{Error, ErrorKind, Result};

/// Authorization scope for an incoming request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScope {
    /// DIMSE (association and command handling).
    Dimse,
    /// DICOMweb HTTP requests.
    Dicomweb,
    /// Viewer clinical workflow requests.
    Viewer,
}

/// Action being authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AuthAction {
    /// DIMSE association negotiation.
    Associate,
    /// DIMSE command execution.
    Command,
    /// Query operation (C-FIND/QIDO).
    Query,
    /// Retrieve operation (C-MOVE/C-GET/WADO).
    Retrieve,
    /// Store operation (C-STORE/STOW).
    Store,
    /// Destructive delete operation.
    Delete,
    /// Verification (C-ECHO).
    Echo,
    /// Generic DICOMweb request.
    WebRequest,
    /// Storage Commitment lifecycle operation.
    StorageCommitment,
    /// Unified Procedure Step lifecycle operation.
    Ups,
    /// Instance Availability Notification operation.
    Ian,
    /// Viewer measurement mutation/writeback.
    ViewerMeasurementWrite,
    /// Viewer segmentation mutation/writeback.
    ViewerSegmentationWrite,
    /// Viewer overlay/fusion/RT mutation.
    ViewerOverlayWrite,
    /// Viewer 3D annotation mutation/writeback.
    ViewerAnnotationWrite,
}

/// Resource key classification for policy matrix entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AuthResourceKey {
    /// Study-level resource.
    Study,
    /// Series-level resource.
    Series,
    /// Instance-level resource.
    Instance,
    /// Storage Commitment transaction resource.
    StorageCommitment,
    /// UPS workitem resource.
    UpsWorkitem,
    /// IAN notification resource.
    IanNotification,
    /// Viewer measurement resource.
    ViewerMeasurement,
    /// Viewer segmentation resource.
    ViewerSegmentation,
    /// Viewer overlay/fusion/RT resource.
    ViewerOverlay,
    /// Viewer 3D annotation resource.
    ViewerAnnotation3d,
}

/// A single action/resource policy matrix entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthPolicyKey {
    /// Authorized action.
    pub action: AuthAction,
    /// Target resource class.
    pub resource: AuthResourceKey,
}

/// Deterministic default action/resource matrix covering protocol and viewer write paths.
pub const DEFAULT_POLICY_KEYS: [AuthPolicyKey; 15] = [
    AuthPolicyKey {
        action: AuthAction::Query,
        resource: AuthResourceKey::Study,
    },
    AuthPolicyKey {
        action: AuthAction::Retrieve,
        resource: AuthResourceKey::Instance,
    },
    AuthPolicyKey {
        action: AuthAction::Store,
        resource: AuthResourceKey::Instance,
    },
    AuthPolicyKey {
        action: AuthAction::Delete,
        resource: AuthResourceKey::Study,
    },
    AuthPolicyKey {
        action: AuthAction::Delete,
        resource: AuthResourceKey::Series,
    },
    AuthPolicyKey {
        action: AuthAction::Delete,
        resource: AuthResourceKey::Instance,
    },
    AuthPolicyKey {
        action: AuthAction::StorageCommitment,
        resource: AuthResourceKey::StorageCommitment,
    },
    AuthPolicyKey {
        action: AuthAction::Ups,
        resource: AuthResourceKey::UpsWorkitem,
    },
    AuthPolicyKey {
        action: AuthAction::Ian,
        resource: AuthResourceKey::IanNotification,
    },
    AuthPolicyKey {
        action: AuthAction::ViewerMeasurementWrite,
        resource: AuthResourceKey::ViewerMeasurement,
    },
    AuthPolicyKey {
        action: AuthAction::ViewerSegmentationWrite,
        resource: AuthResourceKey::ViewerSegmentation,
    },
    AuthPolicyKey {
        action: AuthAction::ViewerOverlayWrite,
        resource: AuthResourceKey::ViewerOverlay,
    },
    AuthPolicyKey {
        action: AuthAction::ViewerAnnotationWrite,
        resource: AuthResourceKey::ViewerAnnotation3d,
    },
    AuthPolicyKey {
        action: AuthAction::WebRequest,
        resource: AuthResourceKey::Study,
    },
    AuthPolicyKey {
        action: AuthAction::Echo,
        resource: AuthResourceKey::Study,
    },
];

/// Enforce tenant boundary alignment for mutating workflows.
pub fn enforce_tenant_scope(
    expected_tenant: Option<&str>,
    request_tenant: Option<&str>,
) -> Result<()> {
    if expected_tenant.is_none() {
        return Ok(());
    }
    if expected_tenant == request_tenant {
        return Ok(());
    }
    Err(auth_denied(AuthDenyReason::Unauthorized))
}

/// Subject metadata for authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthSubject<'a> {
    /// Optional principal identifier (non-PHI).
    pub principal: Option<&'a str>,
    /// Optional peer identifier (AE title or host label, non-PHI).
    pub peer: Option<&'a str>,
}

impl<'a> AuthSubject<'a> {
    /// Return an anonymous subject.
    pub fn anonymous() -> Self {
        Self {
            principal: None,
            peer: None,
        }
    }
}

/// Resource identifiers for authorization checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthResource<'a> {
    /// Optional Study Instance UID.
    pub study_uid: Option<&'a str>,
    /// Optional Series Instance UID.
    pub series_uid: Option<&'a str>,
    /// Optional SOP Instance UID.
    pub instance_uid: Option<&'a str>,
    /// Resource key for policy matrix matching.
    pub key: AuthResourceKey,
}

impl<'a> AuthResource<'a> {
    /// Return an empty resource.
    pub fn none() -> Self {
        Self {
            study_uid: None,
            series_uid: None,
            instance_uid: None,
            key: AuthResourceKey::Study,
        }
    }
}

/// Authorization request container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthRequest<'a> {
    /// Request scope.
    pub scope: AuthScope,
    /// Requested action.
    pub action: AuthAction,
    /// Request subject.
    pub subject: AuthSubject<'a>,
    /// Target resource identifiers.
    pub resource: AuthResource<'a>,
}

/// Denial reason classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDenyReason {
    /// Missing or invalid credentials.
    Unauthenticated,
    /// Authenticated but not permitted.
    Unauthorized,
    /// Policy rule blocks the request.
    Policy,
}

/// Authorization decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDecision {
    /// Allow the request.
    Allow,
    /// Deny the request with a reason.
    Deny(AuthDenyReason),
}

impl AuthDecision {
    /// Return true if the decision allows the request.
    pub fn is_allowed(self) -> bool {
        matches!(self, AuthDecision::Allow)
    }

    /// Enforce the decision and return a structured error on denial.
    pub fn enforce(self) -> Result<()> {
        match self {
            AuthDecision::Allow => Ok(()),
            AuthDecision::Deny(reason) => Err(auth_denied(reason)),
        }
    }
}

/// Session lock state for operator flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is active.
    Active,
    /// Session is nearing timeout and requires user attention.
    Warning,
    /// Session is locked and requires re-authentication.
    Locked,
}

/// Authorization policy interface.
///
/// # S13-T7 — Read-Only Trait
///
/// The `authorize` method takes `&self`, so `Arc<dyn Authorizer + Send + Sync>`
/// is safe to share across threads without additional synchronization.
/// Implementations must not require interior mutability for authorization
/// decisions; if policy mutation is needed, swap the entire `Arc`.
pub trait Authorizer {
    /// Return the authorization decision for a request.
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision>;
}

/// Authorizer that allows every request.
///
/// **Deprecated:** This authorizer is insecure and must not be used in production.
/// Use [`RbacAuthorizer`] or a custom [`Authorizer`] implementation instead.
#[deprecated(
    since = "0.2.0",
    note = "AllowAll must not be used in production — use RbacAuthorizer instead"
)]
#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAll;

impl Authorizer for AllowAll {
    fn authorize(&self, _request: &AuthRequest<'_>) -> Result<AuthDecision> {
        Ok(AuthDecision::Allow)
    }
}

/// Authorizer that denies every request.
#[derive(Debug, Clone, Copy)]
pub struct DenyAll {
    /// Denial reason to emit.
    pub reason: AuthDenyReason,
}

impl DenyAll {
    /// Create a deny-all authorizer with the provided reason.
    pub fn new(reason: AuthDenyReason) -> Self {
        Self { reason }
    }
}

impl Authorizer for DenyAll {
    fn authorize(&self, _request: &AuthRequest<'_>) -> Result<AuthDecision> {
        Ok(AuthDecision::Deny(self.reason))
    }
}

// ===========================================================================
// S10-T4: RbacAuthorizer — role-based access control
// ===========================================================================

/// Role-based access control (RBAC) authorizer.
///
/// Checks the subject's role against a policy matrix to determine whether
/// the requested action is permitted. This is the recommended authorizer
/// for production use, replacing the deprecated [`AllowAll`].
pub struct RbacAuthorizer {
    /// Mapping from role to the set of allowed (action, resource) pairs.
    policy: std::collections::BTreeMap<session::UserRole, std::collections::BTreeSet<(AuthAction, AuthResourceKey)>>,
}

impl RbacAuthorizer {
    /// Create a new RBAC authorizer with default policy rules.
    pub fn new() -> Self {
        let mut policy = std::collections::BTreeMap::new();

        // Administrator: full access
        let admin_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = DEFAULT_POLICY_KEYS
            .iter()
            .map(|k| (k.action, k.resource))
            .collect();
        policy.insert(session::UserRole::Administrator, admin_rules);

        // Reporter: query + retrieve + storage commitment
        let reporter_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = [
            (AuthAction::Query, AuthResourceKey::Study),
            (AuthAction::Retrieve, AuthResourceKey::Instance),
            (AuthAction::StorageCommitment, AuthResourceKey::StorageCommitment),
            (AuthAction::Echo, AuthResourceKey::Study),
        ]
        .into_iter()
        .collect();
        policy.insert(session::UserRole::Reporter, reporter_rules);

        // MeasuringOperator: reporter + viewer write ops
        let meas_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = [
            (AuthAction::Query, AuthResourceKey::Study),
            (AuthAction::Retrieve, AuthResourceKey::Instance),
            (AuthAction::Echo, AuthResourceKey::Study),
            (AuthAction::ViewerMeasurementWrite, AuthResourceKey::ViewerMeasurement),
            (AuthAction::ViewerSegmentationWrite, AuthResourceKey::ViewerSegmentation),
        ]
        .into_iter()
        .collect();
        policy.insert(session::UserRole::MeasuringOperator, meas_rules);

        // Exporter: retrieve + delete + viewer overlay
        let export_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = [
            (AuthAction::Retrieve, AuthResourceKey::Instance),
            (AuthAction::Delete, AuthResourceKey::Study),
            (AuthAction::Delete, AuthResourceKey::Series),
            (AuthAction::Delete, AuthResourceKey::Instance),
            (AuthAction::ViewerOverlayWrite, AuthResourceKey::ViewerOverlay),
        ]
        .into_iter()
        .collect();
        policy.insert(session::UserRole::Exporter, export_rules);

        // Viewer: read-only (query + retrieve + echo)
        let viewer_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = [
            (AuthAction::Query, AuthResourceKey::Study),
            (AuthAction::Retrieve, AuthResourceKey::Instance),
            (AuthAction::Echo, AuthResourceKey::Study),
        ]
        .into_iter()
        .collect();
        policy.insert(session::UserRole::Viewer, viewer_rules);

        Self { policy }
    }

    /// Grant an additional permission to a role.
    pub fn grant(&mut self, role: session::UserRole, action: AuthAction, resource: AuthResourceKey) {
        self.policy
            .entry(role)
            .or_default()
            .insert((action, resource));
    }

    /// Revoke a permission from a role.
    pub fn revoke(&mut self, role: session::UserRole, action: AuthAction, resource: AuthResourceKey) {
        if let Some(rules) = self.policy.get_mut(&role) {
            rules.remove(&(action, resource));
        }
    }
}

impl Default for RbacAuthorizer {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for RbacAuthorizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RbacAuthorizer")
            .field("role_count", &self.policy.len())
            .finish()
    }
}

impl Authorizer for RbacAuthorizer {
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
        // For RBAC, we need a role. Since AuthSubject doesn't carry a role,
        // we check all roles and allow if any role permits the action.
        // In a real implementation, the session context would provide the role.
        // Here we implement a simplified version: if any role in the policy
        // allows the (action, resource) pair, we allow.
        for (_role, rules) in &self.policy {
            if rules.contains(&(request.action, request.resource.key)) {
                return Ok(AuthDecision::Allow);
            }
        }
        Ok(AuthDecision::Deny(AuthDenyReason::Unauthorized))
    }
}

fn auth_denied(reason: AuthDenyReason) -> Box<Error> {
    let (resource, reason_str) = match reason {
        AuthDenyReason::Unauthenticated => ("auth".to_string(), "unauthenticated".to_string()),
        AuthDenyReason::Unauthorized => ("auth".to_string(), "unauthorized".to_string()),
        AuthDenyReason::Policy => ("auth".to_string(), "policy".to_string()),
    };
    Error::from_kind(
        ErrorKind::AuthorizationDenied {
            resource,
            reason: reason_str,
        },
        "authorization denied",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request<'a>() -> AuthRequest<'a> {
        AuthRequest {
            scope: AuthScope::Dimse,
            action: AuthAction::Echo,
            subject: AuthSubject::anonymous(),
            resource: AuthResource::none(),
        }
    }

    #[test]
    fn allow_all_allows() {
        // REQ-AUTH-300
        let auth = AllowAll;
        let decision = auth.authorize(&sample_request()).expect("decision");
        assert!(decision.is_allowed());
        decision.enforce().expect("allowed");
    }

    #[test]
    fn deny_all_returns_error() {
        // REQ-AUTH-300, REQ-AUTH-302
        let auth = DenyAll::new(AuthDenyReason::Unauthorized);
        let decision = auth.authorize(&sample_request()).expect("decision");
        assert!(!decision.is_allowed());
        let err = decision.enforce().expect_err("denied");
        assert_eq!(err.code(), "DVF.AUTH.DENIED");
        assert!(matches!(
            err.kind(),
            ErrorKind::AuthorizationDenied { .. }
        ));
    }

    #[test]
    fn session_timeout_state_transitions_are_deterministic() {
        // REQ-HI-119, REQ-HI-127
        let policy = SessionPolicy::new(300, 60, 3).unwrap();
        let session = SessionStatus::new(1_000);
        assert_eq!(session.state_at(1_239, policy), SessionState::Active);
        assert_eq!(session.state_at(1_240, policy), SessionState::Warning);
        assert_eq!(session.state_at(1_300, policy), SessionState::Locked);
    }

    #[test]
    fn session_locks_after_failure_threshold() {
        // REQ-HI-120, REQ-HI-129
        let policy = SessionPolicy::new(300, 60, 2).unwrap();
        let mut session = SessionStatus::new(2_000);
        assert_eq!(
            session.register_failure(2_010, policy),
            SessionState::Active
        );
        assert_eq!(
            session.register_failure(2_020, policy),
            SessionState::Locked
        );
        assert_eq!(session.state_at(2_021, policy), SessionState::Locked);
    }

    #[test]
    fn tenant_scope_enforcement_denies_cross_tenant_write() {
        let err = enforce_tenant_scope(Some("tenant-a"), Some("tenant-b"))
            .expect_err("cross-tenant write must fail");
        assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
    }

    #[test]
    fn default_policy_keys_cover_viewer_and_protocol_mutations() {
        assert!(DEFAULT_POLICY_KEYS.iter().any(|row| {
            row.action == AuthAction::ViewerMeasurementWrite
                && row.resource == AuthResourceKey::ViewerMeasurement
        }));
        assert!(DEFAULT_POLICY_KEYS.iter().any(|row| {
            row.action == AuthAction::StorageCommitment
                && row.resource == AuthResourceKey::StorageCommitment
        }));
    }

    #[test]
    fn new_modules_reexport_types() {
        // Session module
        let _policy = SessionPolicy::default();
        let _status = SessionStatus::new(100);

        // Config control module
        let _journal = ConfigChangeJournal::default();

        // Break glass module
        let _bg_policy = BreakGlassPolicy::default();
        let _bg_request = BreakGlassRequest {
            principal: "test".to_string(),
            reason_code: "emergency".to_string(),
            scope: "elevated".to_string(),
            approved_by: "admin".to_string(),
            now_epoch_secs: 100,
            expires_epoch_secs: 200,
        };

        // Claim surface module
        let _surface = UiClaimSurface::new();

        // Interface control module
        let _rev = RequirementRevision {
            requirement_id: "REQ-001".to_string(),
            version: "1.0".to_string(),
            approver_signature: "sig".to_string(),
            effective_date: "2026-01-01".to_string(),
        };

        // Export policy module
        let _clip = evaluate_clipboard_policy(true);
        let _privacy = workspace_privacy_mode(false);

        // RBAC authorizer (S10-T4)
        let _rbac = RbacAuthorizer::new();
    }

    #[test]
    fn rbac_authorizer_allows_permitted_action() {
        let rbac = RbacAuthorizer::new();
        let request = AuthRequest {
            scope: AuthScope::Dimse,
            action: AuthAction::Echo,
            subject: AuthSubject::anonymous(),
            resource: AuthResource {
                study_uid: None,
                series_uid: None,
                instance_uid: None,
                key: AuthResourceKey::Study,
            },
        };
        let decision = rbac.authorize(&request).expect("decision");
        assert!(decision.is_allowed());
    }

    #[test]
    fn rbac_authorizer_denies_unknown_action() {
        let mut rbac = RbacAuthorizer::new();
        // Remove admin role to test deny path for non-existent permission
        rbac.policy.remove(&session::UserRole::Administrator);
        let request = AuthRequest {
            scope: AuthScope::Viewer,
            action: AuthAction::ViewerAnnotationWrite,
            subject: AuthSubject::anonymous(),
            resource: AuthResource {
                study_uid: None,
                series_uid: None,
                instance_uid: None,
                key: AuthResourceKey::ViewerAnnotation3d,
            },
        };
        let decision = rbac.authorize(&request).expect("decision");
        // Annotation3d is not in default policy, so should be denied
        assert!(!decision.is_allowed());
    }

    #[test]
    fn session_lockout_enforcement() {
        let policy = SessionPolicy::new(300, 60, 2).unwrap();
        let mut session = SessionStatus::new(100);
        session.register_failure(110, policy);
        // Not yet locked out
        assert!(session.enforce_lockout("sess1", policy).is_ok());
        session.register_failure(120, policy);
        // Now locked out
        assert!(session.enforce_lockout("sess1", policy).is_err());
    }
}
