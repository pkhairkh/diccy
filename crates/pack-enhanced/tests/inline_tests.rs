// Auto-extracted from /home/z/diccy/crates/pack-enhanced/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


use pack_enhanced::*;
use dicom_core::{Dataset, Element, Error, Limits, Result, Value, Vr};
use dicom_io::parse_dataset_bytes;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const ENHANCED_MANIFEST: &str = include_str!("../manifest.toml");
const CORPUS_SAMPLE_ID: &str = "synthetic-enhanced-multiframe";
const CORPUS_SAMPLE_FILE: &str = "enhanced_multiframe_sample.bin";
const CORPUS_CONFIG_ID: &str = "enhanced-geometry-v1";
const CORPUS_FORMAT: &str = "GeometryHash";

fn parse_manifest_uids() -> Vec<String> {
    ENHANCED_MANIFEST
        .split('"')
        .enumerate()
        .filter_map(|(idx, part)| {
            if idx % 2 == 1 {
                Some(part.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn corpus_sample_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus/")
        .join(CORPUS_SAMPLE_FILE)
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

fn extract_quoted(line: &str) -> Option<String> {
    line.split('"').nth(1).map(|value| value.to_string())
}

fn manifest_entry() -> (String, String) {
    let text = fs::read_to_string(manifest_path()).expect("manifest read failed");
    let mut in_sample = false;
    let mut file_hash: Option<String> = None;
    let mut output_hash: Option<String> = None;
    let mut current_config: Option<String> = None;
    let mut current_format: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if line == "[[samples]]" {
            in_sample = false;
            current_config = None;
            current_format = None;
            continue;
        }
        if line.starts_with("id =") {
            let id = extract_quoted(line).expect("sample id parse");
            in_sample = id == CORPUS_SAMPLE_ID;
            continue;
        }
        if !in_sample {
            continue;
        }
        if line.starts_with("sha256 =") && file_hash.is_none() {
            file_hash = extract_quoted(line);
            continue;
        }
        if line == "[[samples.expected_outputs]]" {
            current_config = None;
            current_format = None;
            continue;
        }
        if line.starts_with("config_id =") {
            current_config = extract_quoted(line);
            continue;
        }
        if line.starts_with("format =") {
            current_format = extract_quoted(line);
            continue;
        }
        if line.starts_with("sha256 =")
            && current_config.as_deref() == Some(CORPUS_CONFIG_ID)
            && current_format.as_deref() == Some(CORPUS_FORMAT)
        {
            output_hash = extract_quoted(line);
        }
    }

    (
        file_hash.expect("sample sha256 missing in manifest"),
        output_hash.expect("expected output hash missing in manifest"),
    )
}

fn encode_text_bytes(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes().to_vec();
    assert!(
        bytes.len().is_multiple_of(2),
        "text value must be even length for synthetic corpus"
    );
    bytes
}

fn encode_uid_bytes(value: &str) -> Vec<u8> {
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn encode_item(item: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0xFFFEu16.to_le_bytes());
    out.extend_from_slice(&0xE000u16.to_le_bytes());
    out.extend_from_slice(&(item.len() as u32).to_le_bytes());
    out.extend_from_slice(item);
    out
}

fn encode_sequence_delim() -> [u8; 8] {
    let mut out = [0u8; 8];
    out[0..2].copy_from_slice(&0xFFFEu16.to_le_bytes());
    out[2..4].copy_from_slice(&0xE0DDu16.to_le_bytes());
    out
}

fn vr_bytes(vr: Vr) -> [u8; 2] {
    match vr {
        Vr::Ae => *b"AE",
        Vr::As => *b"AS",
        Vr::Cs => *b"CS",
        Vr::Da => *b"DA",
        Vr::Ds => *b"DS",
        Vr::Is => *b"IS",
        Vr::Lo => *b"LO",
        Vr::Lt => *b"LT",
        Vr::Pn => *b"PN",
        Vr::Sh => *b"SH",
        Vr::St => *b"ST",
        Vr::Tm => *b"TM",
        Vr::Ui => *b"UI",
        Vr::Ut => *b"UT",
        Vr::Ob => *b"OB",
        Vr::Ow => *b"OW",
        Vr::Sq => *b"SQ",
        Vr::Un => *b"UN",
        Vr::Other(bytes) => bytes,
        _ => *b"UN",
    }
}

fn encode_dataset_explicit(dataset: &Dataset) -> Vec<u8> {
    let mut out = Vec::new();
    for element in dataset.iter() {
        out.extend_from_slice(&encode_element_explicit(element));
    }
    out
}

fn encode_element_explicit(element: &Element) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&element.tag().0.to_le_bytes());
    out.extend_from_slice(&element.tag().1.to_le_bytes());
    match element.value() {
        Value::Sequence(items) => {
            let mut seq_bytes = Vec::new();
            for item in items {
                let item_bytes = encode_dataset_explicit(item);
                seq_bytes.extend_from_slice(&encode_item(&item_bytes));
            }
            out.extend_from_slice(b"SQ");
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&u32::MAX.to_le_bytes());
            out.extend_from_slice(&seq_bytes);
            out.extend_from_slice(&encode_sequence_delim());
        }
        Value::Str(value) => {
            let bytes = encode_text_bytes(value);
            out.extend_from_slice(&vr_bytes(*element.vr()));
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        Value::Uid(value) => {
            let bytes = encode_uid_bytes(value);
            out.extend_from_slice(b"UI");
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        Value::Bytes(value) => {
            out.extend_from_slice(&vr_bytes(*element.vr()));
            out.extend_from_slice(&(value.len() as u16).to_le_bytes());
            out.extend_from_slice(value);
        }
        _ => panic!("unsupported value for synthetic corpus encoding"),
    }
    out
}

fn geometry_hash(dataset: &Dataset, limits: &Limits) -> Result<String> {
    let groups = select_frame_groups(dataset, 0, limits)?;
    let geometry = extract_frame_geometry(&groups, limits)?;
    let rescale = extract_frame_rescale(&groups, limits)?;
    let mut text = String::new();
    text.push_str(&format!(
        "ipp={:.6},{:.6},{:.6};",
        geometry.ipp[0], geometry.ipp[1], geometry.ipp[2]
    ));
    text.push_str(&format!(
        "iop={:.6},{:.6},{:.6},{:.6},{:.6},{:.6};",
        geometry.iop[0],
        geometry.iop[1],
        geometry.iop[2],
        geometry.iop[3],
        geometry.iop[4],
        geometry.iop[5]
    ));
    text.push_str(&format!(
        "spacing={:.6},{:.6};",
        geometry.pixel_spacing.0, geometry.pixel_spacing.1
    ));
    match geometry.slice_thickness {
        Some(value) => text.push_str(&format!("thickness={value:.6};")),
        None => text.push_str("thickness=none;"),
    }
    match rescale {
        Some(rescale) => text.push_str(&format!(
            "slope={:.6};intercept={:.6};",
            rescale.slope, rescale.intercept
        )),
        None => text.push_str("slope=none;intercept=none;"),
    }
    Ok(sha256_hex(text.as_bytes()))
}

#[test]
#[cfg(not(feature = "pack-enhanced"))]
fn pack_disabled_rejects() {
    // REQ-FEAT-302, REQ-SOP-301
    assert!(!EnhancedPack::enabled());
    let err = EnhancedPack::ensure_supported(SOP_CLASS_ENHANCED_CT).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
}

#[test]
#[cfg(feature = "pack-enhanced")]
fn pack_enabled_allows() {
    // REQ-FEAT-302, REQ-SOP-301
    assert!(EnhancedPack::enabled());
    EnhancedPack::ensure_supported(SOP_CLASS_ENHANCED_CT).expect("enhanced pack enabled");
}

#[test]
fn manifest_matches_constants() {
    // REQ-CONF-083, REQ-SOP-300
    let parsed = parse_manifest_uids();
    assert!(!parsed.is_empty());
    for uid in ENHANCED_SOP_CLASS_UIDS {
        assert!(parsed.contains(&uid.to_string()));
    }
}

#[test]
fn manifest_uids_look_like_uids() {
    // REQ-CONF-083, REQ-SOP-300
    for uid in ENHANCED_SOP_CLASS_UIDS {
        assert!(uid.chars().all(|ch| ch.is_ascii_digit() || ch == '.'));
    }
}

fn build_enhanced_dataset_with_values(
    number_of_frames: &str,
    pixel_spacing_value: &str,
    slice_thickness_value: Option<&str>,
) -> Dataset {
    let mut shared = Dataset::new();
    let mut pixel_measures = Dataset::new();
    pixel_measures.insert(
        Element::new(
            TAG_PIXEL_SPACING,
            Vr::Ds,
            Value::Str(pixel_spacing_value.to_string()),
        )
        .unwrap(),
    );
    if let Some(slice_thickness) = slice_thickness_value {
        pixel_measures.insert(
            Element::new(
                TAG_SLICE_THICKNESS,
                Vr::Ds,
                Value::Str(slice_thickness.to_string()),
            )
            .unwrap(),
        );
    }
    shared.insert(
        Element::new(
            TAG_PIXEL_MEASURES_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![pixel_measures]),
        )
        .unwrap(),
    );

    let mut per_frame = Dataset::new();
    let mut plane_position = Dataset::new();
    plane_position.insert(
        Element::new(
            TAG_IMAGE_POSITION,
            Vr::Ds,
            Value::Str("0\\0\\00".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PLANE_POSITION_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![plane_position]),
        )
        .unwrap(),
    );
    let mut plane_orientation = Dataset::new();
    plane_orientation.insert(
        Element::new(
            TAG_IMAGE_ORIENTATION,
            Vr::Ds,
            Value::Str("1\\0\\0\\0\\1\\00".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PLANE_ORIENTATION_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![plane_orientation]),
        )
        .unwrap(),
    );
    let mut rescale = Dataset::new();
    rescale.insert(
        Element::new(TAG_RESCALE_SLOPE, Vr::Ds, Value::Str("2.00".to_string())).unwrap(),
    );
    rescale.insert(
        Element::new(
            TAG_RESCALE_INTERCEPT,
            Vr::Ds,
            Value::Str("5.00".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PIXEL_VALUE_TRANSFORM_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![rescale]),
        )
        .unwrap(),
    );
    let mut frame_content = Dataset::new();
    frame_content.insert(
        Element::new(
            TAG_IN_STACK_POSITION_NUMBER,
            Vr::Is,
            Value::Str("1".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_FRAME_CONTENT_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![frame_content]),
        )
        .unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(
            TAG_NUMBER_OF_FRAMES,
            Vr::Is,
            Value::Str(number_of_frames.to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![shared]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![per_frame]),
        )
        .unwrap(),
    );
    dataset
}

fn build_enhanced_dataset() -> Dataset {
    build_enhanced_dataset_with_values("01", "0.5\\0.50", None)
}

#[test]
fn enhanced_groups_extract_rescale() {
    // REQ-CONF-084, REQ-ENH-350
    let dataset = build_enhanced_dataset();
    let limits = Limits::default();
    let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
    let rescale = extract_frame_rescale(&groups, &limits).expect("rescale");
    assert_eq!(
        rescale.unwrap(),
        EnhancedRescale {
            slope: 2.0,
            intercept: 5.0
        }
    );
}

#[test]
fn enhanced_missing_per_frame_groups_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![Dataset::new()]),
        )
        .unwrap(),
    );
    let limits = Limits::default();
    let err = select_frame_groups(&dataset, 0, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
}

#[test]
fn enhanced_per_frame_count_mismatch_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let dataset = build_enhanced_dataset_with_values("2", "0.5\\0.50", None);
    let limits = Limits::default();
    let err = select_frame_groups(&dataset, 0, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_missing_plane_position_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let mut shared = Dataset::new();
    let mut pixel_measures = Dataset::new();
    pixel_measures.insert(
        Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
    );
    shared.insert(
        Element::new(
            TAG_PIXEL_MEASURES_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![pixel_measures]),
        )
        .unwrap(),
    );

    let mut per_frame = Dataset::new();
    let mut plane_orientation = Dataset::new();
    plane_orientation.insert(
        Element::new(
            TAG_IMAGE_ORIENTATION,
            Vr::Ds,
            Value::Str("1\\0\\0\\0\\1\\0".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PLANE_ORIENTATION_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![plane_orientation]),
        )
        .unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![shared]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![per_frame]),
        )
        .unwrap(),
    );

    let limits = Limits::default();
    let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
    let err = extract_frame_geometry(&groups, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
}

#[test]
fn enhanced_invalid_frame_content_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let mut shared = Dataset::new();
    let mut pixel_measures = Dataset::new();
    pixel_measures.insert(
        Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
    );
    shared.insert(
        Element::new(
            TAG_PIXEL_MEASURES_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![pixel_measures]),
        )
        .unwrap(),
    );

    let mut per_frame = Dataset::new();
    let mut plane_position = Dataset::new();
    plane_position.insert(
        Element::new(
            TAG_IMAGE_POSITION,
            Vr::Ds,
            Value::Str("0\\0\\0".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PLANE_POSITION_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![plane_position]),
        )
        .unwrap(),
    );
    let mut plane_orientation = Dataset::new();
    plane_orientation.insert(
        Element::new(
            TAG_IMAGE_ORIENTATION,
            Vr::Ds,
            Value::Str("1\\0\\0\\0\\1\\0".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_PLANE_ORIENTATION_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![plane_orientation]),
        )
        .unwrap(),
    );
    let mut frame_content = Dataset::new();
    frame_content.insert(
        Element::new(
            TAG_IN_STACK_POSITION_NUMBER,
            Vr::Is,
            Value::Str("0".to_string()),
        )
        .unwrap(),
    );
    per_frame.insert(
        Element::new(
            TAG_FRAME_CONTENT_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![frame_content]),
        )
        .unwrap(),
    );

    let mut dataset = Dataset::new();
    dataset.insert(
        Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_SHARED_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![shared]),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_PER_FRAME_FUNCTIONAL_GROUPS_SEQUENCE,
            Vr::Sq,
            Value::Sequence(vec![per_frame]),
        )
        .unwrap(),
    );

    let limits = Limits::default();
    let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
    let err = extract_frame_geometry(&groups, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_non_positive_pixel_spacing_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let dataset = build_enhanced_dataset_with_values("01", "0\\1", None);
    let limits = Limits::default();
    let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
    let err = extract_frame_geometry(&groups, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_non_positive_slice_thickness_fails() {
    // REQ-CONF-084, REQ-ENH-350
    let dataset = build_enhanced_dataset_with_values("01", "1\\1", Some("0"));
    let limits = Limits::default();
    let groups = select_frame_groups(&dataset, 0, &limits).expect("groups");
    let err = extract_frame_geometry(&groups, &limits).unwrap_err();
    assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
}

#[test]
fn enhanced_corpus_sample_matches_manifest() {
    // REQ-ENH-350: enhanced multi-frame functional groups must parse deterministically.
    // REQ-SOP-300: enhanced SOP classes are covered by corpus validation.
    // REQ-TEST-720, REQ-TEST-722: corpus samples must be hash-validated.
    if std::env::var("UPDATE_ENHANCED_CORPUS").is_ok() {
        return;
    }
    let bytes = fs::read(corpus_sample_path()).expect("corpus sample read failed");
    let file_hash = sha256_hex(&bytes);
    let (manifest_hash, expected_output_hash) = manifest_entry();
    assert_eq!(file_hash, manifest_hash);

    let limits = Limits::default();
    let dataset = parse_dataset_bytes(&bytes, "1.2.840.10008.1.2.1", &limits)
        .expect("parse corpus dataset");
    let output_hash = geometry_hash(&dataset, &limits).expect("geometry hash");
    assert_eq!(output_hash, expected_output_hash);
}

#[test]
fn update_enhanced_corpus_sample() {
    if std::env::var("UPDATE_ENHANCED_CORPUS").is_err() {
        return;
    }
    let dataset = build_enhanced_dataset();
    let bytes = encode_dataset_explicit(&dataset);
    fs::write(corpus_sample_path(), &bytes).expect("write corpus sample");
    let file_hash = sha256_hex(&bytes);
    let output_hash = geometry_hash(&dataset, &Limits::default()).expect("geometry hash");
    println!("enhanced corpus sha256={file_hash}");
    println!("enhanced corpus output sha256={output_hash}");
}
