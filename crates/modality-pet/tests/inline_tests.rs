// Auto-extracted from modality-pet/src/lib.rs
// S13-T8: Move inline tests to tests/ directories
// Originally gated behind #[cfg(all(test, feature = "viewer-core"))]

use modality_pet::*;
use sha2::{Digest, Sha256};
use viewer_core::VolumeGrid;

/// Newtype wrapper to provide VolumeGridProvider for VolumeGrid in integration tests.
/// In production code, this impl is behind #[cfg(feature = "viewer-core")].
struct TestVolumeGrid(VolumeGrid);

impl VolumeGridProvider for TestVolumeGrid {
    fn dimensions(&self) -> [usize; 3] {
        self.0.dimensions()
    }
    fn voxel(&self, x: usize, y: usize, z: usize) -> Option<i32> {
        self.0.voxel(x, y, z)
    }
    fn voxels(&self) -> &[i32] {
        self.0.voxels()
    }
    fn has_patient_geometry(&self) -> bool {
        self.0.patient_geometry().is_some()
    }
    fn voxel_to_patient_um(&self, x: usize, y: usize, z: usize) -> Option<[i64; 3]> {
        self.0.voxel_to_patient_um(x, y, z)
    }
    fn patient_to_voxel_f64(&self, patient_um: [f64; 3]) -> Option<[f64; 3]> {
        self.0.patient_to_voxel_f64(patient_um)
    }
}

fn volume(values: [i32; 4], uid: &str) -> TestVolumeGrid {
    let mut grid = VolumeGrid::new([2, 2, 1], [1_000, 1_000, 1_000]).expect("volume");
    for (idx, value) in values.into_iter().enumerate() {
        let x = idx % 2;
        let y = idx / 2;
        assert!(grid.set_voxel(x, y, 0, value));
    }
    grid.migrate_patient_geometry_from_legacy(Some(uid.to_string()));
    TestVolumeGrid(grid)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[test]
#[cfg(not(feature = "modality-pet"))]
fn pet_pack_disabled_rejects_pet() {
    assert!(!PetPack::enabled());
    let err = PetPack::ensure_pet_supported().unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
}

#[test]
#[cfg(feature = "modality-pet")]
fn pet_pack_enabled_allows_pet() {
    assert!(PetPack::enabled());
    PetPack::ensure_pet_supported().expect("pet pack enabled");
}

#[test]
fn fusion_rejects_missing_frame_of_reference() {
    let err = validate_fusion(FusionInput {
        ct_frame_of_reference_uid: None,
        pet_frame_of_reference_uid: Some("1.2.3"),
    })
    .unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
}

#[test]
fn fusion_rejects_mismatch() {
    let err = validate_fusion(FusionInput {
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.4"),
    })
    .unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn fusion_accepts_match() {
    validate_fusion(FusionInput {
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
    })
    .expect("fusion ok");
}

#[test]
fn suv_rejects_non_finite() {
    let err = apply_suv(SuvScaleInput {
        raw_value: f64::INFINITY,
        scale_factor: 1.0,
    })
    .unwrap_err();
    assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
}

#[test]
fn suv_rejects_non_positive_scale() {
    let err = apply_suv(SuvScaleInput {
        raw_value: 1.0,
        scale_factor: 0.0,
    })
    .unwrap_err();
    assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
}

#[test]
fn suv_scales() {
    let value = apply_suv(SuvScaleInput {
        raw_value: 2.0,
        scale_factor: 0.5,
    })
    .expect("suv ok");
    assert_eq!(value, 1.0);
}

#[test]
fn resample_and_blend_pipeline_is_deterministic() {
    let ct = volume([10, 20, 30, 40], "1.2.3");
    let pet = volume([4, 8, 12, 16], "1.2.3");
    let input = PetCtFusionInput {
        ct_volume: &ct,
        pet_volume: &pet,
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
        pet_suv_scale_factor: 0.5,
    };
    let limits = FusionExecutionLimits::default();
    let result =
        execute_pet_ct_fusion(&input, limits, FusionBlendPolicy::default()).expect("fusion");
    assert_eq!(result.dimensions, [2, 2, 1]);
    assert_eq!(result.pet_on_ct_suv, vec![2.0, 4.0, 6.0, 8.0]);
    assert_eq!(result.overlay_rgba.len(), 2 * 2 * 4);
}

#[test]
fn preconditions_fail_closed_for_missing_geometry_and_limits() {
    let mut ct_grid = VolumeGrid::new([2, 2, 1], [1_000, 1_000, 1_000]).expect("ct");
    let mut pet_grid = VolumeGrid::new([2, 2, 1], [1_000, 1_000, 1_000]).expect("pet");
    for (idx, value) in [1, 2, 3, 4].into_iter().enumerate() {
        let x = idx % 2;
        let y = idx / 2;
        assert!(ct_grid.set_voxel(x, y, 0, value));
    }
    for (idx, value) in [5, 6, 7, 8].into_iter().enumerate() {
        let x = idx % 2;
        let y = idx / 2;
        assert!(pet_grid.set_voxel(x, y, 0, value));
    }
    let ct = TestVolumeGrid(ct_grid);
    let pet = TestVolumeGrid(pet_grid);
    let missing_geometry_input = PetCtFusionInput {
        ct_volume: &ct,
        pet_volume: &pet,
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
        pet_suv_scale_factor: 1.0,
    };
    let err = validate_fusion_preconditions(
        &missing_geometry_input,
        FusionExecutionLimits::default(),
    )
    .expect_err("missing geometry");
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");

    let mut ct_grid2 = ct.0.clone();
    let mut pet_grid2 = pet.0.clone();
    ct_grid2.migrate_patient_geometry_from_legacy(None);
    pet_grid2.migrate_patient_geometry_from_legacy(None);
    let ct2 = TestVolumeGrid(ct_grid2);
    let pet2 = TestVolumeGrid(pet_grid2);
    let limited_input = PetCtFusionInput {
        ct_volume: &ct2,
        pet_volume: &pet2,
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
        pet_suv_scale_factor: 1.0,
    };
    let tight_limits = FusionExecutionLimits {
        max_output_voxels: 2,
        max_output_bytes: 16,
    };
    let err = validate_fusion_preconditions(&limited_input, tight_limits)
        .expect_err("limit exceeded");
    assert_eq!(err.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
}

#[test]
fn fusion_golden_hash_is_stable() {
    let ct = volume([10, 20, 30, 40], "1.2.3");
    let pet = volume([4, 8, 12, 16], "1.2.3");
    let input = PetCtFusionInput {
        ct_volume: &ct,
        pet_volume: &pet,
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
        pet_suv_scale_factor: 0.5,
    };
    let result = execute_pet_ct_fusion(
        &input,
        FusionExecutionLimits::default(),
        FusionBlendPolicy::default(),
    )
    .expect("fusion");
    let hash = sha256_hex(&result.overlay_rgba);
    assert_eq!(
        hash,
        "baaef6c84100eaf380da58e40b3028dee3ef90455ab18216e64722e326c4ec80"
    );
}

#[test]
fn fusion_performance_budget_gate_small_profile() {
    let ct = volume([10, 20, 30, 40], "1.2.3");
    let pet = volume([4, 8, 12, 16], "1.2.3");
    let input = PetCtFusionInput {
        ct_volume: &ct,
        pet_volume: &pet,
        ct_frame_of_reference_uid: Some("1.2.3"),
        pet_frame_of_reference_uid: Some("1.2.3"),
        pet_suv_scale_factor: 1.0,
    };
    let started = std::time::Instant::now();
    let _ = execute_pet_ct_fusion(
        &input,
        FusionExecutionLimits::default(),
        FusionBlendPolicy::default(),
    )
    .expect("fusion");
    let elapsed_ms = started.elapsed().as_millis();
    assert!(elapsed_ms < 500, "fusion exceeded budget: {elapsed_ms}ms");
}
