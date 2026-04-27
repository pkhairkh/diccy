// Auto-extracted from /home/z/diccy/crates/dicom-pixel/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_pixel::*;
use dicom_pixel::tags;
use dicom_core::{Dataset, Element, ErrorKind, Limits, Value};
use std::sync::Mutex;
#[cfg(feature = "pack-enhanced")]
use dicom_pixel::SOP_CLASS_ENHANCED_CT;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_env_vars<F, R>(pairs: &[(&str, Option<&str>)], f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous: Vec<(&str, Option<String>)> = pairs
        .iter()
        .map(|(name, _)| (*name, std::env::var(name).ok()))
        .collect();
    for (name, value) in pairs {
        match value {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
    }
    let result = f();
    for (name, value) in previous {
        match value {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn make_dataset(
    rows: u16,
    cols: u16,
    samples_per_pixel: u16,
    photometric: &str,
    bits_allocated: u16,
    bits_stored: u16,
    high_bit: u16,
    pixel_representation: u16,
    planar_configuration: Option<u16>,
    pixel_data: Vec<u8>,
) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            tags::TAG_ROWS,
            dicom_core::Vr::Us,
            Value::Bytes(rows.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_COLUMNS,
            dicom_core::Vr::Us,
            Value::Bytes(cols.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_SAMPLES_PER_PIXEL,
            dicom_core::Vr::Us,
            Value::Bytes(samples_per_pixel.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_PHOTOMETRIC_INTERPRETATION,
            dicom_core::Vr::Cs,
            Value::Str(photometric.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_BITS_ALLOCATED,
            dicom_core::Vr::Us,
            Value::Bytes(bits_allocated.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_BITS_STORED,
            dicom_core::Vr::Us,
            Value::Bytes(bits_stored.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_HIGH_BIT,
            dicom_core::Vr::Us,
            Value::Bytes(high_bit.to_le_bytes().to_vec()),
        )
        .unwrap(),
    );
    if photometric.starts_with("MONOCHROME") {
        dataset.insert(
            Element::new(
                tags::TAG_PIXEL_REPRESENTATION,
                dicom_core::Vr::Us,
                Value::Bytes(pixel_representation.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
    }
    if let Some(planar) = planar_configuration {
        dataset.insert(
            Element::new(
                tags::TAG_PLANAR_CONFIGURATION,
                dicom_core::Vr::Us,
                Value::Bytes(planar.to_le_bytes().to_vec()),
            )
            .unwrap(),
        );
    }
    dataset.insert(
        Element::new(
            tags::TAG_PIXEL_DATA,
            dicom_core::Vr::Ob,
            Value::Bytes(pixel_data),
        )
        .unwrap(),
    );
    dataset
}

#[test]
fn default_pipeline_config_uses_limits_default() {
    // REQ-SEC-402: defaults apply when not overridden.
    let config = PixelPipelineConfig::default();
    assert_eq!(config.limits, Limits::default());
    assert_eq!(config.window_level, WindowLevel::Auto);
}

#[cfg(feature = "pack-enhanced")]
#[test]
fn enhanced_sop_uid_uid_value_is_accepted_in_optional_string_path() {
    // REQ-PIX-245: UID-valued SOP Class tags must be accepted on optional string reads.
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            tags::TAG_SOP_CLASS_UID,
            dicom_core::Vr::Ui,
            Value::Uid(SOP_CLASS_ENHANCED_CT.to_string()),
        )
        .unwrap(),
    );
    assert!(is_enhanced_sop_class(&dataset, &Limits::default()).expect("enhanced sop"));
}

#[test]
fn decode_rejects_missing_required_tag() {
    // REQ-PIX-230
    let dataset = Dataset::new();
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn decode_accepts_padded_photometric_interpretation() {
    // REQ-PIX-245
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2 ", 8, 8, 7, 0, None, vec![128u8]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Luma8);
}

#[test]
fn decode_accepts_padded_rescale_values() {
    // REQ-PIX-245
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 16, 16, 15, 0, None, vec![1, 0]);
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_INTERCEPT,
            dicom_core::Vr::Ds,
            Value::Str("0 ".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_SLOPE,
            dicom_core::Vr::Ds,
            Value::Str("1 ".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Luma8);
}

#[test]
fn decode_enforces_limits() {
    // REQ-PIX-204
    let dataset = make_dataset(
        0x7FFF,
        0x7FFF,
        1,
        "MONOCHROME2",
        8,
        8,
        7,
        0,
        None,
        vec![128u8],
    );
    let mut config = PixelPipelineConfig::default();
    config.limits.set_max_pixels_per_frame(1_000);
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn monochrome1_inversion_applied_after_voi() {
    // REQ-PIX-242
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME1", 8, 8, 7, 0, None, vec![128u8]);
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_CENTER,
            dicom_core::Vr::Ds,
            Value::Str("128".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_WIDTH,
            dicom_core::Vr::Ds,
            Value::Str("256".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Luma8);
    assert_eq!(frame.bytes[0], 127u8);
}

#[test]
fn voi_linear_exact_boundary() {
    // REQ-PIX-265
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![0u8]);
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_CENTER,
            dicom_core::Vr::Ds,
            Value::Str("0".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_WIDTH,
            dicom_core::Vr::Ds,
            Value::Str("2".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_VOI_LUT_FUNCTION,
            dicom_core::Vr::Cs,
            Value::Str("LINEAR_EXACT".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_PIXEL_DATA,
            dicom_core::Vr::Ob,
            Value::Bytes(vec![0u8]),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.bytes[0], 128u8);
}

#[test]
fn auto_window_deterministic() {
    // REQ-PIX-267
    let dataset = make_dataset(
        2,
        2,
        1,
        "MONOCHROME2",
        8,
        8,
        7,
        0,
        None,
        vec![0, 64, 128, 255],
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame_a = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame a");
    let frame_b = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame b");
    assert_eq!(frame_a.bytes, frame_b.bytes);
}

#[test]
fn env_contract_changes_do_not_affect_cpu_voi_modality_oracle() {
    // REQ-OPS-006: CPU VOI/modality transforms remain authoritative and env parsing independent.
    let mut dataset = make_dataset(
        1,
        1,
        1,
        "MONOCHROME2",
        16,
        16,
        15,
        0,
        None,
        vec![0x80, 0x00],
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_INTERCEPT,
            dicom_core::Vr::Ds,
            Value::Str("-1024".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_SLOPE,
            dicom_core::Vr::Ds,
            Value::Str("1".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_CENTER,
            dicom_core::Vr::Ds,
            Value::Str("40".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_WIDTH,
            dicom_core::Vr::Ds,
            Value::Str("400".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let baseline = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("baseline decode");

    with_env_vars(
        &[
            ("DICOM_WEB_WORKERS", Some("64")),
            ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", Some("999")),
            ("DICOM_DIMSE_MAX_INPUT_BYTES", Some("1048576")),
        ],
        || {
            let with_env = pipeline
                .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
                .expect("decode with unrelated env vars set");
            assert_eq!(baseline.bytes, with_env.bytes);
            assert_eq!(baseline.format, with_env.format);
            assert_eq!(baseline.width, with_env.width);
            assert_eq!(baseline.height, with_env.height);
        },
    );
}

#[test]
fn auto_window_rejects_nan_percentile_config() {
    // REQ-PIX-220
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    let config = PixelPipelineConfig {
        auto_window_low_percentile: f64::NAN,
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::InvalidPixelTransform { .. }
    ));
}

#[test]
fn auto_window_rejects_out_of_range_percentiles() {
    // REQ-PIX-220
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    let config = PixelPipelineConfig {
        auto_window_low_percentile: -0.1,
        auto_window_high_percentile: 1.1,
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::InvalidPixelTransform { .. }
    ));
}

#[test]
fn auto_window_rejects_inverted_percentiles() {
    // REQ-PIX-220
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    let config = PixelPipelineConfig {
        auto_window_low_percentile: 0.9,
        auto_window_high_percentile: 0.1,
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::InvalidPixelTransform { .. }
    ));
}

#[test]
fn explicit_window_rejects_non_finite_center() {
    // REQ-PIX-220
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    let config = PixelPipelineConfig {
        window_level: WindowLevel::Explicit {
            center: f64::INFINITY,
            width: 2.0,
        },
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn non_finite_rescale_rejected() {
    // REQ-PIX-220
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_SLOPE,
            dicom_core::Vr::Ds,
            Value::Str("1e309".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_INTERCEPT,
            dicom_core::Vr::Ds,
            Value::Str("0".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::InvalidTagValue { .. } | ErrorKind::InvalidPixelTransform { .. }
    ));
}

#[test]
fn modality_rescale_overflow_rejected() {
    // REQ-PIX-220, REQ-PIX-253
    let mut dataset = make_dataset(
        1,
        1,
        1,
        "MONOCHROME2",
        16,
        16,
        15,
        1,
        None,
        i16::MAX.to_le_bytes().to_vec(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_SLOPE,
            dicom_core::Vr::Ds,
            Value::Str("1e308".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_RESCALE_INTERCEPT,
            dicom_core::Vr::Ds,
            Value::Str("1e308".to_string()),
        )
        .unwrap(),
    );
    let config = PixelPipelineConfig {
        window_level: WindowLevel::Explicit {
            center: 0.0,
            width: 2.0,
        },
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::InvalidPixelTransform { .. }
    ));
}

#[test]
fn rgb_planar_reorder() {
    // REQ-PIX-243
    let dataset = make_dataset(1, 1, 3, "RGB", 8, 8, 7, 0, Some(1), vec![1, 2, 3]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![1, 2, 3, 255]);
}

#[test]
fn ybr_full_uncompressed_decodes_to_rgba() {
    // REQ-PIX-240, REQ-PIX-243
    let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), vec![76, 85, 255]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_EXPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
}

#[test]
fn ybr_full_deflated_decodes_to_rgba() {
    // REQ-TS-204, REQ-PIX-240, REQ-PIX-243
    let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), vec![76, 85, 255]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_DEFLATED_EXPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
}

#[test]
fn ybr_full_rle_decodes_to_rgba() {
    // REQ-PIX-240, REQ-PIX-243
    let dataset = make_dataset(
        1,
        1,
        3,
        "YBR_FULL",
        8,
        8,
        7,
        0,
        Some(0),
        sample_rle_triplet(76, 85, 255),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_RLE_LOSSLESS, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![254, 0, 0, 255]);
}

#[test]
fn ybr_full_422_uncompressed_decodes_to_rgba() {
    // REQ-PIX-240, REQ-PIX-243
    let dataset = make_dataset(
        1,
        2,
        3,
        "YBR_FULL_422",
        8,
        8,
        7,
        0,
        Some(0),
        vec![76, 76, 85, 255],
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![254, 0, 0, 255, 254, 0, 0, 255]);
}

#[test]
fn ybr_full_422_deflated_decodes_to_rgba() {
    // REQ-TS-204, REQ-PIX-240, REQ-PIX-243
    let dataset = make_dataset(
        1,
        2,
        3,
        "YBR_FULL_422",
        8,
        8,
        7,
        0,
        Some(0),
        vec![76, 76, 85, 255],
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_DEFLATED_EXPLICIT_VR_LE, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Rgba8);
    assert_eq!(frame.bytes, vec![254, 0, 0, 255, 254, 0, 0, 255]);
}

#[test]
fn ybr_full_422_rejects_odd_columns() {
    // REQ-PIX-240
    let dataset = make_dataset(1, 1, 3, "YBR_FULL_422", 8, 8, 7, 0, Some(0), vec![76, 76]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn ybr_full_422_rejects_planar_configuration_1() {
    // REQ-PIX-240
    let dataset = make_dataset(
        1,
        2,
        3,
        "YBR_FULL_422",
        8,
        8,
        7,
        0,
        Some(1),
        vec![76, 76, 85, 255],
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn ybr_rejects_non_native_transfer_syntax() {
    // REQ-PIX-240
    let dataset = make_dataset(1, 1, 3, "YBR_FULL", 8, 8, 7, 0, Some(0), sample_jpeg_rgb());
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn jpeg_baseline_multiframe_requires_offset_table_support() {
    // REQ-CONF-084, REQ-CONF-086: encapsulated multi-frame decode must fail closed
    // when offset-table indexing support is not enabled.
    let dataset = make_dataset(1, 1, 3, "RGB", 8, 8, 7, 0, Some(0), sample_jpeg_rgb());
    let mut dataset = dataset;
    dataset.insert(
        Element::new(
            tags::TAG_NUMBER_OF_FRAMES,
            dicom_core::Vr::Is,
            Value::Str("2".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
        .expect_err("expected multi-frame rejection");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("offset table support"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn jpeg_component_mismatch_rejected() {
    // REQ-PIX-238
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, sample_jpeg_rgb());
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEG_BASELINE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn jpegls_decodes_lossless_sample() {
    // REQ-TS-203, REQ-CODEC-352
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 1,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    let encoded = charls.encode(frame, 0, &[128u8]).expect("encode");

    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_CENTER,
            dicom_core::Vr::Ds,
            Value::Str("128".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_WIDTH,
            dicom_core::Vr::Ds,
            Value::Str("256".to_string()),
        )
        .unwrap(),
    );

    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
        .expect("frame");
    assert_eq!(frame.format, PixelFormat::Luma8);
    assert_eq!(frame.bytes, vec![128u8]);
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn jpegls_rejects_dimension_mismatch() {
    // REQ-CODEC-350
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 1,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    let encoded = charls.encode(frame, 0, &[42u8]).expect("encode");

    let dataset = make_dataset(2, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn jpegls_multiframe_requires_offset_table_support() {
    // REQ-CONF-084, REQ-CONF-086
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 1,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    let encoded = charls.encode(frame, 0, &[42u8]).expect("encode");
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
    dataset.insert(
        Element::new(
            tags::TAG_NUMBER_OF_FRAMES,
            dicom_core::Vr::Is,
            Value::Str("2".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
        .expect_err("expected multi-frame rejection");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("offset table support"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn jpegls_enforces_max_decompressed_bytes() {
    // REQ-CODEC-351
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 2,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    let encoded = charls.encode(frame, 0, &[10u8, 20u8]).expect("encode");

    let dataset = make_dataset(1, 2, 1, "MONOCHROME2", 8, 8, 7, 0, None, encoded);
    let mut config = PixelPipelineConfig::default();
    config.limits.set_max_decompressed_bytes(1);
    let pipeline = PixelPipeline::new(config);
    let err = pipeline
        .decode_frame(&dataset, TS_JPEGLS_LOSSLESS, 0)
        .expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::LimitExceeded {
            limit_name: "max_decompressed_bytes",
            ..
        }
    ));
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn jpegls_codec_roundtrip() {
    // REQ-TS-203
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 1,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    let encoded = charls.encode(frame, 0, &[200u8]).expect("encode");
    let decoded = charls.decode(&encoded).expect("decode");
    assert_eq!(decoded, vec![200u8]);
}

#[test]
#[cfg(feature = "codec-j2k")]
fn jpeg2000_decodes_sample() {
    // REQ-TS-203, REQ-CODEC-352
    let bytes = include_bytes!("../../../corpus/j2k_file1.jp2").to_vec();
    let bitmap = hayro_jpeg2000::decode(&bytes, &hayro_jpeg2000::DecodeSettings::default())
        .expect("decode bitmap");
    if bitmap.has_alpha {
        return;
    }
    let (samples_per_pixel, photometric) = match bitmap.color_space {
        hayro_jpeg2000::ColorSpace::Gray => (1u16, "MONOCHROME2"),
        hayro_jpeg2000::ColorSpace::RGB => (3u16, "RGB"),
        _ => return,
    };

    let rows = u16::try_from(bitmap.height).expect("rows");
    let cols = u16::try_from(bitmap.width).expect("cols");
    let dataset = make_dataset(
        rows,
        cols,
        samples_per_pixel,
        photometric,
        8,
        8,
        7,
        0,
        if samples_per_pixel > 1 { Some(0) } else { None },
        bytes,
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let frame = pipeline
        .decode_frame(&dataset, TS_JPEG2000_LOSSLESS, 0)
        .expect("frame");
    let expected_format = if samples_per_pixel == 1 {
        PixelFormat::Luma8
    } else {
        PixelFormat::Rgba8
    };
    assert_eq!(frame.format, expected_format);
}

#[test]
#[cfg(feature = "codec-j2k")]
fn jpeg2000_multiframe_requires_offset_table_support() {
    // REQ-CONF-084, REQ-CONF-086
    let bytes = include_bytes!("../../../corpus/j2k_file1.jp2").to_vec();
    let bitmap = hayro_jpeg2000::decode(&bytes, &hayro_jpeg2000::DecodeSettings::default())
        .expect("decode bitmap");
    if bitmap.has_alpha {
        return;
    }
    let (samples_per_pixel, photometric) = match bitmap.color_space {
        hayro_jpeg2000::ColorSpace::Gray => (1u16, "MONOCHROME2"),
        hayro_jpeg2000::ColorSpace::RGB => (3u16, "RGB"),
        _ => return,
    };
    let rows = u16::try_from(bitmap.height).expect("rows");
    let cols = u16::try_from(bitmap.width).expect("cols");
    let mut dataset = make_dataset(
        rows,
        cols,
        samples_per_pixel,
        photometric,
        8,
        8,
        7,
        0,
        if samples_per_pixel > 1 { Some(0) } else { None },
        bytes,
    );
    dataset.insert(
        Element::new(
            tags::TAG_NUMBER_OF_FRAMES,
            dicom_core::Vr::Is,
            Value::Str("2".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_JPEG2000_LOSSLESS, 0)
        .expect_err("expected multi-frame rejection");
    match &err.kind() {
        ErrorKind::DecodeError { detail, .. } => {
            assert!(detail.contains("offset table support"));
        }
        _ => panic!("expected decode error"),
    }
}

#[test]
fn decode_rejects_short_uncompressed_pixel_data() {
    // REQ-PIX-231
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, Vec::new());
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn voi_function_rejects_sigmoid() {
    // REQ-PIX-262
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_CENTER,
            dicom_core::Vr::Ds,
            Value::Str("1".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_WINDOW_WIDTH,
            dicom_core::Vr::Ds,
            Value::Str("2".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_VOI_LUT_FUNCTION,
            dicom_core::Vr::Cs,
            Value::Str("SIGMOID".to_string()),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn overlay_bounds_rejected() {
    // REQ-PIX-280
    let mut dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![128u8]);
    dataset.insert(
        Element::new(
            tags::TAG_OVERLAY_ROWS,
            dicom_core::Vr::Us,
            Value::Bytes(vec![0x02, 0x00]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_OVERLAY_COLUMNS,
            dicom_core::Vr::Us,
            Value::Bytes(vec![0x02, 0x00]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_OVERLAY_BITS_ALLOCATED,
            dicom_core::Vr::Us,
            Value::Bytes(vec![0x01, 0x00]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_OVERLAY_BIT_POSITION,
            dicom_core::Vr::Us,
            Value::Bytes(vec![0x00, 0x00]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            tags::TAG_OVERLAY_DATA,
            dicom_core::Vr::Ob,
            Value::Bytes(vec![0xFF]),
        )
        .unwrap(),
    );
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn rle_header_too_short_rejected() {
    // REQ-PIX-235
    let dataset = make_dataset(1, 1, 1, "MONOCHROME2", 8, 8, 7, 0, None, vec![0u8; 10]);
    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let err = pipeline
        .decode_frame(&dataset, TS_RLE_LOSSLESS, 0)
        .expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

fn sample_jpeg_rgb() -> Vec<u8> {
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01,
        0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06,
        0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D,
        0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D,
        0x1A, 0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28,
        0x37, 0x29, 0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
        0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x01, 0x00, 0x01,
        0x03, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xFF, 0xC4, 0x00, 0x14,
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0xFF, 0xC4, 0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C, 0x03, 0x01,
        0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0xD2, 0xCF, 0x20, 0xFF, 0xD9,
    ]
}

fn sample_rle_triplet(y: u8, cb: u8, cr: u8) -> Vec<u8> {
    let mut data = vec![0u8; 64];
    data[0..4].copy_from_slice(&(3u32).to_le_bytes());
    data[4..8].copy_from_slice(&(64u32).to_le_bytes());
    data[8..12].copy_from_slice(&(66u32).to_le_bytes());
    data[12..16].copy_from_slice(&(68u32).to_le_bytes());
    data.extend_from_slice(&[0x00, y]);
    data.extend_from_slice(&[0x00, cb]);
    data.extend_from_slice(&[0x00, cr]);
    data
}
