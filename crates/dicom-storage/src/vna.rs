//! Vendor Neutral Archive (VNA) lifecycle bounded-context types.
//!
//! Contains retention policies, study lifecycle management, and the VNA engine.

use dicom_core::{Error, ErrorKind, Result};
use std::collections::BTreeMap;

/// Retention policy for a tenant.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RetentionPolicy {
    /// Tenant identifier.
    pub tenant_id: String,
    /// Minimum retention period in days.
    pub min_retention_days: u32,
    /// Maximum retention period in days.
    pub max_retention_days: u32,
    /// Whether legal hold is active.
    pub legal_hold: bool,
    /// Study types this policy applies to (empty = all).
    pub study_types: Vec<String>,
}

impl RetentionPolicy {
    /// Create a new retention policy.
    pub fn new(tenant_id: &str, min_days: u32, max_days: u32) -> Self {
        Self {
            tenant_id: tenant_id.to_string(),
            min_retention_days: min_days,
            max_retention_days: max_days,
            legal_hold: false,
            study_types: Vec::new(),
        }
    }

    /// Check if a study can be purged based on this policy.
    pub fn can_purge(&self, age_days: u32) -> bool {
        !self.legal_hold && age_days > self.max_retention_days
    }
}

/// Study lifecycle state for VNA management.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StudyLifecycleState {
    /// Study is actively available.
    Active,
    /// Study archived to cold storage.
    Archived,
    /// Study pending deletion after retention period.
    PendingPurge,
    /// Study under legal hold — cannot be deleted.
    LegalHold,
    /// Study permanently deleted.
    Purged,
}

/// VNA study record for lifecycle management.
#[derive(Debug, Clone, PartialEq)]
pub struct VnaStudyRecord {
    /// Study Instance UID.
    pub study_uid: String,
    /// Tenant ID.
    pub tenant_id: String,
    /// Current lifecycle state.
    pub state: StudyLifecycleState,
    /// Age in days.
    pub age_days: u32,
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Retention policy applied.
    pub retention_policy: Option<RetentionPolicy>,
}

/// Vendor Neutral Archive engine for multi-tenant study lifecycle management.
pub struct VnaEngine {
    /// Study records indexed by Study UID.
    studies: BTreeMap<String, VnaStudyRecord>,
    /// Retention policies indexed by tenant ID.
    policies: BTreeMap<String, RetentionPolicy>,
}

impl Default for VnaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl VnaEngine {
    /// Create a new VNA engine.
    pub fn new() -> Self {
        Self {
            studies: BTreeMap::new(),
            policies: BTreeMap::new(),
        }
    }

    /// Register a study in the VNA.
    pub fn register_study(&mut self, record: VnaStudyRecord) -> Result<()> {
        if record.study_uid.is_empty() {
            return Err(vna_error("study UID must not be empty"));
        }
        self.studies.insert(record.study_uid.clone(), record);
        Ok(())
    }

    /// Set a retention policy for a tenant.
    pub fn set_retention_policy(&mut self, policy: RetentionPolicy) {
        self.policies.insert(policy.tenant_id.clone(), policy);
    }

    /// Apply legal hold to a study.
    pub fn apply_legal_hold(&mut self, study_uid: &str) -> Result<()> {
        let study = self
            .studies
            .get_mut(study_uid)
            .ok_or_else(|| vna_error("study not found"))?;
        study.state = StudyLifecycleState::LegalHold;
        Ok(())
    }

    /// Release legal hold from a study.
    pub fn release_legal_hold(&mut self, study_uid: &str) -> Result<()> {
        let study = self
            .studies
            .get_mut(study_uid)
            .ok_or_else(|| vna_error("study not found"))?;
        if matches!(study.state, StudyLifecycleState::LegalHold) {
            study.state = StudyLifecycleState::Active;
        }
        Ok(())
    }

    /// Run retention policy enforcement. Returns the number of studies purged.
    pub fn enforce_retention(&mut self) -> usize {
        let mut to_purge = Vec::new();

        for (uid, study) in &self.studies {
            if let Some(policy) = self.policies.get(&study.tenant_id) {
                if policy.can_purge(study.age_days)
                    && !matches!(study.state, StudyLifecycleState::LegalHold)
                {
                    to_purge.push(uid.clone());
                }
            }
        }

        let purged = to_purge.len();
        for uid in &to_purge {
            if let Some(study) = self.studies.get_mut(uid) {
                study.state = StudyLifecycleState::Purged;
            }
        }

        purged
    }

    /// Archive studies that haven't been accessed recently.
    pub fn archive_stale_studies(&mut self, stale_threshold_days: u32) -> usize {
        let mut archived = 0;
        for study in self.studies.values_mut() {
            if matches!(study.state, StudyLifecycleState::Active)
                && study.age_days > stale_threshold_days
            {
                study.state = StudyLifecycleState::Archived;
                archived += 1;
            }
        }
        archived
    }

    /// Get a study record.
    pub fn get_study(&self, study_uid: &str) -> Option<&VnaStudyRecord> {
        self.studies.get(study_uid)
    }

    /// Return the number of registered studies.
    pub fn study_count(&self) -> usize {
        self.studies.len()
    }
}

/// Lifecycle policy for tiered S3 storage.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LifecyclePolicy {
    /// Days before transitioning to infrequent access storage.
    pub ia_transition_days: u32,
    /// Days before transitioning to glacier storage.
    pub glacier_transition_days: u32,
    /// Days before expiration (permanent deletion).
    pub expiration_days: u32,
    /// Whether to enable cleanup of incomplete multipart uploads.
    pub cleanup_multipart: bool,
    /// Maximum age in days for incomplete multipart uploads.
    pub multipart_cleanup_age_days: u32,
}

impl Default for LifecyclePolicy {
    fn default() -> Self {
        Self {
            ia_transition_days: 30,
            glacier_transition_days: 90,
            expiration_days: 365,
            cleanup_multipart: true,
            multipart_cleanup_age_days: 7,
        }
    }
}

impl LifecyclePolicy {
    /// Apply lifecycle policy to stored objects (simulated).
    pub(crate) fn apply(&self, objects: &mut BTreeMap<String, Vec<u8>>) -> usize {
        // In a real implementation, this would transition objects between
        // storage tiers. For the stub, we just return 0.
        let _ = objects;
        0
    }
}

fn vna_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-storage-ext".to_string(),
            detail: detail.into(),
        },
        "storage extension error",
    )
    .into()
}
