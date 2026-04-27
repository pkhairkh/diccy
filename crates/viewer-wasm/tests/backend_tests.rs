// Auto-extracted from /home/z/diccy/crates/viewer-wasm/src/backend.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_wasm::{
        auto_tune_frame_interval_ms, fallback_latency_budget_ms, select_upload_chunk_bytes,
        BackendCapabilityProbe, BackendRuntimeState, RendererBackend,
    };
    use dicom_core::Limits;

    #[test]
    fn upload_chunk_policy_is_deterministic() {
        assert_eq!(select_upload_chunk_bytes(128 * 1024, 16), 256 * 1024);
        assert_eq!(select_upload_chunk_bytes(2 * 1024 * 1024, 16), 1024 * 1024);
        assert_eq!(
            select_upload_chunk_bytes(8 * 1024 * 1024, 16),
            2 * 1024 * 1024
        );
        assert_eq!(
            select_upload_chunk_bytes(32 * 1024 * 1024, 50),
            4 * 1024 * 1024
        );
    }

    #[test]
    fn auto_tune_interval_policy_has_stable_boundaries() {
        assert_eq!(auto_tune_frame_interval_ms(0), 16);
        assert_eq!(auto_tune_frame_interval_ms(12), 16);
        assert_eq!(auto_tune_frame_interval_ms(13), 24);
        assert_eq!(auto_tune_frame_interval_ms(20), 24);
        assert_eq!(auto_tune_frame_interval_ms(21), 33);
        assert_eq!(auto_tune_frame_interval_ms(33), 33);
        assert_eq!(auto_tune_frame_interval_ms(34), 50);
        assert_eq!(auto_tune_frame_interval_ms(50), 50);
        assert_eq!(auto_tune_frame_interval_ms(51), 66);
    }

    #[test]
    fn fallback_budget_is_bounded_and_repeatable() {
        assert_eq!(fallback_latency_budget_ms(16), 32);
        assert_eq!(fallback_latency_budget_ms(33), 66);
        assert_eq!(fallback_latency_budget_ms(200), 250);
    }

    #[test]
    fn production_webgpu_flag_blocks_backend_activation_when_disabled() {
        let mut state = BackendRuntimeState::default();
        let limits = Limits::default();
        let probe = BackendCapabilityProbe {
            webgpu_api: true,
            adapter_available: true,
            webgl2_api: true,
            max_texture_dimension_2d: 8192,
        };
        let selected = state.configure_backend("webgpu", probe, &limits);
        assert_eq!(selected, RendererBackend::Cpu);
        assert_eq!(state.active_backend(), RendererBackend::Cpu);
        let metrics = state.metrics_json();
        assert!(metrics.contains("production_webgpu_flag_disabled"));
        assert!(metrics.contains("DVF.WASM.GPU.FLAG_DISABLED"));
    }

    #[cfg(feature = "webgpu-backend")]
    #[test]
    fn device_lost_chaos_cycles_have_bounded_transition_growth() {
        let mut state = BackendRuntimeState::default();
        state.set_production_webgpu_enabled(true);
        let limits = Limits::default();
        let probe = BackendCapabilityProbe {
            webgpu_api: true,
            adapter_available: true,
            webgl2_api: true,
            max_texture_dimension_2d: 8192,
        };
        let selected = state.configure_backend("webgpu", probe, &limits);
        assert_eq!(selected, RendererBackend::WebGpu);

        for _ in 0..12 {
            assert!(state.simulate_device_lost());
            assert_eq!(state.active_backend(), RendererBackend::Cpu);
            let recovered = state.recover_device_lost(&limits);
            assert_eq!(recovered, RendererBackend::WebGpu);
        }

        let metrics = state.metrics_json();
        assert!(metrics.contains("\"backend_transition_count\":"));
        assert!(metrics.contains("\"fallback_budget_violations\":0"));
    }
