#![deny(missing_docs)]

//! MPPS update validation and deterministic ingestion rules.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{
    enforce_limit, validate_uid_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);
const TAG_END_DATE: Tag = Tag(0x0040, 0x0250);
const TAG_END_TIME: Tag = Tag(0x0040, 0x0251);
const MPPS_MAGIC: &[u8; 5] = b"DMP01";

/// MPPS status values supported by the envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MppsStatus {
    /// Procedure step is in progress.
    InProgress,
    /// Procedure step completed.
    Completed,
    /// Procedure step discontinued.
    Discontinued,
}

impl MppsStatus {
    fn parse(tag: Tag, value: &str) -> Result<Self> {
        match value {
            "IN PROGRESS" => Ok(MppsStatus::InProgress),
            "COMPLETED" => Ok(MppsStatus::Completed),
            "DISCONTINUED" => Ok(MppsStatus::Discontinued),
            _ => Err(invalid_tag_value(tag, "unsupported MPPS status")),
        }
    }
}

/// Validated MPPS update payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MppsUpdate {
    /// SOP Instance UID for this MPPS instance.
    pub sop_instance_uid: String,
    /// Performed Procedure Step status.
    pub status: MppsStatus,
    /// Performed Procedure Step ID.
    pub performed_step_id: String,
    /// Performed Procedure Step start date.
    pub start_date: String,
    /// Performed Procedure Step start time.
    pub start_time: String,
    /// Performed Procedure Step end date (required for terminal status).
    pub end_date: Option<String>,
    /// Performed Procedure Step end time (required for terminal status).
    pub end_time: Option<String>,
}

/// Outcome of ingesting an MPPS update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestOutcome {
    /// New MPPS instance inserted.
    Inserted,
    /// Existing MPPS instance updated.
    Updated,
    /// Duplicate update ignored.
    Duplicate,
}

/// Audit callback for MPPS service operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// MPPS service configuration.
#[derive(Clone, Default)]
pub struct MppsServiceConfig {
    /// Limits applied to validation and persisted state.
    pub limits: Limits,
    /// Optional audit callback for ingest operations.
    pub audit: Option<AuditCallback>,
}

impl fmt::Debug for MppsServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MppsServiceConfig")
            .field("limits", &self.limits)
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

/// Persisted MPPS workflow service with deterministic transitions and optional auditing.
#[derive(Clone)]
pub struct MppsService {
    limits: Limits,
    store: MppsStore,
    audit: Option<AuditCallback>,
}

impl fmt::Debug for MppsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MppsService")
            .field("limits", &self.limits)
            .field("store_len", &self.store.len())
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

/// MPPS store enforcing deterministic transitions with optional durable persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MppsStore {
    limits: Limits,
    updates: BTreeMap<String, MppsUpdate>,
    persistence: Option<PathBuf>,
}

impl MppsStore {
    /// Create an empty MPPS store.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            updates: BTreeMap::new(),
            persistence: None,
        }
    }

    /// Open an MPPS store backed by a durable snapshot file.
    pub fn open(limits: Limits, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        ensure_parent_dir(&path)?;
        let updates = load_mpps_snapshot(&path, &limits)?;
        Ok(Self {
            limits,
            updates,
            persistence: Some(path),
        })
    }

    /// Ingest an MPPS update dataset into the store.
    pub fn ingest_update(&mut self, dataset: &Dataset) -> Result<IngestOutcome> {
        let update = validate_mpps_update(dataset, &self.limits)?;
        let key = update.sop_instance_uid.clone();
        let previous = self.updates.get(&key).cloned();
        let outcome = match previous.as_ref() {
            None => {
                enforce_limit(
                    "max_dataset_elements",
                    (self.updates.len() as u64) + 1,
                    self.limits.max_dataset_elements(),
                )?;
                self.updates.insert(key.clone(), update);
                IngestOutcome::Inserted
            }
            Some(existing) => {
                if existing == &update {
                    return Ok(IngestOutcome::Duplicate);
                }
                validate_transition(existing, &update)?;
                self.updates.insert(key.clone(), update);
                IngestOutcome::Updated
            }
        };
        if let Err(err) = self.persist_snapshot() {
            match previous {
                Some(previous) => {
                    self.updates.insert(key.clone(), previous);
                }
                None => {
                    self.updates.remove(&key);
                }
            }
            return Err(err);
        }
        Ok(outcome)
    }

    /// Return the number of tracked MPPS instances.
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    /// Return true if no MPPS instances are tracked.
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    /// Return a persisted MPPS update by SOP Instance UID.
    pub fn get(&self, sop_instance_uid: &str) -> Option<&MppsUpdate> {
        self.updates.get(sop_instance_uid)
    }

    /// Return all persisted MPPS updates in deterministic SOP Instance UID order.
    pub fn all_updates(&self) -> Vec<MppsUpdate> {
        self.updates.values().cloned().collect()
    }

    fn persist_snapshot(&self) -> Result<()> {
        let Some(path) = &self.persistence else {
            return Ok(());
        };
        save_mpps_snapshot(path, &self.updates)
    }
}

impl MppsService {
    /// Create a new persisted MPPS workflow service.
    pub fn new(config: MppsServiceConfig) -> Self {
        Self {
            limits: config.limits.clone(),
            store: MppsStore::new(config.limits),
            audit: config.audit,
        }
    }

    /// Create a persisted MPPS workflow service backed by a durable snapshot file.
    pub fn with_persistence(config: MppsServiceConfig, path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            limits: config.limits.clone(),
            store: MppsStore::open(config.limits, path)?,
            audit: config.audit,
        })
    }

    /// Ingest an MPPS update dataset, enforcing deterministic transitions and limits.
    pub fn ingest(&mut self, dataset: &Dataset) -> Result<IngestOutcome> {
        let update = validate_mpps_update(dataset, &self.limits)?;
        let outcome = self.store.ingest_update(dataset)?;
        record_mpps_audit(&self.audit, &update, outcome)?;
        Ok(outcome)
    }

    /// Return a persisted MPPS update by SOP Instance UID.
    pub fn get(&self, sop_instance_uid: &str) -> Option<&MppsUpdate> {
        self.store.get(sop_instance_uid)
    }

    /// Return all persisted MPPS updates in deterministic order.
    pub fn all_updates(&self) -> Vec<MppsUpdate> {
        self.store.all_updates()
    }

    /// Return the number of tracked MPPS instances.
    pub fn len(&self) -> usize {
        self.store.len()
    }

    /// Return true if no MPPS instances are tracked.
    pub fn is_empty(&self) -> bool {
        self.store.is_empty()
    }
}

/// Validate a raw MPPS dataset and extract required fields.
pub fn validate_mpps_update(dataset: &Dataset, limits: &Limits) -> Result<MppsUpdate> {
    let sop_instance_uid = require_uid(dataset, TAG_SOP_INSTANCE_UID, limits)?;
    let status_str = require_str(dataset, TAG_STATUS, limits)?;
    let status = MppsStatus::parse(TAG_STATUS, &status_str)?;
    let performed_step_id = require_str(dataset, TAG_PERFORMED_STEP_ID, limits)?;
    let start_date = require_str(dataset, TAG_START_DATE, limits)?;
    let start_time = require_str(dataset, TAG_START_TIME, limits)?;
    let end_date = optional_str(dataset, TAG_END_DATE, limits)?;
    let end_time = optional_str(dataset, TAG_END_TIME, limits)?;

    if matches!(status, MppsStatus::Completed | MppsStatus::Discontinued)
        && (end_date.is_none() || end_time.is_none())
    {
        return Err(decode_error(
            "end date/time required for completed or discontinued status",
        ));
    }

    Ok(MppsUpdate {
        sop_instance_uid,
        status,
        performed_step_id,
        start_date,
        start_time,
        end_date,
        end_time,
    })
}

fn validate_transition(current: &MppsUpdate, next: &MppsUpdate) -> Result<()> {
    if matches!(
        current.status,
        MppsStatus::Completed | MppsStatus::Discontinued
    ) && next.status != current.status
    {
        return Err(integrity_error("terminal MPPS status cannot be updated"));
    }
    if current.performed_step_id != next.performed_step_id
        || current.start_date != next.start_date
        || current.start_time != next.start_time
    {
        return Err(integrity_error("MPPS identifiers must remain stable"));
    }
    if current.status == MppsStatus::InProgress
        && matches!(
            next.status,
            MppsStatus::Completed | MppsStatus::Discontinued
        )
    {
        return Ok(());
    }
    if current.status == next.status {
        return Ok(());
    }
    Err(integrity_error("invalid MPPS status transition"))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| io_error("create MPPS directory", err))?;
    }
    Ok(())
}

fn load_mpps_snapshot(path: &Path, limits: &Limits) -> Result<BTreeMap<String, MppsUpdate>> {
    if !path.exists() {
        save_mpps_snapshot(path, &BTreeMap::new())?;
    }
    let mut bytes = fs::read(path).map_err(|err| io_error("read MPPS snapshot", err))?;
    if bytes.is_empty() {
        save_mpps_snapshot(path, &BTreeMap::new())?;
        bytes = fs::read(path).map_err(|err| io_error("re-read MPPS snapshot", err))?;
    }
    if bytes.len() < MPPS_MAGIC.len() || &bytes[..MPPS_MAGIC.len()] != MPPS_MAGIC {
        return Err(decode_error("invalid MPPS snapshot header"));
    }

    let mut offset = MPPS_MAGIC.len();
    let count = read_u64(&bytes, &mut offset)? as usize;
    enforce_limit(
        "max_dataset_elements",
        count as u64,
        limits.max_dataset_elements(),
    )?;
    let mut updates = BTreeMap::new();
    for _ in 0..count {
        let sop_instance_uid = read_required_str(&bytes, &mut offset, limits)?;
        let status = match *read_slice(&bytes, &mut offset, 1)?
            .first()
            .ok_or_else(|| decode_error("missing MPPS status marker"))?
        {
            0 => MppsStatus::InProgress,
            1 => MppsStatus::Completed,
            2 => MppsStatus::Discontinued,
            _ => return Err(decode_error("invalid MPPS status marker")),
        };
        let update = MppsUpdate {
            sop_instance_uid: sop_instance_uid.clone(),
            status,
            performed_step_id: read_required_str(&bytes, &mut offset, limits)?,
            start_date: read_required_str(&bytes, &mut offset, limits)?,
            start_time: read_required_str(&bytes, &mut offset, limits)?,
            end_date: read_optional_str(&bytes, &mut offset, limits)?,
            end_time: read_optional_str(&bytes, &mut offset, limits)?,
        };
        if updates.insert(sop_instance_uid, update).is_some() {
            return Err(decode_error("duplicate SOP Instance UID in MPPS snapshot"));
        }
    }
    if offset != bytes.len() {
        return Err(decode_error("MPPS snapshot contains trailing bytes"));
    }
    Ok(updates)
}

fn save_mpps_snapshot(path: &Path, updates: &BTreeMap<String, MppsUpdate>) -> Result<()> {
    let tmp_path = snapshot_temp_path(path);
    let mut file = File::create(&tmp_path).map_err(|err| io_error("create MPPS snapshot", err))?;
    file.write_all(MPPS_MAGIC)
        .and_then(|_| file.write_all(&(updates.len() as u64).to_le_bytes()))
        .map_err(|err| io_error("write MPPS snapshot header", err))?;
    for update in updates.values() {
        write_str(&mut file, &update.sop_instance_uid)?;
        let marker = match update.status {
            MppsStatus::InProgress => 0u8,
            MppsStatus::Completed => 1u8,
            MppsStatus::Discontinued => 2u8,
        };
        file.write_all(&[marker])
            .map_err(|err| io_error("write MPPS status marker", err))?;
        write_str(&mut file, &update.performed_step_id)?;
        write_str(&mut file, &update.start_date)?;
        write_str(&mut file, &update.start_time)?;
        write_optional_str(&mut file, update.end_date.as_deref())?;
        write_optional_str(&mut file, update.end_time.as_deref())?;
    }
    file.sync_all()
        .map_err(|err| io_error("sync MPPS snapshot", err))?;
    fs::rename(&tmp_path, path).map_err(|err| io_error("replace MPPS snapshot", err))?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    OpenOptions::new()
        .read(true)
        .open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|err| io_error("sync MPPS directory", err))?;
    Ok(())
}

fn snapshot_temp_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let extension = match path.extension().and_then(|value| value.to_str()) {
        Some(ext) => format!("{ext}.tmp"),
        None => "tmp".to_string(),
    };
    tmp.set_extension(extension);
    tmp
}

fn write_str(file: &mut File, value: &str) -> Result<()> {
    file.write_all(&(value.len() as u32).to_le_bytes())
        .and_then(|_| file.write_all(value.as_bytes()))
        .map_err(|err| io_error("write MPPS string field", err))?;
    Ok(())
}

fn write_optional_str(file: &mut File, value: Option<&str>) -> Result<()> {
    match value {
        Some(text) => {
            file.write_all(&[1u8])
                .map_err(|err| io_error("write MPPS optional marker", err))?;
            write_str(file, text)?;
        }
        None => {
            file.write_all(&[0u8])
                .map_err(|err| io_error("write MPPS optional marker", err))?;
        }
    }
    Ok(())
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Result<u64> {
    let raw = read_slice(bytes, offset, std::mem::size_of::<u64>())?;
    let mut out = [0u8; std::mem::size_of::<u64>()];
    out.copy_from_slice(raw);
    Ok(u64::from_le_bytes(out))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32> {
    let raw = read_slice(bytes, offset, std::mem::size_of::<u32>())?;
    let mut out = [0u8; std::mem::size_of::<u32>()];
    out.copy_from_slice(raw);
    Ok(u32::from_le_bytes(out))
}

fn read_slice<'a>(bytes: &'a [u8], offset: &mut usize, len: usize) -> Result<&'a [u8]> {
    let end = offset.saturating_add(len);
    if end > bytes.len() {
        return Err(decode_error("truncated MPPS snapshot"));
    }
    let out = &bytes[*offset..end];
    *offset = end;
    Ok(out)
}

fn read_required_str(bytes: &[u8], offset: &mut usize, limits: &Limits) -> Result<String> {
    let len = read_u32(bytes, offset)? as usize;
    if len == 0 {
        return Err(decode_error("required MPPS field is empty"));
    }
    let raw = read_slice(bytes, offset, len)?;
    enforce_limit(
        "max_string_bytes",
        raw.len() as u64,
        limits.max_string_bytes(),
    )?;
    let out =
        std::str::from_utf8(raw).map_err(|_| decode_error("MPPS field is not valid UTF-8"))?;
    Ok(out.to_string())
}

fn read_optional_str(bytes: &[u8], offset: &mut usize, limits: &Limits) -> Result<Option<String>> {
    let marker = *read_slice(bytes, offset, 1)?
        .first()
        .ok_or_else(|| decode_error("missing optional field marker"))?;
    match marker {
        0 => Ok(None),
        1 => Ok(Some(read_required_str(bytes, offset, limits)?)),
        _ => Err(decode_error("invalid optional field marker")),
    }
}

fn record_mpps_audit(
    audit: &Option<AuditCallback>,
    update: &MppsUpdate,
    outcome: IngestOutcome,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };

    let status = match update.status {
        MppsStatus::InProgress => "IN PROGRESS",
        MppsStatus::Completed => "COMPLETED",
        MppsStatus::Discontinued => "DISCONTINUED",
    };
    let outcome = match outcome {
        IngestOutcome::Inserted => "inserted",
        IngestOutcome::Updated => "updated",
        IngestOutcome::Duplicate => "duplicate",
    };
    callback(AuditEvent {
        kind: AuditEventKind::ServiceEvent,
        fields: vec![
            AuditField {
                key: "operation",
                value: AuditValue::Plain("ingest".to_string()),
            },
            AuditField {
                key: "outcome",
                value: AuditValue::Plain(outcome.to_string()),
            },
            AuditField {
                key: "status",
                value: AuditValue::Plain(status.to_string()),
            },
            AuditField {
                key: "sop_instance_uid",
                value: AuditValue::Sensitive(update.sop_instance_uid.clone()),
            },
        ],
    })
}

fn require_uid(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<String> {
    let value = dataset
        .get_uid(tag)
        .ok_or_else(|| missing_required_tag(tag))?;
    enforce_limit(
        "max_string_bytes",
        value.len() as u64,
        limits.max_string_bytes(),
    )?;
    validate_uid_strict(tag, value)?;
    Ok(value.to_string())
}

fn require_str(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<String> {
    let value = dataset.get(tag).ok_or_else(|| missing_required_tag(tag))?;
    let string = match value.value() {
        Value::Str(text) | Value::Uid(text) => text,
        _ => return Err(invalid_tag_value(tag, "expected string value")),
    };
    enforce_limit(
        "max_string_bytes",
        string.len() as u64,
        limits.max_string_bytes(),
    )?;
    if string.is_empty() {
        return Err(invalid_tag_value(tag, "value must not be empty"));
    }
    Ok(string.to_string())
}

fn optional_str(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<Option<String>> {
    let value = match dataset.get(tag) {
        Some(value) => value,
        None => return Ok(None),
    };
    let string = match value.value() {
        Value::Str(text) | Value::Uid(text) => text,
        _ => return Err(invalid_tag_value(tag, "expected string value")),
    };
    enforce_limit(
        "max_string_bytes",
        string.len() as u64,
        limits.max_string_bytes(),
    )?;
    if string.is_empty() {
        return Ok(None);
    }
    Ok(Some(string.to_string()))
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
    )
    .into()
}

fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidTagValue {
            tag,
            detail: detail.into(),
        },
        "invalid tag value",
    )
    .into()
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

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-mpps".to_string(),
            detail: detail.into(),
        },
        "decode error",
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
    use dicom_core::{Dataset, Element, Limits, Value, Vr};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn dataset_with_status(status: &str, sop_uid: &str) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_SOP_INSTANCE_UID, Vr::Ui, Value::Uid(sop_uid.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_STATUS, Vr::Cs, Value::Str(status.to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_PERFORMED_STEP_ID, Vr::Sh, Value::Str("STEP1".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_START_DATE, Vr::Da, Value::Str("20240101".to_string()),
        ).unwrap());
        dataset.insert(Element::new(TAG_START_TIME, Vr::Tm, Value::Str("120000".to_string()),
        ).unwrap());
        dataset
    }

    fn temp_snapshot_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.snapshot"))
    }

    #[test]
    fn mpps_requires_required_tags() {
        // REQ-MPPS-350: required tags must be present.
        let dataset = Dataset::new();
        let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn mpps_rejects_invalid_status() {
        // REQ-MPPS-351: unsupported status must fail closed.
        let dataset = dataset_with_status("BAD", "1.2.3");
        let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn mpps_requires_end_time_for_completed() {
        // REQ-MPPS-351: completed status requires end date/time.
        let dataset = dataset_with_status("COMPLETED", "1.2.3");
        let err = validate_mpps_update(&dataset, &Limits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn mpps_transition_enforced() {
        // REQ-MPPS-352: invalid transitions must fail closed.
        let mut store = MppsStore::new(Limits::default());
        let dataset = dataset_with_status("IN PROGRESS", "1.2.3");
        store.ingest_update(&dataset).expect("insert");
        let mut completed = dataset_with_status("COMPLETED", "1.2.3");
        completed.insert(Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string()),
        ).unwrap());
        completed.insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("130000".to_string()),
        ).unwrap());
        let outcome = store.ingest_update(&completed).expect("update");
        assert_eq!(outcome, IngestOutcome::Updated);

        let reverted = dataset_with_status("IN PROGRESS", "1.2.3");
        let err = store.ingest_update(&reverted).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
    }

    #[test]
    fn mpps_enforces_limit() {
        // REQ-MPPS-353: store size is bounded by max_dataset_elements.
        let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
        let mut store = MppsStore::new(limits);
        let d1 = dataset_with_status("IN PROGRESS", "1.2.3");
        let d2 = dataset_with_status("IN PROGRESS", "1.2.4");
        store.ingest_update(&d1).expect("insert");
        let err = store.ingest_update(&d2).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn mpps_service_persists_and_emits_audit() {
        // REQ-MPPS-354, REQ-AUDIT-350: persisted MPPS ingest emits deterministic audit events.
        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            let mut guard = events_handle.lock().expect("audit lock");
            guard.push(event);
            Ok(())
        });

        let mut service = MppsService::new(MppsServiceConfig {
            limits: Limits::default(),
            audit: Some(audit),
        });

        let in_progress = dataset_with_status("IN PROGRESS", "1.2.3");
        let inserted = service.ingest(&in_progress).expect("insert");
        assert_eq!(inserted, IngestOutcome::Inserted);

        let mut completed = dataset_with_status("COMPLETED", "1.2.3");
        completed.insert(Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string()),
        ).unwrap());
        completed.insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("130000".to_string()),
        ).unwrap());
        let updated = service.ingest(&completed).expect("update");
        assert_eq!(updated, IngestOutcome::Updated);

        let persisted = service.get("1.2.3").expect("persisted update");
        assert_eq!(persisted.status, MppsStatus::Completed);

        let events = events.lock().expect("audit lock");
        assert_eq!(events.len(), 2);
        assert!(events[0].fields.iter().any(|field| {
            field.key == "operation"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "ingest")
        }));
        assert!(events[1].fields.iter().any(|field| {
            field.key == "status"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "COMPLETED")
        }));
    }

    #[test]
    fn durable_mpps_store_recovers_snapshot() {
        // REQ-MPPS-354: durable MPPS snapshot reload preserves tracked transitions.
        let path = temp_snapshot_path("mpps");
        let mut store = MppsStore::open(Limits::default(), &path).expect("open");
        let mut completed = dataset_with_status("COMPLETED", "1.2.3");
        completed.insert(Element::new(TAG_END_DATE, Vr::Da, Value::Str("20240101".to_string()),
        ).unwrap());
        completed.insert(Element::new(TAG_END_TIME, Vr::Tm, Value::Str("121500".to_string()),
        ).unwrap());
        store.ingest_update(&completed).expect("ingest");
        drop(store);

        let reopened = MppsStore::open(Limits::default(), &path).expect("reopen");
        assert_eq!(reopened.len(), 1);
        assert!(reopened.get("1.2.3").is_some());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn durable_mpps_store_rejects_invalid_header() {
        // REQ-MPPS-354: corrupted MPPS snapshot headers fail closed.
        let path = temp_snapshot_path("mpps_corrupt");
        fs::write(&path, b"BAD").expect("write");
        let err = MppsStore::open(Limits::default(), &path).expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
        ));
        let _ = fs::remove_file(path);
    }
}
