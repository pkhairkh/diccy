#![deny(missing_docs)]

//! Deterministic storage ingestion with write-ahead logging and deduplication.

use dicom_core::{enforce_limit, validate_uid_strict, Error, ErrorKind, Limits, Result, Tag};
use dicom_index::{extract_indexed_instance, Index, InsertOutcome};
use dicom_io::{BytesSource, P10Reader};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
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

/// Storage Commitment request state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageCommitmentState {
    /// Request accepted and pending asynchronous outcome delivery.
    Requested,
    /// Event report queued for asynchronous delivery.
    EventQueued,
    /// Event report delivery succeeded.
    ReportDelivered,
    /// Event report delivery failed permanently.
    ReportFailed,
    /// Request was canceled before terminal delivery.
    Canceled,
    /// Request timed out before terminal delivery.
    TimedOut,
}

/// Referenced instance payload row in a Storage Commitment request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentReferencedInstance {
    /// Referenced SOP Class UID.
    pub sop_class_uid: String,
    /// Referenced SOP Instance UID.
    pub sop_instance_uid: String,
}

/// Persisted Storage Commitment request model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentRequest {
    /// Transaction UID for the request lifecycle.
    pub transaction_uid: String,
    /// Calling AE title.
    pub calling_ae_title: String,
    /// Called AE title.
    pub called_ae_title: String,
    /// Referenced instances included in the commitment contract.
    pub referenced_instances: Vec<StorageCommitmentReferencedInstance>,
    /// Current deterministic state for this request.
    pub state: StorageCommitmentState,
}

/// Deterministic event-report delivery job for Storage Commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageCommitmentEventJob {
    /// Transaction UID for request lifecycle.
    pub transaction_uid: String,
    /// Delivery attempt counter.
    pub attempt: u32,
    /// Maximum allowed attempts before terminal failure.
    pub max_attempts: u32,
    /// Logical tick at which this job was queued.
    pub queued_tick: u64,
    /// Logical tick at which this job times out.
    pub timeout_tick: u64,
}

/// Policy knobs for Storage Commitment async delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageCommitmentPolicy {
    /// Maximum queued jobs allowed before backpressure rejection.
    pub max_queued_jobs: usize,
    /// Logical tick budget for queued jobs before timeout.
    pub timeout_ticks: u64,
}

impl Default for StorageCommitmentPolicy {
    fn default() -> Self {
        Self {
            max_queued_jobs: 1024,
            timeout_ticks: 256,
        }
    }
}

/// Storage engine with deterministic hashing and deduplication.
#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    limits: Limits,
    index: Index,
    wal: WriteAheadLog,
    decoded_datasets: Vec<dicom_core::Dataset>,
    by_hash: BTreeMap<String, usize>,
    storage_commitment_requests: BTreeMap<String, StorageCommitmentRequest>,
    storage_commitment_event_queue: VecDeque<StorageCommitmentEventJob>,
    storage_commitment_policy: StorageCommitmentPolicy,
    storage_commitment_tick: u64,
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
            storage_commitment_requests: BTreeMap::new(),
            storage_commitment_event_queue: VecDeque::new(),
            storage_commitment_policy: StorageCommitmentPolicy::default(),
            storage_commitment_tick: 0,
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
            self.limits.max_input_bytes,
        )?;
        let hash = canonical_hash(&bytes);
        if self.by_hash.contains_key(&hash) {
            return Ok(IngestOutcome::Duplicate { hash });
        }
        enforce_limit(
            "max_cache_bytes",
            self.total_bytes + bytes.len() as u64,
            self.limits.max_cache_bytes,
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
                storage.limits.max_input_bytes,
            )?;
            enforce_limit(
                "max_cache_bytes",
                storage.total_bytes + entry.bytes.len() as u64,
                storage.limits.max_cache_bytes,
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
        validate_uid_strict(Tag(0x0008, 0x1195), &request.transaction_uid)?;
        if request.referenced_instances.is_empty() {
            return Err(Error::from_kind(
                ErrorKind::DecodeError {
                    stage: "storage_commitment".to_string(),
                    detail:
                        "storage commitment request must include at least one referenced instance"
                            .to_string(),
                },
                "invalid input",
            )
            .into());
        }
        for reference in &request.referenced_instances {
            validate_uid_strict(Tag(0x0008, 0x1150), &reference.sop_class_uid)?;
            validate_uid_strict(Tag(0x0008, 0x1155), &reference.sop_instance_uid)?;
        }
        if self
            .storage_commitment_requests
            .contains_key(&request.transaction_uid)
        {
            return Err(Error::from_kind(
                ErrorKind::IntegrityError {
                    detail: format!(
                        "duplicate storage commitment transaction UID: {}",
                        request.transaction_uid
                    ),
                },
                "already exists",
            )
            .into());
        }
        self.storage_commitment_requests
            .insert(request.transaction_uid.clone(), request);
        Ok(())
    }

    /// Return a persisted Storage Commitment request model by transaction UID.
    pub fn storage_commitment_request(
        &self,
        transaction_uid: &str,
    ) -> Option<&StorageCommitmentRequest> {
        self.storage_commitment_requests.get(transaction_uid)
    }

    /// Return all persisted Storage Commitment request models in deterministic key order.
    pub fn storage_commitment_requests(&self) -> Vec<&StorageCommitmentRequest> {
        self.storage_commitment_requests.values().collect()
    }

    /// Queue deterministic asynchronous N-EVENT report delivery for a transaction.
    pub fn queue_storage_commitment_event_report(
        &mut self,
        transaction_uid: &str,
        max_attempts: u32,
    ) -> Result<()> {
        if self.storage_commitment_event_queue.len()
            >= self.storage_commitment_policy.max_queued_jobs
        {
            return Err(Error::from_kind(
                ErrorKind::LimitExceeded {
                    limit_name: "storage_commitment_max_queued_jobs",
                    observed: self.storage_commitment_event_queue.len() as u64 + 1,
                    allowed: self.storage_commitment_policy.max_queued_jobs as u64,
                },
                "storage commitment queue backpressure",
            )
            .into());
        }
        let request = self
            .storage_commitment_requests
            .get_mut(transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        request.state = StorageCommitmentState::EventQueued;
        self.storage_commitment_tick = self.storage_commitment_tick.saturating_add(1);
        let queued_tick = self.storage_commitment_tick;
        let timeout_tick = queued_tick.saturating_add(self.storage_commitment_policy.timeout_ticks);
        self.storage_commitment_event_queue
            .push_back(StorageCommitmentEventJob {
                transaction_uid: transaction_uid.to_string(),
                attempt: 0,
                max_attempts: max_attempts.max(1),
                queued_tick,
                timeout_tick,
            });
        Ok(())
    }

    /// Pop next event delivery job in FIFO order.
    pub fn pop_next_storage_commitment_event_job(&mut self) -> Option<StorageCommitmentEventJob> {
        self.storage_commitment_event_queue.pop_front()
    }

    /// Record delivery outcome and schedule retry when allowed.
    pub fn complete_storage_commitment_event_job(
        &mut self,
        mut job: StorageCommitmentEventJob,
        delivered: bool,
    ) -> Result<()> {
        let request = self
            .storage_commitment_requests
            .get_mut(&job.transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        if delivered {
            request.state = StorageCommitmentState::ReportDelivered;
            return Ok(());
        }
        job.attempt = job.attempt.saturating_add(1);
        if job.attempt >= job.max_attempts {
            request.state = StorageCommitmentState::ReportFailed;
            return Ok(());
        }
        request.state = StorageCommitmentState::EventQueued;
        self.storage_commitment_tick = self.storage_commitment_tick.saturating_add(1);
        job.queued_tick = self.storage_commitment_tick;
        job.timeout_tick = job
            .queued_tick
            .saturating_add(self.storage_commitment_policy.timeout_ticks);
        self.storage_commitment_event_queue.push_back(job);
        Ok(())
    }

    /// Cancel a Storage Commitment request and drop queued jobs for the transaction.
    pub fn cancel_storage_commitment_request(&mut self, transaction_uid: &str) -> Result<()> {
        let request = self
            .storage_commitment_requests
            .get_mut(transaction_uid)
            .ok_or_else(|| {
                Error::from_kind(
                    ErrorKind::MissingRequiredTag {
                        tag: Tag(0x0008, 0x1195),
                    },
                    "missing storage commitment transaction UID",
                )
            })?;
        request.state = StorageCommitmentState::Canceled;
        self.storage_commitment_event_queue
            .retain(|job| job.transaction_uid != transaction_uid);
        Ok(())
    }

    /// Advance logical delivery clock and mark expired queued jobs as timed out.
    pub fn advance_storage_commitment_timeouts(&mut self, ticks: u64) -> usize {
        self.storage_commitment_tick = self.storage_commitment_tick.saturating_add(ticks);
        let now = self.storage_commitment_tick;
        let mut timed_out = 0usize;
        let mut retained = VecDeque::new();
        while let Some(job) = self.storage_commitment_event_queue.pop_front() {
            if job.timeout_tick <= now {
                if let Some(request) = self
                    .storage_commitment_requests
                    .get_mut(&job.transaction_uid)
                {
                    request.state = StorageCommitmentState::TimedOut;
                }
                timed_out = timed_out.saturating_add(1);
            } else {
                retained.push_back(job);
            }
        }
        self.storage_commitment_event_queue = retained;
        timed_out
    }

    /// Override Storage Commitment delivery policy.
    pub fn set_storage_commitment_policy(&mut self, policy: StorageCommitmentPolicy) {
        self.storage_commitment_policy = policy;
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
        limits.max_input_bytes,
    )?;
    let dataset = parse_dataset(bytes, limits)?;
    let study_uid = dataset
        .get_uid(TAG_STUDY_UID)
        .ok_or_else(|| missing_required_tag(TAG_STUDY_UID))?;
    enforce_limit(
        "max_string_bytes",
        study_uid.len() as u64,
        limits.max_string_bytes,
    )?;
    validate_uid_strict(TAG_STUDY_UID, study_uid)?;
    Ok(study_uid.to_string())
}

fn parse_dataset(bytes: &[u8], limits: &Limits) -> Result<dicom_core::Dataset> {
    let mut reader = P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
    reader.read_dataset()
}

fn canonical_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{ErrorKind, Tag};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::thread;
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

    fn u16_bytes(value: u16) -> [u8; 2] {
        value.to_le_bytes()
    }

    fn minimal_sc_dataset_explicit(study_uid: &str, series_uid: &str, sop_uid: &str) -> Vec<u8> {
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

    fn sample_p10(study: &str, series: &str, sop: &str) -> Vec<u8> {
        let dataset = minimal_sc_dataset_explicit(study, series, sop);
        build_p10(TS_EXPLICIT_VR_LE, &dataset)
    }

    fn temp_wal_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.wal"))
    }

    #[test]
    fn ingest_deduplicates_by_hash() {
        // REQ-STOR-300: canonical hash and deduplication must be deterministic.
        let mut storage = Storage::new(Limits::default());
        let bytes = sample_p10("1.2.3", "2.3.4", "3.4.5");
        let first = storage.ingest_bytes(bytes.clone()).expect("ingest");
        let second = storage.ingest_bytes(bytes).expect("dedup");
        match first {
            IngestOutcome::Inserted { .. } => {}
            _ => panic!("expected insert"),
        }
        match second {
            IngestOutcome::Duplicate { .. } => {}
            _ => panic!("expected duplicate"),
        }
        assert_eq!(storage.index().total_instances(), 1);
        assert_eq!(storage.log().len(), 1);
    }

    #[test]
    fn ingest_rejects_uid_conflict() {
        // REQ-STOR-301: SOP UID conflicts with different hashes must fail closed.
        let mut storage = Storage::new(Limits::default());
        let bytes1 = sample_p10("1.2.3", "2.3.4", "3.4.5");
        let bytes2 = sample_p10("9.9.9", "2.3.4", "3.4.5");
        storage.ingest_bytes(bytes1).expect("ingest");
        let err = storage.ingest_bytes(bytes2).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::IntegrityError { .. }));
    }

    #[test]
    fn replay_rebuilds_index() {
        // REQ-STOR-302: replay must deterministically rebuild index state.
        let mut storage = Storage::new(Limits::default());
        let bytes = sample_p10("1.2.3", "2.3.4", "3.4.5");
        storage.ingest_bytes(bytes).expect("ingest");
        let log = storage.log().clone();
        let rebuilt = Storage::from_log(Limits::default(), log).expect("replay");
        assert_eq!(rebuilt.index().total_instances(), 1);
        assert_eq!(rebuilt.log().len(), 1);
    }

    #[test]
    fn cache_limit_enforced() {
        // REQ-STOR-303: storage must enforce max_cache_bytes.
        let bytes = sample_p10("1.2.3", "2.3.4", "3.4.5");
        let limits = Limits {
            max_cache_bytes: (bytes.len() as u64).saturating_sub(1),
            ..Limits::default()
        };
        let mut storage = Storage::new(limits);
        let err = storage.ingest_bytes(bytes).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn instance_bytes_returns_only_matching_uid_triplet() {
        // REQ-STOR-300: retrieval by UID tuple is deterministic and fails closed on path mismatch.
        let mut storage = Storage::new(Limits::default());
        let bytes = sample_p10("1.2.3", "2.3.4", "3.4.5");
        storage.ingest_bytes(bytes.clone()).expect("ingest");

        let found = storage
            .instance_bytes("1.2.3", "2.3.4", "3.4.5")
            .expect("bytes");
        assert_eq!(found, bytes.as_slice());
        assert!(storage.instance_bytes("9.9.9", "2.3.4", "3.4.5").is_none());
    }

    #[test]
    fn datasets_returns_decoded_entries_in_log_order() {
        // REQ-STOR-302: derived views from WAL state preserve deterministic insertion order.
        let mut storage = Storage::new(Limits::default());
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
            .expect("first ingest");
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.6"))
            .expect("second ingest");

        let datasets = storage.datasets().expect("datasets");
        assert_eq!(datasets.len(), 2);
        assert_eq!(datasets[0].get_uid(Tag(0x0008, 0x0018)), Some("3.4.5"));
        assert_eq!(datasets[1].get_uid(Tag(0x0008, 0x0018)), Some("3.4.6"));
    }

    #[test]
    fn concurrent_ingest_replay_is_consistent() {
        // REQ-STOR-302: concurrent ingest serialized through storage lock replays deterministically.
        let storage = Arc::new(Mutex::new(Storage::new(Limits::default())));
        let mut handles = Vec::new();
        for idx in 0..8u8 {
            let storage = Arc::clone(&storage);
            handles.push(thread::spawn(move || {
                let sop = format!("3.4.{}", idx + 10);
                let bytes = sample_p10("1.2.3", "2.3.4", &sop);
                let mut guard = storage.lock().expect("lock");
                guard.ingest_bytes(bytes).expect("ingest");
            }));
        }
        for handle in handles {
            handle.join().expect("join");
        }

        let guard = storage.lock().expect("lock");
        assert_eq!(guard.index().total_instances(), 8);
        let log = guard.log().clone();
        drop(guard);

        let rebuilt = Storage::from_log(Limits::default(), log).expect("replay");
        assert_eq!(rebuilt.index().total_instances(), 8);
        assert_eq!(rebuilt.log().len(), 8);
    }

    #[test]
    fn durable_wal_persists_and_recovers() {
        // REQ-STOR-302: durable WAL replay must recover deterministic state.
        let path = temp_wal_path("storage_durable");
        let mut storage = Storage::open(Limits::default(), &path).expect("open");
        storage
            .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
            .expect("ingest");
        assert!(storage.persistence_path().is_some());
        drop(storage);

        let reopened = Storage::open(Limits::default(), &path).expect("reopen");
        assert_eq!(reopened.index().total_instances(), 1);
        assert_eq!(reopened.log().len(), 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn durable_wal_rejects_invalid_header() {
        // REQ-STOR-302: corrupted durable WAL headers fail closed.
        let path = temp_wal_path("storage_corrupt");
        fs::write(&path, b"BAD!").expect("write");
        let err = Storage::open(Limits::default(), &path).expect_err("expected corruption error");
        assert!(matches!(
            err.kind,
            ErrorKind::IntegrityError { .. } | ErrorKind::IoError { .. }
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn register_storage_commitment_request_persists_and_retrieves() {
        let mut storage = Storage::new(Limits::default());
        let request = StorageCommitmentRequest {
            transaction_uid: "1.2.840.10008.1.20.1".to_string(),
            calling_ae_title: "CALLING_AE".to_string(),
            called_ae_title: "CALLED_AE".to_string(),
            referenced_instances: vec![StorageCommitmentReferencedInstance {
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.1".to_string(),
            }],
            state: StorageCommitmentState::Requested,
        };

        storage
            .register_storage_commitment_request(request.clone())
            .expect("register request");

        let persisted = storage
            .storage_commitment_request(&request.transaction_uid)
            .expect("request stored");
        assert_eq!(persisted, &request);
        assert_eq!(storage.storage_commitment_requests().len(), 1);
    }

    #[test]
    fn register_storage_commitment_request_rejects_duplicates() {
        let mut storage = Storage::new(Limits::default());
        let request = StorageCommitmentRequest {
            transaction_uid: "1.2.840.10008.1.20.2".to_string(),
            calling_ae_title: "CALLING_AE".to_string(),
            called_ae_title: "CALLED_AE".to_string(),
            referenced_instances: vec![StorageCommitmentReferencedInstance {
                sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.2".to_string(),
            }],
            state: StorageCommitmentState::Requested,
        };

        storage
            .register_storage_commitment_request(request.clone())
            .expect("first register");
        let err = storage
            .register_storage_commitment_request(request)
            .expect_err("duplicate must fail");
        assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
    }

    #[test]
    fn storage_commitment_event_delivery_retries_then_fails() {
        let mut storage = Storage::new(Limits::default());
        storage
            .register_storage_commitment_request(StorageCommitmentRequest {
                transaction_uid: "1.2.840.10008.1.20.42".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.42".to_string(),
                }],
                state: StorageCommitmentState::Requested,
            })
            .expect("register");

        storage
            .queue_storage_commitment_event_report("1.2.840.10008.1.20.42", 2)
            .expect("queue");
        let first = storage
            .pop_next_storage_commitment_event_job()
            .expect("first job");
        storage
            .complete_storage_commitment_event_job(first, false)
            .expect("first failure");
        let retry = storage
            .pop_next_storage_commitment_event_job()
            .expect("retry job");
        storage
            .complete_storage_commitment_event_job(retry, false)
            .expect("second failure");

        let request = storage
            .storage_commitment_request("1.2.840.10008.1.20.42")
            .expect("request");
        assert_eq!(request.state, StorageCommitmentState::ReportFailed);
    }

    #[test]
    fn storage_commitment_cancellation_drops_queued_jobs() {
        let mut storage = Storage::new(Limits::default());
        storage
            .register_storage_commitment_request(StorageCommitmentRequest {
                transaction_uid: "1.2.840.10008.1.20.43".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.43".to_string(),
                }],
                state: StorageCommitmentState::Requested,
            })
            .expect("register");
        storage
            .queue_storage_commitment_event_report("1.2.840.10008.1.20.43", 2)
            .expect("queue");
        storage
            .cancel_storage_commitment_request("1.2.840.10008.1.20.43")
            .expect("cancel");

        assert!(storage.pop_next_storage_commitment_event_job().is_none());
        let request = storage
            .storage_commitment_request("1.2.840.10008.1.20.43")
            .expect("request");
        assert_eq!(request.state, StorageCommitmentState::Canceled);
    }

    #[test]
    fn storage_commitment_backpressure_enforces_queue_limit() {
        let mut storage = Storage::new(Limits::default());
        storage.set_storage_commitment_policy(StorageCommitmentPolicy {
            max_queued_jobs: 1,
            timeout_ticks: 10,
        });

        for suffix in ["44", "45"] {
            storage
                .register_storage_commitment_request(StorageCommitmentRequest {
                    transaction_uid: format!("1.2.840.10008.1.20.{suffix}"),
                    calling_ae_title: "CALLING_AE".to_string(),
                    called_ae_title: "CALLED_AE".to_string(),
                    referenced_instances: vec![StorageCommitmentReferencedInstance {
                        sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                        sop_instance_uid: format!("1.2.840.10008.5.1.4.1.1.7.{suffix}"),
                    }],
                    state: StorageCommitmentState::Requested,
                })
                .expect("register");
        }

        storage
            .queue_storage_commitment_event_report("1.2.840.10008.1.20.44", 2)
            .expect("queue first");
        let err = storage
            .queue_storage_commitment_event_report("1.2.840.10008.1.20.45", 2)
            .expect_err("expected queue limit");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn storage_commitment_timeout_marks_request_timed_out() {
        let mut storage = Storage::new(Limits::default());
        storage.set_storage_commitment_policy(StorageCommitmentPolicy {
            max_queued_jobs: 10,
            timeout_ticks: 1,
        });
        storage
            .register_storage_commitment_request(StorageCommitmentRequest {
                transaction_uid: "1.2.840.10008.1.20.46".to_string(),
                calling_ae_title: "CALLING_AE".to_string(),
                called_ae_title: "CALLED_AE".to_string(),
                referenced_instances: vec![StorageCommitmentReferencedInstance {
                    sop_class_uid: "1.2.840.10008.5.1.4.1.1.7".to_string(),
                    sop_instance_uid: "1.2.840.10008.5.1.4.1.1.7.46".to_string(),
                }],
                state: StorageCommitmentState::Requested,
            })
            .expect("register");
        storage
            .queue_storage_commitment_event_report("1.2.840.10008.1.20.46", 2)
            .expect("queue");
        let timed_out = storage.advance_storage_commitment_timeouts(2);
        assert_eq!(timed_out, 1);
        let request = storage
            .storage_commitment_request("1.2.840.10008.1.20.46")
            .expect("request");
        assert_eq!(request.state, StorageCommitmentState::TimedOut);
    }

    #[test]
    fn soft_delete_instance_hides_wado_and_qido_views() {
        let mut storage = Storage::new(Limits::default());
        let bytes = sample_p10("1.2.3", "2.3.4", "3.4.5");
        storage.ingest_bytes(bytes).expect("ingest");
        assert!(storage.instance_bytes("1.2.3", "2.3.4", "3.4.5").is_some());
        storage.soft_delete_instance("1.2.3", "2.3.4", "3.4.5");
        assert!(storage.instance_bytes("1.2.3", "2.3.4", "3.4.5").is_none());
        assert!(storage.datasets().expect("datasets").is_empty());
    }
}
