// Auto-extracted from /home/z/diccy/crates/viewer-wgpu/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_wgpu::{GpuBudget, GpuCapabilities, Renderer, WgpuRenderer};
    use dicom_core::Limits;
    use dicom_pixel::{DisplayFrame, PixelFormat};
    use viewer_core::Viewport2D;

    fn test_renderer() -> WgpuRenderer {
        let limits = Limits::builder()
            .max_gpu_texture_bytes(1024 * 1024)
            .build()
            .unwrap();
        WgpuRenderer::from_capabilities(
            GpuCapabilities {
                max_texture_dimension_2d: 4096,
            },
            &limits,
        )
    }

    #[test]
    fn gpu_capabilities_from_limits_reads_dimension() {
        // REQ-ARCH-140
        let limits = wgpu::Limits {
            max_texture_dimension_2d: 4096,
            ..Default::default()
        };
        let caps = GpuCapabilities::from_limits(&limits);
        assert_eq!(caps.max_texture_dimension_2d, 4096);
    }

    #[test]
    fn gpu_budget_enforces_max_bytes() {
        // REQ-UI-040 (budget enforcement) / REQ-ARCH-140 (budget applied after probing).
        let mut budget = GpuBudget::new(64);
        budget.reserve(32).expect("first reserve ok");
        let err = budget.reserve(40).expect_err("should exceed budget");
        let message = format!("{err}");
        assert!(message.contains("GPU texture budget exceeded"));
    }

    #[test]
    fn render_accepts_quantized_cpu_boundary_frame() {
        // REQ-GPU-210: renderer accepts CPU quantized formats.
        let mut renderer = test_renderer();
        renderer
            .configure_surface(2, 2, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("configure");
        let viewport = Viewport2D::new(2, 2);
        let frame = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0, 1, 2, 3],
        };
        renderer.render(&frame, &viewport).expect("render ok");
    }

    #[test]
    fn render_rejects_non_quantized_luma16() {
        // REQ-GPU-210 / REQ-PIX-201: GPU path must reject non-oracle semantics.
        let mut renderer = test_renderer();
        renderer
            .configure_surface(2, 2, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("configure");
        let viewport = Viewport2D::new(2, 2);
        let frame = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma16,
            bytes: vec![0; 8],
        };
        let err = renderer
            .render(&frame, &viewport)
            .expect_err("Luma16 must be rejected");
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn render_rejects_invalid_byte_length() {
        // REQ-GPU-210: renderer validates byte-length agreement with quantized format.
        let mut renderer = test_renderer();
        renderer
            .configure_surface(2, 2, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("configure");
        let viewport = Viewport2D::new(2, 2);
        let frame = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8,
            bytes: vec![0; 15],
        };
        let err = renderer
            .render(&frame, &viewport)
            .expect_err("invalid frame size");
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn render_does_not_mutate_input_frame_bytes() {
        // REQ-PIX-201: GPU render uses CPU output as immutable source bytes.
        let mut renderer = test_renderer();
        renderer
            .configure_surface(1, 1, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("configure");
        let viewport = Viewport2D::new(1, 1);
        let frame = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Rgba8,
            bytes: vec![12, 34, 56, 255],
        };
        let before = frame.bytes.clone();
        renderer.render(&frame, &viewport).expect("render ok");
        assert_eq!(frame.bytes, before);
    }

    #[test]
    fn render_requires_surface_configuration() {
        // REQ-UI-040/042: lifecycle must fail closed before configure.
        let mut renderer = test_renderer();
        let viewport = Viewport2D::new(1, 1);
        let frame = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![42],
        };
        let err = renderer
            .render(&frame, &viewport)
            .expect_err("surface required");
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn configure_surface_rejects_zero_extent() {
        // REQ-UI-042: invalid lifecycle transitions fail closed.
        let mut renderer = test_renderer();
        let err = renderer
            .configure_surface(0, 128, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect_err("zero width rejected");
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn resize_surface_increments_generation_deterministically() {
        // REQ-UI-042: resize is deterministic and tracked.
        let mut renderer = test_renderer();
        renderer
            .configure_surface(128, 128, wgpu::TextureFormat::Rgba8UnormSrgb)
            .expect("configure");
        let first = renderer.lifecycle_state().surface.expect("surface");
        renderer.resize_surface(256, 128).expect("resize");
        let second = renderer.lifecycle_state().surface.expect("surface");
        assert!(second.generation > first.generation);
    }

    #[test]
    fn render_increments_lifecycle_counters() {
        // REQ-UI-042: render path submits deterministic pass counts.
        let mut renderer = test_renderer();
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
        renderer.render(&frame, &viewport).expect("render");
        let lifecycle = renderer.lifecycle_state();
        assert_eq!(lifecycle.rendered_frames, 1);
        assert_eq!(lifecycle.submitted_passes, 1);
        assert_eq!(lifecycle.acquired_frames, 0);
        assert_eq!(lifecycle.presented_frames, 0);
        assert_eq!(lifecycle.encoded_passes, 0);
    }
