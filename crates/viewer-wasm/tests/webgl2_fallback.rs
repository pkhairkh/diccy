// S14-T1: WebGL2 fallback renderer integration tests
//
// Tests for:
// - WebGL2 backend selection when WebGPU unavailable
// - MPR rendering with WebGL2 produces same cache keys as WebGPU
// - Backend cascade: WebGPU → WebGL2 → CPU
// - WebGL2 context loss and recovery
// - Deterministic pixel output across backends

use viewer_wasm::{
    BackendCapabilityProbe, BackendErrorCode, BackendRuntimeState, RendererBackend,
    WebGL2Lifecycle,
};
use dicom_core::Limits;
use viewer_wasm::WasmViewer;
use sha2::{Digest, Sha256};

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// --- Backend selection tests ---

#[test]
fn webgl2_backend_selected_when_webgpu_unavailable() {
    // When WebGPU is unavailable but WebGL2 is available and enabled,
    // the runtime should select WebGL2 as the active backend.
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::WebGL2);
    assert_eq!(state.active_backend(), RendererBackend::WebGL2);
}

#[test]
fn webgl2_backend_selected_with_explicit_preference() {
    // When preferred is "webgl2" and WebGL2 is available and enabled,
    // the runtime should select WebGL2 directly.
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: true,
        adapter_available: true,
        webgl2_api: true,
        max_texture_dimension_2d: 8192,
    };
    let selected = state.configure_backend("webgl2", probe, &limits);
    assert_eq!(selected, RendererBackend::WebGL2);
    assert_eq!(state.active_backend(), RendererBackend::WebGL2);
}

#[test]
fn webgl2_not_selected_when_flag_disabled() {
    // When the WebGL2 feature flag is disabled, the runtime should fall
    // back to CPU even if WebGL2 API is available.
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(false); // Explicitly disable
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::Cpu);
}

#[test]
fn webgl2_not_selected_when_api_unavailable() {
    // When WebGL2 API is not available, the runtime should not select WebGL2.
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: false,
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::Cpu);
}

// --- Backend cascade tests ---

#[test]
fn cascade_webgpu_to_webgl2_to_cpu() {
    // Full cascade: WebGPU unavailable → WebGL2 unavailable → CPU
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: false,
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::Cpu);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"fallback_to_cpu_count\":"));
}

#[test]
fn cascade_webgpu_to_webgl2_when_webgpu_fails() {
    // WebGPU API present but adapter missing → fall to WebGL2
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    state.set_production_webgpu_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: true,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 8192,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    // WebGPU init should fail (no adapter), then fall back to WebGL2
    assert_eq!(selected, RendererBackend::WebGL2);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgpu_init_failures\":1"));
    assert!(metrics.contains("\"webgl2_init_attempts\":1"));
}

#[test]
fn cascade_webgpu_to_webgl2_to_cpu_both_fail() {
    // Both WebGPU and WebGL2 init fail → CPU
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    state.set_production_webgpu_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: true,
        adapter_available: false,
        webgl2_api: false, // WebGL2 API not available
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::Cpu);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgpu_init_failures\":1"));
    assert!(metrics.contains("\"webgl2_init_failures\":1"));
}

#[test]
fn webgpu_preferred_over_webgl2_when_available() {
    // When both WebGPU and WebGL2 are available and enabled,
    // WebGPU should be preferred.
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
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
}

// --- MPR rendering cache key determinism ---

fn sample_p10() -> Vec<u8> {
    use dicom_core::Tag;

    fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(b"UI");
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&bytes);
        buf
    }

    fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&vr);
        let mut bytes = value.to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            _ => {
                buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            }
        }
        buf.extend_from_slice(&bytes);
        buf
    }

    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        b"1.2.840.10008.5.1.4.1.1.7",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        b"1.2.3.4.5",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000D),
        *b"UI",
        b"1.2.3",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000E),
        *b"UI",
        b"1.2.3.4",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0002),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0004),
        *b"CS",
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0010),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0011),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0100),
        *b"US",
        &16u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &12u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &11u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &0u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[0u8],
    ));

    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), "1.2.840.10008.1.2.1"));
    bytes.extend_from_slice(&dataset);
    bytes
}

#[test]
fn mpr_cache_key_identical_across_webgpu_and_webgl2_backends() {
    // MPR reslice results (which are CPU-computed) must produce identical
    // cache keys regardless of which GPU backend is active. The cache key
    // is computed from the same volume data and request parameters.
    let valid = sample_p10();

    // WebGPU backend
    let mut viewer_gpu = WasmViewer::new(64, 64);
    assert!(viewer_gpu.load_bytes(&valid));
    viewer_gpu.set_production_webgpu_renderer(true);
    let _ = viewer_gpu.configure_renderer_backend("webgpu", true, true, true, 8192);
    let gpu_result = viewer_gpu.mpr_reslice_json("axial", 0, 16, 16);
    assert!(gpu_result.contains("\"ok\":true"));

    // WebGL2 backend
    let mut viewer_gl2 = WasmViewer::new(64, 64);
    assert!(viewer_gl2.load_bytes(&valid));
    viewer_gl2.set_webgl2_renderer_enabled(true);
    let _ = viewer_gl2.configure_renderer_backend("webgl2", false, false, true, 4096);
    let gl2_result = viewer_gl2.mpr_reslice_json("axial", 0, 16, 16);
    assert!(gl2_result.contains("\"ok\":true"));

    // Extract and compare cache keys
    let gpu_cache_key = extract_cache_key(&gpu_result);
    let gl2_cache_key = extract_cache_key(&gl2_result);
    assert_eq!(
        gpu_cache_key, gl2_cache_key,
        "MPR cache keys must be identical across WebGPU and WebGL2 backends"
    );
}

#[test]
fn viewport_pixels_identical_across_webgpu_and_webgl2_backends() {
    // Viewport rendering uses the same CPU path regardless of backend;
    // the pixel output must be bit-identical.
    let valid = sample_p10();

    let mut viewer_gpu = WasmViewer::new(64, 64);
    assert!(viewer_gpu.load_bytes(&valid));
    viewer_gpu.set_production_webgpu_renderer(true);
    let _ = viewer_gpu.configure_renderer_backend("webgpu", true, true, true, 8192);
    let gpu_pixels = viewer_gpu.render_viewport_rgba_bytes_for_size(64, 64);
    let gpu_hash = sha256_hex(&gpu_pixels);

    let mut viewer_gl2 = WasmViewer::new(64, 64);
    assert!(viewer_gl2.load_bytes(&valid));
    viewer_gl2.set_webgl2_renderer_enabled(true);
    let _ = viewer_gl2.configure_renderer_backend("webgl2", false, false, true, 4096);
    let gl2_pixels = viewer_gl2.render_viewport_rgba_bytes_for_size(64, 64);
    let gl2_hash = sha256_hex(&gl2_pixels);

    assert_eq!(
        gpu_hash, gl2_hash,
        "Viewport pixel output must be identical across backends"
    );
}

fn extract_cache_key(json: &str) -> String {
    // Extract cache_key value from JSON string like: ..."cache_key":"abc123",...
    if let Some(start) = json.find("\"cache_key\":\"") {
        let rest = &json[start + "\"cache_key\":\"".len()..];
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    String::new()
}

// --- WebGL2 context loss and recovery ---

#[test]
fn webgl2_context_loss_falls_back_to_cpu() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgpu", probe, &limits);
    assert_eq!(selected, RendererBackend::WebGL2);

    // Simulate context loss
    let lost = state.simulate_webgl2_context_lost();
    assert!(lost);
    assert_eq!(state.active_backend(), RendererBackend::Cpu);
}

#[test]
fn webgl2_context_recovery_succeeds() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    assert_eq!(state.active_backend(), RendererBackend::WebGL2);

    // Context loss
    assert!(state.simulate_webgl2_context_lost());
    assert_eq!(state.active_backend(), RendererBackend::Cpu);

    // Recovery
    let recovered = state.recover_device_lost(&limits);
    assert_eq!(recovered, RendererBackend::WebGL2);
}

#[test]
fn webgl2_context_loss_via_simulate_device_lost() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    assert_eq!(state.active_backend(), RendererBackend::WebGL2);

    // simulate_device_lost should handle WebGL2 context loss too
    let lost = state.simulate_device_lost();
    assert!(lost);
    assert_eq!(state.active_backend(), RendererBackend::Cpu);
}

#[test]
fn simulate_device_lost_returns_false_for_cpu_backend() {
    let mut state = BackendRuntimeState::default();
    assert!(!state.simulate_device_lost());
    assert!(!state.simulate_webgl2_context_lost());
}

// --- WebGL2 lifecycle state ---

#[test]
fn webgl2_lifecycle_transitions() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();

    // Initially uninitialized
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_lifecycle\":\"Uninitialized\""));

    // After successful init → Ready
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_lifecycle\":\"Ready\""));

    // After context loss → ContextLost
    state.simulate_webgl2_context_lost();
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_lifecycle\":\"ContextLost\""));

    // After recovery → Ready
    state.recover_device_lost(&limits);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_lifecycle\":\"Ready\""));
}

#[test]
fn webgl2_init_failure_lifecycle() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: false, // WebGL2 not available
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_lifecycle\":\"Failed\""));
    assert!(metrics.contains("DVF.WASM.WEBGL2.INIT_FAILED"));
}

// --- WebGL2 metrics tracking ---

#[test]
fn webgl2_metrics_track_init_attempts_and_failures() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: false,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_init_attempts\":1"));
    assert!(metrics.contains("\"webgl2_init_failures\":1"));
}

#[test]
fn webgl2_present_count_increments() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);

    // Record a present on the WebGL2 backend
    state.record_present_with_timing(
        dicom_pixel::PixelFormat::Rgba8,
        1024,
        &limits,
        16,
    );
    let metrics = state.metrics_json();
    assert!(metrics.contains("\"webgl2_present_count\":1"));
}

// --- Volume rendering availability ---

#[test]
fn volume_rendering_available_with_webgl2() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    assert!(state.volume_rendering_available());
}

#[test]
fn volume_rendering_unavailable_when_webgl2_context_lost() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: true,
        max_texture_dimension_2d: 4096,
    };
    state.configure_backend("webgpu", probe, &limits);
    assert!(state.volume_rendering_available());
    state.simulate_webgl2_context_lost();
    assert!(!state.volume_rendering_available());
}

// --- WebGL2 shader sources ---

#[test]
fn webgl2_shader_sources_return_valid_json() {
    let mpr_json = viewer_wasm::webgl2_mpr_shader_sources_json();
    assert!(mpr_json.contains("\"vert\":"));
    assert!(mpr_json.contains("\"frag\":"));
    assert!(mpr_json.contains("#version 300 es"));

    let mip_json = viewer_wasm::webgl2_mip_shader_sources_json();
    assert!(mip_json.contains("\"vert\":"));
    assert!(mip_json.contains("\"frag\":"));
    assert!(mip_json.contains("sampler2DArray"));

    let vr_json = viewer_wasm::webgl2_vr_shader_sources_json();
    assert!(vr_json.contains("\"vert\":"));
    assert!(vr_json.contains("\"frag\":"));
    assert!(vr_json.contains("sampler2DArray"));
    assert!(vr_json.contains("u_transfer_func"));
}

#[test]
fn webgl2_mpr_shader_uses_texture_quad() {
    let mpr_json = viewer_wasm::webgl2_mpr_shader_sources_json();
    assert!(mpr_json.contains("sampler2D"));
    assert!(mpr_json.contains("u_slice_texture"));
    assert!(mpr_json.contains("u_window_center"));
    assert!(mpr_json.contains("u_window_width"));
}

#[test]
fn webgl2_mip_shader_uses_ray_marching() {
    let mip_json = viewer_wasm::webgl2_mip_shader_sources_json();
    assert!(mip_json.contains("ray_dir"));
    assert!(mip_json.contains("u_step_size"));
    assert!(mip_json.contains("u_max_steps"));
    assert!(mip_json.contains("u_mip_mode"));
    assert!(mip_json.contains("sample_volume"));
}

#[test]
fn webgl2_vr_shader_uses_transfer_function_and_gradient() {
    let vr_json = viewer_wasm::webgl2_vr_shader_sources_json();
    assert!(vr_json.contains("u_transfer_func"));
    assert!(vr_json.contains("compute_gradient"));
    assert!(vr_json.contains("accumulated"));
    assert!(vr_json.contains("u_light_dir"));
    assert!(vr_json.contains("specular"));
}

// --- WebGL2 feature flag gating ---

#[test]
fn webgl2_feature_flag_can_be_toggled() {
    let mut state = BackendRuntimeState::default();
    // With webgl2-backend feature, default is true; without, false.
    // Test that toggling works regardless of initial state.
    state.set_webgl2_backend_enabled(false);
    assert!(!state.webgl2_backend_enabled());

    state.set_webgl2_backend_enabled(true);
    assert!(state.webgl2_backend_enabled());

    state.set_webgl2_backend_enabled(false);
    assert!(!state.webgl2_backend_enabled());
}

// --- WasmViewer integration ---

#[test]
fn wasm_viewer_webgl2_backend_selection() {
    let mut viewer = WasmViewer::new(64, 64);
    viewer.set_webgl2_renderer_enabled(true);
    let selected = viewer.configure_renderer_backend("webgl2", false, false, true, 4096);
    assert_eq!(selected, "WebGL2");
    assert_eq!(viewer.active_renderer_backend(), "WebGL2");
}

#[test]
fn wasm_viewer_webgl2_context_loss_and_recovery() {
    let mut viewer = WasmViewer::new(64, 64);
    viewer.set_webgl2_renderer_enabled(true);
    let _ = viewer.configure_renderer_backend("webgl2", false, false, true, 4096);
    assert_eq!(viewer.active_renderer_backend(), "WebGL2");

    // Simulate context loss
    assert!(viewer.simulate_webgl2_context_lost());
    assert_eq!(viewer.active_renderer_backend(), "CPU");

    // Recovery
    let recovered = viewer.recover_gpu_device();
    assert_eq!(recovered, "WebGL2");
    assert_eq!(viewer.active_renderer_backend(), "WebGL2");
}

#[test]
fn wasm_viewer_webgl2_shader_sources() {
    let viewer = WasmViewer::new(64, 64);
    let sources = viewer.webgl2_shader_sources_json();
    assert!(sources.contains("\"mpr\":"));
    assert!(sources.contains("\"mip\":"));
    assert!(sources.contains("\"vr\":"));
    assert!(sources.contains("#version 300 es"));
}

// --- RendererBackend enum tests ---

#[test]
fn renderer_backend_as_str() {
    assert_eq!(RendererBackend::Cpu.as_str(), "CPU");
    assert_eq!(RendererBackend::WebGpu.as_str(), "WebGPU");
    assert_eq!(RendererBackend::WebGL2.as_str(), "WebGL2");
}

// --- BackendErrorCode tests ---

#[test]
fn webgl2_error_codes() {
    assert_eq!(
        BackendErrorCode::WebGL2InitFailed.as_code(),
        "DVF.WASM.WEBGL2.INIT_FAILED"
    );
    assert_eq!(
        BackendErrorCode::WebGL2ContextLost.as_code(),
        "DVF.WASM.WEBGL2.CONTEXT_LOST"
    );
}

// --- WebGL2Lifecycle tests ---

#[test]
fn webgl2_lifecycle_variants() {
    assert_eq!(WebGL2Lifecycle::Uninitialized, WebGL2Lifecycle::Uninitialized);
    assert_eq!(WebGL2Lifecycle::Ready, WebGL2Lifecycle::Ready);
    assert_eq!(WebGL2Lifecycle::ContextLost, WebGL2Lifecycle::ContextLost);
    assert_eq!(WebGL2Lifecycle::Failed, WebGL2Lifecycle::Failed);
    assert_ne!(WebGL2Lifecycle::Ready, WebGL2Lifecycle::Failed);
}

// --- Fail-closed: WebGL2 init failure falls back to CPU, never silently corrupts ---

#[test]
fn webgl2_init_failure_falls_back_to_cpu_never_corrupts() {
    let mut state = BackendRuntimeState::default();
    state.set_webgl2_backend_enabled(true);
    let limits = Limits::default();
    let probe = BackendCapabilityProbe {
        webgpu_api: false,
        adapter_available: false,
        webgl2_api: false, // WebGL2 unavailable
        max_texture_dimension_2d: 4096,
    };
    let selected = state.configure_backend("webgl2", probe, &limits);
    // Must fall back to CPU, never stay in a broken WebGL2 state
    assert_eq!(selected, RendererBackend::Cpu);
    assert_eq!(state.active_backend(), RendererBackend::Cpu);
    let metrics = state.metrics_json();
    assert!(metrics.contains("DVF.WASM.WEBGL2.INIT_FAILED"));
}
