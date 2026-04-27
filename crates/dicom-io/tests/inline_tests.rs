// Auto-extracted from /home/z/diccy/crates/dicom-io/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


#[cfg(feature = "codec-j2k")]
use dicom_io::TS_JPEG2000_LOSSLESS;
#[cfg(feature = "codec-jpegls")]
use dicom_io::TS_JPEGLS_LOSSLESS;
use dicom_io::{
    BytesSource, DicomSource, FileSource, P10Reader, ReaderConfig, Tag,
    TS_EXPLICIT_VR_LE, TS_IMPLICIT_VR_LE, TS_JPEG_BASELINE, TS_RLE_LOSSLESS,
};
use dicom_core::{ErrorKind, Limits};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct EmptySource;

impl DicomSource for EmptySource {
    fn read_to_end(&mut self) -> dicom_core::Result<Vec<u8>> {
        Ok(Vec::new())
    }
}

fn build_p10(meta_ts: &str, dataset: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), meta_ts));
    bytes.extend_from_slice(dataset);
    bytes
}

fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(b"UI");
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    buf.extend_from_slice(
        &u16::try_from(bytes.len())
            .map_err(|_| "test: bytes exceed u16".to_string())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    buf.extend_from_slice(&bytes);
    buf
}

fn dataset_element_implicit(tag: Tag, value: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(
        &u32::try_from(value.len())
            .map_err(|_| "test: value exceeds u32".to_string())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    buf.extend_from_slice(value);
    buf
}

fn dataset_element_implicit_undefined_length(tag: Tag) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(&u32::MAX.to_le_bytes());
    buf
}

fn item_tag_with_length(len: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&dicom_io::TAG_ITEM.0.to_le_bytes());
    buf.extend_from_slice(&dicom_io::TAG_ITEM.1.to_le_bytes());
    buf.extend_from_slice(&len.to_le_bytes());
    buf
}

fn sequence_delim_tag() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&dicom_io::TAG_SEQ_DELIM.0.to_le_bytes());
    buf.extend_from_slice(&dicom_io::TAG_SEQ_DELIM.1.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
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
            buf.extend_from_slice(
                &u32::try_from(bytes.len())
                    .map_err(|_| "test: bytes exceed u32".to_string())
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
        }
        _ => {
            buf.extend_from_slice(
                &u16::try_from(bytes.len())
                    .map_err(|_| "test: bytes exceed u16".to_string())
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
        }
    }
    buf.extend_from_slice(&bytes);
    buf
}

fn u16_bytes(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

fn minimal_sc_dataset_explicit(sop_class_uid: &str) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        sop_class_uid.as_bytes(),
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
        b"2.3.4",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0002),
        *b"US",
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0004),
        *b"CS",
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0010),
        *b"US",
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0011),
        *b"US",
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0100),
        *b"US",
        &u16_bytes(16),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &u16_bytes(12),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &u16_bytes(11),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &u16_bytes(0),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[0u8],
    ));
    dataset
}

fn minimal_enhanced_dataset_explicit(sop_class_uid: &str) -> Vec<u8> {
    let mut dataset = minimal_sc_dataset_explicit(sop_class_uid);
    dataset.extend_from_slice(&dataset_element_explicit(Tag(0x0028, 0x0008), *b"IS", b"1"));
    dataset.extend_from_slice(&dataset_element_explicit(Tag(0x5200, 0x9229), *b"SQ", &[]));
    dataset.extend_from_slice(&dataset_element_explicit(Tag(0x5200, 0x9230), *b"SQ", &[]));
    dataset
}

fn assert_dataset_accepted(dataset: Vec<u8>) {
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let parsed = reader.read_dataset().expect("dataset");
    assert!(parsed.get(Tag(0x0008, 0x0016)).is_some());
}

fn minimal_rt_dose_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
    let mut dataset = Vec::new();
    let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
        if Some(tag) != missing {
            dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
        }
    };

    push(
        Tag(0x0008, 0x0016),
        *b"UI",
        dicom_io::SOP_CLASS_RT_DOSE.as_bytes(),
    );
    push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
    push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
    push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
    push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
    push(Tag(0x0028, 0x0002), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0004), *b"CS", b"MONOCHROME2");
    push(Tag(0x0028, 0x0010), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0011), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0100), *b"US", &u16_bytes(16));
    push(Tag(0x0028, 0x0101), *b"US", &u16_bytes(16));
    push(Tag(0x0028, 0x0102), *b"US", &u16_bytes(15));
    push(Tag(0x0028, 0x0103), *b"US", &u16_bytes(0));
    push(Tag(0x0028, 0x0008), *b"IS", b"1");
    push(Tag(0x0028, 0x0030), *b"DS", b"1\\1");
    push(Tag(0x0020, 0x0032), *b"DS", b"0\\0\\0");
    push(Tag(0x0020, 0x0037), *b"DS", b"1\\0\\0\\0\\1\\0");
    push(Tag(0x7FE0, 0x0010), *b"OW", &[0, 0]);

    dataset
}

fn minimal_rt_structure_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
    let mut dataset = Vec::new();
    let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
        if Some(tag) != missing {
            dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
        }
    };

    push(
        Tag(0x0008, 0x0016),
        *b"UI",
        dicom_io::SOP_CLASS_RT_STRUCTURE.as_bytes(),
    );
    push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
    push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
    push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
    push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
    push(Tag(0x3006, 0x0039), *b"SQ", &[]);

    dataset
}

fn minimal_rt_plan_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
    let mut dataset = Vec::new();
    let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
        if Some(tag) != missing {
            dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
        }
    };

    push(
        Tag(0x0008, 0x0016),
        *b"UI",
        dicom_io::SOP_CLASS_RT_PLAN.as_bytes(),
    );
    push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
    push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
    push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
    push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
    push(Tag(0x300C, 0x0060), *b"SQ", &[]);

    dataset
}

fn minimal_seg_dataset_explicit(missing: Option<Tag>) -> Vec<u8> {
    let mut dataset = Vec::new();
    let mut push = |tag: Tag, vr: [u8; 2], value: &[u8]| {
        if Some(tag) != missing {
            dataset.extend_from_slice(&dataset_element_explicit(tag, vr, value));
        }
    };

    push(Tag(0x0008, 0x0016), *b"UI", dicom_io::SOP_CLASS_SEG.as_bytes());
    push(Tag(0x0008, 0x0018), *b"UI", b"1.2.3.4.5");
    push(Tag(0x0020, 0x000D), *b"UI", b"1.2.3");
    push(Tag(0x0020, 0x000E), *b"UI", b"2.3.4");
    push(Tag(0x0020, 0x0052), *b"UI", b"9.8.7");
    push(Tag(0x0028, 0x0002), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0004), *b"CS", b"MONOCHROME2");
    push(Tag(0x0028, 0x0010), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0011), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0100), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0101), *b"US", &u16_bytes(1));
    push(Tag(0x0028, 0x0102), *b"US", &u16_bytes(0));
    push(Tag(0x0028, 0x0103), *b"US", &u16_bytes(0));
    push(Tag(0x0062, 0x0001), *b"CS", b"BINARY");
    push(Tag(0x0062, 0x0004), *b"US", &u16_bytes(1));
    push(Tag(0x7FE0, 0x0010), *b"OB", &[0x01]);

    dataset
}

fn minimal_sc_dataset_explicit_missing_study_uid(sop_class_uid: &str) -> Vec<u8> {
    let dataset = minimal_sc_dataset_explicit(sop_class_uid);
    let mut stripped = Vec::new();
    let study_tag = Tag(0x0020, 0x000D);
    let mut offset = 0usize;
    while offset + 8 <= dataset.len() {
        let group = u16::from_le_bytes([dataset[offset], dataset[offset + 1]]);
        let element = u16::from_le_bytes([dataset[offset + 2], dataset[offset + 3]]);
        let tag = Tag(group, element);
        let vr = [dataset[offset + 4], dataset[offset + 5]];
        let (len, header_len) = match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                let len_bytes = &dataset[offset + 8..offset + 12];
                (
                    u32::from_le_bytes([
                        len_bytes[0],
                        len_bytes[1],
                        len_bytes[2],
                        len_bytes[3],
                    ]),
                    12,
                )
            }
            _ => {
                let len_bytes = &dataset[offset + 6..offset + 8];
                (
                    u32::from(u16::from_le_bytes([len_bytes[0], len_bytes[1]])),
                    8,
                )
            }
        };
        let len_usize = usize::try_from(len).unwrap_or(usize::MAX);
        let element_end = offset + header_len + len_usize;
        if tag != study_tag {
            stripped.extend_from_slice(&dataset[offset..element_end]);
        }
        offset = element_end;
    }
    stripped
}

fn minimal_sc_dataset_implicit(sop_class_uid: &str) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0008, 0x0016),
        sop_class_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0008, 0x0018), b"1.2.3.4.5"));
    dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0020, 0x000D), b"1.2.3"));
    dataset.extend_from_slice(&dataset_element_implicit(Tag(0x0020, 0x000E), b"2.3.4"));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0002),
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0004),
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0010),
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0011),
        &u16_bytes(1),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0100),
        &u16_bytes(16),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0101),
        &u16_bytes(12),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0102),
        &u16_bytes(11),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(
        Tag(0x0028, 0x0103),
        &u16_bytes(0),
    ));
    dataset.extend_from_slice(&dataset_element_implicit(Tag(0x7FE0, 0x0010), &[0u8]));
    dataset
}

fn tmp_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let mut path = std::env::temp_dir();
    path.push(format!("diccy-{name}-{nonce}"));
    path
}

#[test]
fn p10reader_defaults_to_limits() {
    // REQ-SEC-402: defaults are enforced unless explicitly overridden.
    let reader = P10Reader::new(EmptySource);
    assert_eq!(reader.limits(), &Limits::default());
}

#[test]
fn p10reader_uses_explicit_limits() {
    // REQ-SEC-402: explicit limits override defaults.
    let limits = Limits::builder().max_input_bytes(1).build().unwrap();
    let reader = P10Reader::with_limits(EmptySource, limits.clone());
    assert_eq!(reader.limits(), &limits);
}

#[test]
fn raw_mode_fallback_is_explicit_and_fails_closed_by_default() {
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");

    let mut strict = P10Reader::new(BytesSource::new(dataset.clone()));
    let err = strict
        .read_dataset()
        .expect_err("strict mode must reject raw bytes");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));

    let mut raw_reader = P10Reader::with_limits_and_options(
        BytesSource::new(dataset),
        Limits::default(),
        ReaderConfig {
            raw_mode: true,
            capture_warnings: true,
            capture_debug_offsets: true,
        },
    );
    let diagnostics = raw_reader
        .read_dataset_with_diagnostics()
        .expect("raw-mode parse");
    assert!(diagnostics.raw_mode_used);
    assert!(diagnostics
        .warnings
        .iter()
        .any(|warning| warning.code == "DVF.IO.RAW_MODE_META_FALLBACK"));
    assert!(!diagnostics.debug_meta.is_empty());
    assert!(diagnostics.dataset.get(Tag(0x0008, 0x0016)).is_some());
}

#[test]
fn charset_warnings_are_structured_and_deterministic() {
    let mut dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0005),
        *b"CS",
        b"ISO_IR 100",
    ));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::with_limits_and_options(
        BytesSource::new(bytes),
        Limits::default(),
        ReaderConfig {
            raw_mode: false,
            capture_warnings: true,
            capture_debug_offsets: false,
        },
    );
    let diagnostics = reader
        .read_dataset_with_diagnostics()
        .expect("dataset with charset warning");
    let codes: Vec<&str> = diagnostics
        .warnings
        .iter()
        .map(|warning| warning.code)
        .collect();
    let mut sorted = codes.clone();
    sorted.sort_unstable();
    assert_eq!(codes, sorted);
    assert!(codes.contains(&"DVF.IO.CHARSET_UNSUPPORTED"));
}

#[test]
fn read_meta_rejects_missing_prefix() {
    // REQ-IO-010: malformed P10 prefix is rejected.
    let mut reader = P10Reader::new(BytesSource::new(vec![0u8; 10]));
    let err = reader.read_meta().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    assert_eq!(err.code(), "DVF.DICOM.DECODE_ERROR");
}

#[test]
fn read_meta_reads_transfer_syntax() {
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &[]);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let meta = reader.read_meta().expect("meta");
    assert_eq!(meta.transfer_syntax_uid, TS_EXPLICIT_VR_LE);
}

#[test]
fn read_dataset_respects_max_input_bytes() {
    // REQ-API-210: limit violations return LimitExceeded.
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &[]);
    let limits = Limits::builder().max_input_bytes(4).build().unwrap();
    let mut reader = P10Reader::with_limits(BytesSource::new(bytes), limits);
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn read_dataset_supports_implicit_vr() {
    // REQ-CONF-002/003/020: implicit VR path still enforces envelope validation.
    let dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
}

#[test]
fn read_dataset_accepts_implicit_undefined_length_sequence() {
    // REQ-IO-013: implicit-VR undefined-length sequences with item/delimiter tokens decode.
    let mut dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
    dataset.extend_from_slice(&dataset_element_implicit_undefined_length(Tag(
        0x0008, 0x1115,
    )));
    let item_payload = dataset_element_implicit(Tag(0x0008, 0x1155), b"1.2.3");
    dataset.extend_from_slice(&item_tag_with_length(
        u32::try_from(item_payload.len())
            .map_err(|_| "test: payload exceeds u32".to_string())
            .unwrap_or_default(),
    ));
    dataset.extend_from_slice(&item_payload);
    dataset.extend_from_slice(&sequence_delim_tag());

    let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let parsed = reader.read_dataset().expect("dataset");
    let seq = parsed.get(Tag(0x0008, 0x1115)).expect("sequence element");
    match seq.value() {
        dicom_core::Value::Sequence(items) => assert_eq!(items.len(), 1),
        _ => panic!("expected sequence value"),
    }
}

#[test]
fn read_dataset_rejects_implicit_undefined_length_non_sequence_container() {
    // REQ-IO-013: undefined-length non-sequence containers fail closed.
    let mut dataset = minimal_sc_dataset_implicit("1.2.840.10008.5.1.4.1.1.7");
    dataset.extend_from_slice(&dataset_element_implicit_undefined_length(Tag(
        0x0010, 0x0010,
    )));
    dataset.extend_from_slice(&[0u8; 8]);
    let bytes = build_p10(TS_IMPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected decode error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn read_dataset_enforces_element_limit() {
    // REQ-SEC-404: limits are enforced before allocating element values.
    let value = vec![0u8; 8];
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&Tag(0x0010, 0x0010).0.to_le_bytes());
    dataset.extend_from_slice(&Tag(0x0010, 0x0010).1.to_le_bytes());
    dataset.extend_from_slice(b"LO");
    dataset.extend_from_slice(
        &u16::try_from(value.len())
            .map_err(|_| "test: value exceeds u16".to_string())
            .unwrap_or_default()
            .to_le_bytes(),
    );
    dataset.extend_from_slice(&value);

    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let limits = Limits::builder().max_element_vl_bytes(4).build().unwrap();
    let mut reader = P10Reader::with_limits(BytesSource::new(bytes), limits);
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
}

#[test]
fn read_dataset_rejects_unsupported_transfer_syntax() {
    // REQ-CONF-002: unsupported Transfer Syntax is rejected early.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10("1.2.3.4.5.6.7.8", &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
fn read_dataset_rejects_explicit_vr_big_endian_transfer_syntax() {
    // REQ-CONF-002: out-of-envelope Big Endian transfer syntax must fail closed.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10("1.2.840.10008.1.2.2", &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(not(feature = "tier1-deflate"))]
fn read_dataset_rejects_deflated_transfer_syntax_without_feature() {
    // REQ-TS-204: deflated transfer syntax is deferred unless tier1-deflate is enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_DEFLATED_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
fn read_dataset_accepts_rle_transfer_syntax() {
    // REQ-TS-202: RLE transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_RLE_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
fn read_dataset_accepts_jpeg_baseline_transfer_syntax() {
    // REQ-TS-201: JPEG Baseline transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_JPEG_BASELINE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(feature = "tier1-deflate")]
fn read_dataset_accepts_deflated_transfer_syntax() {
    // REQ-TS-204: deflated transfer syntax is accepted when feature enabled.
    use flate2::{write::ZlibEncoder, Compression};
    use std::io::Write;

    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&dataset).expect("deflate write");
    let compressed = encoder.finish().expect("deflate finish");
    let bytes = build_p10(dicom_io::TS_DEFLATED_EXPLICIT_VR_LE, &compressed);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn read_dataset_accepts_jpegls_transfer_syntax() {
    // REQ-TS-203: codec pack transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_JPEGLS_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(feature = "codec-jpegls")]
fn read_dataset_accepts_jpegls_near_lossless_transfer_syntax() {
    // REQ-TS-203: near-lossless JPEG-LS transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_JPEGLS_NEAR_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(not(feature = "codec-jpegls"))]
fn read_dataset_rejects_jpegls_transfer_syntax_without_feature() {
    // REQ-TS-203: JPEG-LS transfer syntax is deferred unless codec-jpegls is enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_JPEGLS_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(feature = "codec-j2k")]
fn read_dataset_accepts_jpeg2000_transfer_syntax() {
    // REQ-TS-203: codec pack transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_JPEG2000_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(feature = "codec-j2k")]
fn read_dataset_accepts_jpeg2000_lossy_transfer_syntax() {
    // REQ-TS-203: lossy JPEG 2000 transfer syntax parsed as explicit VR LE.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_JPEG2000_LOSSY, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(not(feature = "codec-j2k"))]
fn read_dataset_rejects_jpeg2000_transfer_syntax_without_feature() {
    // REQ-TS-203: JPEG 2000 transfer syntax is deferred unless codec-j2k is enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_JPEG2000_LOSSLESS, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(not(feature = "codec-mpeg2"))]
fn read_dataset_rejects_mpeg2_transfer_syntax_without_feature() {
    // REQ-TS-205: MPEG-2 transfer syntax requires feature gate.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_MPEG2_MPML, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(feature = "codec-mpeg2")]
fn read_dataset_accepts_mpeg2_transfer_syntax_when_enabled() {
    // REQ-TS-205: MPEG-2 transfer syntax accepted when feature enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_MPEG2_MPML, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(not(feature = "codec-h264"))]
fn read_dataset_rejects_h264_transfer_syntax_without_feature() {
    // REQ-TS-205: H.264 transfer syntax requires feature gate.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_H264_HP41, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(feature = "codec-h264")]
fn read_dataset_accepts_h264_transfer_syntax_when_enabled() {
    // REQ-TS-205: H.264 transfer syntax accepted when feature enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_H264_HP41, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
#[cfg(not(feature = "codec-hevc"))]
fn read_dataset_rejects_hevc_transfer_syntax_without_feature() {
    // REQ-TS-205: HEVC transfer syntax requires feature gate.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_HEVC_MP51, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::UnsupportedTransferSyntax { .. }
    ));
}

#[test]
#[cfg(feature = "codec-hevc")]
fn read_dataset_accepts_hevc_transfer_syntax_when_enabled() {
    // REQ-TS-205: HEVC transfer syntax accepted when feature enabled.
    let dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(dicom_io::TS_HEVC_MP51, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x7FE0, 0x0010)).is_some());
}

#[test]
fn read_dataset_rejects_unsupported_sop_class() {
    // REQ-CONF-002: unsupported SOP Class is rejected.
    let dataset = minimal_sc_dataset_explicit("1.2.3.4.5.6.7");
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_rejects_deferred_enhanced_ct_sop_when_pack_disabled() {
    // REQ-SOP-301: deferred pack SOP classes must fail closed when pack is not enabled.
    if dicom_core::capabilities().pack_enhanced() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_ENHANCED_CT);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_rejects_enhanced_mr_sop_when_pack_disabled() {
    // REQ-SOP-301: pack SOP classes must fail closed when pack is not enabled.
    if dicom_core::capabilities().pack_enhanced() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_ENHANCED_MR);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_enhanced_ct_mr_sops_when_pack_enabled() {
    // REQ-CONF-084/REQ-SOP-301: enhanced SOP classes are accepted when pack-enhanced is enabled.
    if !dicom_core::capabilities().pack_enhanced() {
        return;
    }
    assert_dataset_accepted(minimal_enhanced_dataset_explicit(
        dicom_io::SOP_CLASS_ENHANCED_CT,
    ));
    assert_dataset_accepted(minimal_enhanced_dataset_explicit(
        dicom_io::SOP_CLASS_ENHANCED_MR,
    ));
}

#[test]
fn read_dataset_rejects_us_sops_when_pack_disabled() {
    // REQ-SOP-301: US SOP classes fail closed when pack-us is disabled.
    if dicom_core::capabilities().pack_us() {
        return;
    }
    for sop in [dicom_io::SOP_CLASS_US, dicom_io::SOP_CLASS_US_MF] {
        let dataset = minimal_sc_dataset_explicit(sop);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }
}

#[test]
fn read_dataset_accepts_us_sops_when_pack_enabled() {
    // REQ-CONF-085/REQ-SOP-301: US SOP classes are accepted when pack-us is enabled.
    if !dicom_core::capabilities().pack_us() {
        return;
    }
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_US));
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_US_MF));
}

#[test]
fn read_dataset_rejects_nm_sop_when_pack_disabled() {
    // REQ-SOP-301: NM SOP class fails closed when pack-nm is disabled.
    if dicom_core::capabilities().pack_nm() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_NM);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_nm_sop_when_pack_enabled() {
    // REQ-CONF-085/REQ-SOP-301: NM SOP class is accepted when pack-nm is enabled.
    if !dicom_core::capabilities().pack_nm() {
        return;
    }
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_NM));
}

#[test]
fn read_dataset_rejects_xa_xrf_sops_when_pack_disabled() {
    // REQ-SOP-301: XA/XRF SOP classes fail closed when pack-xa is disabled.
    if dicom_core::capabilities().pack_xa() {
        return;
    }
    for sop in [dicom_io::SOP_CLASS_XA, dicom_io::SOP_CLASS_XRF] {
        let dataset = minimal_sc_dataset_explicit(sop);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }
}

#[test]
fn read_dataset_accepts_xa_xrf_sops_when_pack_enabled() {
    // REQ-CONF-085/REQ-SOP-301: XA/XRF SOP classes are accepted when pack-xa is enabled.
    if !dicom_core::capabilities().pack_xa() {
        return;
    }
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_XA));
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_XRF));
}

#[test]
fn read_dataset_rejects_deferred_pet_sop_when_feature_not_enabled() {
    // REQ-SOP-301: deferred modality SOP classes must fail closed when feature is not enabled.
    if dicom_core::capabilities().modality_pet() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_PET);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_pet_sop_when_feature_enabled() {
    // REQ-CONF-002/REQ-SOP-301: promoted modality SOP classes are accepted when feature is enabled.
    if !dicom_core::capabilities().modality_pet() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_PET);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
}

#[test]
fn read_dataset_rejects_cr_sop_when_feature_not_enabled() {
    // REQ-SOP-301: CR SOP class fails closed when modality-xr is disabled.
    if dicom_core::capabilities().modality_xr() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_CR);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_rejects_dx_presentation_sop_when_feature_not_enabled() {
    // REQ-SOP-301: DX SOP class fails closed when modality-xr is disabled.
    if dicom_core::capabilities().modality_xr() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_DX_PRESENTATION);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_cr_sop_when_feature_enabled() {
    // REQ-CONF-002/REQ-SOP-301: promoted CR SOP class is accepted when modality-xr is enabled.
    if !dicom_core::capabilities().modality_xr() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_CR);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
}

#[test]
fn read_dataset_accepts_dx_presentation_sop_when_feature_enabled() {
    // REQ-CONF-002/REQ-SOP-301: promoted DX SOP class is accepted when modality-xr is enabled.
    if !dicom_core::capabilities().modality_xr() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_DX_PRESENTATION);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let dataset = reader.read_dataset().expect("dataset");
    assert!(dataset.get(Tag(0x0008, 0x0016)).is_some());
}

#[test]
fn read_dataset_rejects_seg_sop_when_pack_disabled() {
    // REQ-SOP-301: SEG SOP class fails closed when pack-seg is disabled.
    if dicom_core::capabilities().pack_seg() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_SEG);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_seg_sop_when_pack_enabled() {
    // REQ-CONF-086/REQ-SOP-301: SEG SOP class is accepted when pack-seg is enabled.
    if !dicom_core::capabilities().pack_seg() {
        return;
    }
    assert_dataset_accepted(minimal_seg_dataset_explicit(None));
}

#[test]
fn read_dataset_rejects_rt_sops_when_pack_disabled() {
    // REQ-SOP-301: RT SOP classes fail closed when pack-rt is disabled.
    if dicom_core::capabilities().pack_rt() {
        return;
    }
    for sop in [
        dicom_io::SOP_CLASS_RT_DOSE,
        dicom_io::SOP_CLASS_RT_STRUCTURE,
        dicom_io::SOP_CLASS_RT_PLAN,
    ] {
        let dataset = minimal_sc_dataset_explicit(sop);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }
}

#[test]
fn read_dataset_accepts_rt_sops_when_pack_enabled() {
    // REQ-CONF-087/REQ-SOP-301: RT SOP classes are accepted when pack-rt is enabled.
    if !dicom_core::capabilities().pack_rt() {
        return;
    }
    assert_dataset_accepted(minimal_rt_dose_dataset_explicit(None));
    assert_dataset_accepted(minimal_rt_structure_dataset_explicit(None));
    assert_dataset_accepted(minimal_rt_plan_dataset_explicit(None));
}

#[test]
fn read_dataset_rejects_sr_sops_when_pack_disabled() {
    // REQ-SOP-301: SR SOP classes fail closed when pack-sr is disabled.
    if dicom_core::capabilities().pack_sr() {
        return;
    }
    for sop in [
        dicom_io::SOP_CLASS_SR_BASIC_TEXT,
        dicom_io::SOP_CLASS_SR_COMPREHENSIVE,
    ] {
        let dataset = minimal_sc_dataset_explicit(sop);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }
}

#[test]
fn read_dataset_accepts_sr_sops_when_pack_enabled() {
    // REQ-CONF-088/REQ-SOP-301: SR SOP classes are accepted when pack-sr is enabled.
    if !dicom_core::capabilities().pack_sr() {
        return;
    }
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_SR_BASIC_TEXT));
    assert_dataset_accepted(minimal_sc_dataset_explicit(
        dicom_io::SOP_CLASS_SR_COMPREHENSIVE,
    ));
}

#[test]
fn read_dataset_rejects_gsps_sop_when_feature_disabled() {
    // REQ-SOP-301: GSPS SOP class fails closed when gsps feature is disabled.
    if dicom_core::capabilities().gsps() {
        return;
    }
    let dataset = minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_GSPS);
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
}

#[test]
fn read_dataset_accepts_gsps_sop_when_feature_enabled() {
    // REQ-CONF-089/REQ-SOP-301: GSPS SOP class is accepted when gsps is enabled.
    if !dicom_core::capabilities().gsps() {
        return;
    }
    assert_dataset_accepted(minimal_sc_dataset_explicit(dicom_io::SOP_CLASS_GSPS));
}

#[test]
fn read_dataset_rejects_mammography_sops_even_when_modality_mg_enabled() {
    // REQ-CONF-002: MG SOP classes remain out-of-envelope in the IO conformance matrix and fail closed.
    if !dicom_core::capabilities().modality_mg() {
        return;
    }
    for sop in [
        "1.2.840.10008.5.1.4.1.1.1.2",
        "1.2.840.10008.5.1.4.1.1.1.2.1",
    ] {
        let dataset = minimal_sc_dataset_explicit(sop);
        let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
        let mut reader = P10Reader::new(BytesSource::new(bytes));
        let err = reader.read_dataset().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::UnsupportedSopClass { .. }));
    }
}

#[test]
fn read_dataset_rejects_missing_required_tag() {
    // REQ-CONF-020: missing required tags fail closed.
    let dataset = minimal_sc_dataset_explicit_missing_study_uid("1.2.840.10008.5.1.4.1.1.7");
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    let kind = err.kind().clone();
    assert!(
        matches!(kind, ErrorKind::MissingRequiredTag { .. }),
        "expected MissingRequiredTag, got {:?}",
        kind
    );
}

#[test]
fn read_dataset_rejects_rt_dose_missing_geometry() {
    // REQ-CONF-087
    if !dicom_core::capabilities().pack_rt() {
        return;
    }
    let dataset = minimal_rt_dose_dataset_explicit(Some(Tag(0x0020, 0x0037)));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn read_dataset_rejects_rt_structure_missing_contours() {
    // REQ-CONF-003
    if !dicom_core::capabilities().pack_rt() {
        return;
    }
    let dataset = minimal_rt_structure_dataset_explicit(Some(Tag(0x3006, 0x0039)));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn read_dataset_rejects_rt_plan_missing_reference() {
    // REQ-CONF-003
    if !dicom_core::capabilities().pack_rt() {
        return;
    }
    let dataset = minimal_rt_plan_dataset_explicit(Some(Tag(0x300C, 0x0060)));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn read_dataset_rejects_seg_missing_frame_of_reference() {
    // REQ-CONF-086
    if !dicom_core::capabilities().pack_seg() {
        return;
    }
    let dataset = minimal_seg_dataset_explicit(Some(Tag(0x0020, 0x0052)));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
}

#[test]
fn read_dataset_rejects_float_pixel_data() {
    // REQ-CONF-070: Float Pixel Data is out-of-envelope by default.
    let mut dataset = minimal_sc_dataset_explicit("1.2.840.10008.5.1.4.1.1.7");
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0008),
        *b"OF",
        &[0u8, 1u8, 2u8, 3u8],
    ));
    let bytes = build_p10(TS_EXPLICIT_VR_LE, &dataset);
    let mut reader = P10Reader::new(BytesSource::new(bytes));
    let err = reader.read_dataset().expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[test]
fn file_source_rejects_symlink() {
    // REQ-SEC-426..428: symlinks are rejected by default.
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let target_path = tmp_path("dicom-io-target");
        let link_path = tmp_path("dicom-io-link");
        fs::write(&target_path, b"test").expect("write target");
        symlink(&target_path, &link_path).expect("symlink");
        let mut reader = P10Reader::new(FileSource::new(&link_path));
        let err = reader.read_meta().expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::IoError { .. }));
        let _ = fs::remove_file(&link_path);
        let _ = fs::remove_file(&target_path);
    }
}
