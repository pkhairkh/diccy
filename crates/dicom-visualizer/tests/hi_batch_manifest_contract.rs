use dicom_core::Tag;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin_path() -> PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_dicom-visualizer") {
        return PathBuf::from(path);
    }
    let target_dir = std::env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target_dir.join("debug").join("dicom-visualizer")
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("rdvf_hi_batch_{label}_{nonce}"))
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

fn sample_p10(study_uid: &str, series_uid: &str, instance_uid: &str) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"LO",
        b"1.2.840.10008.5.1.4.1.1.20",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        instance_uid.as_bytes(),
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
        &8u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &8u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &7u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &0u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[17u8],
    ));

    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), "1.2.840.10008.1.2.1"));
    bytes.extend_from_slice(&dataset);
    bytes
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn json_string(doc: &str, key: &str) -> String {
    let needle = format!("\"{key}\":\"");
    let start = doc.find(&needle).expect("json key") + needle.len();
    let tail = &doc[start..];
    let end = tail.find('"').expect("json value end");
    tail[..end].to_string()
}

fn json_number(doc: &str, key: &str) -> usize {
    let needle = format!("\"{key}\":");
    let start = doc.find(&needle).expect("json key") + needle.len();
    let tail = &doc[start..];
    let end = tail
        .find(|ch: char| !ch.is_ascii_digit())
        .expect("json number end");
    tail[..end].parse::<usize>().expect("json number")
}

#[test]
fn batch_manifest_and_index_are_deterministic_escaped_and_hashed() {
    // REQ-HI-279, REQ-HI-311, REQ-HI-312, REQ-HI-313, REQ-HI-314, REQ-HI-315, REQ-HI-356, REQ-HI-359
    let root = unique_temp_dir("manifest");
    let input = root.join("input");
    let output = root.join("output");
    fs::create_dir_all(&input).expect("input dir");
    fs::create_dir_all(&output).expect("output dir");

    let ok_name = "001_ok_<img,\"evil\">.dcm";
    let bad_name = "002_bad_<script>.dcm";
    fs::write(
        input.join(ok_name),
        sample_p10("1.2.840.123", "1.2.840.123.1", "1.2.840.123.1.1"),
    )
    .expect("write valid p10");
    fs::write(input.join(bad_name), b"not-a-dicom").expect("write invalid p10");

    let output_run = Command::new(bin_path())
        .arg(&input)
        .arg(&output)
        .arg("--limit")
        .arg("10")
        .env("DICOM_ENVELOPE_VERSION", "envelope-hi-test")
        .output()
        .expect("run binary");
    assert!(
        output_run.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output_run.stdout),
        String::from_utf8_lossy(&output_run.stderr)
    );

    let stdout = String::from_utf8_lossy(&output_run.stdout);
    assert!(stdout.contains("recursive=true"));
    assert!(stdout.contains("max_files=10"));
    assert!(stdout.contains("write_index=true"));

    let manifest_path = output.join("manifest.csv");
    let manifest_text = fs::read_to_string(&manifest_path).expect("manifest");
    let lines = manifest_text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 3, "header + 2 records");
    assert!(
        lines[1].starts_with(
            "\"001_ok_<img,\"\"evil\"\">.dcm\",00000_001_ok__img__evil___dcm.png,ok,1,1,"
        ),
        "unexpected ok row: {}",
        lines[1]
    );
    assert!(lines[2].starts_with("002_bad_<script>.dcm,,error,,,"));
    assert!(lines[2].contains("failed:"));

    let index_path = output.join("index.html");
    let index_text = fs::read_to_string(&index_path).expect("index");
    assert!(index_text.contains("alt=\"001_ok_&lt;img,&quot;evil&quot;&gt;.dcm\""));
    assert!(!index_text.contains("alt=\"001_ok_<img,\"evil\">.dcm\""));
    assert!(!index_text.contains("<script>"));

    let manifest_bytes = fs::read(&manifest_path).expect("manifest bytes");
    let index_bytes = fs::read(&index_path).expect("index bytes");
    let integrity_text =
        fs::read_to_string(output.join("manifest.integrity.json")).expect("integrity");
    assert_eq!(
        json_string(&integrity_text, "format"),
        "hi_artifact_integrity_v1"
    );
    assert_eq!(json_string(&integrity_text, "hash_algorithm"), "sha256");
    assert_eq!(
        json_string(&integrity_text, "manifest_file"),
        "manifest.csv"
    );
    assert_eq!(json_string(&integrity_text, "index_file"), "index.html");
    assert_eq!(
        json_string(&integrity_text, "envelope_version"),
        "envelope-hi-test"
    );
    assert_eq!(
        json_string(&integrity_text, "manifest_sha256"),
        sha256_hex(&manifest_bytes)
    );
    assert_eq!(
        json_string(&integrity_text, "index_sha256"),
        sha256_hex(&index_bytes)
    );
    assert_eq!(json_number(&integrity_text, "record_count"), 2);
    assert_eq!(json_number(&integrity_text, "exported_count"), 1);
    assert_eq!(json_number(&integrity_text, "failed_count"), 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn no_index_mode_disables_gallery_output() {
    // REQ-HI-314, REQ-HI-357
    let root = unique_temp_dir("no_index");
    let input = root.join("input");
    let output = root.join("output");
    fs::create_dir_all(&input).expect("input dir");
    fs::create_dir_all(&output).expect("output dir");

    fs::write(
        input.join("001_ok.dcm"),
        sample_p10("1.2.840.222", "1.2.840.222.1", "1.2.840.222.1.1"),
    )
    .expect("write valid p10");

    let output_run = Command::new(bin_path())
        .arg(&input)
        .arg(&output)
        .arg("--limit")
        .arg("1")
        .arg("--no-index")
        .output()
        .expect("run binary");
    assert!(
        output_run.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output_run.stdout),
        String::from_utf8_lossy(&output_run.stderr)
    );
    let stdout = String::from_utf8_lossy(&output_run.stdout);
    assert!(stdout.contains("write_index=false"));
    assert!(!output.join("index.html").exists());

    let integrity_text =
        fs::read_to_string(output.join("manifest.integrity.json")).expect("integrity");
    assert_eq!(json_string(&integrity_text, "index_file"), "");
    assert_eq!(json_string(&integrity_text, "index_sha256"), "");

    let _ = fs::remove_dir_all(&root);
}
