//! Claim surface bounded-context module.
//!
//! Contains `UiClaimSurface`, `ClaimedRect`, `ClaimConflictResolution`,
//! controlled wording policy, and release text validation.

use dicom_core::{Error, ErrorKind, Result};

/// Restricted claim terms that must not appear in user-visible text.
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

/// Validate user-entered text for claim-surface safety.
pub fn validate_release_text_input(text: &str) -> Result<()> {
    if contains_restricted_claim_terms(text) {
        return Err(policy_violation(
            "claim_surface",
            "release/configuration text contains unapproved regulatory claim wording",
        ));
    }
    Ok(())
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
        return Err(policy_violation(
            "ui_string_change",
            "UI string change record requires change_id",
        ));
    }
    if !review.claim_surface_lint_passed {
        return Err(policy_violation(
            "ui_string_change",
            "UI string change review requires claim-surface lint pass",
        ));
    }
    if review.controlled_wording_approved_by.trim().is_empty() {
        return Err(policy_violation(
            "ui_string_change",
            "UI string change review requires controlled wording approval",
        ));
    }
    Ok(())
}

/// A rectangular area on the UI that has been claimed for a specific purpose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedRect {
    /// X coordinate of the rectangle.
    pub x: u32,
    /// Y coordinate of the rectangle.
    pub y: u32,
    /// Width of the rectangle.
    pub width: u32,
    /// Height of the rectangle.
    pub height: u32,
    /// Claim identifier.
    pub claim_id: String,
    /// Purpose of the claim.
    pub purpose: String,
}

/// UI claim surface for managing screen area allocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UiClaimSurface {
    /// Active claims on the surface.
    claims: Vec<ClaimedRect>,
}

impl UiClaimSurface {
    /// Create a new empty claim surface.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a claim to the surface.
    pub fn add_claim(&mut self, claim: ClaimedRect) -> Result<()> {
        if claim.claim_id.trim().is_empty() {
            return Err(policy_violation("claim_surface", "claim_id must not be empty"));
        }
        if claim.width == 0 || claim.height == 0 {
            return Err(policy_violation("claim_surface", "claimed rectangle must have non-zero dimensions"));
        }
        self.claims.push(claim);
        Ok(())
    }

    /// Return all claims on the surface.
    pub fn claims(&self) -> &[ClaimedRect] {
        &self.claims
    }

    /// Remove a claim by its identifier.
    pub fn remove_claim(&mut self, claim_id: &str) -> bool {
        let before = self.claims.len();
        self.claims.retain(|c| c.claim_id != claim_id);
        self.claims.len() < before
    }

    /// Check if any claims overlap with the given rectangle.
    pub fn has_conflict(&self, x: u32, y: u32, width: u32, height: u32) -> bool {
        self.claims.iter().any(|c| {
            c.x < x + width
                && x < c.x + c.width
                && c.y < y + height
                && y < c.y + c.height
        })
    }
}

/// Resolution strategy for claim conflicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimConflictResolution {
    /// The new claim replaces the existing one.
    Replace,
    /// The new claim is rejected in favor of the existing one.
    Reject,
    /// Both claims coexist (no overlap resolution).
    Coexist,
}

fn contains_restricted_claim_terms(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    CLAIM_RESTRICTED_TERMS
        .iter()
        .any(|term| lowered.contains(term))
}

fn policy_violation(policy: impl Into<String>, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::PolicyViolation {
            policy: policy.into(),
            detail: detail.into(),
        },
        "policy violation",
    )
    .into()
}
