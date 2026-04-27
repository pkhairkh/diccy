#![deny(missing_docs)]

//! Deterministic storage ingestion with write-ahead logging and deduplication.
//!
//! The storage crate is organized into bounded-context modules:
//! - **commitment** — Storage Commitment lifecycle types and policy
//! - **s3_backend** — S3-compatible object storage backend
//! - **vna** — Vendor Neutral Archive lifecycle management
//!
//! Core WAL-based ingestion remains in this module.

pub mod commitment;
pub mod s3_backend;
pub mod vna;

// Re-export all commitment types for backward compatibility.
pub use commitment::{
    StorageCommitmentEventJob, StorageCommitmentPolicy, StorageCommitmentReferencedInstance,
    StorageCommitmentRequest, StorageCommitmentState,
};

// Re-export all S3 backend types for backward compatibility.
pub use s3_backend::{MultipartUploadResult, S3Backend, S3Config};

// Re-export all VNA types for backward compatibility.
pub use vna::{LifecyclePolicy, RetentionPolicy, StudyLifecycleState, VnaEngine, VnaStudyRecord};

// ===========================================================================
// S10-T3: BlobStore trait for dependency injection
// ===========================================================================

/// Abstract blob storage trait for dependency injection.
///
/// Production code may use [`InMemoryBlobStore`] for testing or
/// [`FileBlobStore`] for filesystem persistence. S3 and other backends
/// can implement this trait as well.
pub trait BlobStore: Send + Sync {
    /// Store data under the given key.
    fn put(&self, key: &str, data: &[u8]) -> std::result::Result<(), Box<dyn std::error::Error>>;
    /// Retrieve data by key.
    fn get(&self, key: &str) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>>;
    /// Delete data by key.
    fn delete(&self, key: &str) -> std::result::Result<(), Box<dyn std::error::Error>>;
    /// List keys with the given prefix.
    fn list(&self, prefix: &str) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>>;
}

// ---------------------------------------------------------------------------
// InMemoryBlobStore — HashMap-backed, for testing
// ---------------------------------------------------------------------------

/// In-memory blob store backed by a [`std::collections::HashMap`].
#[derive(Debug, Default)]
pub struct InMemoryBlobStore {
    data: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl InMemoryBlobStore {
    /// Create a new empty in-memory blob store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlobStore for InMemoryBlobStore {
    fn put(&self, key: &str, data: &[u8]) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.data
            .lock()
            .unwrap()
            .insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn get(&self, key: &str) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.data
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or_else(|| format!("key not found: {key}").into())
    }

    fn delete(&self, key: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.data
            .lock()
            .unwrap()
            .remove(key)
            .map(|_| ())
            .ok_or_else(|| format!("key not found: {key}").into())
    }

    fn list(&self, prefix: &str) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut keys: Vec<String> = self
            .data
            .lock()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        keys.sort();
        Ok(keys)
    }
}

// ---------------------------------------------------------------------------
// FileBlobStore — filesystem-backed
// ---------------------------------------------------------------------------

/// Filesystem-backed blob store. Each key maps to a file under a root directory.
pub struct FileBlobStore {
    root: std::path::PathBuf,
}

impl FileBlobStore {
    /// Create a new file blob store rooted at the given directory.
    pub fn new(root: impl Into<std::path::PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn key_path(&self, key: &str) -> std::path::PathBuf {
        self.root.join(key)
    }
}

impl BlobStore for FileBlobStore {
    fn put(&self, key: &str, data: &[u8]) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = self.key_path(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, data)?;
        Ok(())
    }

    fn get(&self, key: &str) -> std::result::Result<Vec<u8>, Box<dyn std::error::Error>> {
        let path = self.key_path(key);
        let data = std::fs::read(&path).map_err(|e| format!("key not found: {key}: {e}"))?;
        Ok(data)
    }

    fn delete(&self, key: &str) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let path = self.key_path(key);
        std::fs::remove_file(&path).map_err(|e| format!("key not found: {key}: {e}"))?;
        Ok(())
    }

    fn list(&self, prefix: &str) -> std::result::Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut keys = Vec::new();
        if self.root.exists() {
            self.list_recursive(&self.root, prefix, &mut keys)?;
        }
        keys.sort();
        Ok(keys)
    }
}

impl FileBlobStore {
    fn list_recursive(
        &self,
        dir: &std::path::Path,
        prefix: &str,
        keys: &mut Vec<String>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.list_recursive(&path, prefix, keys)?;
            } else if let Ok(rel) = path.strip_prefix(&self.root) {
                let key = rel.to_string_lossy().to_string();
                if key.starts_with(prefix) {
                    keys.push(key);
                }
            }
        }
        Ok(())
    }
}

use commitment::CommitmentEngine;
use dicom_core::{enforce_limit, validate_uid_strict, Error, ErrorKind, Limits, Result, Tag};
use dicom_index::{extract_indexed_instance, Index, InsertOutcome};
use dicom_io::{BytesSource, P10Reader};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const WAL_MAGIC: &[u8; 5] = b"DWAL1";
const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);

/// Single write-ahead log entry storing raw bytes and canonical hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalEntry {
    /// Canonical hash of the stored bytes.
    pub hash: String,
    /// Raw DICOM bytes for replay.
    pub bytes: Vec<u8>,
}

/// Write-ahead log for ingestion replay and recovery.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WriteAheadLog {
    entries: Vec<WalEntry>,
}

impl WriteAheadLog {
    /// Create an empty write-ahead log.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Append an entry, returning its index.
    pub fn append(&mut self, entry: WalEntry) -> usize {
        self.entries.push(entry);
        self.entries.len() - 1
    }

    /// Remove the last entry if present.
    pub fn pop_last(&mut self) -> Option<WalEntry> {
        self.entries.pop()
    }

    /// Return all log entries in order.
    pub fn entries(&self) -> &[WalEntry] {
        &self.entries
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Outcome of a storage ingestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    /// New instance stored.
    Inserted {
        /// Canonical hash of the stored bytes.
        hash: String,
    },
    /// Input deduplicated by hash.
    Duplicate {
        /// Canonical hash of the stored bytes.
        hash: String,
    },
}

/// Storage engine with deterministic hashing and deduplication.
#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    limits: Limits,
    index: Index,
    wal: WriteAheadLog,
    decoded_datasets: Vec<dicom_core::Dataset>,
    by_hash: BTreeMap<String, usize>,
    commitment: CommitmentEngine,
    tombstoned_studies: BTreeSet<String>,
    tombstoned_series: BTreeSet<(String, String)>,
    tombstoned_instances: BTreeSet<(String, String, String)>,
    total_bytes: u64,
    persistence: Option<PathBuf>,
}

impl Storage {
    /// Create a new storage engine with the provided limits.
    pub fn new(limits: Limits) -> Self {
        Self {
            index: Index::new(limits.clone()),
            limits,
            wal: WriteAheadLog::new(),
            decoded_datasets: Vec::new(),
            by_hash: BTreeMap::new(),
            commitment: CommitmentEngine::new(),
            tombstoned_studies: BTreeSet::new(),
            tombstoned_series: BTreeSet::new(),
            tombstoned_instances: BTreeSet::new(),
            total_bytes: 0,
            persistence: None,
        }
    }

    /// Open a storage engine backed by a durable write-ahead log file.
    pub fn open(limits: Limits, wal_path: impl AsRef<Path>) -> Result<Self> {
        let wal_path = wal_path.as_ref().to_path_buf();
        ensure_parent_dir(&wal_path)?;
        let log = load_wal_file(&wal_path)?;
        let mut storage = Storage::from_log(limits, log)?;
        storage.persistence = Some(wal_path);
        Ok(storage)
    }

    /// Ingest a DICOM P10 byte buffer, returning the hash outcome.
    pub fn ingest_bytes(&mut self, bytes: Vec<u8>) -> Result<IngestOutcome> {
        enforce_limit(
            "max_input_bytes",
            bytes.len() as u64,
            self.limits.max_input_bytes(),
        )?;
        let hash = canonical_hash(&bytes);
        if self.by_hash.contains_key(&hash) {
            return Ok(IngestOutcome::Duplicate { hash });
        }
        enforce_limit(
            "max_cache_bytes",
            self.total_bytes + bytes.len() as u64,
            self.limits.max_cache_bytes(),
        )?;

        let dataset = parse_dataset(&bytes, &self.limits)?;
        let instance =
            extract_indexed_instance(&dataset, &self.limits, hash.clone(), bytes.len() as u64)?;

        let entry = WalEntry {
            hash: hash.clone(),
            bytes,
        };
        let rollback_len = if let Some(path) = &self.persistence {
            Some(append_wal_entry(path, &entry)?)
        } else {
            None
        };
        let entry_index = self.wal.append(entry);
        match self.index.insert(instance) {
            Ok(InsertOutcome::Inserted) => {
                self.decoded_datasets.push(dataset);
                self.by_hash.insert(hash.clone(), entry_index);
                self.total_bytes += self.wal.entries()[entry_index].bytes.len() as u64;
                Ok(IngestOutcome::Inserted { hash })
            }
            Ok(InsertOutcome::DuplicateHash) => {
                let _ = self.wal.pop_last();
                if let (Some(path), Some(old_len)) = (&self.persistence, rollback_len) {
                    rollback_wal_append(path, old_len)?;
                }
                Ok(IngestOutcome::Duplicate { hash })
            }
            Err(err) => {
                let _ = self.wal.pop_last();
                if let (Some(path), Some(old_len)) = (&self.persistence, rollback_len) {
                    rollback_wal_append(path, old_len)?;
                }
                Err(err)
            }
        }
    }

    /// Replay a write-ahead log into a fresh storage engine.
    pub fn from_log(limits: Limits, log: WriteAheadLog) -> Result<Self> {
        let mut storage = Storage::new(limits);
        storage.wal = log.clone();
        for (idx, entry) in log.entries().iter().enumerate() {
            let computed = canonical_hash(&entry.bytes);
            if computed != entry.hash {
                return Err(integrity_error("write-ahead log hash mismatch"));
            }
            enforce_limit(
                "max_input_bytes",
                entry.bytes.len() as u64,
                storage.limits.max_input_bytes(),
            )?;
            enforce_limit(
                "max_cache_bytes",
                storage.total_bytes + entry.bytes.len() as u64,
                storage.limits.max_cache_bytes(),
            )?;
            let dataset = parse_dataset(&entry.bytes, &storage.limits)?;
            let instance = extract_indexed_instance(
                &dataset,
                &storage.limits,
                entry.hash.clone(),
                entry.bytes.len() as u64,
            )?;
            match storage.index.insert(instance) {
                Ok(InsertOutcome::Inserted) => {}
                Ok(InsertOutcome::DuplicateHash) => {}
                Err(err) => return Err(err),
            }
            storage.decoded_datasets.push(dataset);
            storage.by_hash.insert(entry.hash.clone(), idx);
            storage.total_bytes += entry.bytes.len() as u64;
        }
        Ok(storage)
    }

    /// Return the current metadata index.
    pub fn index(&self) -> &Index {
        &self.index
    }

    /// Return the write-ahead log.
    pub fn log(&self) -> &WriteAheadLog {
        &self.wal
    }

    /// Return total bytes stored in the log.
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Return the durable write-ahead log path when persistence is enabled.
    pub fn persistence_path(&self) -> Option<&Path> {
        self.persistence.as_deref()
    }

    /// Return decoded datasets in deterministic write-ahead log order.
    pub fn datasets(&self) -> Result<Vec<dicom_core::Dataset>> {
        let mut visible = Vec::new();
        for dataset in &self.decoded_datasets {
            let study_uid = dataset.get_uid(TAG_STUDY_UID);
            let series_uid = dataset.get_uid(TAG_SERIES_UID);
            let instance_uid = dataset.get_uid(TAG_INSTANCE_UID);
            if let (Some(study_uid), Some(series_uid), Some(instance_uid)) =
                (study_uid, series_uid, instance_uid)
            {
                if self.is_tombstoned(study_uid, series_uid, instance_uid) {
                    continue;
                }
            }
            visible.push(dataset.clone());
        }
        Ok(visible)
    }

    /// Return raw bytes for a canonical hash if present.
    pub fn bytes_for_hash(&self, hash: &str) -> Option<&[u8]> {
        let index = *self.by_hash.get(hash)?;
        self.wal
            .entries()
            .get(index)
            .map(|entry| entry.bytes.as_slice())
    }

    /// Return raw bytes for an instance if the Study/Series/SOP UID tuple matches index state.
    pub fn instance_bytes(
        &self,
        study_uid: &str,
        series_uid: &str,
        sop_instance_uid: &str,
    ) -> Option<&[u8]> {
        if self.is_tombstoned(study_uid, series_uid, sop_instance_uid) {
            return None;
        }
        let (stored_study_uid, stored_series_uid, source_hash) =
            self.index.location_for_sop(sop_instance_uid)?;
        if stored_study_uid != study_uid || stored_series_uid != series_uid {
            return None;
        }
        self.bytes_for_hash(&source_hash)
    }

    /// Soft-delete all instances in a study via tombstone marker.
    pub fn soft_delete_study(&mut self, study_uid: &str) {
        self.tombstoned_studies.insert(study_uid.to_string());
    }

    /// Soft-delete all instances in a series via tombstone marker.
    pub fn soft_delete_series(&mut self, study_uid: &str, series_uid: &str) {
        self.tombstoned_series
            .insert((study_uid.to_string(), series_uid.to_string()));
    }

    /// Soft-delete an instance via tombstone marker.
    pub fn soft_delete_instance(&mut self, study_uid: &str, series_uid: &str, instance_uid: &str) {
        self.tombstoned_instances.insert((
            study_uid.to_string(),
            series_uid.to_string(),
            instance_uid.to_string(),
        ));
    }

    /// Register and persist a Storage Commitment request model in deterministic key order.
    pub fn register_storage_commitment_request(
        &mut self,
        request: StorageCommitmentRequest,
    ) -> Result<()> {
        self.commitment.register_request(request)
    }

    /// Return a persisted Storage Commitment request model by transaction UID.
    pub fn storage_commitment_request(
        &self,
        transaction_uid: &str,
    ) -> Option<&StorageCommitmentRequest> {
        self.commitment.request(transaction_uid)
    }

    /// Return all persisted Storage Commitment request models in deterministic key order.
    pub fn storage_commitment_requests(&self) -> Vec<&StorageCommitmentRequest> {
        self.commitment.requests()
    }

    /// Queue deterministic asynchronous N-EVENT report delivery for a transaction.
    pub fn queue_storage_commitment_event_report(
        &mut self,
        transaction_uid: &str,
        max_attempts: u32,
    ) -> Result<()> {
        self.commitment
            .queue_event_report(transaction_uid, max_attempts)
    }

    /// Pop next event delivery job in FIFO order.
    pub fn pop_next_storage_commitment_event_job(&mut self) -> Option<StorageCommitmentEventJob> {
        self.commitment.pop_next_job()
    }

    /// Record delivery outcome and schedule retry when allowed.
    pub fn complete_storage_commitment_event_job(
        &mut self,
        job: StorageCommitmentEventJob,
        delivered: bool,
    ) -> Result<()> {
        self.commitment.complete_job(job, delivered)
    }

    /// Cancel a Storage Commitment request and drop queued jobs for the transaction.
    pub fn cancel_storage_commitment_request(&mut self, transaction_uid: &str) -> Result<()> {
        self.commitment.cancel_request(transaction_uid)
    }

    /// Advance logical delivery clock and mark expired queued jobs as timed out.
    pub fn advance_storage_commitment_timeouts(&mut self, ticks: u64) -> usize {
        self.commitment.advance_timeouts(ticks)
    }

    /// Override Storage Commitment delivery policy.
    pub fn set_storage_commitment_policy(&mut self, policy: StorageCommitmentPolicy) {
        self.commitment.set_policy(policy);
    }

    fn is_tombstoned(&self, study_uid: &str, series_uid: &str, instance_uid: &str) -> bool {
        self.tombstoned_studies.contains(study_uid)
            || self
                .tombstoned_series
                .contains(&(study_uid.to_string(), series_uid.to_string()))
            || self.tombstoned_instances.contains(&(
                study_uid.to_string(),
                series_uid.to_string(),
                instance_uid.to_string(),
            ))
    }
}

/// Extract and strictly validate Study Instance UID from DICOM P10 bytes.
pub fn extract_study_uid(bytes: &[u8], limits: &Limits) -> Result<String> {
    enforce_limit(
        "max_input_bytes",
        bytes.len() as u64,
        limits.max_input_bytes(),
    )?;
    let dataset = parse_dataset(bytes, limits)?;
    let study_uid = dataset
        .get_uid(TAG_STUDY_UID)
        .ok_or_else(|| missing_required_tag(TAG_STUDY_UID))?;
    enforce_limit(
        "max_string_bytes",
        study_uid.len() as u64,
        limits.max_string_bytes(),
    )?;
    validate_uid_strict(TAG_STUDY_UID, study_uid)?;
    Ok(study_uid.to_string())
}

/// Deduplicate a slice of byte slices by canonical SHA-256 hash.
pub fn deduplicate(slices: &[&[u8]]) -> Vec<usize> {
    let mut seen = BTreeMap::new();
    let mut result = Vec::new();
    for (i, slice) in slices.iter().enumerate() {
        let hash = canonical_hash(slice);
        if let Some(&prev) = seen.get(&hash) {
            // Duplicate — keep reference to the earlier index
            let _ = prev;
        } else {
            seen.insert(hash, i);
            result.push(i);
        }
    }
    result
}

fn parse_dataset(bytes: &[u8], limits: &Limits) -> Result<dicom_core::Dataset> {
    let mut reader = P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
    reader.read_dataset()
}

/// Compute the canonical SHA-256 hash of a byte slice.
pub fn canonical_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    dicom_util::missing_required_tag(tag)
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| io_error("create storage directory", err))?;
    }
    Ok(())
}

fn load_wal_file(path: &Path) -> Result<WriteAheadLog> {
    if !path.exists() {
        fs::write(path, WAL_MAGIC).map_err(|err| io_error("initialize WAL file", err))?;
    }
    let mut bytes = fs::read(path).map_err(|err| io_error("read WAL file", err))?;
    if bytes.is_empty() {
        fs::write(path, WAL_MAGIC).map_err(|err| io_error("repair empty WAL file", err))?;
        bytes = fs::read(path).map_err(|err| io_error("re-read WAL file", err))?;
    }
    if bytes.len() < WAL_MAGIC.len() || &bytes[..WAL_MAGIC.len()] != WAL_MAGIC {
        return Err(integrity_error("invalid WAL file header"));
    }

    let mut offset = WAL_MAGIC.len();
    let mut log = WriteAheadLog::new();
    while offset < bytes.len() {
        let hash_len = read_u32(&bytes, &mut offset)? as usize;
        if hash_len == 0 {
            return Err(integrity_error("WAL entry hash length must be non-zero"));
        }
        let hash_bytes = read_slice(&bytes, &mut offset, hash_len)?;
        let hash = std::str::from_utf8(hash_bytes)
            .map_err(|_| integrity_error("WAL entry hash is not valid UTF-8"))?
            .to_string();
        let payload_len = read_u64(&bytes, &mut offset)? as usize;
        let payload = read_slice(&bytes, &mut offset, payload_len)?.to_vec();
        log.append(WalEntry {
            hash,
            bytes: payload,
        });
    }
    Ok(log)
}

fn append_wal_entry(path: &Path, entry: &WalEntry) -> Result<u64> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|err| io_error("open WAL file for append", err))?;
    let mut rollback_len = file
        .metadata()
        .map_err(|err| io_error("stat WAL file", err))?
        .len();
    if rollback_len == 0 {
        file.write_all(WAL_MAGIC)
            .and_then(|_| file.sync_data())
            .map_err(|err| io_error("write WAL header", err))?;
        rollback_len = WAL_MAGIC.len() as u64;
    }
    if rollback_len < WAL_MAGIC.len() as u64 {
        return Err(integrity_error("truncated WAL file header"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|err| io_error("seek WAL header", err))?;
    let mut header = [0u8; WAL_MAGIC.len()];
    file.read_exact(&mut header)
        .map_err(|err| io_error("read WAL header", err))?;
    if &header != WAL_MAGIC {
        return Err(integrity_error("WAL header mismatch"));
    }

    file.seek(SeekFrom::End(0))
        .map_err(|err| io_error("seek WAL end", err))?;
    let hash = entry.hash.as_bytes();
    file.write_all(&(hash.len() as u32).to_le_bytes())
        .and_then(|_| file.write_all(hash))
        .and_then(|_| file.write_all(&(entry.bytes.len() as u64).to_le_bytes()))
        .and_then(|_| file.write_all(&entry.bytes))
        .and_then(|_| file.sync_data())
        .map_err(|err| io_error("append WAL entry", err))?;
    Ok(rollback_len)
}

fn rollback_wal_append(path: &Path, old_len: u64) -> Result<()> {
    let file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|err| io_error("open WAL file for rollback", err))?;
    file.set_len(old_len)
        .map_err(|err| io_error("truncate WAL file during rollback", err))?;
    file.sync_data()
        .map_err(|err| io_error("sync WAL rollback", err))?;
    Ok(())
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32> {
    let raw = read_slice(bytes, offset, std::mem::size_of::<u32>())?;
    let mut arr = [0u8; std::mem::size_of::<u32>()];
    arr.copy_from_slice(raw);
    Ok(u32::from_le_bytes(arr))
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64> {
    let raw = read_slice(bytes, offset, std::mem::size_of::<u64>())?;
    let mut arr = [0u8; std::mem::size_of::<u64>()];
    arr.copy_from_slice(raw);
    Ok(u64::from_le_bytes(arr))
}

fn read_slice<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = offset.saturating_add(len);
    if end > bytes.len() {
        return Err(integrity_error("truncated WAL entry"));
    }
    let slice = &bytes[*offset..end];
    *offset = end;
    Ok(slice)
}

fn integrity_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: detail.into(),
        },
        "integrity error",
    )
    .into()
}

fn io_error(context: impl Into<String>, source: std::io::Error) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: format!("{}: {}", context.into(), source),
        },
        "i/o error",
    )
    .into()
}
