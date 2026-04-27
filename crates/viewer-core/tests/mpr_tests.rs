// Auto-extracted from /home/z/diccy/crates/viewer-core/src/mpr.rs
// S13-T8: Move inline tests to tests/ directories

use viewer_core::{
    patient_request_to_voxel_request, reslice_volume, reslice_volume_patient, MprError,
    MprLimits, MprPlane, MprRequest, PatientMprPlane, PatientMprRequest, ResampleKernel,
    SlabMode, VolumeGrid,
};

fn synthetic_volume() -> VolumeGrid {
    // z0
    // [10, 20]
    // [30, 40]
    // z1
    // [50, 60]
    // [70, 80]
    // z2
    // [90, 100]
    // [110, 120]
    let voxels = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120];
    let mut rebuilt = VolumeGrid::new([2, 2, 3], [1_000, 1_000, 1_000]).expect("volume");
    for z in 0..3usize {
        for y in 0..2usize {
            for x in 0..2usize {
                let src_idx = z * 4 + y * 2 + x;
                rebuilt.set_voxel(x, y, z, voxels[src_idx]);
            }
        }
    }
    rebuilt
}

#[test]
fn axial_coronal_sagittal_are_deterministic() {
    let volume = synthetic_volume();
    let limits = MprLimits::default();

    let axial = reslice_volume(
        &volume,
        &MprRequest {
            plane: MprPlane::Axial,
            index: 1,
            output_width: 2,
            output_height: 2,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 1,
            slab_mode: SlabMode::Average,
        },
        limits,
    )
    .expect("axial");
    assert_eq!(axial.pixels, vec![50, 60, 70, 80]);

    let coronal = reslice_volume(
        &volume,
        &MprRequest {
            plane: MprPlane::Coronal,
            index: 1,
            output_width: 2,
            output_height: 3,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 1,
            slab_mode: SlabMode::Average,
        },
        limits,
    )
    .expect("coronal");
    assert_eq!(coronal.pixels, vec![30, 40, 70, 80, 110, 120]);

    let sagittal = reslice_volume(
        &volume,
        &MprRequest {
            plane: MprPlane::Sagittal,
            index: 0,
            output_width: 2,
            output_height: 3,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 1,
            slab_mode: SlabMode::Average,
        },
        limits,
    )
    .expect("sagittal");
    assert_eq!(sagittal.pixels, vec![10, 30, 50, 70, 90, 110]);
}

#[test]
fn axis_aligned_patient_requests_match_voxel_requests() {
    let mut volume = synthetic_volume();
    volume.migrate_patient_geometry_from_legacy(None);
    let limits = MprLimits::default();

    let voxel_request = MprRequest {
        plane: MprPlane::Axial,
        index: 1,
        output_width: 2,
        output_height: 2,
        kernel: ResampleKernel::Nearest,
        background: 0,
        voi_window: None,
        slab_thickness: 1,
        slab_mode: SlabMode::Average,
    };
    let patient_request = PatientMprRequest {
        plane: PatientMprPlane::Axial { offset_um: 1_000 },
        output_width: 2,
        output_height: 2,
        kernel: ResampleKernel::Nearest,
        background: 0,
        voi_window: None,
        slab_thickness: 1,
        slab_mode: SlabMode::Average,
    };

    let voxel = reslice_volume(&volume, &voxel_request, limits).expect("voxel reslice");
    let patient =
        reslice_volume_patient(&volume, &patient_request, limits).expect("patient reslice");
    assert_eq!(voxel.pixels, patient.pixels);

    let converted =
        patient_request_to_voxel_request(&volume, &patient_request).expect("convert request");
    assert_eq!(converted.index, voxel_request.index);
}

#[test]
fn slab_composition_average_is_deterministic() {
    let volume = synthetic_volume();
    let limits = MprLimits::default();
    let slab = reslice_volume(
        &volume,
        &MprRequest {
            plane: MprPlane::Axial,
            index: 1,
            output_width: 2,
            output_height: 2,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 3,
            slab_mode: SlabMode::Average,
        },
        limits,
    )
    .expect("slab");
    assert_eq!(slab.pixels, vec![50, 60, 70, 80]);
}

#[test]
fn mpr_error_codes_are_stable() {
    assert_eq!(
        MprError::InvalidRequest("bad").code(),
        "DVF.MPR.INVALID_REQUEST"
    );
    assert_eq!(
        MprError::SourceLimitExceeded {
            observed: 10,
            allowed: 1
        }
        .code(),
        "DVF.MPR.SOURCE_LIMIT_EXCEEDED"
    );
    assert_eq!(
        MprError::OutputLimitExceeded {
            observed: 10,
            allowed: 1
        }
        .code(),
        "DVF.MPR.OUTPUT_LIMIT_EXCEEDED"
    );
}
