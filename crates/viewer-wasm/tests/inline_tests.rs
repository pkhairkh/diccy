// Auto-extracted from /home/z/diccy/crates/viewer-wasm/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use viewer_wasm::*;
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

#[test]
fn escape_metadata_html_escapes_entities() {
    // REQ-SEC-433: HTML-escape metadata in web contexts.
    let raw = r#"<script>alert("x")</script>&"#;
    let escaped = escape_metadata_html(raw);
    assert_eq!(
        escaped,
        "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;"
    );
}

#[test]
fn network_disabled_by_default() {
    // REQ-SEC-433: no network by default in WASM builds.
    let mut viewer = WasmViewer::new(1, 1);
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
    assert!(viewer.set_network_enabled(false));
    assert!(!viewer.network_enabled());
}

#[test]
fn zoom_rejects_non_finite_values() {
    // REQ-WASM-301: reject non-finite numeric inputs at the WASM boundary.
    let mut viewer = WasmViewer::new(10, 10);
    let original = viewer.viewport_zoom();
    assert!(!viewer.set_zoom(f64::NAN));
    assert!(!viewer.set_zoom(f64::INFINITY));
    assert_eq!(viewer.viewport_zoom(), original);
}

#[test]
fn ingest_bytes_accepts_valid_p10_and_tracks_size() {
    // REQ-WASM-301: byte-ingestion validates DICOM payloads before accepting.
    let mut viewer = WasmViewer::new(1, 1);
    let bytes = sample_p10();
    assert!(
        viewer.ingest_bytes(&bytes),
        "ingest_bytes should accept valid P10 payload"
    );
    assert_eq!(viewer.loaded_input_bytes(), bytes.len());
    assert!(viewer.has_preview());
    assert_eq!(viewer.preview_width(), 1);
    assert_eq!(viewer.preview_height(), 1);
    assert_eq!(viewer.preview_rgba_bytes().len(), 4);
}

#[test]
fn load_bytes_rejects_non_dicom_input() {
    // REQ-WASM-301: malformed bytes must fail closed.
    let mut viewer = WasmViewer::new(1, 1);
    assert!(!viewer.load_bytes(b"not a dicom payload"));
    assert_eq!(viewer.loaded_input_bytes(), 0);
    assert!(!viewer.has_preview());
    assert!(viewer.preview_rgba_bytes().is_empty());
}

#[test]
fn rejected_payload_does_not_mutate_existing_preview() {
    // REQ-WASM-301: failed ingest must not clobber previously accepted preview state.
    let mut viewer = WasmViewer::new(1, 1);
    let valid = sample_p10();
    assert!(
        viewer.load_bytes(&valid),
        "load_bytes should accept valid P10 payload"
    );

    let before_len = viewer.loaded_input_bytes();
    let before_width = viewer.preview_width();
    let before_height = viewer.preview_height();
    let before_rgba = viewer.preview_rgba_bytes();

    assert!(!viewer.load_bytes(b"invalid dicom"));
    assert_eq!(viewer.loaded_input_bytes(), before_len);
    assert_eq!(viewer.preview_width(), before_width);
    assert_eq!(viewer.preview_height(), before_height);
    assert_eq!(viewer.preview_rgba_bytes(), before_rgba);
}

#[test]
fn interpolation_mode_accepts_known_values_only() {
    // REQ-HI-145/REQ-HI-361: interpolation mode is explicit and deterministic.
    let mut viewer = WasmViewer::new(1, 1);
    assert_eq!(viewer.interpolation_mode(), "linear");
    assert!(viewer.set_interpolation_mode("nearest"));
    assert_eq!(viewer.interpolation_mode(), "nearest");
    assert!(viewer.set_interpolation_mode("linear"));
    assert_eq!(viewer.interpolation_mode(), "linear");
    assert!(!viewer.set_interpolation_mode("cubic"));
    assert_eq!(viewer.interpolation_mode(), "linear");
}

#[test]
fn viewport_rerender_returns_requested_size_bytes() {
    // REQ-HI-145: viewport rendering must be regenerated from source for target output size.
    let mut viewer = WasmViewer::new(16, 12);
    let valid = sample_p10();
    assert!(
        viewer.load_bytes(&valid),
        "load_bytes should accept valid P10 payload"
    );
    assert!(viewer.set_zoom(2.0));
    let bytes = viewer.render_viewport_rgba_bytes_for_size(20, 10);
    assert_eq!(bytes.len(), 20 * 10 * 4);
}

#[test]
fn recenter_fraction_enforces_bounds() {
    // REQ-HI-145: ROI recentering rejects invalid normalized fractions.
    let mut viewer = WasmViewer::new(16, 12);
    let valid = sample_p10();
    assert!(
        viewer.load_bytes(&valid),
        "load_bytes should accept valid P10 payload"
    );
    assert!(!viewer.recenter_from_viewport_fraction(-0.1, 0.5));
    assert!(!viewer.recenter_from_viewport_fraction(0.5, 1.1));
    assert!(viewer.recenter_from_viewport_fraction(0.5, 0.5));
}
