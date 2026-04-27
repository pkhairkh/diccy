// Auto-extracted from /home/z/diccy/crates/dicom-visualizer/src/main.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_visualizer::*;
use dicom_pixel::{DisplayFrame, PixelFormat};
use std::path::Path;

#[test]
fn sanitize_path_component_replaces_non_alnum() {
    assert_eq!(sanitize_path_component(Path::new("a/b-c.dcm")), "a_b_c_dcm");
}

#[test]
fn frame_to_rgba8_from_luma8_expands_channels() {
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Luma8,
        bytes: vec![7],
    };
    let rgba = frame_to_rgba8(&frame).expect("rgba");
    assert_eq!(rgba, vec![7, 7, 7, 255]);
}

#[test]
fn frame_to_rgba8_rejects_odd_luma16_buffer() {
    let frame = DisplayFrame {
        width: 1,
        height: 1,
        format: PixelFormat::Luma16,
        bytes: vec![1],
    };
    assert!(frame_to_rgba8(&frame).is_err());
}

#[test]
fn csv_escape_guards_formula_prefixes() {
    assert_eq!(csv_escape("=SUM(A1:A2)"), "'=SUM(A1:A2)");
    assert_eq!(csv_escape("+cmd"), "'+cmd");
}

#[test]
fn escape_html_escapes_attribute_sensitive_characters() {
    assert_eq!(
        escape_html("<img src=\"x\" onerror='a'>"),
        "&lt;img src=&quot;x&quot; onerror=&#x27;a&#x27;&gt;"
    );
}
