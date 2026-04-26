use dicom_core::{ErrorKind, Limits, Tag};
use dicom_storage::{extract_study_uid, IngestOutcome, Storage};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const SOP_CLASS_SC: &str = "1.2.840.10008.5.1.4.1.1.7";
const TS_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

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

fn minimal_sc_dataset_explicit(
    study_uid: &str,
    series_uid: &str,
    sop_uid: &str,
    pixel: u8,
) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        SOP_CLASS_SC.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        sop_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000D),
        *b"UI",
        study_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000E),
        *b"UI",
        series_uid.as_bytes(),
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
        &[pixel],
    ));
    dataset
}

fn sample_p10(study: &str, series: &str, sop: &str, pixel: u8) -> Vec<u8> {
    let dataset = minimal_sc_dataset_explicit(study, series, sop, pixel);
    build_p10(TS_EXPLICIT_VR_LE, &dataset)
}

fn temp_wal_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("rdvf_hi_storage_{name}_{nonce}.wal"))
}

#[test]
fn ingest_runtime_distinguishes_inserted_duplicate_and_integrity_conflict_outcomes() {
    // REQ-HI-351, REQ-HI-352
    let mut storage = Storage::new(Limits::default());
    let first = sample_p10("1.2.840.101", "1.2.840.101.1", "1.2.840.101.1.1", 0);
    let inserted = storage.ingest_bytes(first.clone()).expect("insert");
    let duplicate = storage.ingest_bytes(first).expect("duplicate");
    match inserted {
        IngestOutcome::Inserted { .. } => {}
        _ => panic!("expected inserted outcome"),
    }
    match duplicate {
        IngestOutcome::Duplicate { .. } => {}
        _ => panic!("expected duplicate outcome"),
    }

    let conflict = sample_p10("1.2.840.999", "1.2.840.101.1", "1.2.840.101.1.1", 1);
    let err = storage
        .ingest_bytes(conflict)
        .expect_err("mismatched SOP/hash tuple must fail closed");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
    assert_eq!(storage.log().len(), 1);
}

#[test]
fn listing_and_tuple_lookup_are_deterministic_and_fail_closed() {
    // REQ-HI-299, REQ-HI-353
    let mut storage = Storage::new(Limits::default());
    let a = sample_p10("1.2.840.200", "1.2.840.200.1", "1.2.840.200.1.1", 1);
    let b = sample_p10("1.2.840.200", "1.2.840.200.1", "1.2.840.200.1.2", 2);
    storage.ingest_bytes(a.clone()).expect("insert a");
    storage.ingest_bytes(b.clone()).expect("insert b");

    let datasets = storage.datasets().expect("datasets");
    assert_eq!(datasets.len(), 2);
    assert_eq!(
        datasets[0].get_uid(Tag(0x0008, 0x0018)),
        Some("1.2.840.200.1.1")
    );
    assert_eq!(
        datasets[1].get_uid(Tag(0x0008, 0x0018)),
        Some("1.2.840.200.1.2")
    );

    let found = storage
        .instance_bytes("1.2.840.200", "1.2.840.200.1", "1.2.840.200.1.1")
        .expect("tuple bytes");
    assert_eq!(found, a.as_slice());
    assert!(storage
        .instance_bytes("1.2.840.999", "1.2.840.200.1", "1.2.840.200.1.1")
        .is_none());

    let extracted = extract_study_uid(&b, &Limits::default()).expect("study uid");
    assert_eq!(extracted, "1.2.840.200");
}

#[test]
fn durable_runtime_recovery_and_corruption_handling_follow_fail_closed_contracts() {
    // REQ-HI-300, REQ-HI-354, REQ-HI-355
    let wal_path = temp_wal_path("recover");
    let mut storage = Storage::open(Limits::default(), &wal_path).expect("open durable");
    storage
        .ingest_bytes(sample_p10(
            "1.2.840.300",
            "1.2.840.300.1",
            "1.2.840.300.1.1",
            7,
        ))
        .expect("ingest");
    drop(storage);

    let reopened = Storage::open(Limits::default(), &wal_path).expect("reopen");
    assert_eq!(reopened.index().total_instances(), 1);
    assert_eq!(reopened.log().len(), 1);
    assert!(reopened.persistence_path().is_some());
    let _ = fs::remove_file(&wal_path);

    let corrupt_path = temp_wal_path("corrupt");
    fs::write(&corrupt_path, b"BAD!").expect("corrupt wal");
    let err = Storage::open(Limits::default(), &corrupt_path).expect_err("must fail closed");
    assert!(matches!(
        err.kind(),
        ErrorKind::IntegrityError { .. } | ErrorKind::IoError { .. }
    ));
    let _ = fs::remove_file(&corrupt_path);
}
