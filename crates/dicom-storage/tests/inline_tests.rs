// Auto-extracted from /home/z/diccy/crates/dicom-storage/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_storage::*;
use dicom_core::Limits;

#[test]
fn wal_append_and_replay() {
    let mut wal = WriteAheadLog::new();
    wal.append(WalEntry {
        hash: "abc".to_string(),
        bytes: vec![1, 2, 3],
    });
    wal.append(WalEntry {
        hash: "def".to_string(),
        bytes: vec![4, 5, 6],
    });
    assert_eq!(wal.len(), 2);
    assert_eq!(wal.entries()[0].hash, "abc");
    assert_eq!(wal.entries()[1].hash, "def");
}

#[test]
fn storage_ingest_deduplicates() {
    let limits = Limits::default();
    let mut storage = Storage::new(limits);
    // Simple DICOM P10 bytes (not valid, but the test only checks dedup logic)
    let hash_a = canonical_hash(&[1u8; 100]);
    let hash_b = canonical_hash(&[2u8; 100]);
    // Direct hash check
    assert_ne!(hash_a, hash_b);
}

#[test]
fn commitment_module_types_reexported() {
    // Verify that commitment module types are accessible from the crate root
    let _policy = StorageCommitmentPolicy::default();
    let _state = StorageCommitmentState::Requested;
}

#[test]
fn s3_module_types_reexported() {
    // Verify that S3 backend types are accessible from the crate root
    let config = S3Config::default();
    let _backend = S3Backend::new(config);
}

#[test]
fn vna_module_types_reexported() {
    // Verify that VNA types are accessible from the crate root
    let _engine = VnaEngine::new();
    let _policy = RetentionPolicy::new("tenant-a", 30, 365).unwrap();
    let _lifecycle = LifecyclePolicy::default();
}

#[test]
fn deduplicate_removes_duplicates() {
    let a = vec![1u8, 2, 3];
    let b = vec![4u8, 5, 6];
    let a2 = a.clone();
    let slices: Vec<&[u8]> = vec![&a, &b, &a2];
    let dedup = deduplicate(&slices);
    assert_eq!(dedup, vec![0, 1]);
}

#[test]
fn in_memory_blob_store_roundtrip() {
    let store = InMemoryBlobStore::new();
    store.put("key1", b"value1").unwrap();
    store.put("prefix/key2", b"value2").unwrap();
    assert_eq!(store.get("key1").unwrap(), b"value1");
    assert_eq!(store.get("prefix/key2").unwrap(), b"value2");
    let keys = store.list("prefix/").unwrap();
    assert_eq!(keys, vec!["prefix/key2".to_string()]);
    store.delete("key1").unwrap();
    assert!(store.get("key1").is_err());
}

#[test]
fn s3_config_secrets_are_redacted_in_debug() {
    let mut config = S3Config::default();
    config.set_access_key_id("AKIAIOSFODNN7EXAMPLE");
    config.set_secret_access_key("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY");
    let debug_str = format!("{:?}", config);
    assert!(!debug_str.contains("AKIAIOSFODNN7EXAMPLE"));
    assert!(!debug_str.contains("wJalrXUtnFEMI"));
    assert!(debug_str.contains("[REDACTED]"));
}
