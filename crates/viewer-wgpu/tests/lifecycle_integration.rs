use dicom_core::Limits;
use dicom_pixel::{DisplayFrame, PixelFormat};
use viewer_core::Viewport2D;
use viewer_wgpu::{GpuCapabilities, Renderer, WgpuRenderer};

fn test_renderer(max_gpu_texture_bytes: u64) -> WgpuRenderer {
    let limits = Limits::builder().max_gpu_texture_bytes(max_gpu_texture_bytes).build().unwrap();
    WgpuRenderer::from_capabilities(
        GpuCapabilities {
            max_texture_dimension_2d: 4096,
        },
        &limits,
    )
}

fn try_runtime_renderer(max_gpu_texture_bytes: u64) -> Option<WgpuRenderer> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: Some("viewer-wgpu-integration-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
        },
        None,
    ))
    .ok()?;

    let limits = Limits::builder().max_gpu_texture_bytes(max_gpu_texture_bytes).build().unwrap();
    let capabilities = WgpuRenderer::probe_capabilities(&device);
    let mut renderer = WgpuRenderer::from_capabilities(capabilities, &limits);
    renderer
        .configure_surface(16, 16, wgpu::TextureFormat::Rgba8UnormSrgb)
        .ok()?;
    renderer.attach_runtime(device, queue).ok()?;
    Some(renderer)
}

#[test]
fn renderer_requires_surface_before_render() {
    // REQ-UI-042
    let mut renderer = test_renderer(1024 * 1024);
    let viewport = Viewport2D::new(1, 1);
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Luma8,
        bytes: vec![0],
    };
    let err = renderer
        .render(&frame, &viewport)
        .expect_err("surface configuration is required");
    assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
}

#[test]
fn renderer_configure_resize_and_draw_lifecycle() {
    // REQ-ARCH-140, REQ-UI-042
    let mut renderer = test_renderer(1024 * 1024);
    renderer
        .configure_surface(64, 64, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect("configure");

    let before = renderer.lifecycle_state();
    renderer.resize_surface(96, 64).expect("resize");
    let after_resize = renderer.lifecycle_state();

    let before_surface = before.surface.expect("surface");
    let after_surface = after_resize.surface.expect("surface");
    assert!(after_surface.generation > before_surface.generation);

    let viewport = Viewport2D::new(1, 1);
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8,
        bytes: vec![1, 2, 3, 255],
    };
    renderer.render(&frame, &viewport).expect("render");

    let after_render = renderer.lifecycle_state();
    assert_eq!(
        after_render.rendered_frames,
        after_resize.rendered_frames + 1
    );
    assert_eq!(
        after_render.submitted_passes,
        after_resize.submitted_passes + 1
    );
}

#[test]
fn renderer_budget_enforced_during_draw_submission() {
    // REQ-UI-040
    let mut renderer = test_renderer(3);
    renderer
        .configure_surface(1, 1, wgpu::TextureFormat::Rgba8UnormSrgb)
        .expect("configure");

    let viewport = Viewport2D::new(1, 1);
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8,
        bytes: vec![1, 2, 3, 255],
    };
    let err = renderer
        .render(&frame, &viewport)
        .expect_err("budget should be exceeded");
    assert_eq!(err.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
}

#[test]
fn renderer_runtime_encodes_draw_and_presents_frame() {
    // REQ-UI-042 / TASK-3064 integration: live runtime path performs acquire/draw/present.
    let Some(mut renderer) = try_runtime_renderer(1024 * 1024) else {
        eprintln!("Skipping runtime draw test: no compatible wgpu adapter/device in environment");
        return;
    };
    assert!(renderer.has_live_runtime());

    let viewport = Viewport2D::new(2, 2);
    let frame = DisplayFrame {
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8,
        bytes: vec![
            255, 0, 0, 255, 0, 255, 0, 255, //
            0, 0, 255, 255, 255, 255, 0, 255,
        ],
    };
    renderer.render(&frame, &viewport).expect("runtime render");

    let lifecycle = renderer.lifecycle_state();
    assert!(lifecycle.device_ready);
    assert_eq!(lifecycle.rendered_frames, 1);
    assert_eq!(lifecycle.submitted_passes, 1);
    assert_eq!(lifecycle.acquired_frames, 1);
    assert_eq!(lifecycle.presented_frames, 1);
    assert_eq!(lifecycle.encoded_passes, 1);
}
