//! QIDO-RS query engine types, functions, and handlers.

#[cfg(feature = "qido")]
use dicom_core::Dataset;
#[cfg(feature = "qido")]
use dicom_core::{Limits, Result, Tag};
#[cfg(feature = "qido")]
use dicom_query::{query as run_query, Query, QueryKey, QueryLevel, QueryMatch};

#[cfg(feature = "qido")]
use super::{decode_error, enforce_limit, DicomWebRequest, QueryParam};

#[cfg(feature = "qido")]
pub(crate) const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
#[cfg(feature = "qido")]
pub(crate) const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
#[cfg(feature = "qido")]
pub(crate) const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
#[cfg(feature = "qido")]
pub(crate) const TAG_STUDY_DATE: Tag = Tag(0x0008, 0x0020);

/// Execute a QIDO-RS query against decoded datasets using `dicom-query`.
#[cfg(feature = "qido")]
pub fn qido_query_matches(
    request: &DicomWebRequest,
    datasets: &[Dataset],
    limits: &Limits,
) -> Result<Vec<QueryMatch>> {
    use super::{TAG_SERIES_UID, TAG_INSTANCE_UID, TAG_STUDY_UID};

    let (level, params, path_keys): (QueryLevel, &[QueryParam], Vec<(Tag, String)>) = match request
    {
        DicomWebRequest::QidoStudies { params } => (QueryLevel::Study, params, Vec::new()),
        DicomWebRequest::QidoAllSeries { params } => (QueryLevel::Series, params, Vec::new()),
        DicomWebRequest::QidoAllInstances { params } => (QueryLevel::Instance, params, Vec::new()),
        DicomWebRequest::QidoSeries {
            study_uid, params, ..
        } => (
            QueryLevel::Series,
            params,
            vec![(TAG_STUDY_UID, study_uid.clone())],
        ),
        DicomWebRequest::QidoStudyInstances {
            study_uid, params, ..
        } => (
            QueryLevel::Instance,
            params,
            vec![(TAG_STUDY_UID, study_uid.clone())],
        ),
        DicomWebRequest::QidoInstances {
            study_uid,
            series_uid,
            params,
            ..
        } => (
            QueryLevel::Instance,
            params,
            vec![
                (TAG_STUDY_UID, study_uid.clone()),
                (TAG_SERIES_UID, series_uid.clone()),
            ],
        ),
        _ => return Err(decode_error("non-QIDO request for QIDO query handler")),
    };

    let mut keys = Vec::new();
    let mut seen_study: Option<String> = None;
    let mut seen_series: Option<String> = None;
    let mut seen_instance: Option<String> = None;
    let mut seen_modality: Option<String> = None;
    let mut seen_patient_id: Option<String> = None;
    let mut seen_accession_number: Option<String> = None;
    let mut seen_study_date: Option<String> = None;

    let mut insert_key = |tag: Tag, value: &str| -> Result<()> {
        let slot = match tag {
            TAG_STUDY_UID => &mut seen_study,
            TAG_SERIES_UID => &mut seen_series,
            TAG_INSTANCE_UID => &mut seen_instance,
            TAG_MODALITY => &mut seen_modality,
            TAG_PATIENT_ID => &mut seen_patient_id,
            TAG_ACCESSION_NUMBER => &mut seen_accession_number,
            TAG_STUDY_DATE => &mut seen_study_date,
            _ => return Err(decode_error("unsupported query key")),
        };
        match slot {
            Some(existing) => {
                if existing != value
                    && matches!(tag, TAG_STUDY_UID | TAG_SERIES_UID | TAG_INSTANCE_UID)
                {
                    return Err(decode_error("query key conflicts with path UID"));
                }
                Ok(())
            }
            None => {
                *slot = Some(value.to_string());
                keys.push(QueryKey {
                    tag,
                    value: value.to_string(),
                });
                Ok(())
            }
        }
    };

    for (tag, value) in path_keys {
        insert_key(tag, &value)?;
    }

    let mut offset: usize = 0;
    let mut limit: Option<usize> = None;

    for param in params {
        match param.key.to_ascii_lowercase().as_str() {
            "offset" => {
                offset = parse_qido_pagination_value("offset", &param.value, limits)?;
                continue;
            }
            "limit" => {
                limit = Some(parse_qido_pagination_value("limit", &param.value, limits)?);
                continue;
            }
            "includefield" => {
                continue;
            }
            "fuzzy" => {
                let value = param.value.trim().to_ascii_lowercase();
                if !matches!(value.as_str(), "0" | "1" | "false" | "true") {
                    return Err(decode_error("fuzzy must be one of 0,1,false,true"));
                }
                continue;
            }
            _ => {}
        }

        let tag = normalize_qido_param_key(&param.key)?;
        let supported = match level {
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
                    | TAG_INSTANCE_UID
                    | TAG_PATIENT_ID
                    | TAG_MODALITY
                    | TAG_ACCESSION_NUMBER
                    | TAG_STUDY_DATE
            ),
        };
        if !supported {
            return Err(decode_error("unsupported query parameter for level"));
        }
        insert_key(tag, &param.value)?;
    }

    let query = Query { level, keys };
    let mut matches = run_query(datasets, &query, limits)?;
    if offset >= matches.len() {
        return Ok(Vec::new());
    }
    if offset > 0 {
        matches.drain(0..offset);
    }
    if let Some(limit) = limit {
        matches.truncate(limit);
    }
    Ok(matches)
}

#[cfg(feature = "qido")]
fn parse_qido_pagination_value(key: &'static str, value: &str, limits: &Limits) -> Result<usize> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| decode_error(format!("{key} must be a non-negative integer")))?;
    enforce_limit(
        "max_dataset_elements",
        parsed as u64,
        limits.max_dataset_elements(),
    )?;
    Ok(parsed)
}

#[cfg(feature = "qido")]
fn normalize_qido_param_key(key: &str) -> Result<Tag> {
    use super::{TAG_SERIES_UID, TAG_INSTANCE_UID, TAG_STUDY_UID};

    let normalized = key
        .trim()
        .chars()
        .filter(|ch| *ch != '_' && !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let by_keyword = match normalized.as_str() {
        "studyinstanceuid" => Some(TAG_STUDY_UID),
        "seriesinstanceuid" => Some(TAG_SERIES_UID),
        "sopinstanceuid" => Some(TAG_INSTANCE_UID),
        "patientid" => Some(TAG_PATIENT_ID),
        "modality" => Some(TAG_MODALITY),
        "accessionnumber" => Some(TAG_ACCESSION_NUMBER),
        "studydate" => Some(TAG_STUDY_DATE),
        _ => None,
    };
    if let Some(tag) = by_keyword {
        return Ok(tag);
    }

    let parsed = Tag::parse_str(key)?;
    match parsed {
        TAG_STUDY_UID | TAG_SERIES_UID | TAG_INSTANCE_UID | TAG_PATIENT_ID | TAG_MODALITY
        | TAG_ACCESSION_NUMBER | TAG_STUDY_DATE => Ok(parsed),
        _ => Err(decode_error("unsupported query parameter")),
    }
}
