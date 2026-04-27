#![deny(missing_docs)]

//! Modality Worklist dataset validation and deterministic response formation.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{
    enforce_limit, Dataset, Element, Error, ErrorKind, Limits, Result, Tag, Value, Vr,
};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
const TAG_SCHEDULED_STATION_AE_TITLE: Tag = Tag(0x0040, 0x0001);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
const TAG_REQUESTED_PROCEDURE_ID: Tag = Tag(0x0040, 0x1001);
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
const WORKLIST_MAGIC: &[u8; 5] = b"DWL01";

/// Validated Modality Worklist entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistItem {
    /// Scheduled Procedure Step ID.
    pub scheduled_step_id: String,
    /// Modality code.
    pub modality: String,
    /// Scheduled Procedure Step start date (DA).
    pub start_date: String,
    /// Scheduled Procedure Step start time (TM).
    pub start_time: String,
    /// Requested Procedure ID, if supplied.
    pub requested_procedure_id: Option<String>,
    /// Scheduled Station AE Title, if supplied.
    pub scheduled_station_ae_title: Option<String>,
    /// Patient ID, if supplied.
    pub patient_id: Option<String>,
    /// Accession Number, if supplied.
    pub accession_number: Option<String>,
}

/// Audit callback for worklist service operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

/// Deterministic worklist query filter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorklistQuery {
    /// Optional modality filter.
    pub modality: Option<String>,
    /// Optional Scheduled Procedure Step ID filter.
    pub scheduled_step_id: Option<String>,
    /// Optional Patient ID filter.
    pub patient_id: Option<String>,
    /// Optional Requested Procedure ID filter.
    pub requested_procedure_id: Option<String>,
}

/// Outcome of upserting a worklist item into the persisted store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsertOutcome {
    /// New item inserted.
    Inserted,
    /// Existing item updated.
    Updated,
    /// Existing item unchanged.
    Duplicate,
}

/// Worklist store with deterministic query behavior and optional durable persistence.
pub struct WorklistStore {
    limits: Limits,
    items_by_step_id: BTreeMap<String, WorklistItem>,
    audit: Option<AuditCallback>,
    persistence: Option<PathBuf>,
}

impl fmt::Debug for WorklistStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorklistStore")
            .field("limits", &self.limits)
            .field("items_len", &self.items_by_step_id.len())
            .field("audit", &self.audit.is_some())
            .field("persistence", &self.persistence)
            .finish()
    }
}

impl WorklistStore {
    /// Create an empty worklist store.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            items_by_step_id: BTreeMap::new(),
            audit: None,
            persistence: None,
        }
    }

    /// Create an empty worklist store with an optional audit callback.
    pub fn with_audit(limits: Limits, audit: Option<AuditCallback>) -> Self {
        Self {
            limits,
            items_by_step_id: BTreeMap::new(),
            audit,
            persistence: None,
        }
    }

    /// Open a worklist store backed by a durable snapshot file.
    pub fn open(limits: Limits, path: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_audit(limits, path, None)
    }

    /// Open a worklist store with durable snapshot persistence and optional audit callback.
    pub fn open_with_audit(
        limits: Limits,
        path: impl AsRef<Path>,
        audit: Option<AuditCallback>,
    ) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        ensure_parent_dir(&path)?;
        let items_by_step_id = load_worklist_snapshot(&path, &limits)?;
        Ok(Self {
            limits,
            items_by_step_id,
            audit,
            persistence: Some(path),
        })
    }

    /// Upsert a worklist dataset into persisted state.
    pub fn upsert_dataset(&mut self, dataset: &Dataset) -> Result<UpsertOutcome> {
        let item = validate_worklist_item(dataset, &self.limits)?;
        let key = item.scheduled_step_id.clone();
        let previous = self.items_by_step_id.get(&key).cloned();
        let outcome = match previous.as_ref() {
            None => {
                enforce_limit(
                    "max_dataset_elements",
                    (self.items_by_step_id.len() as u64) + 1,
                    self.limits.max_dataset_elements(),
                )?;
                self.items_by_step_id.insert(key.clone(), item.clone());
                UpsertOutcome::Inserted
            }
            Some(existing) if existing == &item => UpsertOutcome::Duplicate,
            Some(_) => {
                self.items_by_step_id.insert(key.clone(), item.clone());
                UpsertOutcome::Updated
            }
        };
        if matches!(outcome, UpsertOutcome::Inserted | UpsertOutcome::Updated) {
            if let Err(err) = self.persist_snapshot() {
                match previous {
                    Some(previous) => {
                        self.items_by_step_id.insert(key, previous);
                    }
                    None => {
                        self.items_by_step_id.remove(&item.scheduled_step_id);
                    }
                }
                return Err(err);
            }
        }
        record_worklist_audit(&self.audit, "upsert", Some(&item), Some(outcome), None)?;
        Ok(outcome)
    }

    /// Query persisted worklist items and return deterministic datasets.
    pub fn query(&self, query: &WorklistQuery) -> Result<Vec<Dataset>> {
        validate_query_filters(query, &self.limits)?;
        let mut filtered = Vec::new();
        for item in self.items_by_step_id.values() {
            if !matches_query(item, query) {
                continue;
            }
            filtered.push(item.clone());
        }
        let response = build_worklist_response(&filtered, &self.limits)?;
        record_worklist_audit(
            &self.audit,
            "query",
            None,
            None,
            Some(response.len() as u64),
        )?;
        Ok(response)
    }

    /// Return number of persisted worklist items.
    pub fn len(&self) -> usize {
        self.items_by_step_id.len()
    }

    /// Return true when no worklist items are persisted.
    pub fn is_empty(&self) -> bool {
        self.items_by_step_id.is_empty()
    }

    fn persist_snapshot(&self) -> Result<()> {
        let Some(path) = &self.persistence else {
            return Ok(());
        };
        save_worklist_snapshot(path, &self.items_by_step_id)
    }
}

/// Validate a Modality Worklist dataset and extract required fields.
pub fn validate_worklist_item(dataset: &Dataset, limits: &Limits) -> Result<WorklistItem> {
    let sequence = dataset
        .get(TAG_SPS_SEQUENCE)
        .ok_or_else(|| missing_required_tag(TAG_SPS_SEQUENCE))?;
    let items = match sequence.value() {
        Value::Sequence(items) => items,
        _ => {
            return Err(decode_error(
                "Scheduled Procedure Step Sequence must be a sequence",
            ))
        }
    };
    enforce_limit(
        "max_dataset_elements",
        items.len() as u64,
        limits.max_dataset_elements(),
    )?;
    if items.len() != 1 {
        return Err(decode_error(
            "Scheduled Procedure Step Sequence must contain exactly one item",
        ));
    }
    let item = &items[0];

    let scheduled_step_id = require_str(item, TAG_SPS_ID, limits)?;
    let modality = require_str(item, TAG_MODALITY, limits)?;
    let start_date = require_str(item, TAG_SPS_START_DATE, limits)?;
    let start_time = require_str(item, TAG_SPS_START_TIME, limits)?;
    let requested_procedure_id = optional_str(item, TAG_REQUESTED_PROCEDURE_ID, limits)?;
    let scheduled_station_ae_title = optional_str(item, TAG_SCHEDULED_STATION_AE_TITLE, limits)?;
    let patient_id = optional_str(dataset, TAG_PATIENT_ID, limits)?;
    let accession_number = optional_str(dataset, TAG_ACCESSION_NUMBER, limits)?;

    Ok(WorklistItem {
        scheduled_step_id,
        modality,
        start_date,
        start_time,
        requested_procedure_id,
        scheduled_station_ae_title,
        patient_id,
        accession_number,
    })
}

/// Build a deterministic worklist response from validated items.
pub fn build_worklist_response(items: &[WorklistItem], limits: &Limits) -> Result<Vec<Dataset>> {
    enforce_limit(
        "max_dataset_elements",
        items.len() as u64,
        limits.max_dataset_elements(),
    )?;
    let mut ordered: Vec<WorklistItem> = items.to_vec();
    ordered.sort_by(|a, b| {
        (
            a.start_date.as_str(),
            a.start_time.as_str(),
            a.scheduled_step_id.as_str(),
            a.modality.as_str(),
        )
            .cmp(&(
                b.start_date.as_str(),
                b.start_time.as_str(),
                b.scheduled_step_id.as_str(),
                b.modality.as_str(),
            ))
    });
    Ok(ordered.into_iter().map(worklist_item_to_dataset).collect())
}

fn validate_query_filters(query: &WorklistQuery, limits: &Limits) -> Result<()> {
    for value in [
        query.modality.as_deref(),
        query.scheduled_step_id.as_deref(),
        query.patient_id.as_deref(),
        query.requested_procedure_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
        )?;
        if value.is_empty() {
            return Err(decode_error("query filter values must not be empty"));
        }
    }
    Ok(())
}

fn matches_query(item: &WorklistItem, query: &WorklistQuery) -> bool {
    if let Some(modality) = &query.modality {
        if &item.modality != modality {
            return false;
        }
    }
    if let Some(step_id) = &query.scheduled_step_id {
        if &item.scheduled_step_id != step_id {
            return false;
        }
    }
    if let Some(patient_id) = &query.patient_id {
        if item.patient_id.as_deref() != Some(patient_id.as_str()) {
            return false;
        }
    }
    if let Some(requested_procedure_id) = &query.requested_procedure_id {
        if item.requested_procedure_id.as_deref() != Some(requested_procedure_id.as_str()) {
            return false;
        }
    }
    true
}

fn record_worklist_audit(
    audit: &Option<AuditCallback>,
    operation: &'static str,
    item: Option<&WorklistItem>,
    outcome: Option<UpsertOutcome>,
    result_count: Option<u64>,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };
    let mut fields = vec![AuditField {
        key: "operation",
        value: AuditValue::Plain(operation.to_string()),
    }];
    if let Some(outcome) = outcome {
        fields.push(AuditField {
            key: "outcome",
            value: AuditValue::Plain(match outcome {
                UpsertOutcome::Inserted => "inserted".to_string(),
                UpsertOutcome::Updated => "updated".to_string(),
                UpsertOutcome::Duplicate => "duplicate".to_string(),
            }),
        });
    }
    if let Some(result_count) = result_count {
        fields.push(AuditField {
            key: "result_count",
            value: AuditValue::Plain(result_count.to_string()),
        });
    }
    if let Some(item) = item {
        fields.push(AuditField {
            key: "scheduled_step_id",
            value: AuditValue::Sensitive(item.scheduled_step_id.clone()),
        });
        if let Some(patient_id) = &item.patient_id {
            fields.push(AuditField {
                key: "patient_id",
                value: AuditValue::Sensitive(patient_id.clone()),
            });
        }
    }
    callback(AuditEvent {
        kind: AuditEventKind::ServiceEvent,
        fields,
    })
}

fn worklist_item_to_dataset(item: WorklistItem) -> Dataset {
    let mut sps_item = Dataset::new();
    sps_item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(item.scheduled_step_id)).unwrap());
    sps_item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(item.modality)).unwrap());
    sps_item.insert(Element::new(TAG_SPS_START_DATE, Vr::Da, Value::Str(item.start_date)).unwrap());
    sps_item.insert(Element::new(TAG_SPS_START_TIME, Vr::Tm, Value::Str(item.start_time)).unwrap());
    if let Some(requested_procedure_id) = item.requested_procedure_id {
        sps_item.insert(
            Element::new(
                TAG_REQUESTED_PROCEDURE_ID,
                Vr::Sh,
                Value::Str(requested_procedure_id),
            )
            .unwrap(),
        );
    }
    if let Some(scheduled_station_ae_title) = item.scheduled_station_ae_title {
        sps_item.insert(
            Element::new(
                TAG_SCHEDULED_STATION_AE_TITLE,
                Vr::Ae,
                Value::Str(scheduled_station_ae_title),
            )
            .unwrap(),
        );
    }

    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![sps_item])).unwrap());
    if let Some(patient_id) = item.patient_id {
        dataset.insert(Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str(patient_id)).unwrap());
    }
    if let Some(accession_number) = item.accession_number {
        dataset.insert(
            Element::new(TAG_ACCESSION_NUMBER, Vr::Sh, Value::Str(accession_number)).unwrap(),
        );
    }
    dataset
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| io_error("create worklist directory", err))?;
    }
    Ok(())
}

fn load_worklist_snapshot(path: &Path, limits: &Limits) -> Result<BTreeMap<String, WorklistItem>> {
    if !path.exists() {
        save_worklist_snapshot(path, &BTreeMap::new())?;
    }
    let mut bytes = fs::read(path).map_err(|err| io_error("read worklist snapshot", err))?;
    if bytes.is_empty() {
        save_worklist_snapshot(path, &BTreeMap::new())?;
        bytes = fs::read(path).map_err(|err| io_error("re-read worklist snapshot", err))?;
    }
    if bytes.len() < WORKLIST_MAGIC.len() || &bytes[..WORKLIST_MAGIC.len()] != WORKLIST_MAGIC {
        return Err(decode_error("invalid worklist snapshot header"));
    }

    let mut offset = WORKLIST_MAGIC.len();
    let count = read_u64(&bytes, &mut offset)? as usize;
    enforce_limit(
        "max_dataset_elements",
        count as u64,
        limits.max_dataset_elements(),
    )?;

    let mut items = BTreeMap::new();
    for _ in 0..count {
        let item = WorklistItem {
            scheduled_step_id: read_required_str(&bytes, &mut offset, limits)?,
            modality: read_required_str(&bytes, &mut offset, limits)?,
            start_date: read_required_str(&bytes, &mut offset, limits)?,
            start_time: read_required_str(&bytes, &mut offset, limits)?,
            requested_procedure_id: read_optional_str(&bytes, &mut offset, limits)?,
            scheduled_station_ae_title: read_optional_str(&bytes, &mut offset, limits)?,
            patient_id: read_optional_str(&bytes, &mut offset, limits)?,
            accession_number: read_optional_str(&bytes, &mut offset, limits)?,
        };
        if items.insert(item.scheduled_step_id.clone(), item).is_some() {
            return Err(decode_error(
                "duplicate scheduled step id in worklist snapshot",
            ));
        }
    }
    if offset != bytes.len() {
        return Err(decode_error("worklist snapshot contains trailing bytes"));
    }
    Ok(items)
}

fn save_worklist_snapshot(path: &Path, items: &BTreeMap<String, WorklistItem>) -> Result<()> {
    let tmp_path = snapshot_temp_path(path);
    let mut file =
        File::create(&tmp_path).map_err(|err| io_error("create worklist snapshot", err))?;
    file.write_all(WORKLIST_MAGIC)
        .and_then(|_| file.write_all(&(items.len() as u64).to_le_bytes()))
        .map_err(|err| io_error("write worklist snapshot header", err))?;
    for item in items.values() {
        write_str(&mut file, &item.scheduled_step_id)?;
        write_str(&mut file, &item.modality)?;
        write_str(&mut file, &item.start_date)?;
        write_str(&mut file, &item.start_time)?;
        write_optional_str(&mut file, item.requested_procedure_id.as_deref())?;
        write_optional_str(&mut file, item.scheduled_station_ae_title.as_deref())?;
        write_optional_str(&mut file, item.patient_id.as_deref())?;
        write_optional_str(&mut file, item.accession_number.as_deref())?;
    }
    file.sync_all()
        .map_err(|err| io_error("sync worklist snapshot", err))?;
    fs::rename(&tmp_path, path).map_err(|err| io_error("replace worklist snapshot", err))?;
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    OpenOptions::new()
        .read(true)
        .open(directory)
        .and_then(|dir| dir.sync_all())
        .map_err(|err| io_error("sync worklist directory", err))?;
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
        .map_err(|err| io_error("write worklist string field", err))?;
    Ok(())
}

fn write_optional_str(file: &mut File, value: Option<&str>) -> Result<()> {
    match value {
        Some(text) => {
            file.write_all(&[1u8])
                .map_err(|err| io_error("write worklist optional marker", err))?;
            write_str(file, text)?;
        }
        None => {
            file.write_all(&[0u8])
                .map_err(|err| io_error("write worklist optional marker", err))?;
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
        return Err(decode_error("truncated worklist snapshot"));
    }
    let out = &bytes[*offset..end];
    *offset = end;
    Ok(out)
}

fn read_required_str(bytes: &[u8], offset: &mut usize, limits: &Limits) -> Result<String> {
    let len = read_u32(bytes, offset)? as usize;
    if len == 0 {
        return Err(decode_error("required worklist field is empty"));
    }
    let raw = read_slice(bytes, offset, len)?;
    enforce_limit(
        "max_string_bytes",
        raw.len() as u64,
        limits.max_string_bytes(),
    )?;
    let out =
        std::str::from_utf8(raw).map_err(|_| decode_error("worklist field is not valid UTF-8"))?;
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

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-worklist".to_string(),
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
    use std::fs;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn limits() -> Limits {
        Limits::default()
    }

    fn worklist_dataset(step_id: &str, modality: &str, date: &str, time: &str) -> Dataset {
        let mut item = Dataset::new();
        item.insert(Element::new(TAG_SPS_ID, Vr::Sh, Value::Str(step_id.to_string())).unwrap());
        item.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str(modality.to_string())).unwrap());
        item.insert(
            Element::new(TAG_SPS_START_DATE, Vr::Da, Value::Str(date.to_string())).unwrap(),
        );
        item.insert(
            Element::new(TAG_SPS_START_TIME, Vr::Tm, Value::Str(time.to_string())).unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(vec![item])).unwrap());
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
    fn worklist_requires_sps_sequence() {
        // REQ-WL-300: missing Scheduled Procedure Step Sequence must fail closed.
        let dataset = Dataset::new();
        let err = validate_worklist_item(&dataset, &limits()).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn worklist_rejects_empty_sequence() {
        // REQ-WL-300: empty sequence must fail closed.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_SPS_SEQUENCE, Vr::Sq, Value::Sequence(Vec::new())).unwrap());
        let err = validate_worklist_item(&dataset, &limits()).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn worklist_orders_deterministically() {
        // REQ-WL-301: response ordering is deterministic.
        let a = WorklistItem {
            scheduled_step_id: "B".to_string(),
            modality: "CT".to_string(),
            start_date: "20240102".to_string(),
            start_time: "120000".to_string(),
            requested_procedure_id: None,
            scheduled_station_ae_title: None,
            patient_id: None,
            accession_number: None,
        };
        let b = WorklistItem {
            scheduled_step_id: "A".to_string(),
            modality: "MR".to_string(),
            start_date: "20240101".to_string(),
            start_time: "080000".to_string(),
            requested_procedure_id: None,
            scheduled_station_ae_title: None,
            patient_id: None,
            accession_number: None,
        };
        let ordered = build_worklist_response(&[a, b], &limits()).expect("response");
        let first = validate_worklist_item(&ordered[0], &limits()).expect("item");
        assert_eq!(first.start_date, "20240101");
    }

    #[test]
    fn worklist_enforces_limits() {
        // REQ-WL-302: string limits must be enforced.
        let limits = Limits::builder().max_string_bytes(2).build().unwrap();
        let dataset = worklist_dataset("AAA", "CT", "20240101", "120000");
        let err = validate_worklist_item(&dataset, &limits).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn response_enforces_item_limit() {
        // REQ-WL-302: response size is bounded by max_dataset_elements.
        let limits = Limits::builder().max_dataset_elements(1).build().unwrap();
        let item = validate_worklist_item(
            &worklist_dataset("A", "CT", "20240101", "090000"),
            &Limits::default(),
        )
        .expect("item");
        let err = build_worklist_response(&[item.clone(), item], &limits).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn worklist_store_persists_and_queries_items() {
        // REQ-WL-303: persisted worklist queries are deterministic and filterable.
        let mut store = WorklistStore::new(Limits::default());
        let mut dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
        dataset.insert(
            Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_ACCESSION_NUMBER,
                Vr::Sh,
                Value::Str("ACC123".to_string()),
            )
            .unwrap(),
        );
        let outcome = store.upsert_dataset(&dataset).expect("upsert");
        assert_eq!(outcome, UpsertOutcome::Inserted);

        let response = store
            .query(&WorklistQuery {
                modality: Some("CT".to_string()),
                scheduled_step_id: None,
                patient_id: Some("PATIENT_A".to_string()),
                requested_procedure_id: None,
            })
            .expect("query");
        assert_eq!(response.len(), 1);
        let item = validate_worklist_item(&response[0], &Limits::default()).expect("item");
        assert_eq!(item.patient_id.as_deref(), Some("PATIENT_A"));
        assert_eq!(item.accession_number.as_deref(), Some("ACC123"));
    }

    #[test]
    fn worklist_store_emits_audit_for_upsert_and_query() {
        // REQ-WL-303, REQ-AUDIT-350: persisted operations emit deterministic audit events.
        let events = Arc::new(Mutex::new(Vec::<AuditEvent>::new()));
        let events_handle = Arc::clone(&events);
        let audit: AuditCallback = Arc::new(move |event| {
            let mut guard = events_handle.lock().expect("audit lock");
            guard.push(event);
            Ok(())
        });
        let mut store = WorklistStore::with_audit(Limits::default(), Some(audit));
        let dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
        store.upsert_dataset(&dataset).expect("upsert");
        let _ = store.query(&WorklistQuery::default()).expect("query");

        let events = events.lock().expect("audit lock");
        assert_eq!(events.len(), 2);
        assert!(events[0].fields.iter().any(|field| {
            field.key == "operation"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "upsert")
        }));
        assert!(events[1].fields.iter().any(|field| {
            field.key == "operation"
                && matches!(field.value, AuditValue::Plain(ref v) if v == "query")
        }));
    }

    #[test]
    fn durable_worklist_store_recovers_snapshot() {
        // REQ-WL-303: durable snapshot reload must preserve deterministic worklist state.
        let path = temp_snapshot_path("worklist");
        let mut store = WorklistStore::open(Limits::default(), &path).expect("open");
        let dataset = worklist_dataset("STEP1", "CT", "20240101", "090000");
        store.upsert_dataset(&dataset).expect("upsert");
        drop(store);

        let reopened = WorklistStore::open(Limits::default(), &path).expect("reopen");
        assert_eq!(reopened.len(), 1);
        let rows = reopened.query(&WorklistQuery::default()).expect("query");
        assert_eq!(rows.len(), 1);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn durable_worklist_store_rejects_invalid_header() {
        // REQ-WL-303: corrupted worklist snapshots fail closed.
        let path = temp_snapshot_path("worklist_corrupt");
        fs::write(&path, b"BAD").expect("write");
        let err = WorklistStore::open(Limits::default(), &path).expect_err("expected error");
        assert!(matches!(
            err.kind(),
            ErrorKind::DecodeError { .. } | ErrorKind::IoError { .. }
        ));
        let _ = fs::remove_file(path);
    }
}
