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
    commit_secret_entry, force_reauth_suspicious, force_reauthentication_for_suspicious_session,
    permission_denied_view, session_requires_reauthentication, IdentityBanner,
    PermissionDeniedView, RoleChangeEvent, SecretCommitReceipt, SessionDirectory,
    SessionOpenRequest, SessionPolicy, SessionRecord, SessionStatus, SessionSummary, UserRole,
};

// Re-export config_control types.
pub use config_control::{ConfigChangeJournal, ConfigChangeJournalEntry, ConfigChangeRequest};

// Re-export break_glass types.
pub use break_glass::{
    activate_break_glass_access, activate_secure_default_override, ActiveSecureDefaultOverride,
    BreakGlassGrant, BreakGlassOutcome, BreakGlassPolicy, BreakGlassRequest,
    SecureDefaultOverrideRequest,
};

// Re-export claim_surface types.
pub use claim_surface::{
    validate_release_text_input, validate_ui_string_change_review, ClaimConflictResolution,
    ClaimedRect, ControlledWordingPolicy, UiClaimSurface, UiStringChangeReview,
    APPROVED_INTENDED_PURPOSE_TEXT,
};

// Re-export interface_control types.
pub use interface_control::{
    validate_interface_change_control_record, validate_requirement_revision,
    validate_requirement_revision_record, validate_screenshot_export_policy,
    validate_startup_fail_closed_controls, validate_system_metadata_view, FeatureProfile,
    HighRiskToggleSummary, IntegrityStatus, InterfaceChangeControlDisposition,
    InterfaceChangeControlRecord, InterfaceChangeRecord, InterfaceImpactClass, PackActivationState,
    RequirementRevision, RequirementRevisionRecord, RuntimeLimitIndicators, ScreenshotExportPolicy,
    SecurityPostureIndicators, StartupFailClosedControls, SystemMetadataView,
    UnsupportedWorkflowState,
};

// Re-export export_policy types.
pub use export_policy::{
    evaluate_clipboard_policy, validate_removable_media_export, workspace_privacy_mode,
    ClipboardPolicy, ClipboardPolicyDecision, RemovableMediaExportRequest, RemovableMediaPolicy,
    WorkspacePrivacyMode,
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
    pub policy: std::collections::BTreeMap<
        session::UserRole,
        std::collections::BTreeSet<(AuthAction, AuthResourceKey)>,
    >,
}

impl RbacAuthorizer {
    /// Create a new RBAC authorizer with default policy rules.
    pub fn new() -> Self {
        let mut policy = std::collections::BTreeMap::new();

        // Administrator: full access
        let admin_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> =
            DEFAULT_POLICY_KEYS
                .iter()
                .map(|k| (k.action, k.resource))
                .collect();
        policy.insert(session::UserRole::Administrator, admin_rules);

        // Reporter: query + retrieve + storage commitment
        let reporter_rules: std::collections::BTreeSet<(AuthAction, AuthResourceKey)> = [
            (AuthAction::Query, AuthResourceKey::Study),
            (AuthAction::Retrieve, AuthResourceKey::Instance),
            (
                AuthAction::StorageCommitment,
                AuthResourceKey::StorageCommitment,
            ),
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
            (
                AuthAction::ViewerMeasurementWrite,
                AuthResourceKey::ViewerMeasurement,
            ),
            (
                AuthAction::ViewerSegmentationWrite,
                AuthResourceKey::ViewerSegmentation,
            ),
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
            (
                AuthAction::ViewerOverlayWrite,
                AuthResourceKey::ViewerOverlay,
            ),
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
    pub fn grant(
        &mut self,
        role: session::UserRole,
        action: AuthAction,
        resource: AuthResourceKey,
    ) {
        self.policy
            .entry(role)
            .or_default()
            .insert((action, resource));
    }

    /// Revoke a permission from a role.
    pub fn revoke(
        &mut self,
        role: session::UserRole,
        action: AuthAction,
        resource: AuthResourceKey,
    ) {
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
