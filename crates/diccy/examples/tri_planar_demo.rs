use diccy::{reslice_volume, MprLimits, MprRequest, TriPlanarPlane, ViewerModel, VolumeGrid};

fn main() {
    let mut volume = VolumeGrid::new([4, 4, 4], [1_000, 1_000, 1_000]).expect("volume");
    for z in 0..4 {
        for y in 0..4 {
            for x in 0..4 {
                let value = (z * 100 + y * 10 + x) as i32;
                assert!(volume.set_voxel(x, y, z, value));
            }
        }
    }

    let mut model = ViewerModel::new(4, 4);
    let dims = volume.dimensions();
    model.set_mpr_volume_dimensions([dims[0] as u32, dims[1] as u32, dims[2] as u32]);
    let _ = model.set_mpr_crosshair_voxel([2, 1, 3]);
    let _ = model.step_mpr_plane(TriPlanarPlane::Sagittal, 1);
    let state = model.tri_planar_state();
    println!(
        "crosshair=[{},{},{}] axial={} coronal={} sagittal={}",
        state.crosshair_voxel[0],
        state.crosshair_voxel[1],
        state.crosshair_voxel[2],
        state.axial_index,
        state.coronal_index,
        state.sagittal_index
    );

    let request = MprRequest {
        index: state.axial_index as i32,
        output_width: 4,
        output_height: 4,
        ..MprRequest::default()
    };
    let frame = reslice_volume(&volume, &request, MprLimits::default()).expect("reslice");
    println!(
        "axial frame {}x{} first={} last={}",
        frame.width,
        frame.height,
        frame.pixels.first().copied().unwrap_or_default(),
        frame.pixels.last().copied().unwrap_or_default()
    );
}
