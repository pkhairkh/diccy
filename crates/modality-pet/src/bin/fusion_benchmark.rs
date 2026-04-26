use modality_pet::{
    execute_pet_ct_fusion, FusionBlendPolicy, FusionExecutionLimits, PetCtFusionInput,
    VolumeGridProvider,
};
#[cfg(feature = "viewer-core")]
use viewer_core::VolumeGrid;

#[cfg(feature = "viewer-core")]
fn build_volume(dims: [usize; 3], seed: i32, frame_uid: &str) -> VolumeGrid {
    let mut volume = VolumeGrid::new(dims, [1_000, 1_000, 1_000]).expect("volume");
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let value = seed + (z * dims[0] * dims[1] + y * dims[0] + x) as i32;
                assert!(volume.set_voxel(x, y, z, value));
            }
        }
    }
    volume.migrate_patient_geometry_from_legacy(Some(frame_uid.to_string()));
    volume
}

fn parse_arg<T: std::str::FromStr>(args: &[String], name: &str, default: T) -> T {
    let prefix = format!("--{name}=");
    args.iter()
        .find_map(|arg| arg.strip_prefix(&prefix))
        .and_then(|value| value.parse::<T>().ok())
        .unwrap_or(default)
}

#[cfg(feature = "viewer-core")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let width = parse_arg(&args, "width", 64usize);
    let height = parse_arg(&args, "height", 64usize);
    let depth = parse_arg(&args, "depth", 32usize);
    let iterations = parse_arg(&args, "iterations", 10usize);
    let budget_ms = parse_arg(&args, "budget-ms", 2_000u128);
    let scale = parse_arg(&args, "scale", 0.5f64);
    let dims = [width, height, depth];

    let ct = build_volume(dims, 10, "1.2.3.4");
    let pet = build_volume(dims, 20, "1.2.3.4");
    let input = PetCtFusionInput {
        ct_volume: &ct,
        pet_volume: &pet,
        ct_frame_of_reference_uid: Some("1.2.3.4"),
        pet_frame_of_reference_uid: Some("1.2.3.4"),
        pet_suv_scale_factor: scale,
    };

    let limits = FusionExecutionLimits::default();
    let policy = FusionBlendPolicy::default();

    let started = std::time::Instant::now();
    let mut overlay_bytes = 0usize;
    for _ in 0..iterations {
        let output = execute_pet_ct_fusion(&input, limits, policy).expect("fusion");
        overlay_bytes = output.overlay_rgba.len();
    }
    let elapsed_ms = started.elapsed().as_millis();
    println!(
        "fusion_benchmark dims={}x{}x{} iterations={} elapsed_ms={} budget_ms={} overlay_bytes={}",
        width, height, depth, iterations, elapsed_ms, budget_ms, overlay_bytes
    );
    if elapsed_ms > budget_ms {
        eprintln!(
            "fusion benchmark exceeded budget: elapsed_ms={} budget_ms={}",
            elapsed_ms, budget_ms
        );
        std::process::exit(2);
    }
}

#[cfg(not(feature = "viewer-core"))]
fn main() {
    eprintln!("fusion_benchmark requires the viewer-core feature");
    std::process::exit(1);
}
