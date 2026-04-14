#![deny(missing_docs)]

//! Deterministic query matching for DICOM Query/Retrieve services.

use dicom_core::{
    enforce_limit, validate_uid_strict, Dataset, Error, ErrorKind, Limits, Result, Tag, Value,
};

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_SOP_UID: Tag = Tag(0x0008, 0x0018);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);

/// Query key coverage row used for deterministic key/cardinality audit output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryKeyCoverageRow {
    /// Tag value for the key.
    pub tag: Tag,
    /// Canonical keyword.
    pub keyword: &'static str,
    /// Lowest supported query level.
    pub min_level: QueryLevel,
    /// Highest supported query level.
    pub max_level: QueryLevel,
    /// Cardinality expectation for request values.
    pub cardinality: &'static str,
}

/// Deterministic query key coverage matrix for QIDO/MWL/MPPS-aligned keys.
pub fn query_key_coverage_matrix() -> Vec<QueryKeyCoverageRow> {
    vec![
        QueryKeyCoverageRow {
            tag: TAG_STUDY_UID,
            keyword: "StudyInstanceUID",
            min_level: QueryLevel::Study,
            max_level: QueryLevel::Instance,
            cardinality: "single uid",
        },
        QueryKeyCoverageRow {
            tag: TAG_SERIES_UID,
            keyword: "SeriesInstanceUID",
            min_level: QueryLevel::Series,
            max_level: QueryLevel::Instance,
            cardinality: "single uid",
        },
        QueryKeyCoverageRow {
            tag: TAG_SOP_UID,
            keyword: "SOPInstanceUID",
            min_level: QueryLevel::Instance,
            max_level: QueryLevel::Instance,
            cardinality: "single uid",
        },
        QueryKeyCoverageRow {
            tag: TAG_MODALITY,
            keyword: "Modality",
            min_level: QueryLevel::Series,
            max_level: QueryLevel::Instance,
            cardinality: "single code",
        },
        QueryKeyCoverageRow {
            tag: TAG_PATIENT_ID,
            keyword: "PatientID",
            min_level: QueryLevel::Study,
            max_level: QueryLevel::Instance,
            cardinality: "single string",
        },
        QueryKeyCoverageRow {
            tag: TAG_ACCESSION_NUMBER,
            keyword: "AccessionNumber",
            min_level: QueryLevel::Study,
            max_level: QueryLevel::Instance,
            cardinality: "single string",
        },
        QueryKeyCoverageRow {
            tag: TAG_STUDY_DATE,
            keyword: "StudyDate",
            min_level: QueryLevel::Study,
            max_level: QueryLevel::Instance,
            cardinality: "single date",
        },
    ]
}

/// Query/Retrieve level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryLevel {
    /// Study-level query.
    Study,
    /// Series-level query.
    Series,
    /// Instance-level query.
    Instance,
}

/// A single query key/value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryKey {
    /// Query tag.
    pub tag: Tag,
    /// Query value (string/UID form).
    pub value: String,
}

/// Query descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Query level.
    pub level: QueryLevel,
    /// Query keys.
    pub keys: Vec<QueryKey>,
}

/// Query match result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryMatch {
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID (if level >= Series).
    pub series_uid: Option<String>,
    /// SOP Instance UID (if level == Instance).
    pub instance_uid: Option<String>,
}

/// Cursor token for deterministic query pagination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryCursor {
    /// Zero-based offset in sorted result order.
    pub offset: usize,
}

/// Paginated query page payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPage {
    /// Page matches in deterministic order.
    pub matches: Vec<QueryMatch>,
    /// Next cursor when more results remain.
    pub next: Option<QueryCursor>,
}

/// Query guardrail configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryGuardrailConfig {
    /// Maximum allowed in-flight query requests.
    pub max_inflight: u64,
    /// Maximum allowed query duration in milliseconds.
    pub max_duration_ms: u64,
}

impl Default for QueryGuardrailConfig {
    fn default() -> Self {
        Self {
            max_inflight: 256,
            max_duration_ms: 30_000,
        }
    }
}

/// Query failure metric class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryFailureMetric {
    /// Malformed or invalid query input.
    MalformedInput,
    /// Query exceeded configured timeout.
    Timeout,
    /// Unsupported query syntax/key.
    UnsupportedSyntax,
    /// Query rejected due to rate limit/backpressure.
    RateLimited,
}

/// Evaluate a query against datasets, returning deterministic matches.
pub fn query(datasets: &[Dataset], query: &Query, limits: &Limits) -> Result<Vec<QueryMatch>> {
    validate_query(query, limits)?;

    let mut matches = Vec::new();
    for dataset in datasets {
        let study_uid = require_uid(dataset, TAG_STUDY_UID, limits)?;
        let series_uid = match query.level {
            QueryLevel::Study => None,
            QueryLevel::Series | QueryLevel::Instance => {
                Some(require_uid(dataset, TAG_SERIES_UID, limits)?)
            }
        };
        let instance_uid = match query.level {
            QueryLevel::Instance => Some(require_uid(dataset, TAG_SOP_UID, limits)?),
            QueryLevel::Study | QueryLevel::Series => None,
        };

        if !matches_query(dataset, query, limits)? {
            continue;
        }

        enforce_limit(
            "max_dataset_elements",
            (matches.len() as u64) + 1,
            limits.max_dataset_elements,
        )?;

        matches.push(QueryMatch {
            study_uid,
            series_uid,
            instance_uid,
        });
    }

    matches.sort_by(|a, b| {
        let a_key = (
            a.study_uid.as_str(),
            a.series_uid.as_deref().unwrap_or(""),
            a.instance_uid.as_deref().unwrap_or(""),
        );
        let b_key = (
            b.study_uid.as_str(),
            b.series_uid.as_deref().unwrap_or(""),
            b.instance_uid.as_deref().unwrap_or(""),
        );
        a_key.cmp(&b_key)
    });

    Ok(matches)
}

/// Paginate deterministic query matches with cursor/offset semantics.
pub fn paginate_query_matches(
    matches: &[QueryMatch],
    cursor: Option<QueryCursor>,
    page_size: usize,
    limits: &Limits,
) -> Result<QueryPage> {
    if page_size == 0 {
        return Err(decode_error("page size must be >= 1"));
    }
    enforce_limit(
        "max_dataset_elements",
        page_size as u64,
        limits.max_dataset_elements,
    )?;
    let offset = cursor.map(|token| token.offset).unwrap_or(0);
    if offset >= matches.len() {
        return Ok(QueryPage {
            matches: Vec::new(),
            next: None,
        });
    }
    let end = (offset + page_size).min(matches.len());
    let page = matches[offset..end].to_vec();
    let next = if end < matches.len() {
        Some(QueryCursor { offset: end })
    } else {
        None
    };
    Ok(QueryPage {
        matches: page,
        next,
    })
}

/// Enforce in-flight and timeout guardrails for query execution.
pub fn enforce_query_guardrails(
    inflight_requests: u64,
    observed_duration_ms: u64,
    config: QueryGuardrailConfig,
) -> Result<()> {
    enforce_limit("max_query_inflight", inflight_requests, config.max_inflight)?;
    enforce_limit(
        "max_query_duration_ms",
        observed_duration_ms,
        config.max_duration_ms,
    )?;
    Ok(())
}

/// Classify query failures into deterministic metric buckets.
pub fn classify_query_failure(error: &Error) -> QueryFailureMetric {
    match &error.kind {
        ErrorKind::LimitExceeded { limit_name, .. } => {
            if *limit_name == "max_query_duration_ms" {
                QueryFailureMetric::Timeout
            } else {
                QueryFailureMetric::RateLimited
            }
        }
        ErrorKind::DecodeError { .. } | ErrorKind::InvalidTagValue { .. } => {
            QueryFailureMetric::MalformedInput
        }
        ErrorKind::UnsupportedSopClass { .. } | ErrorKind::UnsupportedTransferSyntax { .. } => {
            QueryFailureMetric::UnsupportedSyntax
        }
        _ => QueryFailureMetric::MalformedInput,
    }
}

/// Build a query from an identifier dataset, rejecting unsupported keys.
pub fn query_from_identifier(dataset: &Dataset, limits: &Limits) -> Result<Query> {
    enforce_limit(
        "max_dataset_elements",
        dataset.len() as u64,
        limits.max_dataset_elements,
    )?;

    let mut keys = Vec::new();
    let mut seen_study: Option<String> = None;
    let mut seen_series: Option<String> = None;
    let mut seen_instance: Option<String> = None;
    let mut seen_modality: Option<String> = None;
    let mut seen_patient_id: Option<String> = None;
    let mut seen_accession_number: Option<String> = None;
    let mut seen_study_date: Option<String> = None;

    for element in dataset.elements() {
        let tag = element.tag;
        let slot = match tag {
            TAG_STUDY_UID => &mut seen_study,
            TAG_SERIES_UID => &mut seen_series,
            TAG_SOP_UID => &mut seen_instance,
            TAG_MODALITY => &mut seen_modality,
            TAG_PATIENT_ID => &mut seen_patient_id,
            TAG_ACCESSION_NUMBER => &mut seen_accession_number,
            TAG_STUDY_DATE => &mut seen_study_date,
            _ => return Err(decode_error("unsupported query key")),
        };

        let value = match (&element.value, is_uid_key(tag)) {
            (Value::Uid(value), _) => value.as_str(),
            (Value::Str(value), _) => value.as_str(),
            _ if is_uid_key(tag) => return Err(decode_error("query key must be a UID value")),
            _ => return Err(decode_error("query key must be a string value")),
        };
        if let Some(existing) = slot {
            if existing != value {
                return Err(decode_error("conflicting query key values"));
            }
            continue;
        }
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        )?;
        if is_uid_key(tag) {
            validate_uid_strict(tag, value)?;
        } else {
            validate_text_key_value(tag, value, "query key value")?;
        }
        *slot = Some(value.to_string());
        keys.push(QueryKey {
            tag,
            value: value.to_string(),
        });
    }

    let level = if seen_instance.is_some() {
        QueryLevel::Instance
    } else if seen_series.is_some() {
        QueryLevel::Series
    } else {
        QueryLevel::Study
    };

    let query = Query { level, keys };
    validate_query(&query, limits)?;
    Ok(query)
}

fn validate_query(query: &Query, limits: &Limits) -> Result<()> {
    enforce_limit(
        "max_dataset_elements",
        query.keys.len() as u64,
        limits.max_dataset_elements,
    )?;

    for key in &query.keys {
        if !is_supported_key(query.level, key.tag) {
            return Err(decode_error("unsupported query key"));
        }
        enforce_limit(
            "max_string_bytes",
            key.value.len() as u64,
            limits.max_string_bytes,
        )?;
        if is_uid_key(key.tag) {
            validate_uid_strict(key.tag, &key.value)?;
        } else {
            validate_text_key_value(key.tag, &key.value, "query key value")?;
        }
    }

    Ok(())
}

fn matches_query(dataset: &Dataset, query: &Query, limits: &Limits) -> Result<bool> {
    for key in &query.keys {
        if is_uid_key(key.tag) {
            let value = require_uid(dataset, key.tag, limits)?;
            if value != key.value {
                return Ok(false);
            }
        } else {
            let Some(value) = dataset.get_str(key.tag) else {
                return Ok(false);
            };
            enforce_limit(
                "max_string_bytes",
                value.len() as u64,
                limits.max_string_bytes,
            )?;
            validate_text_key_value(key.tag, value, "candidate query value")?;
            if value != key.value {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn is_supported_key(level: QueryLevel, tag: Tag) -> bool {
    match level {
        QueryLevel::Study => matches!(
            tag,
            TAG_STUDY_UID | TAG_PATIENT_ID | TAG_ACCESSION_NUMBER | TAG_STUDY_DATE
        ),
        QueryLevel::Series => matches!(
            tag,
            TAG_STUDY_UID
                | TAG_SERIES_UID
                | TAG_PATIENT_ID
                | TAG_MODALITY
                | TAG_ACCESSION_NUMBER
                | TAG_STUDY_DATE
        ),
        QueryLevel::Instance => matches!(
            tag,
            TAG_STUDY_UID
                | TAG_SERIES_UID
                | TAG_SOP_UID
                | TAG_PATIENT_ID
                | TAG_MODALITY
                | TAG_ACCESSION_NUMBER
                | TAG_STUDY_DATE
        ),
    }
}

fn is_uid_key(tag: Tag) -> bool {
    matches!(tag, TAG_STUDY_UID | TAG_SERIES_UID | TAG_SOP_UID)
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

fn validate_text_key_value(tag: Tag, value: &str, label: &str) -> Result<()> {
    match tag {
        TAG_STUDY_DATE => validate_study_date(value, label),
        TAG_PATIENT_ID | TAG_MODALITY | TAG_ACCESSION_NUMBER => ensure_ascii_graphic(value, label),
        _ => ensure_ascii_graphic(value, label),
    }
}

fn validate_study_date(value: &str, label: &str) -> Result<()> {
    if value.len() != 8 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(decode_error(format!("{label} must be YYYYMMDD")));
    }

    let year = value[0..4]
        .parse::<u32>()
        .map_err(|_| decode_error(format!("{label} must be YYYYMMDD")))?;
    let month = value[4..6]
        .parse::<u32>()
        .map_err(|_| decode_error(format!("{label} must be YYYYMMDD")))?;
    let day = value[6..8]
        .parse::<u32>()
        .map_err(|_| decode_error(format!("{label} must be YYYYMMDD")))?;

    if month == 0 || month > 12 {
        return Err(decode_error(format!("{label} has invalid month")));
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => unreachable!(),
    };
    if day == 0 || day > max_day {
        return Err(decode_error(format!("{label} has invalid day")));
    }
    Ok(())
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn ensure_ascii_graphic(value: &str, label: &str) -> Result<()> {
    if value.bytes().any(|b| !(0x21..=0x7e).contains(&b)) {
        return Err(decode_error(format!(
            "{label} contains non-ASCII characters"
        )));
    }
    Ok(())
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-query".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::{Dataset, Element, Value, Vr};
    use std::sync::Arc;
    use std::thread;

    fn limits() -> Limits {
        Limits::default()
    }

    fn dataset_with_uids(study: &str, series: &str, sop: &str) -> Dataset {
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
        dataset
    }

    fn dataset_with_metadata(
        study: &str,
        series: &str,
        sop: &str,
        patient_id: &str,
        modality: &str,
    ) -> Dataset {
        let mut dataset = dataset_with_uids(study, series, sop);
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str(patient_id.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_MODALITY,
            vr: Vr::Cs,
            value: Value::Str(modality.to_string()),
        });
        dataset
    }

    fn dataset_with_clinical_metadata(
        study: &str,
        series: &str,
        sop: &str,
        patient_id: &str,
        modality: &str,
        accession_number: &str,
        study_date: &str,
    ) -> Dataset {
        let mut dataset = dataset_with_metadata(study, series, sop, patient_id, modality);
        dataset.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Lo,
            value: Value::Str(accession_number.to_string()),
        });
        dataset.insert(Element {
            tag: TAG_STUDY_DATE,
            vr: Vr::Da,
            value: Value::Str(study_date.to_string()),
        });
        dataset
    }

    #[test]
    fn query_matches_study_uid() {
        // REQ-QR-300: only supported UID keys may be used for matching.
        let datasets = vec![
            dataset_with_uids("1.2.3", "2.3.4", "3.4.5"),
            dataset_with_uids("9.9", "8.8", "7.7"),
        ];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![QueryKey {
                tag: TAG_STUDY_UID,
                value: "1.2.3".to_string(),
            }],
        };
        let matches = query(&datasets, &query_request, &limits()).expect("query");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[test]
    fn query_matches_patient_id_at_study_level() {
        // REQ-QR-300: study-level queries support Patient ID in addition to Study UID.
        let datasets = vec![
            dataset_with_metadata("1.2.3", "2.3.4", "3.4.5", "PATIENT_A", "CT"),
            dataset_with_metadata("9.9", "8.8", "7.7", "PATIENT_B", "MR"),
        ];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![QueryKey {
                tag: TAG_PATIENT_ID,
                value: "PATIENT_A".to_string(),
            }],
        };
        let matches = query(&datasets, &query_request, &limits()).expect("query");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[test]
    fn query_matches_modality_at_series_level() {
        // REQ-QR-300: series-level queries support modality filters.
        let datasets = vec![
            dataset_with_metadata("1.2.3", "2.3.4", "3.4.5", "PATIENT_A", "CT"),
            dataset_with_metadata("1.2.3", "2.3.5", "3.4.6", "PATIENT_A", "MR"),
        ];
        let query_request = Query {
            level: QueryLevel::Series,
            keys: vec![
                QueryKey {
                    tag: TAG_STUDY_UID,
                    value: "1.2.3".to_string(),
                },
                QueryKey {
                    tag: TAG_MODALITY,
                    value: "MR".to_string(),
                },
            ],
        };
        let matches = query(&datasets, &query_request, &limits()).expect("query");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].series_uid.as_deref(), Some("2.3.5"));
    }

    #[test]
    fn query_rejects_unsupported_key() {
        // REQ-QR-300: unsupported keys must fail closed.
        let datasets = vec![dataset_with_uids("1.2.3", "2.3.4", "3.4.5")];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![QueryKey {
                tag: Tag(0x0008, 0x1030),
                value: "CT HEAD".to_string(),
            }],
        };
        let err = query(&datasets, &query_request, &limits()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn query_matches_accession_number_and_study_date() {
        // REQ-QR-300: Study-level queries support Accession Number and Study Date filters.
        let datasets = vec![
            dataset_with_clinical_metadata(
                "1.2.3",
                "2.3.4",
                "3.4.5",
                "PATIENT_A",
                "CT",
                "ACC123",
                "20260211",
            ),
            dataset_with_clinical_metadata(
                "9.9",
                "8.8",
                "7.7",
                "PATIENT_B",
                "MR",
                "ACC999",
                "20260101",
            ),
        ];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![
                QueryKey {
                    tag: TAG_ACCESSION_NUMBER,
                    value: "ACC123".to_string(),
                },
                QueryKey {
                    tag: TAG_STUDY_DATE,
                    value: "20260211".to_string(),
                },
            ],
        };
        let matches = query(&datasets, &query_request, &limits()).expect("query");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].study_uid, "1.2.3");
    }

    #[test]
    fn query_rejects_invalid_study_date() {
        // REQ-QR-302: Study Date query values must be exact valid YYYYMMDD strings.
        let datasets = vec![dataset_with_clinical_metadata(
            "1.2.3",
            "2.3.4",
            "3.4.5",
            "PATIENT_A",
            "CT",
            "ACC123",
            "20260211",
        )];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![QueryKey {
                tag: TAG_STUDY_DATE,
                value: "2026-02-11".to_string(),
            }],
        };
        let err = query(&datasets, &query_request, &limits()).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn query_orders_deterministically() {
        // REQ-QR-301: results must be ordered deterministically by UID.
        let datasets = vec![
            dataset_with_uids("2.2", "9.9", "1.1"),
            dataset_with_uids("1.1", "5.5", "2.2"),
        ];
        let query_request = Query {
            level: QueryLevel::Instance,
            keys: Vec::new(),
        };
        let matches = query(&datasets, &query_request, &limits()).expect("query");
        assert_eq!(matches[0].study_uid, "1.1");
        assert_eq!(matches[1].study_uid, "2.2");
    }

    #[test]
    fn query_enforces_limit() {
        // REQ-QR-302: results must be bounded by max_dataset_elements.
        let datasets = vec![
            dataset_with_uids("1.1", "2.2", "3.3"),
            dataset_with_uids("4.4", "5.5", "6.6"),
        ];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: Vec::new(),
        };
        let limits = Limits {
            max_dataset_elements: 1,
            ..Limits::default()
        };
        let err = query(&datasets, &query_request, &limits).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn query_rejects_invalid_uid_value() {
        // REQ-QR-302: invalid UID values must fail closed.
        let datasets = vec![dataset_with_uids("1.2.3", "2.3.4", "3.4.5")];
        let query_request = Query {
            level: QueryLevel::Study,
            keys: vec![QueryKey {
                tag: TAG_STUDY_UID,
                value: "1.2.x".to_string(),
            }],
        };
        let err = query(&datasets, &query_request, &limits()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn query_rejects_candidate_missing_required_uid_for_level() {
        // REQ-QR-303: candidate datasets missing required UIDs must fail closed.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });

        let query_request = Query {
            level: QueryLevel::Series,
            keys: Vec::new(),
        };
        let err = query(&[dataset], &query_request, &limits()).expect_err("expected error");
        assert!(matches!(
            err.kind,
            ErrorKind::MissingRequiredTag {
                tag: TAG_SERIES_UID
            }
        ));
    }

    #[test]
    fn query_rejects_candidate_invalid_required_uid_for_level() {
        // REQ-QR-303: candidate datasets with invalid required UIDs must fail closed.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("2.3.x".to_string()),
        });

        let query_request = Query {
            level: QueryLevel::Series,
            keys: Vec::new(),
        };
        let err = query(&[dataset], &query_request, &limits()).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::InvalidTagValue { .. }));
    }

    #[test]
    fn identifier_query_builds_from_dataset() {
        // REQ-QR-300: supported UID keys are parsed from identifier datasets.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_SERIES_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3.4".to_string()),
        });
        let query = query_from_identifier(&dataset, &limits()).expect("query");
        assert_eq!(query.level, QueryLevel::Series);
        assert_eq!(query.keys.len(), 2);
    }

    #[test]
    fn identifier_query_rejects_unsupported_key() {
        // REQ-QR-300: unsupported keys must fail closed.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: Tag(0x0008, 0x1030),
            vr: Vr::Lo,
            value: Value::Str("CT HEAD".to_string()),
        });
        let err = query_from_identifier(&dataset, &limits()).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn identifier_query_accepts_patient_id() {
        // REQ-QR-300: Patient ID is accepted as a supported identifier key.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str("PATIENT_A".to_string()),
        });
        let query = query_from_identifier(&dataset, &limits()).expect("query");
        assert_eq!(query.level, QueryLevel::Study);
        assert_eq!(query.keys.len(), 1);
        assert_eq!(query.keys[0].tag, TAG_PATIENT_ID);
        assert_eq!(query.keys[0].value, "PATIENT_A");
    }

    #[test]
    fn identifier_query_accepts_accession_and_study_date() {
        // REQ-QR-300: Accession Number and Study Date are accepted identifier keys.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Lo,
            value: Value::Str("ACC123".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_STUDY_DATE,
            vr: Vr::Da,
            value: Value::Str("20260211".to_string()),
        });
        let query = query_from_identifier(&dataset, &limits()).expect("query");
        assert_eq!(query.level, QueryLevel::Study);
        assert_eq!(query.keys.len(), 2);
        assert_eq!(query.keys[0].tag, TAG_ACCESSION_NUMBER);
        assert_eq!(query.keys[0].value, "ACC123");
        assert_eq!(query.keys[1].tag, TAG_STUDY_DATE);
        assert_eq!(query.keys[1].value, "20260211");
    }

    #[test]
    fn identifier_query_rejects_conflicting_values() {
        // REQ-QR-302: UID values must be strictly validated and consistent.
        let mut dataset = Dataset::new();
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("1.2.3".to_string()),
        });
        dataset.insert(Element {
            tag: TAG_STUDY_UID,
            vr: Vr::Ui,
            value: Value::Uid("9.9".to_string()),
        });
        let err = query_from_identifier(&dataset, &limits()).expect_err("expected error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn query_key_coverage_matrix_lists_supported_hot_keys() {
        let rows = query_key_coverage_matrix();
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|row| row.tag == TAG_STUDY_UID));
        assert!(rows.iter().any(|row| row.tag == TAG_SERIES_UID));
        assert!(rows.iter().any(|row| row.tag == TAG_SOP_UID));
    }

    #[test]
    fn paginate_query_matches_emits_next_cursor() {
        let matches = vec![
            QueryMatch {
                study_uid: "1".to_string(),
                series_uid: Some("1.1".to_string()),
                instance_uid: Some("1.1.1".to_string()),
            },
            QueryMatch {
                study_uid: "2".to_string(),
                series_uid: Some("2.1".to_string()),
                instance_uid: Some("2.1.1".to_string()),
            },
        ];
        let first = paginate_query_matches(&matches, None, 1, &limits()).expect("first page");
        assert_eq!(first.matches.len(), 1);
        assert_eq!(first.next, Some(QueryCursor { offset: 1 }));
        let second =
            paginate_query_matches(&matches, first.next, 1, &limits()).expect("second page");
        assert_eq!(second.matches.len(), 1);
        assert_eq!(second.next, None);
    }

    #[test]
    fn query_guardrails_and_failure_classification_are_deterministic() {
        let err = enforce_query_guardrails(
            1,
            100,
            QueryGuardrailConfig {
                max_inflight: 10,
                max_duration_ms: 10,
            },
        )
        .expect_err("duration limit");
        assert_eq!(
            classify_query_failure(err.as_ref()),
            QueryFailureMetric::Timeout
        );
    }

    #[test]
    fn concurrent_query_replay_is_idempotent() {
        let datasets = Arc::new(vec![
            dataset_with_uids("1.2.3", "2.3.4", "3.4.5"),
            dataset_with_uids("1.2.3", "2.3.4", "3.4.6"),
        ]);
        let query_request = Query {
            level: QueryLevel::Instance,
            keys: vec![QueryKey {
                tag: TAG_STUDY_UID,
                value: "1.2.3".to_string(),
            }],
        };

        let mut handles = Vec::new();
        for _ in 0..6 {
            let datasets = Arc::clone(&datasets);
            let query_request = query_request.clone();
            handles.push(thread::spawn(move || {
                query(&datasets, &query_request, &Limits::default()).expect("query")
            }));
        }

        let baseline = handles
            .pop()
            .expect("baseline handle")
            .join()
            .expect("baseline join");
        for handle in handles {
            let current = handle.join().expect("join");
            assert_eq!(current, baseline);
        }
    }
}
