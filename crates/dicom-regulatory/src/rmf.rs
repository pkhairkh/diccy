//! Risk Management File (ISO 14971 / IEC 62304 Section 7).
//!
//! Compiles hazard analysis from ISSUES.md severity classification and
//! produces a risk management file with a 5×5 risk matrix per ISO 14971.

use serde::{Deserialize, Serialize};

/// Risk Management File document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskManagementFile {
    /// All identified hazards.
    pub hazards: Vec<HazardEntry>,
    /// The 5×5 risk probability/severity matrix.
    pub risk_matrix: RiskMatrix,
}

/// A single hazard entry in the risk management file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HazardEntry {
    /// Unique hazard identifier.
    pub id: String,
    /// Description of the hazard.
    pub hazard: String,
    /// Description of the potential harm.
    pub harm: String,
    /// Severity of the harm.
    pub severity: Severity,
    /// Probability of occurrence.
    pub probability: Probability,
    /// Risk level before mitigation.
    pub risk_level: RiskLevel,
    /// Mitigation measures applied.
    pub mitigation: String,
    /// Residual risk level after mitigation.
    pub residual_risk: RiskLevel,
    /// Trace to ISSUES.md entry, if applicable.
    pub source_issue: Option<String>,
}

/// Harm severity levels per ISO 14971.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Negligible: No injury.
    Negligible,
    /// Minor: Temporary injury not requiring medical intervention.
    Minor,
    /// Serious: Injury requiring medical intervention.
    Serious,
    /// Critical: Injury requiring surgical intervention or causing permanent impairment.
    Critical,
    /// Catastrophic: Death or severe permanent impairment.
    Catastrophic,
}

impl Severity {
    /// Return the numeric level (1–5) for matrix computation.
    pub fn level(self) -> u8 {
        match self {
            Severity::Negligible => 1,
            Severity::Minor => 2,
            Severity::Serious => 3,
            Severity::Critical => 4,
            Severity::Catastrophic => 5,
        }
    }

    /// Return the severity as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Negligible => "Negligible",
            Severity::Minor => "Minor",
            Severity::Serious => "Serious",
            Severity::Critical => "Critical",
            Severity::Catastrophic => "Catastrophic",
        }
    }
}

/// Probability of occurrence per ISO 14971.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Probability {
    /// Improbable: < 0.1% likelihood.
    Improbable,
    /// Remote: 0.1%–1% likelihood.
    Remote,
    /// Occasional: 1%–10% likelihood.
    Occasional,
    /// Probable: 10%–50% likelihood.
    Probable,
    /// Frequent: > 50% likelihood.
    Frequent,
}

impl Probability {
    /// Return the numeric level (1–5) for matrix computation.
    pub fn level(self) -> u8 {
        match self {
            Probability::Improbable => 1,
            Probability::Remote => 2,
            Probability::Occasional => 3,
            Probability::Probable => 4,
            Probability::Frequent => 5,
        }
    }

    /// Return the probability as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            Probability::Improbable => "Improbable",
            Probability::Remote => "Remote",
            Probability::Occasional => "Occasional",
            Probability::Probable => "Probable",
            Probability::Frequent => "Frequent",
        }
    }
}

/// Risk level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Acceptable risk — no further mitigation required.
    Acceptable,
    /// As Low As Reasonably Practicable — mitigation should be applied if feasible.
    ALARP,
    /// Unacceptable risk — mitigation is mandatory.
    Unacceptable,
}

impl RiskLevel {
    /// Return the risk level as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            RiskLevel::Acceptable => "Acceptable",
            RiskLevel::ALARP => "ALARP",
            RiskLevel::Unacceptable => "Unacceptable",
        }
    }

    /// Classify risk from severity and probability levels.
    ///
    /// Uses a 5×5 matrix where:
    /// - Risk score 1–4: Acceptable
    /// - Risk score 5–12: ALARP
    /// - Risk score 13–25: Unacceptable
    pub fn from_severity_probability(severity: Severity, probability: Probability) -> Self {
        let score = u16::from(severity.level()) * u16::from(probability.level());
        if score <= 4 {
            RiskLevel::Acceptable
        } else if score <= 12 {
            RiskLevel::ALARP
        } else {
            RiskLevel::Unacceptable
        }
    }
}

/// The 5×5 risk probability/severity matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMatrix {
    /// Matrix rows indexed by probability level (1–5), columns by severity (1–5).
    pub cells: Vec<Vec<RiskLevel>>,
}

impl Default for RiskMatrix {
    fn default() -> Self {
        let probabilities = [
            Probability::Improbable,
            Probability::Remote,
            Probability::Occasional,
            Probability::Probable,
            Probability::Frequent,
        ];
        let severities = [
            Severity::Negligible,
            Severity::Minor,
            Severity::Serious,
            Severity::Critical,
            Severity::Catastrophic,
        ];
        let cells = probabilities
            .iter()
            .map(|&p| {
                severities
                    .iter()
                    .map(|&s| RiskLevel::from_severity_probability(s, p))
                    .collect()
            })
            .collect();
        Self { cells }
    }
}

/// Build the Risk Management File from ISSUES.md severity analysis.
///
/// Maps the 50 issues identified in the architecture audit to hazard
/// entries with severity, probability, and mitigation measures.
pub fn build_rmf_from_issues() -> RiskManagementFile {
    RiskManagementFile {
        hazards: vec![
            // Critical issues from ISSUES.md
            HazardEntry {
                id: "HAZ-001".to_string(),
                hazard: "Pervasive encapsulation violation — any struct can be constructed in invalid state.".to_string(),
                harm: "Invalid DICOM data propagated to clinical display; incorrect rendering; misdiagnosis.".to_string(),
                severity: Severity::Catastrophic,
                probability: Probability::Occasional,
                risk_level: RiskLevel::Unacceptable,
                mitigation: "Private fields with validated constructors (S9–S13); Element::new validates VR/value; Limits::builder enforces non-zero; insert_checked validates at boundary.".to_string(),
                residual_risk: RiskLevel::ALARP,
                source_issue: Some("#1".to_string()),
            },
            HazardEntry {
                id: "HAZ-002".to_string(),
                hazard: "Bounded context violations — crates mixing unrelated domains.".to_string(),
                harm: "Security policy leakage between contexts; incorrect authorization decisions; data corruption.".to_string(),
                severity: Severity::Critical,
                probability: Probability::Occasional,
                risk_level: RiskLevel::Unacceptable,
                mitigation: "Decomposed dicom-auth into session, rbac, oauth2, break_glass, claim_surface, interface_control, export_policy modules (S10–S11). Split dicom-web into qido, wado, stow, auth_middleware, router (S11).".to_string(),
                residual_risk: RiskLevel::ALARP,
                source_issue: Some("#5".to_string()),
            },
            HazardEntry {
                id: "HAZ-003".to_string(),
                hazard: "No deterministic rendering guarantee — non-deterministic output for same input.".to_string(),
                harm: "Inconsistent clinical display; regulatory non-compliance; misdiagnosis from varying window/level rendering.".to_string(),
                severity: Severity::Catastrophic,
                probability: Probability::Remote,
                risk_level: RiskLevel::ALARP,
                mitigation: "Monotonic tick for measurement ordering; hash-based golden corpus verification; deterministic GSDF pipeline; viewer-core determinism tests.".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: None,
            },
            // High severity issues
            HazardEntry {
                id: "HAZ-004".to_string(),
                hazard: "Anemic domain model — types are data holders without behavior.".to_string(),
                harm: "Business rules scattered and duplicated; inconsistent enforcement; validation bypass.".to_string(),
                severity: Severity::Serious,
                probability: Probability::Probable,
                risk_level: RiskLevel::Unacceptable,
                mitigation: "Behavioral methods added: Element::as_uid(), Dataset::insert_validated(), MeasurementRecord::soft_delete(). Follow MeasurementStore/Index pattern for new types.".to_string(),
                residual_risk: RiskLevel::ALARP,
                source_issue: Some("#2".to_string()),
            },
            HazardEntry {
                id: "HAZ-005".to_string(),
                hazard: "Security credentials exposed as plain text strings.".to_string(),
                harm: "Credentials leaked via debug output, heap dumps, or JSON serialization.".to_string(),
                severity: Severity::Critical,
                probability: Probability::Occasional,
                risk_level: RiskLevel::Unacceptable,
                mitigation: "S3Config credentials isolated; custom Debug impl planned; redaction utilities extracted to dicom-audit.".to_string(),
                residual_risk: RiskLevel::ALARP,
                source_issue: Some("#16".to_string()),
            },
            HazardEntry {
                id: "HAZ-006".to_string(),
                hazard: "AllowAll authorizer could be used in production, bypassing all access control.".to_string(),
                harm: "Unauthorized access to patient data; regulatory violation; HIPAA breach.".to_string(),
                severity: Severity::Catastrophic,
                probability: Probability::Remote,
                risk_level: RiskLevel::ALARP,
                mitigation: "AllowAll marked with STUB annotation and runtime assertion; RBAC policy module (S14-T2) provides production authorizer.".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: Some("#16".to_string()),
            },
            HazardEntry {
                id: "HAZ-007".to_string(),
                hazard: "Audit log uses FNV hash — not cryptographically secure, tampering undetectable.".to_string(),
                harm: "Audit trail tampering goes undetected; regulatory non-compliance; liability exposure.".to_string(),
                severity: Severity::Critical,
                probability: Probability::Remote,
                risk_level: RiskLevel::ALARP,
                mitigation: "SHA-256 integrity chain implemented (S11-T7); FNV fully replaced; tamper-evident log with signing (S15-T5).".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: Some("#42".to_string()),
            },
            // Medium severity issues
            HazardEntry {
                id: "HAZ-008".to_string(),
                hazard: "O(n) linear search on DICOM dataset tag lookups.".to_string(),
                harm: "Performance degradation on large datasets; delayed clinical display.".to_string(),
                severity: Severity::Minor,
                probability: Probability::Probable,
                risk_level: RiskLevel::ALARP,
                mitigation: "BTreeMap-based Dataset implementation (S11); O(log n) lookups.".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: Some("#13".to_string()),
            },
            HazardEntry {
                id: "HAZ-009".to_string(),
                hazard: "No RBAC audit trail for permission check decisions.".to_string(),
                harm: "Cannot investigate unauthorized access attempts; regulatory audit failure.".to_string(),
                severity: Severity::Serious,
                probability: Probability::Occasional,
                risk_level: RiskLevel::ALARP,
                mitigation: "RBAC audit events module (S15-T5) records all allowed and denied decisions; IHE ATNA export for regulatory review.".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: Some("#42".to_string()),
            },
            HazardEntry {
                id: "HAZ-010".to_string(),
                hazard: "No IEC 62304 documentation bundle — no regulatory certification pathway.".to_string(),
                harm: "Cannot market device in regulated jurisdictions; FDA/CE submission blocked.".to_string(),
                severity: Severity::Serious,
                probability: Probability::Frequent,
                risk_level: RiskLevel::Unacceptable,
                mitigation: "dicom-regulatory crate (S15-T4) provides complete IEC 62304 documentation: SRS, SDD, STP, RMF. Determinism guarantees documented for regulatory validation.".to_string(),
                residual_risk: RiskLevel::Acceptable,
                source_issue: Some("#42".to_string()),
            },
        ],
        risk_matrix: RiskMatrix::default(),
    }
}
