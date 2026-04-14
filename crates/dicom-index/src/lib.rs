#![deny(missing_docs)]

//! Metadata index for Study/Series/Instance ordering.

use dicom_core::{
    enforce_limit, validate_uid_strict, Dataset, Error, ErrorKind, Limits, Result, Tag,
};
use std::collections::BTreeMap;

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);

/// High-frequency index strategy row for query/workflow hot keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStrategyRow {
    /// Indexed tag.
    pub tag: Tag,
    /// Canonical keyword.
    pub keyword: &'static str,
    /// Strategy backend.
    pub backend: &'static str,
    /// Cardinality expectation.
    pub cardinality: &'static str,
}

/// Deterministic high-frequency indexing strategy used by QIDO/MWL/MPPS hot paths.
pub fn high_frequency_index_strategy() -> Vec<IndexStrategyRow> {
    vec![
        IndexStrategyRow {
            tag: TAG_STUDY_UID,
            keyword: "StudyInstanceUID",
            backend: "BTreeMap(studies)",
            cardinality: "1:N series",
        },
        IndexStrategyRow {
            tag: TAG_SERIES_UID,
            keyword: "SeriesInstanceUID",
            backend: "BTreeMap(series)",
            cardinality: "1:N instances",
        },
        IndexStrategyRow {
            tag: TAG_SOP_UID,
            keyword: "SOPInstanceUID",
            backend: "BTreeMap(by_sop_uid)",
            cardinality: "1:1 instance",
        },
        IndexStrategyRow {
            tag: TAG_SOP_CLASS_UID,
            keyword: "SOPClassUID",
            backend: "instance payload",
            cardinality: "single uid",
        },
    ]
}

/// Metadata required to index a single instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedInstance {
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID.
    pub series_uid: String,
    /// SOP Instance UID.
    pub sop_instance_uid: String,
    /// SOP Class UID.
    pub sop_class_uid: String,
    /// Canonical hash of the source bytes.
    pub source_hash: String,
    /// Source byte length.
    pub byte_len: u64,
}

/// Result of an index insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// New instance inserted into the index.
    Inserted,
    /// Duplicate instance hash already present.
    DuplicateHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstanceEntry {
    sop_instance_uid: String,
    sop_class_uid: String,
    source_hash: String,
    byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeriesEntry {
    instances: BTreeMap<String, InstanceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StudyEntry {
    series: BTreeMap<String, SeriesEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstanceKey {
    study_uid: String,
    series_uid: String,
    source_hash: String,
}

/// Deterministic metadata index for studies, series, and instances.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    limits: Limits,
    studies: BTreeMap<String, StudyEntry>,
    by_sop_uid: BTreeMap<String, InstanceKey>,
    total_instances: u64,
}

impl Index {
    /// Create an empty index with the provided limits.
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            studies: BTreeMap::new(),
            by_sop_uid: BTreeMap::new(),
            total_instances: 0,
        }
    }

    /// Insert an indexed instance, enforcing deterministic ordering and limits.
    pub fn insert(&mut self, instance: IndexedInstance) -> Result<InsertOutcome> {
        if let Some(existing) = self.by_sop_uid.get(&instance.sop_instance_uid) {
            if existing.source_hash == instance.source_hash {
                return Ok(InsertOutcome::DuplicateHash);
            }
            return Err(integrity_error("SOP Instance UID conflict"));
        }

        enforce_limit(
            "max_dataset_elements",
            self.total_instances + 1,
            self.limits.max_dataset_elements,
        )?;

        let study_entry = self
            .studies
            .entry(instance.study_uid.clone())
            .or_insert_with(|| StudyEntry {
                series: BTreeMap::new(),
            });
        let series_entry = study_entry
            .series
            .entry(instance.series_uid.clone())
            .or_insert_with(|| SeriesEntry {
                instances: BTreeMap::new(),
            });

        series_entry.instances.insert(
            instance.sop_instance_uid.clone(),
            InstanceEntry {
                sop_instance_uid: instance.sop_instance_uid.clone(),
                sop_class_uid: instance.sop_class_uid.clone(),
                source_hash: instance.source_hash.clone(),
                byte_len: instance.byte_len,
            },
        );

        self.by_sop_uid.insert(
            instance.sop_instance_uid.clone(),
            InstanceKey {
                study_uid: instance.study_uid,
                series_uid: instance.series_uid,
                source_hash: instance.source_hash,
            },
        );

        self.total_instances += 1;
        Ok(InsertOutcome::Inserted)
    }

    /// Return total number of indexed instances.
    pub fn total_instances(&self) -> u64 {
        self.total_instances
    }

    /// Return study UIDs in deterministic order.
    pub fn study_uids(&self) -> Vec<String> {
        self.studies.keys().cloned().collect()
    }

    /// Return series UIDs for a study in deterministic order.
    pub fn series_uids(&self, study_uid: &str) -> Option<Vec<String>> {
        self.studies
            .get(study_uid)
            .map(|study| study.series.keys().cloned().collect::<Vec<String>>())
    }

    /// Return instance UIDs for a series in deterministic order.
    pub fn instance_uids(&self, study_uid: &str, series_uid: &str) -> Option<Vec<String>> {
        self.studies.get(study_uid).and_then(|study| {
            study
                .series
                .get(series_uid)
                .map(|series| series.instances.keys().cloned().collect::<Vec<String>>())
        })
    }

    /// Return the canonical hash for a SOP Instance UID if present.
    pub fn hash_for_sop(&self, sop_instance_uid: &str) -> Option<&str> {
        self.by_sop_uid
            .get(sop_instance_uid)
            .map(|key| key.source_hash.as_str())
    }

    /// Return `(study_uid, series_uid, source_hash)` for a SOP Instance UID, if present.
    pub fn location_for_sop(&self, sop_instance_uid: &str) -> Option<(String, String, String)> {
        self.by_sop_uid.get(sop_instance_uid).map(|key| {
            (
                key.study_uid.clone(),
                key.series_uid.clone(),
                key.source_hash.clone(),
            )
        })
    }
}

/// Extract an indexed instance from a dataset and limits.
pub fn extract_indexed_instance(
    dataset: &Dataset,
    limits: &Limits,
    source_hash: String,
    byte_len: u64,
) -> Result<IndexedInstance> {
    let study_uid = require_uid(dataset, TAG_STUDY_UID, limits)?;
    let series_uid = require_uid(dataset, TAG_SERIES_UID, limits)?;
    let sop_instance_uid = require_uid(dataset, TAG_SOP_UID, limits)?;
    let sop_class_uid = require_uid(dataset, TAG_SOP_CLASS_UID, limits)?;

    Ok(IndexedInstance {
        study_uid,
        series_uid,
        sop_instance_uid,
        sop_class_uid,
        source_hash,
        byte_len,
    })
}

fn require_uid(dataset: &Dataset, tag: Tag, limits: &Limits) -> Result<String> {
    let value = dataset
        .get_uid(tag)
        .ok_or_else(|| missing_required_tag(tag))?;
    enforce_limit(
        "max_string_bytes",
        value.len() as u64,
        limits.max_string_bytes,
    )?;
    validate_uid_strict(tag, value)?;
    Ok(value.to_string())
}

fn missing_required_tag(tag: Tag) -> Box<Error> {
    Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        "missing required tag",
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

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Dataset, Element, Value, Vr};

    fn limits() -> Limits {
        Limits::default()
    }

    fn dataset_with_uids(study: &str, series: &str, sop: &str, sop_class: &str) -> Dataset {
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid(study.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid(series.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SOP_UID,
            vr: Vr::Ui,
            value: Value::Uid(sop.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SOP_CLASS_UID,
            vr: Vr::Ui,
            value: Value::Uid(sop_class.to_string()),
        });
        dataset
    }

    #[test]
    fn extract_requires_required_uids() {
        // REQ-META-300: required UIDs must be present and valid.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        let err = extract_indexed_instance(&dataset, &limits(), "hash".to_string(), 10)
            .expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::MissingRequiredTag { .. }));
    }

    #[test]
    fn index_orders_deterministically() {
        // REQ-META-301: index ordering is deterministic by UID.
        let mut index = Index::new(limits());
        let d1 = dataset_with_uids("2.2", "9.9", "1.1", "1.2.840.10008.1.1");
        let d2 = dataset_with_uids("1.1", "5.5", "2.2", "1.2.840.10008.1.1");
        let i1 = extract_indexed_instance(&d1, &limits(), "h1".to_string(), 10).unwrap();
        let i2 = extract_indexed_instance(&d2, &limits(), "h2".to_string(), 10).unwrap();
        index.insert(i1).unwrap();
        index.insert(i2).unwrap();
        assert_eq!(
            index.study_uids(),
            vec!["1.1".to_string(), "2.2".to_string()]
        );
        assert_eq!(index.series_uids("1.1"), Some(vec!["5.5".to_string()]));
    }

    #[test]
    fn index_rejects_uid_conflict() {
        // REQ-META-301: SOP UID conflicts with different hashes must fail closed.
        let mut index = Index::new(limits());
        let dataset = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
        let i1 = extract_indexed_instance(&dataset, &limits(), "h1".to_string(), 10).unwrap();
        let i2 = extract_indexed_instance(&dataset, &limits(), "h2".to_string(), 10).unwrap();
        index.insert(i1).unwrap();
        let err = index.insert(i2).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::IntegrityError { .. }));
    }

    #[test]
    fn index_enforces_max_instances() {
        // REQ-META-302: max_dataset_elements bounds total indexed instances.
        let limits = Limits {
            max_dataset_elements: 1,
            ..Limits::default()
        };
        let mut index = Index::new(limits);
        let d1 = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
        let d2 = dataset_with_uids("1.1", "2.2", "4.4", "1.2.840.10008.1.1");
        let i1 = extract_indexed_instance(&d1, &Limits::default(), "h1".to_string(), 10).unwrap();
        let i2 = extract_indexed_instance(&d2, &Limits::default(), "h2".to_string(), 10).unwrap();
        index.insert(i1).unwrap();
        let err = index.insert(i2).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn location_for_sop_returns_study_series_and_hash() {
        // REQ-META-301: SOP UID lookup returns deterministic UID/hash mapping.
        let mut index = Index::new(limits());
        let dataset = dataset_with_uids("1.1", "2.2", "3.3", "1.2.840.10008.1.1");
        let instance = extract_indexed_instance(&dataset, &Limits::default(), "h1".to_string(), 10)
            .expect("indexed instance");
        index.insert(instance).expect("insert");

        let location = index.location_for_sop("3.3").expect("location");
        assert_eq!(location.0, "1.1");
        assert_eq!(location.1, "2.2");
        assert_eq!(location.2, "h1");
    }

    #[test]
    fn high_frequency_index_strategy_lists_core_uids() {
        let rows = high_frequency_index_strategy();
        assert!(rows.iter().any(|row| row.tag == TAG_STUDY_UID));
        assert!(rows.iter().any(|row| row.tag == TAG_SERIES_UID));
        assert!(rows.iter().any(|row| row.tag == TAG_SOP_UID));
    }
}
