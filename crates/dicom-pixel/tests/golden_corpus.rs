use dicom_core::{Dataset, Element, Tag, Value, Vr};
use dicom_pixel::{PixelFormat, PixelPipeline, PixelPipelineConfig, WindowLevel};
use jpeg_encoder::{ColorType, Encoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const TS_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";
const TS_RLE_LOSSLESS: &str = "1.2.840.10008.1.2.5";
const TS_JPEG_BASELINE: &str = "1.2.840.10008.1.2.4.50";
#[cfg(feature = "codec-jpegls")]
#[allow(dead_code)]
const TS_JPEGLS_LOSSLESS: &str = "1.2.840.10008.1.2.4.80";
#[cfg(feature = "codec-jpegls")]
#[allow(dead_code)]
const TS_JPEGLS_NEAR_LOSSLESS: &str = "1.2.840.10008.1.2.4.81";
#[cfg(feature = "codec-j2k")]
#[allow(dead_code)]
const TS_JPEG2000_LOSSLESS: &str = "1.2.840.10008.1.2.4.90";
#[cfg(feature = "codec-j2k")]
#[allow(dead_code)]
const TS_JPEG2000_LOSSY: &str = "1.2.840.10008.1.2.4.91";
const SOP_SECONDARY_CAPTURE: &str = "1.2.840.10008.5.1.4.1.1.7";
const SOP_CT_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.2";
const SOP_MR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.4";
const SOP_SC_MF_BYTE: &str = "1.2.840.10008.5.1.4.1.1.7.2";
const SOP_SC_MF_WORD: &str = "1.2.840.10008.5.1.4.1.1.7.3";
const SOP_SC_MF_TRUE_COLOR: &str = "1.2.840.10008.5.1.4.1.1.7.4";
const SOP_PET_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.128";
const SOP_CR_IMAGE_STORAGE: &str = "1.2.840.10008.5.1.4.1.1.1";
const SOP_DX_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.1";

const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
const TAG_PLANAR_CONFIGURATION: Tag = Tag(0x0028, 0x0006);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);
const TAG_PIXEL_PADDING_VALUE: Tag = Tag(0x0028, 0x0120);
const TAG_RESCALE_INTERCEPT: Tag = Tag(0x0028, 0x1052);
const TAG_RESCALE_SLOPE: Tag = Tag(0x0028, 0x1053);
const TAG_WINDOW_CENTER: Tag = Tag(0x0028, 0x1050);
const TAG_WINDOW_WIDTH: Tag = Tag(0x0028, 0x1051);

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Manifest {
    version: u32,
    hash_algorithm: String,
    samples: Vec<Sample>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Sample {
    id: String,
    sha256: String,
    sop_class_uid: String,
    transfer_syntax_uid: String,
    expected_outputs: Vec<ExpectedOutput>,
    source_kind: Option<String>,
    feature: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ExpectedOutput {
    config_id: String,
    frame_index: u32,
    format: String,
    sha256: String,
}

struct Fixture {
    id: &'static str,
    sop_class_uid: &'static str,
    transfer_syntax_uid: &'static str,
    source_bytes: Vec<u8>,
    dataset: Dataset,
    config_id: &'static str,
    config: PixelPipelineConfig,
    expected_format: PixelFormat,
    frame_index: u32,
    feature: Option<&'static str>,
    source_kind: Option<&'static str>,
}

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/manifest.toml")
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
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
    dataset.insert(Element {
        tag: TAG_ROWS,
        vr: Vr::Us,
        value: Value::Bytes(rows.to_le_bytes().to_vec()),
    });
    dataset.insert(Element {
        tag: TAG_COLUMNS,
        vr: Vr::Us,
        value: Value::Bytes(cols.to_le_bytes().to_vec()),
    });
    dataset.insert(Element {
        tag: TAG_SAMPLES_PER_PIXEL,
        vr: Vr::Us,
        value: Value::Bytes(samples_per_pixel.to_le_bytes().to_vec()),
    });
    dataset.insert(Element {
        tag: TAG_PHOTOMETRIC_INTERPRETATION,
        vr: Vr::Cs,
        value: Value::Str(photometric.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_BITS_ALLOCATED,
        vr: Vr::Us,
        value: Value::Bytes(bits_allocated.to_le_bytes().to_vec()),
    });
    dataset.insert(Element {
        tag: TAG_BITS_STORED,
        vr: Vr::Us,
        value: Value::Bytes(bits_stored.to_le_bytes().to_vec()),
    });
    dataset.insert(Element {
        tag: TAG_HIGH_BIT,
        vr: Vr::Us,
        value: Value::Bytes(high_bit.to_le_bytes().to_vec()),
    });
    if photometric.starts_with("MONOCHROME") {
        dataset.insert(Element {
            tag: TAG_PIXEL_REPRESENTATION,
            vr: Vr::Us,
            value: Value::Bytes(pixel_representation.to_le_bytes().to_vec()),
        });
    }
    if let Some(planar) = planar_configuration {
        dataset.insert(Element {
            tag: TAG_PLANAR_CONFIGURATION,
            vr: Vr::Us,
            value: Value::Bytes(planar.to_le_bytes().to_vec()),
        });
    }
    dataset.insert(Element {
        tag: TAG_PIXEL_DATA,
        vr: Vr::Ob,
        value: Value::Bytes(pixel_data),
    });
    dataset
}

fn insert_ds(dataset: &mut Dataset, tag: Tag, value: &str) {
    dataset.insert(Element {
        tag,
        vr: Vr::Ds,
        value: Value::Str(value.to_string()),
    });
}

fn insert_padding_value(dataset: &mut Dataset, value: i16) {
    dataset.insert(Element {
        tag: TAG_PIXEL_PADDING_VALUE,
        vr: Vr::Ss,
        value: Value::Bytes(value.to_le_bytes().to_vec()),
    });
}

fn sample_rle_mono_2x2() -> Vec<u8> {
    let mut header = vec![0u8; 64];
    header[0..4].copy_from_slice(&1u32.to_le_bytes());
    header[4..8].copy_from_slice(&64u32.to_le_bytes());
    let segment = vec![0x03, 0x00, 0x40, 0x80, 0xFF];
    header.extend_from_slice(&segment);
    header
}

fn sample_jpeg_rgb() -> Vec<u8> {
    let mut bytes = Vec::new();
    let encoder = Encoder::new(&mut bytes, 90);
    let pixel = [0x12u8, 0x34, 0x56];
    encoder
        .encode(&pixel, 1, 1, ColorType::Rgb)
        .expect("jpeg encode failed");
    bytes
}

#[cfg(feature = "codec-jpegls")]
fn sample_jpegls_mono() -> Vec<u8> {
    let mut charls = charls::CharLS::default();
    let frame = charls::FrameInfo {
        width: 1,
        height: 1,
        bits_per_sample: 8,
        component_count: 1,
    };
    charls.encode(frame, 0, &[128u8]).expect("jpegls encode")
}

#[cfg(feature = "codec-j2k")]
fn sample_j2k() -> Vec<u8> {
    include_bytes!("../../../corpus/j2k_file1.jp2").to_vec()
}

fn build_fixtures() -> Vec<Fixture> {
    let windowed = PixelPipelineConfig {
        window_level: WindowLevel::Explicit {
            center: 128.0,
            width: 256.0,
        },
        ..PixelPipelineConfig::default()
    };

    let mono_bytes = vec![0u8, 64, 128, 255];
    let mono_dataset = make_dataset(2, 2, 1, "MONOCHROME2", 8, 8, 7, 0, None, mono_bytes.clone());
    let mut mono1_rescale_dataset =
        make_dataset(2, 2, 1, "MONOCHROME1", 8, 8, 7, 0, None, mono_bytes.clone());
    insert_ds(&mut mono1_rescale_dataset, TAG_RESCALE_SLOPE, "2");
    insert_ds(&mut mono1_rescale_dataset, TAG_RESCALE_INTERCEPT, "-1024");
    insert_ds(&mut mono1_rescale_dataset, TAG_WINDOW_CENTER, "-768");
    insert_ds(&mut mono1_rescale_dataset, TAG_WINDOW_WIDTH, "1024");

    let rle_bytes = sample_rle_mono_2x2();
    let rle_dataset = make_dataset(2, 2, 1, "MONOCHROME2", 8, 8, 7, 0, None, rle_bytes.clone());

    let jpeg_bytes = sample_jpeg_rgb();
    let jpeg_dataset = make_dataset(1, 1, 3, "RGB", 8, 8, 7, 0, Some(0), jpeg_bytes.clone());
    let ybr422_bytes = vec![76u8, 76, 85, 255];
    let ybr422_dataset = make_dataset(
        1,
        2,
        3,
        "YBR_FULL_422",
        8,
        8,
        7,
        0,
        Some(0),
        ybr422_bytes.clone(),
    );

    let mut mono16_calibration_bytes = Vec::new();
    for sample in [-1024i16, 0, 1024, 2047] {
        mono16_calibration_bytes.extend_from_slice(&sample.to_le_bytes());
    }
    let mut mono16_calibration_dataset = make_dataset(
        2,
        2,
        1,
        "MONOCHROME2",
        16,
        16,
        15,
        1,
        None,
        mono16_calibration_bytes.clone(),
    );
    insert_ds(&mut mono16_calibration_dataset, TAG_RESCALE_SLOPE, "0.5");
    insert_ds(
        &mut mono16_calibration_dataset,
        TAG_RESCALE_INTERCEPT,
        "-1024",
    );
    let mono16_calibration_config = PixelPipelineConfig {
        window_level: WindowLevel::Explicit {
            center: -512.0,
            width: 2048.0,
        },
        ..PixelPipelineConfig::default()
    };

    let mut mono16_padding_bytes = Vec::new();
    for sample in [0u16, 500, 1000, 0] {
        mono16_padding_bytes.extend_from_slice(&sample.to_le_bytes());
    }
    let mut mono16_padding_dataset = make_dataset(
        2,
        2,
        1,
        "MONOCHROME2",
        16,
        16,
        15,
        0,
        None,
        mono16_padding_bytes.clone(),
    );
    insert_padding_value(&mut mono16_padding_dataset, 0);
    let auto_window_padding = PixelPipelineConfig {
        window_level: WindowLevel::Auto,
        auto_window_samples: 4,
        ..PixelPipelineConfig::default()
    };

    let ct_dataset = mono_dataset.clone();
    let mr_dataset = mono_dataset.clone();
    let sc_mf_byte_dataset = mono_dataset.clone();
    let pet_dataset = mono_dataset.clone();
    let cr_dataset = mono_dataset.clone();
    let dx_dataset = mono_dataset.clone();
    let explicit_vr_dataset = mono_dataset.clone();
    let sc_mf_word_dataset = mono16_padding_dataset.clone();
    let sc_mf_true_color_dataset = jpeg_dataset.clone();

    #[allow(unused_mut)]
    let mut fixtures = vec![
        Fixture {
            id: "synthetic-mono8-uncompressed",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: mono_dataset,
            config_id: "wl-explicit",
            config: windowed.clone(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mono1-rescale-inversion",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: mono1_rescale_dataset,
            config_id: "modality-rescale-mono1-v1",
            config: PixelPipelineConfig::default(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mono8-rle",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_RLE_LOSSLESS,
            source_bytes: rle_bytes,
            dataset: rle_dataset,
            config_id: "wl-explicit",
            config: windowed.clone(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-rgb8-jpeg-baseline",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_JPEG_BASELINE,
            source_bytes: jpeg_bytes.clone(),
            dataset: jpeg_dataset,
            config_id: "wl-explicit",
            config: windowed,
            expected_format: PixelFormat::Rgba8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-ybr422-uncompressed",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: ybr422_bytes,
            dataset: ybr422_dataset,
            config_id: "color-ybr422-v1",
            config: PixelPipelineConfig::default(),
            expected_format: PixelFormat::Rgba8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mono16-rescale-calibrated",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono16_calibration_bytes,
            dataset: mono16_calibration_dataset,
            config_id: "modality-rescale-calibrated16-v1",
            config: mono16_calibration_config,
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mono16-auto-padding",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono16_padding_bytes.clone(),
            dataset: mono16_padding_dataset,
            config_id: "auto-window-padding16-v1",
            config: auto_window_padding.clone(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-ct-uncompressed",
            sop_class_uid: SOP_CT_IMAGE_STORAGE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: ct_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mr-uncompressed",
            sop_class_uid: SOP_MR_IMAGE_STORAGE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: mr_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-sc-multiframe-byte-uncompressed",
            sop_class_uid: SOP_SC_MF_BYTE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: sc_mf_byte_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-sc-multiframe-word-uncompressed",
            sop_class_uid: SOP_SC_MF_WORD,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono16_padding_bytes.clone(),
            dataset: sc_mf_word_dataset,
            config_id: "auto-window-padding16-v1",
            config: auto_window_padding.clone(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-sc-multiframe-truecolor-jpeg-baseline",
            sop_class_uid: SOP_SC_MF_TRUE_COLOR,
            transfer_syntax_uid: TS_JPEG_BASELINE,
            source_bytes: jpeg_bytes.clone(),
            dataset: sc_mf_true_color_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Rgba8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-pet-uncompressed",
            sop_class_uid: SOP_PET_IMAGE_STORAGE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: pet_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-cr-uncompressed",
            sop_class_uid: SOP_CR_IMAGE_STORAGE,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: cr_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-dx-uncompressed",
            sop_class_uid: SOP_DX_PRESENTATION,
            transfer_syntax_uid: TS_IMPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: dx_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
        Fixture {
            id: "synthetic-mono8-explicit-vr",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_EXPLICIT_VR_LE,
            source_bytes: mono_bytes.clone(),
            dataset: explicit_vr_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig {
                window_level: WindowLevel::Explicit {
                    center: 128.0,
                    width: 256.0,
                },
                ..PixelPipelineConfig::default()
            },
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: None,
            source_kind: Some("synthetic"),
        },
    ];

    #[cfg(feature = "codec-jpegls")]
    {
        let lossless_bytes = sample_jpegls_mono();
        let lossless_dataset = make_dataset(
            1,
            1,
            1,
            "MONOCHROME2",
            8,
            8,
            7,
            0,
            None,
            lossless_bytes.clone(),
        );
        fixtures.push(Fixture {
            id: "synthetic-mono8-jpegls",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_JPEGLS_LOSSLESS,
            source_bytes: lossless_bytes,
            dataset: lossless_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig::default(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: Some("codec-jpegls"),
            source_kind: Some("synthetic"),
        });

        let near_lossless_bytes = sample_jpegls_mono();
        let near_lossless_dataset = make_dataset(
            1,
            1,
            1,
            "MONOCHROME2",
            8,
            8,
            7,
            0,
            None,
            near_lossless_bytes.clone(),
        );
        fixtures.push(Fixture {
            id: "synthetic-mono8-jpegls-near",
            sop_class_uid: SOP_SECONDARY_CAPTURE,
            transfer_syntax_uid: TS_JPEGLS_NEAR_LOSSLESS,
            source_bytes: near_lossless_bytes,
            dataset: near_lossless_dataset,
            config_id: "wl-explicit",
            config: PixelPipelineConfig::default(),
            expected_format: PixelFormat::Luma8,
            frame_index: 0,
            feature: Some("codec-jpegls"),
            source_kind: Some("synthetic"),
        });
    }

    #[cfg(feature = "codec-j2k")]
    {
        let bytes = sample_j2k();
        let bitmap = hayro_jpeg2000::decode(&bytes, &hayro_jpeg2000::DecodeSettings::default())
            .expect("decode bitmap");
        if !bitmap.has_alpha {
            let (samples_per_pixel, photometric) = match bitmap.color_space {
                hayro_jpeg2000::ColorSpace::Gray => (1u16, "MONOCHROME2"),
                hayro_jpeg2000::ColorSpace::RGB => (3u16, "RGB"),
                _ => (0u16, "UNSUPPORTED"),
            };
            if samples_per_pixel > 0 {
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
                    bytes.clone(),
                );
                fixtures.push(Fixture {
                    id: "external-jpeg2000-lossless",
                    sop_class_uid: SOP_SECONDARY_CAPTURE,
                    transfer_syntax_uid: TS_JPEG2000_LOSSLESS,
                    source_bytes: bytes.clone(),
                    dataset: dataset.clone(),
                    config_id: "wl-explicit",
                    config: PixelPipelineConfig::default(),
                    expected_format: if samples_per_pixel == 1 {
                        PixelFormat::Luma8
                    } else {
                        PixelFormat::Rgba8
                    },
                    frame_index: 0,
                    feature: Some("codec-j2k"),
                    source_kind: Some("external"),
                });

                fixtures.push(Fixture {
                    id: "external-jpeg2000-lossy",
                    sop_class_uid: SOP_SECONDARY_CAPTURE,
                    transfer_syntax_uid: TS_JPEG2000_LOSSY,
                    source_bytes: bytes,
                    dataset,
                    config_id: "wl-explicit",
                    config: PixelPipelineConfig::default(),
                    expected_format: if samples_per_pixel == 1 {
                        PixelFormat::Luma8
                    } else {
                        PixelFormat::Rgba8
                    },
                    frame_index: 0,
                    feature: Some("codec-j2k"),
                    source_kind: Some("external"),
                });
            }
        }
    }

    fixtures
}

fn format_name(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::Luma8 => "Luma8",
        PixelFormat::Luma16 => "Luma16",
        PixelFormat::Rgba8 => "Rgba8",
    }
}

fn load_manifest() -> Manifest {
    let text = fs::read_to_string(manifest_path()).expect("manifest read failed");
    toml::from_str(&text).expect("manifest parse failed")
}

fn build_manifest(fixtures: &[Fixture]) -> Manifest {
    let mut samples = Vec::new();
    let fixture_ids: BTreeSet<&str> = fixtures.iter().map(|fixture| fixture.id).collect();
    for fixture in fixtures {
        let output_hash = compute_output_hash(fixture);
        samples.push(Sample {
            id: fixture.id.to_string(),
            sha256: sha256_hex(&fixture.source_bytes),
            sop_class_uid: fixture.sop_class_uid.to_string(),
            transfer_syntax_uid: fixture.transfer_syntax_uid.to_string(),
            expected_outputs: vec![ExpectedOutput {
                config_id: fixture.config_id.to_string(),
                frame_index: fixture.frame_index,
                format: format_name(fixture.expected_format).to_string(),
                sha256: output_hash,
            }],
            source_kind: fixture.source_kind.map(|v| v.to_string()),
            feature: fixture.feature.map(|v| v.to_string()),
        });
    }
    if let Ok(text) = fs::read_to_string(manifest_path()) {
        if let Ok(existing) = toml::from_str::<Manifest>(&text) {
            for sample in existing.samples {
                if !fixture_ids.contains(sample.id.as_str()) {
                    samples.push(sample);
                }
            }
        }
    }
    samples.sort_by(|a, b| a.id.cmp(&b.id));
    Manifest {
        version: 1,
        hash_algorithm: "sha256".to_string(),
        samples,
    }
}

fn compute_output_hash(fixture: &Fixture) -> String {
    let pipeline = PixelPipeline::new(fixture.config.clone());
    let frame = pipeline
        .decode_frame(
            &fixture.dataset,
            fixture.transfer_syntax_uid,
            fixture.frame_index,
        )
        .expect("pipeline decode failed");
    assert_eq!(frame.format, fixture.expected_format);
    sha256_hex(&frame.bytes)
}

#[allow(clippy::needless_bool)]
fn feature_enabled(feature: &str) -> bool {
    if feature == "codec-jpegls" {
        cfg!(feature = "codec-jpegls")
    } else if feature == "codec-j2k" {
        cfg!(feature = "codec-j2k")
    } else {
        false
    }
}

fn validate_fixture(fixture: &Fixture, sample: &Sample) -> Result<(), String> {
    if fixture.sop_class_uid != sample.sop_class_uid {
        return Err(format!("sop_class_uid mismatch for {}", fixture.id));
    }
    if fixture.transfer_syntax_uid != sample.transfer_syntax_uid {
        return Err(format!("transfer_syntax_uid mismatch for {}", fixture.id));
    }
    let source_hash = sha256_hex(&fixture.source_bytes);
    if source_hash != sample.sha256 {
        return Err(format!("source hash mismatch for {}", fixture.id));
    }
    let expected = sample
        .expected_outputs
        .iter()
        .find(|entry| {
            entry.config_id == fixture.config_id
                && entry.frame_index == fixture.frame_index
                && entry.format == format_name(fixture.expected_format)
        })
        .ok_or_else(|| format!("missing expected output entry for {}", fixture.id))?;

    let output_hash = compute_output_hash(fixture);
    if output_hash != expected.sha256 {
        return Err(format!("output hash mismatch for {}", fixture.id));
    }
    Ok(())
}

#[test]
fn golden_corpus_hashes_match_manifest() {
    // REQ-TEST-705, REQ-TEST-706, REQ-TEST-707, REQ-TEST-710, REQ-TEST-720, REQ-TEST-722
    // REQ-API-230, REQ-PIX-200, REQ-PIX-203
    let fixtures = build_fixtures();

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        let manifest = build_manifest(&fixtures);
        let text = toml::to_string_pretty(&manifest).expect("manifest serialize failed");
        fs::write(manifest_path(), text).expect("manifest write failed");
        return;
    }

    let manifest = load_manifest();
    assert_eq!(manifest.hash_algorithm, "sha256");
    let fixture_ids: BTreeSet<&str> = fixtures.iter().map(|fixture| fixture.id).collect();
    let active_samples: Vec<&Sample> = manifest
        .samples
        .iter()
        .filter(|sample| sample.feature.as_deref().is_none_or(feature_enabled))
        .collect();
    let manifest_ids: BTreeSet<&str> = active_samples
        .iter()
        .map(|sample| sample.id.as_str())
        .collect();
    assert_eq!(fixture_ids, manifest_ids, "manifest sample ids mismatch");

    let mut samples_by_id: BTreeMap<&str, &Sample> = BTreeMap::new();
    for sample in &active_samples {
        samples_by_id.insert(sample.id.as_str(), sample);
    }

    for fixture in &fixtures {
        let sample = samples_by_id
            .get(fixture.id)
            .expect("sample missing from manifest");
        validate_fixture(fixture, sample).expect("fixture validation failed");
    }
}

#[test]
fn native_repeat_run_determinism_hashes_stable() {
    // REQ-TEST-740, REQ-PIX-203
    let fixtures = build_fixtures();
    for fixture in &fixtures {
        let baseline = compute_output_hash(fixture);
        for _ in 0..16 {
            let run_hash = compute_output_hash(fixture);
            assert_eq!(
                run_hash, baseline,
                "repeat-run hash mismatch for fixture {}",
                fixture.id
            );
        }
    }
}

#[test]
fn manifest_rejects_missing_fields() {
    // REQ-TEST-720
    let text = r#"
version = 1
hash_algorithm = "sha256"
[[samples]]
id = "missing-fields"
"#;
    let parsed = toml::from_str::<Manifest>(text);
    assert!(parsed.is_err());
}

#[test]
fn manifest_rejects_hash_mismatch() {
    // REQ-TEST-722
    let fixtures = build_fixtures();
    let manifest = build_manifest(&fixtures);
    let mut sample = manifest
        .samples
        .iter()
        .find(|entry| entry.id == fixtures[0].id)
        .expect("sample missing")
        .clone();
    sample.sha256 = "00".repeat(32);
    let err = validate_fixture(&fixtures[0], &sample).expect_err("expected mismatch error");
    assert!(err.contains("source hash mismatch"));
}
