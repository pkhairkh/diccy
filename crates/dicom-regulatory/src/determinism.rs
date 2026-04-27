//! Deterministic rendering guarantees for regulatory validation.
//!
//! Documents the guarantees that the DiCCY rendering pipeline produces
//! identical output for identical input data and configuration, which is
//! a critical safety property for diagnostic medical imaging.

use serde::{Deserialize, Serialize};

/// Collection of determinism guarantees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismGuarantees {
    /// All documented guarantees.
    pub guarantees: Vec<DeterminismGuarantee>,
}

/// A single determinism guarantee for a rendering component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeterminismGuarantee {
    /// Unique guarantee identifier.
    pub id: String,
    /// Component that provides the guarantee.
    pub component: String,
    /// The determinism guarantee statement.
    pub guarantee: String,
    /// How the guarantee is validated.
    pub validation_method: String,
    /// Test coverage status.
    pub test_coverage: String,
}

/// Build the determinism guarantees document from the known rendering pipeline.
///
/// Each guarantee is derived from the rendering architecture and verified
/// by the golden corpus hash comparison system.
pub fn build_determinism_guarantees() -> DeterminismGuarantees {
    DeterminismGuarantees {
        guarantees: vec![
            DeterminismGuarantee {
                id: "DET-001".to_string(),
                component: "dicom-pixel (codec pipeline)".to_string(),
                guarantee: "Decompressed pixel data is bit-identical for identical compressed input and decode configuration. No floating-point variation between runs.".to_string(),
                validation_method: "Golden corpus SHA-256 hash comparison: corpus/manifest.toml defines expected sha256 for each sample's decompressed output.".to_string(),
                test_coverage: "Full — every codec path (JPEG Baseline, JPEG-LS, JPEG 2000, RLE, raw) has golden corpus entries with hash verification.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-002".to_string(),
                component: "viewer-core (window/level transform)".to_string(),
                guarantee: "Window/level lookup table output is deterministic for identical input pixel data, window center/width, and rescale slope/intercept values.".to_string(),
                validation_method: "Manifest config_id entries (e.g., wl-explicit, modality-rescale-mono1-v1, modality-rescale-calibrated16-v1) verify output hash for each window configuration.".to_string(),
                test_coverage: "Full — mono1 inversion, mono16 padding, mono16 rescale calibration, and explicit window configs all have hash-verified test cases.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-003".to_string(),
                component: "viewer-core (GSDF pipeline)".to_string(),
                guarantee: "Grayscale Standard Display Function (GSDF) produces identical JND index mappings for identical luminance input values.".to_string(),
                validation_method: "Unit tests verify GSDF output against reference JND values; viewer-core determinism tests compare cross-run output.".to_string(),
                test_coverage: "Full — gsdf_tests.rs verifies JND mapping against DICOM PS3.14 reference values.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-004".to_string(),
                component: "viewer-core (measurement ordering)".to_string(),
                guarantee: "Measurement records are ordered by monotonic tick counter; undo/redo state machine produces identical measurement lists for identical operation sequences.".to_string(),
                validation_method: "MeasurementStore unit tests verify monotonic tick increment; undo/redo round-trip produces original state; snapshot hash is deterministic.".to_string(),
                test_coverage: "Full — measurement_integration.rs and inline_tests.rs cover tick monotonicity and undo/redo determinism.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-005".to_string(),
                component: "viewer-core (hanging protocol)".to_string(),
                guarantee: "Hanging protocol layout assignment produces identical viewport configurations for identical study metadata and protocol rules.".to_string(),
                validation_method: "hanging_protocol_tests.rs verifies layout assignment against expected viewport mappings.".to_string(),
                test_coverage: "Full — protocol matching, priority ordering, and fallback behavior tested.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-006".to_string(),
                component: "viewer-core (MPR crosshair)".to_string(),
                guarantee: "MPR crosshair positioning is deterministic for identical volume data and crosshair voxel coordinates.".to_string(),
                validation_method: "mpr_tests.rs verifies crosshair rendering at known voxel positions.".to_string(),
                test_coverage: "Full — axial, sagittal, and coronal crosshair positions verified.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-007".to_string(),
                component: "dicom-audit (integrity chain)".to_string(),
                guarantee: "SHA-256 integrity hash computation is deterministic: identical audit records and previous hash produce identical integrity hashes across runs.".to_string(),
                validation_method: "audit_chain_deterministic test verifies that two independent AuditLog instances produce identical hashes for the same event sequence.".to_string(),
                test_coverage: "Full — inline_tests.rs contains explicit determinism verification test.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-008".to_string(),
                component: "pack-rt / pack-seg / pack-enhanced (overlay rendering)".to_string(),
                guarantee: "RT dose overlay, segmentation labelmap, and enhanced multi-frame geometry produce deterministic pixel output for identical input data.".to_string(),
                validation_method: "Manifest expected_outputs verify overlay rendering hashes; hi_rt_overlay_contract and hi_seg_overlay_contract tests verify overlay determinism.".to_string(),
                test_coverage: "Full — RT dose color mapping, segmentation label colors, and enhanced geometry hash verified.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-009".to_string(),
                component: "dicom-core (UID validation)".to_string(),
                guarantee: "UID validation is deterministic: identical UID strings produce identical validation results (accept or reject) across all runs.".to_string(),
                validation_method: "Unit tests verify acceptance of valid UIDs and rejection of malformed UIDs with consistent error types.".to_string(),
                test_coverage: "Full — validate_uid_strict covers leading zeros, trailing dots, length violations, and invalid characters.".to_string(),
            },
            DeterminismGuarantee {
                id: "DET-010".to_string(),
                component: "viewer-core (color space conversion)".to_string(),
                guarantee: "YBR-RGB color space conversion produces identical RGBA8 output for identical YBR input data.".to_string(),
                validation_method: "Manifest color-ybr422-v1 config verifies output hash for YBR422 input.".to_string(),
                test_coverage: "Full — YBR422 conversion verified against reference hash.".to_string(),
            },
        ],
    }
}
