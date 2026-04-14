use dicom_core::Tag;
use sha2::{Digest, Sha256};
use viewer_wasm::WasmViewer;

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

fn sample_p10() -> Vec<u8> {
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

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[test]
fn network_boundary_defaults_to_disabled_and_is_feature_gated() {
    // REQ-HI-360
    let mut viewer = WasmViewer::new(512, 512);
    assert!(!viewer.network_enabled());

    #[cfg(feature = "network")]
    {
        assert!(viewer.set_network_enabled(true));
        assert!(viewer.network_enabled());
    }

    #[cfg(not(feature = "network"))]
    {
        assert!(!viewer.set_network_enabled(true));
        assert!(!viewer.network_enabled());
    }
}

#[test]
fn numeric_boundary_rejects_non_finite_or_non_positive_zoom_without_mutation() {
    // REQ-HI-361
    let mut viewer = WasmViewer::new(320, 240);
    let original_zoom = viewer.viewport_zoom();

    assert!(!viewer.set_zoom(f64::NAN));
    assert!(!viewer.set_zoom(f64::INFINITY));
    assert!(!viewer.set_zoom(0.0));
    assert!(!viewer.set_zoom(-1.0));
    assert!(!viewer.zoom_by(f64::NAN));
    assert_eq!(viewer.viewport_zoom(), original_zoom);
}

#[test]
fn ingest_boundary_validates_p10_before_state_mutation() {
    // REQ-HI-362
    let mut viewer = WasmViewer::new(256, 256);
    assert_eq!(viewer.loaded_input_bytes(), 0);
    assert!(!viewer.has_preview());

    assert!(!viewer.ingest_bytes(b"not-dicom"));
    assert_eq!(viewer.loaded_input_bytes(), 0);
    assert!(!viewer.has_preview());

    let valid = sample_p10();
    assert!(viewer.load_bytes(&valid));
    assert_eq!(viewer.loaded_input_bytes(), valid.len());
    assert!(viewer.has_preview());
    assert_eq!(viewer.preview_width(), 1);
    assert_eq!(viewer.preview_height(), 1);
    assert_eq!(viewer.preview_rgba_bytes().len(), 4);
    let mpr = viewer.mpr_reslice_json("axial", 0, 1, 1);
    assert!(mpr.contains("\"ok\":true"));
    assert!(mpr.contains("\"pixel_count\":1"));
}

#[test]
fn metadata_renderer_escapes_html_sensitive_tokens() {
    // REQ-HI-363
    let viewer = WasmViewer::new(64, 64);
    let escaped = viewer.render_metadata_html("<&>\"'");
    assert_eq!(escaped, "&lt;&amp;&gt;&quot;&#x27;");
}

#[test]
fn backend_selection_and_fallback_are_deterministic() {
    // REQ-HI-364
    let mut viewer = WasmViewer::new(320, 240);
    assert_eq!(viewer.active_renderer_backend(), "CPU");
    viewer.set_production_webgpu_renderer(true);

    let selected = viewer.configure_renderer_backend("webgpu", true, false, true, 4096);
    assert_eq!(selected, "CPU");
    assert_eq!(viewer.active_renderer_backend(), "CPU");

    let probe = viewer.backend_capability_probe_json();
    assert!(probe.contains("\"webgpu_api\":true"));
    assert!(probe.contains("\"adapter_available\":false"));

    let metrics = viewer.backend_selection_metrics_json();
    assert!(metrics.contains("\"fallback_to_cpu_count\":1"));
    assert!(metrics.contains("DVF.WASM.GPU.INIT_FAILED"));
}

#[test]
fn backend_runtime_controls_expose_force_cpu_and_frame_interval() {
    // REQ-HI-365
    let mut viewer = WasmViewer::new(128, 128);
    assert!(viewer.set_render_frame_interval_ms(33));
    assert_eq!(viewer.render_frame_interval_ms(), 33);
    assert!(!viewer.set_render_frame_interval_ms(0));

    viewer.set_force_cpu_renderer(true);
    let selected = viewer.configure_renderer_backend("webgpu", true, true, true, 8192);
    assert_eq!(selected, "CPU");
    assert!(!viewer.simulate_gpu_device_lost());
    assert_eq!(viewer.recover_gpu_device(), "CPU");
    assert!(viewer.csp_safe_shader_source().contains("@vertex"));
}

#[test]
fn gpu_selection_does_not_change_cpu_oracle_pixels() {
    // REQ-HI-366
    let mut viewer = WasmViewer::new(64, 64);
    let valid = sample_p10();
    assert!(viewer.load_bytes(&valid));

    viewer.set_production_webgpu_renderer(false);
    let cpu = viewer.render_viewport_rgba_bytes_for_size(64, 64);
    assert!(!cpu.is_empty());
    let cpu_hash = sha256_hex(&cpu);

    viewer.set_production_webgpu_renderer(true);
    let _ = viewer.configure_renderer_backend("webgpu", true, true, true, 8192);
    let gpu_selected = viewer.render_viewport_rgba_bytes_for_size(64, 64);
    assert_eq!(gpu_selected.len(), cpu.len());
    let gpu_hash = sha256_hex(&gpu_selected);
    assert_eq!(cpu_hash, gpu_hash);

    let metrics = viewer.backend_selection_metrics_json();
    assert!(metrics.contains("\"rendered_frames\":"));
}

#[test]
fn tri_planar_controls_keep_crosshair_and_planes_synchronized() {
    let mut viewer = WasmViewer::new(64, 64);
    let valid = sample_p10();
    assert!(viewer.load_bytes(&valid));

    assert!(viewer.set_mpr_crosshair(0, 0, 0));
    assert!(viewer.set_mpr_plane_index("axial", 0));
    let state = viewer.tri_planar_state_json();
    assert!(state.contains("\"axial_index\":0"));
    assert!(state.contains("\"coronal_index\":0"));
    assert!(state.contains("\"sagittal_index\":0"));

    let axial = viewer.mpr_plane_rgba_bytes("axial", 8, 8);
    let coronal = viewer.mpr_plane_rgba_bytes("coronal", 8, 8);
    let sagittal = viewer.mpr_plane_rgba_bytes("sagittal", 8, 8);
    assert_eq!(axial.len(), 8 * 8 * 4);
    assert_eq!(coronal.len(), 8 * 8 * 4);
    assert_eq!(sagittal.len(), 8 * 8 * 4);
}

#[test]
fn patient_space_requests_and_slab_controls_are_available() {
    let mut viewer = WasmViewer::new(64, 64);
    let valid = sample_p10();
    assert!(viewer.load_bytes(&valid));

    assert!(viewer.set_mpr_slab(3, "max"));
    assert!(!viewer.set_mpr_slab(0, "max"));
    assert!(!viewer.set_mpr_slab(1, "invalid-mode"));

    let voxel = viewer.mpr_reslice_json("axial", 0, 8, 8);
    let patient = viewer.mpr_reslice_patient_json("axial", 0, 8, 8);
    assert!(voxel.contains("\"ok\":true"));
    assert!(patient.contains("\"ok\":true"));
}

#[test]
fn tri_planar_slab_golden_hash_is_stable() {
    // REQ-HI-366, REQ-TEST-740
    let mut viewer = WasmViewer::new(64, 64);
    let valid = sample_p10();
    assert!(viewer.load_bytes(&valid));
    assert!(viewer.set_mpr_crosshair(0, 0, 0));
    assert!(viewer.set_mpr_slab(3, "max"));

    let axial_a = viewer.mpr_plane_rgba_bytes("axial", 16, 16);
    let coronal_a = viewer.mpr_plane_rgba_bytes("coronal", 16, 16);
    let sagittal_a = viewer.mpr_plane_rgba_bytes("sagittal", 16, 16);

    let axial_b = viewer.mpr_plane_rgba_bytes("axial", 16, 16);
    let coronal_b = viewer.mpr_plane_rgba_bytes("coronal", 16, 16);
    let sagittal_b = viewer.mpr_plane_rgba_bytes("sagittal", 16, 16);

    assert_eq!(axial_a, axial_b);
    assert_eq!(coronal_a, coronal_b);
    assert_eq!(sagittal_a, sagittal_b);

    let mut combined = Vec::new();
    combined.extend_from_slice(&axial_a);
    combined.extend_from_slice(&coronal_a);
    combined.extend_from_slice(&sagittal_a);
    let hash = sha256_hex(&combined);
    assert_eq!(
        hash,
        "1a7493ff2f2bde5f78405d46eb01abfeaea9b0a95fa5b6cec211b05faa0cf1d3"
    );
}
