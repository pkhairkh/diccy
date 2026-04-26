//! Interface change control bounded-context module.
//!
//! Contains `InterfaceChangeRecord`, `RequirementRevision`, `ScreenshotExportPolicy`,
//! system metadata view types, and startup fail-closed controls.

use crate::claim_surface::{ControlledWordingPolicy, validate_release_text_input};
use dicom_core::{Error, ErrorKind, Result};

/// Backward-compatible alias for [`InterfaceChangeRecord`].
pub type InterfaceChangeControlRecord = InterfaceChangeRecord;

/// Backward-compatible alias for [`RequirementRevision`].
pub type RequirementRevisionRecord = RequirementRevision;

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
pub struct InterfaceChangeRecord {
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
    record: &InterfaceChangeRecord,
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
pub struct RequirementRevision {
    /// Requirement identifier.
    pub requirement_id: String,
    /// Version string.
    pub version: String,
    /// Approver signature identifier.
    pub approver_signature: String,
    /// Effective date in `YYYY-MM-DD`.
    pub effective_date: String,
}

/// Backward-compatible alias for [`validate_requirement_revision`].
pub fn validate_requirement_revision_record(record: &RequirementRevision) -> Result<()> {
    validate_requirement_revision(record)
}

/// Validate requirement revision governance metadata.
pub fn validate_requirement_revision(record: &RequirementRevision) -> Result<()> {
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
