use dicom_core::Limits;
use dicom_pixel::{DisplayFrame, PixelFormat};
use viewer_core::Viewport2D;
use viewer_wgpu::{GpuBudget, GpuCapabilities, Renderer, WgpuRenderer};

fn renderer(max_texture_bytes: u64, max_dimension: u32) -> WgpuRenderer {
    let limits = Limits::builder().max_gpu_texture_bytes(max_texture_bytes).build().unwrap();
    WgpuRenderer::from_capabilities(
        GpuCapabilities {
            max_texture_dimension_2d: max_dimension,
        },
        &limits,
    )
}

#[test]
fn lifecycle_fail_closed_before_surface_configuration() {
    // REQ-HI-364
    let mut gpu = renderer(1024 * 1024, 4096);

    let resize_err = gpu
        .resize_surface(1, 1)
        .expect_err("resize before configure must fail");
    assert_eq!(resize_err.code(), "DVF.PIXEL.INVALID_TRANSFORM");

    let viewport = Viewport2D::new(1, 1);
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Luma8,
        bytes: vec![0],
    };
    let render_err = gpu
        .render(&frame, &viewport)
        .expect_err("render before configure must fail");
    assert_eq!(render_err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
}

#[test]
fn surface_configuration_rejects_zero_and_excessive_extents() {
    // REQ-HI-365
    let mut gpu = renderer(1024 * 1024, 128);

    let zero = gpu
        .configure_surface(0, 64, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect_err("zero extent must fail");
    assert_eq!(zero.code(), "DVF.PIXEL.INVALID_TRANSFORM");

    let too_large = gpu
        .configure_surface(256, 64, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect_err("oversized extent must fail");
    assert_eq!(too_large.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
}

#[test]
fn render_boundary_accepts_only_cpu_oracle_quantized_formats_and_lengths() {
    // REQ-HI-366, REQ-HI-367
    let mut gpu = renderer(1024 * 1024, 4096);
    gpu.configure_surface(2, 2, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect("configure");

    let viewport = Viewport2D::new(2, 2);

    let luma16 = DisplayFrame {
        width: 2,
        height: 2,
        format: PixelFormat::Luma16,
        bytes: vec![0; 8],
    };
    let format_err = gpu
        .render(&luma16, &viewport)
        .expect_err("non-oracle format must fail");
    assert_eq!(format_err.code(), "DVF.PIXEL.INVALID_TRANSFORM");

    let invalid_len = DisplayFrame {
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8,
        bytes: vec![1; 15],
    };
    let len_err = gpu
        .render(&invalid_len, &viewport)
        .expect_err("invalid byte length must fail");
    assert_eq!(len_err.code(), "DVF.PIXEL.INVALID_TRANSFORM");

    let valid = DisplayFrame {
        width: 2,
        height: 2,
        format: PixelFormat::Luma8,
        bytes: vec![1, 2, 3, 4],
    };
    gpu.render(&valid, &viewport)
        .expect("valid quantized frame should render");
}

#[test]
fn lifecycle_counters_progress_deterministically() {
    // REQ-HI-368
    let mut gpu = renderer(1024 * 1024, 4096);
    gpu.configure_surface(64, 64, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect("configure");
    let first = gpu.lifecycle_state();
    let first_surface = first.surface.expect("surface");
    assert_eq!(first_surface.generation, 1);

    gpu.resize_surface(96, 64).expect("resize");
    let second = gpu.lifecycle_state();
    let second_surface = second.surface.expect("surface");
    assert_eq!(second_surface.generation, 2);

    let viewport = Viewport2D::new(1, 1);
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8,
        bytes: vec![1, 2, 3, 255],
    };
    gpu.render(&frame, &viewport).expect("render");
    let third = gpu.lifecycle_state();
    assert_eq!(third.rendered_frames, second.rendered_frames + 1);
    assert_eq!(third.submitted_passes, second.submitted_passes + 1);
}

#[test]
fn texture_budget_enforces_limit_and_saturating_release_semantics() {
    // REQ-HI-369
    let mut budget = GpuBudget::new(16);
    budget.reserve(12).expect("first reserve");
    let err = budget.reserve(8).expect_err("reserve should exceed budget");
    assert_eq!(err.code(), "DVF.SECURITY.LIMIT_EXCEEDED");

    budget.release(4);
    assert_eq!(budget.used_texture_bytes, 8);
    budget.release(1000);
    assert_eq!(budget.used_texture_bytes, 0);
}
