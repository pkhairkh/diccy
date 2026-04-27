//! Software Requirements Specification (IEC 62304 Section 5.2).
//!
//! Compiles software requirements from `manifest.toml` REQ identifiers and
//! the broader requirements traceability infrastructure. Each requirement
//! is classified by category, priority, and verification method per IEC 62304.

use serde::{Deserialize, Serialize};

/// Software Requirements Specification document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareRequirementsSpec {
    /// Document version.
    pub version: String,
    /// IEC 62304 software safety classification.
    pub safety_class: SafetyClass,
    /// All software requirements.
    pub requirements: Vec<SoftwareRequirement>,
}

/// IEC 62304 software safety classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyClass {
    /// Class A: No injury or damage to health is possible.
    ClassA,
    /// Class B: Non-serious injury is possible.
    ClassB,
    /// Class C: Death or serious injury is possible.
    ClassC,
}

impl SafetyClass {
    /// Return the IEC 62304 classification string.
    pub fn as_str(self) -> &'static str {
        match self {
            SafetyClass::ClassA => "A",
            SafetyClass::ClassB => "B",
            SafetyClass::ClassC => "C",
        }
    }
}

/// A single software requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareRequirement {
    /// Unique requirement identifier (e.g., "REQ-CORE-001").
    pub id: String,
    /// Requirement category.
    pub category: ReqCategory,
    /// Human-readable requirement description.
    pub description: String,
    /// Requirement priority.
    pub priority: ReqPriority,
    /// How this requirement is verified.
    pub verification_method: VerificationMethod,
    /// Current requirement status.
    pub status: ReqStatus,
    /// Traceability links to test cases that verify this requirement.
    pub trace_to_tests: Vec<String>,
}

/// Requirement category classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReqCategory {
    /// Functional requirement.
    Functional,
    /// Performance requirement.
    Performance,
    /// Safety requirement.
    Safety,
    /// Security requirement.
    Security,
    /// Usability requirement.
    Usability,
    /// Interoperability requirement.
    Interoperability,
}

impl ReqCategory {
    /// Return the category as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            ReqCategory::Functional => "Functional",
            ReqCategory::Performance => "Performance",
            ReqCategory::Safety => "Safety",
            ReqCategory::Security => "Security",
            ReqCategory::Usability => "Usability",
            ReqCategory::Interoperability => "Interoperability",
        }
    }
}

/// Requirement priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReqPriority {
    /// Must-have requirement (Mandatory).
    Must,
    /// Should-have requirement (Important).
    Should,
    /// May-have requirement (Optional).
    May,
}

impl ReqPriority {
    /// Return the priority as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            ReqPriority::Must => "Must",
            ReqPriority::Should => "Should",
            ReqPriority::May => "May",
        }
    }
}

/// Verification method for a requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationMethod {
    /// Verified by testing.
    Test,
    /// Verified by analysis.
    Analysis,
    /// Verified by inspection.
    Inspection,
    /// Verified by demonstration.
    Demonstration,
}

impl VerificationMethod {
    /// Return the method as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            VerificationMethod::Test => "Test",
            VerificationMethod::Analysis => "Analysis",
            VerificationMethod::Inspection => "Inspection",
            VerificationMethod::Demonstration => "Demonstration",
        }
    }
}

/// Requirement lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReqStatus {
    /// Draft requirement (not yet reviewed).
    Draft,
    /// Approved requirement.
    Approved,
    /// Implemented in software.
    Implemented,
    /// Verified by testing.
    Verified,
}

impl ReqStatus {
    /// Return the status as a string.
    pub fn as_str(self) -> &'static str {
        match self {
            ReqStatus::Draft => "Draft",
            ReqStatus::Approved => "Approved",
            ReqStatus::Implemented => "Implemented",
            ReqStatus::Verified => "Verified",
        }
    }
}

/// Build the Software Requirements Specification from the known REQ identifiers
/// in the DiCCY codebase.
///
/// This function compiles requirements from:
/// - `manifest.toml` sample identifiers (corpus validation)
/// - Known REQ-* identifiers found in test annotations across crates
/// - ISSUES.md severity classification for safety requirements
/// - IEC 62304 mandatory documentation requirements
pub fn build_srs_from_manifests() -> SoftwareRequirementsSpec {
    SoftwareRequirementsSpec {
        version: "0.1.0".to_string(),
        safety_class: SafetyClass::ClassC,
        requirements: vec![
            // ===== Core requirements =====
            SoftwareRequirement {
                id: "REQ-CORE-001".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall parse DICOM Part 10 files with explicit VR little-endian transfer syntax.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-CORE-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-CORE-002".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall parse DICOM Part 10 files with implicit VR little-endian transfer syntax.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-CORE-002".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-CORE-003".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall validate DICOM UIDs per PS3.5 formatting rules (0-9, '.', 1-64 chars, no leading/trailing dots).".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-CORE-003".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-CORE-004".to_string(),
                category: ReqCategory::Safety,
                description: "The system shall enforce configurable resource limits on input parsing to prevent denial-of-service from malformed DICOM data.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-CORE-004".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-CORE-005".to_string(),
                category: ReqCategory::Safety,
                description: "The system shall enforce VR/value consistency on element construction to prevent invalid DICOM data from propagating.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-CORE-005".to_string()],
            },
            // ===== IO requirements =====
            SoftwareRequirement {
                id: "REQ-IO-001".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall decode JPEG Baseline (1.2.840.10008.1.2.4.50) transfer syntax.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-IO-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-IO-002".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall decode RLE Lossless (1.2.840.10008.1.2.5) transfer syntax.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-IO-002".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-IO-003".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall reject unsupported SOP Class UIDs with a conformance error.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-IO-003".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-IO-004".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall reject unsupported Transfer Syntax UIDs with a conformance error.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-IO-004".to_string()],
            },
            // ===== Audit requirements =====
            SoftwareRequirement {
                id: "REQ-AUDIT-001".to_string(),
                category: ReqCategory::Security,
                description: "The system shall record audit events with SHA-256 integrity hash chain verification.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUDIT-002".to_string(),
                category: ReqCategory::Security,
                description: "The system shall redact sensitive audit fields before recording.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-002".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUDIT-003".to_string(),
                category: ReqCategory::Security,
                description: "The system shall detect tampering in the audit chain via SHA-256 hash verification.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-003".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUDIT-004".to_string(),
                category: ReqCategory::Security,
                description: "The system shall export audit records in IHE ATNA RFC 3881 XML format.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-004".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUDIT-005".to_string(),
                category: ReqCategory::Security,
                description: "The system shall audit all RBAC permission check decisions (allowed and denied).".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-005".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUDIT-006".to_string(),
                category: ReqCategory::Security,
                description: "The system shall provide tamper-evident audit log with append-only, signed entries.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUDIT-006".to_string()],
            },
            // ===== Auth requirements =====
            SoftwareRequirement {
                id: "REQ-AUTH-001".to_string(),
                category: ReqCategory::Security,
                description: "The system shall enforce role-based access control (RBAC) for all clinical operations.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUTH-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUTH-002".to_string(),
                category: ReqCategory::Security,
                description: "The system shall support OAuth2 authentication for enterprise deployments.".to_string(),
                priority: ReqPriority::Should,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Implemented,
                trace_to_tests: vec!["TC-AUTH-002".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-AUTH-003".to_string(),
                category: ReqCategory::Security,
                description: "The system shall enforce session timeout and lockout policies.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-AUTH-003".to_string()],
            },
            // ===== Rendering requirements =====
            SoftwareRequirement {
                id: "REQ-RENDER-001".to_string(),
                category: ReqCategory::Safety,
                description: "The system shall produce deterministic rendering output for identical input data and configuration.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-RENDER-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-RENDER-002".to_string(),
                category: ReqCategory::Functional,
                description: "The system shall apply window/level transforms deterministically across all viewport renderings.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-RENDER-002".to_string()],
            },
            // ===== Network requirements =====
            SoftwareRequirement {
                id: "REQ-NET-001".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall establish DICOM associations per PS3.8 upper-layer protocol.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-NET-001".to_string()],
            },
            // ===== DICOMweb requirements =====
            SoftwareRequirement {
                id: "REQ-WEB-001".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall implement QIDO-RS study/series/instance query endpoints.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-WEB-001".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-WEB-002".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall implement WADO-RS retrieval endpoints.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-WEB-002".to_string()],
            },
            SoftwareRequirement {
                id: "REQ-WEB-003".to_string(),
                category: ReqCategory::Interoperability,
                description: "The system shall implement STOW-RS storage endpoints.".to_string(),
                priority: ReqPriority::Must,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Verified,
                trace_to_tests: vec!["TC-WEB-003".to_string()],
            },
            // ===== Performance requirements =====
            SoftwareRequirement {
                id: "REQ-PERF-001".to_string(),
                category: ReqCategory::Performance,
                description: "The system shall decode a single-frame CR image within 500ms on reference hardware.".to_string(),
                priority: ReqPriority::Should,
                verification_method: VerificationMethod::Test,
                status: ReqStatus::Implemented,
                trace_to_tests: vec!["TC-PERF-001".to_string()],
            },
        ],
    }
}
