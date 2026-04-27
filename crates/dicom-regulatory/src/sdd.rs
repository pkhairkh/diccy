//! Software Design Description (IEC 62304 Section 5.3).
//!
//! Describes the software architecture derived from the workspace crate
//! structure, including crate purposes, dependencies, and bounded contexts.

use serde::{Deserialize, Serialize};

/// Software Design Description document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareDesignDesc {
    /// Document version.
    pub version: String,
    /// Architecture description.
    pub architecture: ArchitectureDescription,
    /// Design elements mapping requirements to implementations.
    pub design_elements: Vec<DesignElement>,
}

/// Description of the software architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchitectureDescription {
    /// All workspace crates.
    pub crates: Vec<CrateDescription>,
    /// Dependency edges between crates.
    pub dependencies: Vec<DependencyEdge>,
}

/// Description of a single workspace crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateDescription {
    /// Crate name.
    pub name: String,
    /// Purpose of this crate.
    pub purpose: String,
    /// Bounded context (DDD) this crate belongs to.
    pub bounded_context: String,
    /// Public API surface (key types/traits).
    pub public_api: Vec<String>,
}

/// A directed dependency edge between two crates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// Source crate (dependent).
    pub from: String,
    /// Target crate (dependency).
    pub to: String,
}

/// A design element mapping a requirement to its implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignElement {
    /// Design element identifier.
    pub id: String,
    /// Parent requirement identifier.
    pub parent_requirement: String,
    /// Description of the design approach.
    pub description: String,
    /// Crates/modules that implement this design element.
    pub implements: Vec<String>,
}

/// Build the Software Design Description from the workspace crate structure.
///
/// This function enumerates all known crates, their purposes, bounded
/// contexts, and dependency relationships.
pub fn build_sdd_from_workspace() -> SoftwareDesignDesc {
    SoftwareDesignDesc {
        version: "0.1.0".to_string(),
        architecture: ArchitectureDescription {
            crates: vec![
                CrateDescription {
                    name: "dicom-core".to_string(),
                    purpose: "Core DICOM types, error model, limits, capabilities, and tag constants.".to_string(),
                    bounded_context: "Domain Core".to_string(),
                    public_api: vec![
                        "Tag".to_string(),
                        "Vr".to_string(),
                        "Value".to_string(),
                        "Element".to_string(),
                        "Dataset".to_string(),
                        "Limits".to_string(),
                        "Capabilities".to_string(),
                        "Error".to_string(),
                        "ErrorKind".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-io".to_string(),
                    purpose: "DICOM Part 10 file parsing and transfer syntax handling.".to_string(),
                    bounded_context: "IO/Parsing".to_string(),
                    public_api: vec![
                        "P10Parser".to_string(),
                        "TransferSyntax".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-net".to_string(),
                    purpose: "DICOM upper-layer association protocol (PS3.8).".to_string(),
                    bounded_context: "Network/Transport".to_string(),
                    public_api: vec![
                        "AssociationRequest".to_string(),
                        "AssociationAccept".to_string(),
                        "PresentationContext".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-dimse".to_string(),
                    purpose: "DIMSE message encoding/decoding (C-ECHO, C-STORE, C-FIND, C-MOVE, C-GET).".to_string(),
                    bounded_context: "Network/DIMSE".to_string(),
                    public_api: vec![
                        "DimseMessage".to_string(),
                        "Pdv".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-dimse-service".to_string(),
                    purpose: "DIMSE service layer with storage-backed handlers and TLS support.".to_string(),
                    bounded_context: "Network/Service".to_string(),
                    public_api: vec![
                        "DimseService".to_string(),
                        "StorageBackedDimseService".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-web".to_string(),
                    purpose: "DICOMweb endpoints: QIDO-RS, WADO-RS, STOW-RS.".to_string(),
                    bounded_context: "Web/API".to_string(),
                    public_api: vec![
                        "QidoRsHandler".to_string(),
                        "WadoRsHandler".to_string(),
                        "StowRsHandler".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-auth".to_string(),
                    purpose: "Authentication, RBAC authorization, session management, and OAuth2.".to_string(),
                    bounded_context: "Security/Auth".to_string(),
                    public_api: vec![
                        "Authorizer".to_string(),
                        "RbacPolicy".to_string(),
                        "Role".to_string(),
                        "Permission".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-audit".to_string(),
                    purpose: "Audit logging with SHA-256 integrity chain, IHE ATNA export, and RBAC audit events.".to_string(),
                    bounded_context: "Security/Audit".to_string(),
                    public_api: vec![
                        "AuditLog".to_string(),
                        "AuditRecord".to_string(),
                        "AuditRedactor".to_string(),
                        "AtnaExporter".to_string(),
                        "TamperEvidentLog".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-pixel".to_string(),
                    purpose: "Pixel data codec support: JPEG Baseline, JPEG-LS, JPEG 2000, RLE.".to_string(),
                    bounded_context: "IO/Codec".to_string(),
                    public_api: vec![
                        "PixelCodec".to_string(),
                        "DecodeOptions".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-storage".to_string(),
                    purpose: "WAL-based DICOM object storage with S3 backend and VNA lifecycle.".to_string(),
                    bounded_context: "Storage/Core".to_string(),
                    public_api: vec![
                        "Storage".to_string(),
                        "WalEntry".to_string(),
                        "S3Config".to_string(),
                    ],
                },
                CrateDescription {
                    name: "dicom-index".to_string(),
                    purpose: "DICOM study/series/instance indexing with tag-based lookups.".to_string(),
                    bounded_context: "Storage/Index".to_string(),
                    public_api: vec!["Index".to_string()],
                },
                CrateDescription {
                    name: "viewer-core".to_string(),
                    purpose: "Core viewer model: measurements, segmentations, MPR, GSDF, hanging protocols.".to_string(),
                    bounded_context: "Viewer/Domain".to_string(),
                    public_api: vec![
                        "ViewerModel".to_string(),
                        "MeasurementStore".to_string(),
                        "SegmentationStore".to_string(),
                        "VolumeWorkflowState".to_string(),
                    ],
                },
                CrateDescription {
                    name: "viewer-wgpu".to_string(),
                    purpose: "GPU-accelerated rendering via wgpu for MPR, volume, and fusion.".to_string(),
                    bounded_context: "Viewer/Rendering".to_string(),
                    public_api: vec![
                        "VolumeRenderer".to_string(),
                        "GpuContext".to_string(),
                    ],
                },
                CrateDescription {
                    name: "viewer-wasm".to_string(),
                    purpose: "WebAssembly viewer boundary for browser-based rendering.".to_string(),
                    bounded_context: "Viewer/Web".to_string(),
                    public_api: vec!["WasmBridge".to_string()],
                },
                CrateDescription {
                    name: "dicom-regulatory".to_string(),
                    purpose: "IEC 62304 regulatory documentation bundle: SRS, SDD, STP, RMF.".to_string(),
                    bounded_context: "Regulatory/Compliance".to_string(),
                    public_api: vec![
                        "SoftwareRequirementsSpec".to_string(),
                        "SoftwareDesignDesc".to_string(),
                        "SoftwareTestPlan".to_string(),
                        "RiskManagementFile".to_string(),
                        "DeterminismGuarantees".to_string(),
                    ],
                },
            ],
            dependencies: vec![
                DependencyEdge { from: "dicom-io".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-net".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-dimse".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-dimse-service".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-dimse-service".to_string(), to: "dicom-dimse".to_string() },
                DependencyEdge { from: "dicom-dimse-service".to_string(), to: "dicom-net".to_string() },
                DependencyEdge { from: "dicom-dimse-service".to_string(), to: "dicom-storage".to_string() },
                DependencyEdge { from: "dicom-web".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-auth".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-audit".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-pixel".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-storage".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-index".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "viewer-core".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "viewer-wgpu".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "viewer-wgpu".to_string(), to: "viewer-core".to_string() },
                DependencyEdge { from: "viewer-wasm".to_string(), to: "dicom-core".to_string() },
                DependencyEdge { from: "dicom-regulatory".to_string(), to: "dicom-core".to_string() },
            ],
        },
        design_elements: vec![
            DesignElement {
                id: "DE-001".to_string(),
                parent_requirement: "REQ-CORE-001".to_string(),
                description: "P10Parser in dicom-io implements explicit VR LE parsing with validated Element construction.".to_string(),
                implements: vec!["dicom-io".to_string()],
            },
            DesignElement {
                id: "DE-002".to_string(),
                parent_requirement: "REQ-CORE-004".to_string(),
                description: "Limits struct in dicom-core enforces resource bounds on parsing; Dataset::insert_checked validates against Limits.".to_string(),
                implements: vec!["dicom-core".to_string()],
            },
            DesignElement {
                id: "DE-003".to_string(),
                parent_requirement: "REQ-CORE-005".to_string(),
                description: "Element::new validates VR/value consistency at construction; Dataset::insert_validated provides additional safety.".to_string(),
                implements: vec!["dicom-core".to_string()],
            },
            DesignElement {
                id: "DE-004".to_string(),
                parent_requirement: "REQ-AUDIT-001".to_string(),
                description: "AuditLog in dicom-audit computes SHA-256 integrity hash per record, chained to the previous record's hash.".to_string(),
                implements: vec!["dicom-audit".to_string()],
            },
            DesignElement {
                id: "DE-005".to_string(),
                parent_requirement: "REQ-AUDIT-004".to_string(),
                description: "AtnaExporter in dicom-audit converts AuditRecord to IHE ATNA RFC 3881 XML format.".to_string(),
                implements: vec!["dicom-audit".to_string()],
            },
            DesignElement {
                id: "DE-006".to_string(),
                parent_requirement: "REQ-AUDIT-005".to_string(),
                description: "audit_rbac_check in dicom-audit generates audit events for RBAC permission decisions.".to_string(),
                implements: vec!["dicom-audit".to_string()],
            },
            DesignElement {
                id: "DE-007".to_string(),
                parent_requirement: "REQ-AUDIT-006".to_string(),
                description: "TamperEvidentLog in dicom-audit wraps AuditLog with cryptographic signing and chain verification.".to_string(),
                implements: vec!["dicom-audit".to_string()],
            },
            DesignElement {
                id: "DE-008".to_string(),
                parent_requirement: "REQ-AUTH-001".to_string(),
                description: "RbacPolicy in dicom-auth provides role-to-permission mapping and study-level access control.".to_string(),
                implements: vec!["dicom-auth".to_string()],
            },
            DesignElement {
                id: "DE-009".to_string(),
                parent_requirement: "REQ-RENDER-001".to_string(),
                description: "ViewerModel and rendering pipeline produce deterministic output; verified by hash-based golden corpus.".to_string(),
                implements: vec!["viewer-core".to_string(), "viewer-wgpu".to_string()],
            },
            DesignElement {
                id: "DE-010".to_string(),
                parent_requirement: "REQ-IO-003".to_string(),
                description: "Conformance envelope in dicom-pixel rejects unsupported SOP Class UIDs with UnsupportedSopClass error.".to_string(),
                implements: vec!["dicom-pixel".to_string()],
            },
        ],
    }
}
