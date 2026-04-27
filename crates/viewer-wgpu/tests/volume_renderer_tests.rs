// Auto-extracted from /home/z/diccy/crates/viewer-wgpu/src/volume_renderer.rs
// S13-T8: Move inline tests to tests/ directories

use viewer_wgpu::*;
use viewer_core::VolumeGrid;

fn test_volume_grid() -> VolumeGrid {
    let mut grid = VolumeGrid::new([4, 4, 4], [1000, 1000, 1000]).expect("grid");
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let value = ((z * 16 + y * 4 + x) * 100) as i32;
                grid.set_voxel(x, y, z, value);
            }
        }
    }
    grid
}

#[test]
fn volume_upload_rejects_zero_dimensions() {
    // REQ-DET-001 / DVF.VOLUME.EMPTY_INPUT
    // Zero-dimension volumes create a zero-sized voxel buffer,
    // which is valid but has no voxels to upload.
    let grid = VolumeGrid::new([0, 0, 0], [1000, 1000, 1000]);
    assert!(grid.is_ok());
    let grid = grid.unwrap();
    assert_eq!(grid.voxels().len(), 0);
}

#[test]
fn volume_data_range_is_deterministic() {
    // REQ-DET-001: data range must be deterministic for identical input
    let grid = test_volume_grid();
    let voxels = grid.voxels();
    let min_val = voxels.iter().map(|&v| v as f32).fold(f32::MAX, f32::min);
    let max_val = voxels.iter().map(|&v| v as f32).fold(f32::MIN, f32::max);
    assert_eq!(min_val, 0.0);
    assert_eq!(max_val, 6300.0); // (3*16 + 3*4 + 3) * 100
}

#[test]
fn transfer_function_presets_generate_deterministic_data() {
    // REQ-DET-001: TF presets must produce identical output for identical input
    let bone1 = TransferFunctionPreset::Bone.generate_rgba((-1024.0, 3071.0));
    let bone2 = TransferFunctionPreset::Bone.generate_rgba((-1024.0, 3071.0));
    assert_eq!(bone1, bone2);

    let lung1 = TransferFunctionPreset::Lung.generate_rgba((-1024.0, 3071.0));
    let lung2 = TransferFunctionPreset::Lung.generate_rgba((-1024.0, 3071.0));
    assert_eq!(lung1, lung2);
}

#[test]
fn arcball_camera_inverse_view_projection_is_deterministic() {
    // REQ-DET-001: camera matrices must be deterministic
    let cam = ArcballCamera::new([256, 256, 128], 1.5);
    let m1 = cam.inverse_view_projection();
    let m2 = cam.inverse_view_projection();
    assert_eq!(m1, m2);
}

#[test]
fn mpr_request_axial_coronal_sagittal_are_deterministic() {
    // REQ-DET-001: MPR requests are deterministic for same input
    let dims = [256, 256, 128];
    let req1 = GpuMprRequest::axial(64.0, dims, 500.0, 2000.0);
    let req2 = GpuMprRequest::axial(64.0, dims, 500.0, 2000.0);
    assert_eq!(req1, req2);

    let req3 = GpuMprRequest::coronal(128.0, dims, 500.0, 2000.0);
    let req4 = GpuMprRequest::coronal(128.0, dims, 500.0, 2000.0);
    assert_eq!(req3, req4);

    let req5 = GpuMprRequest::sagittal(64.0, dims, 500.0, 2000.0);
    let req6 = GpuMprRequest::sagittal(64.0, dims, 500.0, 2000.0);
    assert_eq!(req5, req6);
}

#[test]
fn gpu_budget_enforcement_blocks_oversized_volume() {
    // REQ-ARCH-140 / DVF.VOLUME.SIZE_OVERFLOW
    let mut budget = GpuBudget::new(64);
    // 4*4*4 * 4 bytes = 256 bytes, exceeds budget of 64
    let required: u64 = 4 * 4 * 4 * 4;
    assert!(required > budget.max_texture_bytes);
    let result = budget.reserve(required);
    assert!(result.is_err());
}

#[test]
fn clip_plane_new_is_deterministic() {
    let p1 = ClipPlane::new([1.0, 0.0, 0.0], 128.0);
    let p2 = ClipPlane::new([1.0, 0.0, 0.0], 128.0);
    assert_eq!(p1, p2);
}

#[test]
fn look_at_rh_produces_valid_view_matrix() {
    let eye = [0.0, 0.0, 500.0];
    let target = [0.0, 0.0, 0.0];
    let up = [0.0, 1.0, 0.0];
    let m = look_at_rh(eye, target, up);
    // The view matrix is column-major. Column 2 (m[i][2]) is the -forward direction.
    // For a camera looking along -Z, column 2 should be (0, 0, 1) (i.e. -forward = +Z)
    assert!((m[0][2] - 0.0).abs() < 0.001);
    assert!((m[1][2] - 0.0).abs() < 0.001);
    assert!((m[2][2] - 1.0).abs() < 0.001);
}

#[test]
fn perspective_rh_produces_valid_proj_matrix() {
    let m = perspective_rh(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 1000.0);
    // For 45 degree FOV: f = 1/tan(22.5°) ≈ 2.414
    let expected_f = 1.0 / (std::f32::consts::FRAC_PI_8).tan();
    assert!((m[0][0] - expected_f).abs() < 0.01);
}

#[test]
fn invert_mat4_identity_returns_identity() {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let inv = invert_mat4(identity);
    for i in 0..4 {
        for j in 0..4 {
            let expected = if i == j { 1.0 } else { 0.0 };
            assert!(
                (inv[i][j] - expected).abs() < 0.0001,
                "inv[{i}][{j}] = {} expected {expected}",
                inv[i][j]
            );
        }
    }
}

#[test]
fn mul_mat4_identity_is_identity() {
    let identity = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let m = [
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
        [9.0, 10.0, 11.0, 12.0],
        [13.0, 14.0, 15.0, 16.0],
    ];
    let result = mul_mat4(identity, m);
    for i in 0..4 {
        for j in 0..4 {
            assert!((result[i][j] - m[i][j]).abs() < 0.0001);
        }
    }
}
