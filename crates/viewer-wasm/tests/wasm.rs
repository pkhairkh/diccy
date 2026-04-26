#![cfg(target_arch = "wasm32")]
#![allow(unexpected_cfgs)]

use dicom_core::{Dataset, Element, Tag, Value, Vr};
use dicom_pixel::{PixelFormat, PixelPipeline, PixelPipelineConfig, WindowLevel};
use sha2::{Digest, Sha256};
use wasm_bindgen_test::wasm_bindgen_test;

const TS_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
const TAG_ROWS: Tag = Tag(0x0028, 0x0010);
const TAG_COLUMNS: Tag = Tag(0x0028, 0x0011);
const TAG_SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
const TAG_PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
const TAG_BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
const TAG_BITS_STORED: Tag = Tag(0x0028, 0x0101);
const TAG_HIGH_BIT: Tag = Tag(0x0028, 0x0102);
const TAG_PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);
const TAG_PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn synthetic_mono_dataset() -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Bytes((2u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Bytes((2u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SAMPLES_PER_PIXEL, Vr::Us, Value::Bytes((1u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PHOTOMETRIC_INTERPRETATION, Vr::Cs, Value::Str("MONOCHROME2".to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Bytes((8u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Bytes((8u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Bytes((7u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PIXEL_REPRESENTATION, Vr::Us, Value::Bytes((0u16).to_le_bytes().to_vec()),
    ).unwrap());
    dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0u8, 64, 128, 255]),
    ).unwrap());
    dataset
}

#[wasm_bindgen_test]
fn wasm_capabilities_default_profile_is_stable() {
    // REQ-FEAT-303: default WASM capability profile remains deterministic.
    let caps = dicom_core::capabilities();
    assert!(!caps.tier1_deflate);
    assert!(!caps.codec_jpegls);
    assert!(!caps.codec_j2k);
    assert!(!caps.gsps);
    assert!(!caps.pack_enhanced);
    assert!(!caps.pack_us);
    assert!(!caps.pack_nm);
    assert!(!caps.pack_xa);
    assert!(!caps.pack_seg);
    assert!(!caps.pack_rt);
    assert!(!caps.pack_sr);
    assert!(!caps.modality_ct);
    assert!(!caps.modality_pet);
    assert!(!caps.modality_mg);
    assert!(!caps.modality_xr);
}

#[wasm_bindgen_test]
fn wasm_metadata_escape_is_stable() {
    // REQ-SEC-433: metadata strings must be HTML-escaped.
    let raw = "<b>unsafe</b> & \"quoted\"";
    let escaped = viewer_wasm::escape_metadata_html(raw);
    assert_eq!(
        escaped,
        "&lt;b&gt;unsafe&lt;/b&gt; &amp; &quot;quoted&quot;"
    );
}

#[wasm_bindgen_test]
fn wasm_cpu_oracle_hash_matches_native_baseline() {
    // REQ-WASM-302 / REQ-TEST-740: CPU-boundary determinism must match native baseline hash.
    let dataset = synthetic_mono_dataset();
    let config = PixelPipelineConfig {
        window_level: WindowLevel::Explicit {
            center: 128.0,
            width: 256.0,
        },
        ..PixelPipelineConfig::default()
    };
    let pipeline = PixelPipeline::new(config);
    let frame = pipeline
        .decode_frame(&dataset, TS_IMPLICIT_VR_LE, 0)
        .expect("decode frame");
    assert_eq!(frame.format, PixelFormat::Luma8);
    let hash = sha256_hex(&frame.bytes);
    assert_eq!(
        hash,
        "81f9456ee0bb909b7fb7c887c21c21f9fc45430e881dd5d92450609410fc3936"
    );
}
