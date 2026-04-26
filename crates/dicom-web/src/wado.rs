//! WADO-RS retrieval engine + WADO-URI legacy types, functions, and handlers.

#[cfg(feature = "wado")]
use dicom_core::{Result, Tag};
#[cfg(feature = "wado")]
use dicom_storage::Storage;

#[cfg(feature = "wado")]
use super::{
    decode_error, ensure_ascii_printable, header_value, not_found_error, validate_uid,
    Header, QueryParam, TAG_INSTANCE_UID, TAG_SERIES_UID, TAG_STUDY_UID,
    TAG_TRANSFER_SYNTAX_UID,
};

#[cfg(feature = "wado")]
const TAG_NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
#[cfg(feature = "wado")]
const WADO_MULTIPART_BOUNDARY: &str = "dicomweb-dataset";
#[cfg(feature = "wado")]
pub(crate) const WADO_MULTIPART_CONTENT_TYPE: &str =
    "multipart/related; type=\"application/dicom\"; boundary=\"dicomweb-dataset\"";

#[cfg(feature = "wado")]
pub(crate) const TRANSFER_SYNTAX_IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";
#[cfg(feature = "wado")]
pub(crate) const TRANSFER_SYNTAX_EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";

#[cfg(feature = "wado")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WadoUriQuery {
    pub(crate) study_uid: String,
    pub(crate) series_uid: String,
    pub(crate) instance_uid: String,
    pub(crate) transfer_syntax_uid: Option<String>,
}

#[cfg(feature = "wado")]
pub(crate) fn parse_wado_uri_query(params: &[QueryParam]) -> Result<WadoUriQuery> {
    let mut request_type: Option<String> = None;
    let mut study_uid: Option<String> = None;
    let mut series_uid: Option<String> = None;
    let mut object_uid: Option<String> = None;
    let mut transfer_syntax_uid: Option<String> = None;

    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        let value = param.value.trim();
        match key.as_str() {
            "requesttype" => request_type = Some(value.to_string()),
            "studyuid" => study_uid = Some(validate_uid(TAG_STUDY_UID, value)?),
            "seriesuid" => series_uid = Some(validate_uid(TAG_SERIES_UID, value)?),
            "objectuid" => object_uid = Some(validate_uid(TAG_INSTANCE_UID, value)?),
            "transfersyntax" => {
                transfer_syntax_uid = Some(parse_transfer_syntax_uid(value)?);
            }
            "contenttype" => {
                if !value.eq_ignore_ascii_case("application/dicom") {
                    return Err(decode_error(
                        "WADO-URI contentType must be application/dicom",
                    ));
                }
            }
            _ => return Err(decode_error("unsupported WADO-URI parameter")),
        }
    }

    match request_type {
        Some(value) if value.eq_ignore_ascii_case("WADO") => {}
        Some(_) => return Err(decode_error("WADO-URI requestType must be WADO")),
        None => return Err(decode_error("WADO-URI requestType is required")),
    }

    Ok(WadoUriQuery {
        study_uid: study_uid.ok_or_else(|| decode_error("WADO-URI studyUID is required"))?,
        series_uid: series_uid.ok_or_else(|| decode_error("WADO-URI seriesUID is required"))?,
        instance_uid: object_uid.ok_or_else(|| decode_error("WADO-URI objectUID is required"))?,
        transfer_syntax_uid,
    })
}

#[cfg(feature = "wado")]
pub(crate) fn parse_wado_retrieve_query(params: &[QueryParam]) -> Result<Option<String>> {
    let mut transfer_syntax_uid = None;
    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        if key != "transfersyntax" {
            return Err(decode_error("unsupported query parameters for WADO"));
        }
        transfer_syntax_uid = Some(parse_transfer_syntax_uid(param.value.trim())?);
    }
    Ok(transfer_syntax_uid)
}

#[cfg(feature = "wado")]
pub(crate) fn parse_wado_rendered_query(
    params: &[QueryParam],
    headers: &[Header],
) -> Result<(Option<String>, String)> {
    parse_wado_media_query(
        params,
        headers,
        &["image/png", "image/jpeg", "application/octet-stream"],
        "image/png",
        "rendered",
    )
}

#[cfg(feature = "wado")]
pub(crate) fn parse_wado_bulkdata_query(
    params: &[QueryParam],
    headers: &[Header],
) -> Result<(Option<String>, String)> {
    parse_wado_media_query(
        params,
        headers,
        &["application/octet-stream", "application/dicom"],
        "application/octet-stream",
        "bulkdata",
    )
}

#[cfg(feature = "wado")]
fn parse_wado_media_query(
    params: &[QueryParam],
    headers: &[Header],
    allowed_media_types: &[&str],
    default_media_type: &str,
    route_label: &str,
) -> Result<(Option<String>, String)> {
    let mut transfer_syntax_uid = None;
    let mut media_type: Option<String> = None;
    for param in params {
        let key = param.key.trim().to_ascii_lowercase();
        match key.as_str() {
            "transfersyntax" => {
                transfer_syntax_uid = Some(parse_transfer_syntax_uid(param.value.trim())?);
            }
            "accept" => {
                media_type = Some(parse_wado_media_type(
                    param.value.trim(),
                    allowed_media_types,
                    route_label,
                )?);
            }
            _ => {
                return Err(decode_error(format!(
                    "unsupported query parameters for WADO {route_label}"
                )));
            }
        }
    }
    if media_type.is_none() {
        if let Some(value) = header_value(headers, "accept") {
            let candidate = value.split(',').next().map(str::trim).unwrap_or_default();
            if !candidate.is_empty() {
                media_type = Some(parse_wado_media_type(
                    candidate,
                    allowed_media_types,
                    route_label,
                )?);
            }
        }
    }
    Ok((
        transfer_syntax_uid,
        media_type.unwrap_or_else(|| default_media_type.to_string()),
    ))
}

#[cfg(feature = "wado")]
fn parse_wado_media_type(value: &str, allowed: &[&str], route_label: &str) -> Result<String> {
    ensure_ascii_printable(value, "accept media type")?;
    let normalized = value.to_ascii_lowercase();
    if !allowed.iter().any(|candidate| *candidate == normalized) {
        return Err(decode_error(format!(
            "unsupported media type for {route_label} retrieve"
        )));
    }
    Ok(normalized)
}

#[cfg(feature = "wado")]
pub(crate) fn parse_frame_number(value: &str) -> Result<u32> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| decode_error("frame number must be a positive integer"))?;
    if parsed == 0 {
        return Err(decode_error("frame number must be >= 1"));
    }
    Ok(parsed)
}

#[cfg(feature = "wado")]
pub(crate) fn parse_transfer_syntax_uid(value: &str) -> Result<String> {
    use dicom_core::validate_uid_strict;
    use super::unsupported_transfer_syntax;

    validate_uid_strict(TAG_TRANSFER_SYNTAX_UID, value)?;
    if value != TRANSFER_SYNTAX_IMPLICIT_VR_LE && value != TRANSFER_SYNTAX_EXPLICIT_VR_LE {
        return Err(unsupported_transfer_syntax(value));
    }
    Ok(value.to_string())
}

#[cfg(feature = "wado")]
pub(crate) fn validate_frame_number(
    storage: &Storage,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
    frame_number: u32,
) -> Result<()> {
    let max_frames = instance_number_of_frames(storage, study_uid, series_uid, instance_uid)?;
    if frame_number > max_frames {
        return Err(decode_error("frame number exceeds NumberOfFrames"));
    }
    Ok(())
}

#[cfg(feature = "wado")]
fn instance_number_of_frames(
    storage: &Storage,
    study_uid: &str,
    series_uid: &str,
    instance_uid: &str,
) -> Result<u32> {
    let datasets = storage.datasets()?;
    let dataset = datasets.into_iter().find(|dataset| {
        dataset.get_uid(TAG_STUDY_UID) == Some(study_uid)
            && dataset.get_uid(TAG_SERIES_UID) == Some(series_uid)
            && dataset.get_uid(TAG_INSTANCE_UID) == Some(instance_uid)
    });
    let Some(dataset) = dataset else {
        return Err(not_found_error("requested WADO instance not found"));
    };
    if let Some(value) = dataset.get_i32(TAG_NUMBER_OF_FRAMES) {
        if value <= 0 {
            return Err(decode_error("NumberOfFrames must be >= 1"));
        }
        return Ok(value as u32);
    }
    if let Some(value) = dataset.get_str(TAG_NUMBER_OF_FRAMES) {
        let parsed = value
            .trim()
            .parse::<u32>()
            .map_err(|_| decode_error("invalid NumberOfFrames"))?;
        if parsed == 0 {
            return Err(decode_error("NumberOfFrames must be >= 1"));
        }
        return Ok(parsed);
    }
    Ok(1)
}

#[cfg(feature = "wado")]
pub(crate) fn wado_dataset_retrieve_multipart_bytes(
    storage: &Storage,
    study_uid: Option<&str>,
    series_uid: Option<&str>,
) -> Result<Vec<u8>> {
    let datasets = storage.datasets()?;
    let mut out = Vec::new();
    let mut parts = 0usize;
    for dataset in datasets {
        let Some(current_study) = dataset.get_uid(TAG_STUDY_UID) else {
            continue;
        };
        let Some(current_series) = dataset.get_uid(TAG_SERIES_UID) else {
            continue;
        };
        let Some(current_instance) = dataset.get_uid(TAG_INSTANCE_UID) else {
            continue;
        };
        if let Some(expected_study) = study_uid {
            if current_study != expected_study {
                continue;
            }
        }
        if let Some(expected_series) = series_uid {
            if current_series != expected_series {
                continue;
            }
        }
        let Some(bytes) = storage.instance_bytes(current_study, current_series, current_instance)
        else {
            continue;
        };
        out.extend_from_slice(format!("--{WADO_MULTIPART_BOUNDARY}\r\n").as_bytes());
        out.extend_from_slice(b"content-type: application/dicom\r\n\r\n");
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
        parts += 1;
    }
    if parts == 0 {
        return Err(not_found_error("requested WADO retrieve not found"));
    }
    out.extend_from_slice(format!("--{WADO_MULTIPART_BOUNDARY}--\r\n").as_bytes());
    Ok(out)
}

#[cfg(feature = "wado")]
pub(crate) fn metadata_response_bytes(
    storage: &Storage,
    study_uid: Option<&str>,
    series_uid: Option<&str>,
    instance_uid: Option<&str>,
) -> Result<Vec<u8>> {
    let datasets = storage.datasets()?;
    let mut matched = Vec::new();
    for dataset in datasets {
        let Some(current_study) = dataset.get_uid(TAG_STUDY_UID) else {
            continue;
        };
        let Some(current_series) = dataset.get_uid(TAG_SERIES_UID) else {
            continue;
        };
        let Some(current_instance) = dataset.get_uid(TAG_INSTANCE_UID) else {
            continue;
        };
        if let Some(expected_study) = study_uid {
            if current_study != expected_study {
                continue;
            }
        }
        if let Some(expected_series) = series_uid {
            if current_series != expected_series {
                continue;
            }
        }
        if let Some(expected_instance) = instance_uid {
            if current_instance != expected_instance {
                continue;
            }
        }
        matched.push(dataset);
    }
    if matched.is_empty() {
        return Err(not_found_error("requested WADO metadata not found"));
    }
    Ok(render_metadata_json(&matched).into_bytes())
}

#[cfg(feature = "wado")]
fn render_metadata_json(datasets: &[dicom_core::Dataset]) -> String {
    let mut out = String::from("[");
    for (index, dataset) in datasets.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('{');
        let mut elements = dataset.iter().collect::<Vec<_>>();
        elements.sort_by_key(|element| element.tag().as_u32());
        for (element_index, element) in elements.iter().enumerate() {
            if element_index > 0 {
                out.push(',');
            }
            out.push('"');
            out.push_str(&format!("{:04X}{:04X}", element.tag().0, element.tag().1));
            out.push_str("\":{");
            out.push_str("\"vr\":\"");
            let vr_text = String::from_utf8_lossy(&element.vr().as_bytes()).to_string();
            out.push_str(&escape_json_string(&vr_text));
            out.push('"');
            match element.value() {
                dicom_core::Value::Empty => {}
                dicom_core::Value::Str(value) | dicom_core::Value::Uid(value) => {
                    out.push_str(",\"Value\":[\"");
                    out.push_str(&escape_json_string(value));
                    out.push_str("\"]");
                }
                dicom_core::Value::I32(value) => {
                    out.push_str(&format!(",\"Value\":[{value}]"));
                }
                dicom_core::Value::F64(value) => {
                    out.push_str(&format!(",\"Value\":[{}]", value));
                }
                dicom_core::Value::Bytes(value) => {
                    let hex = value
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<String>();
                    out.push_str(",\"InlineBinary\":\"");
                    out.push_str(&hex);
                    out.push('"');
                }
                dicom_core::Value::Sequence(sequence) => {
                    out.push_str(",\"Value\":[");
                    let nested = render_metadata_json(sequence);
                    out.push_str(nested.trim_start_matches('[').trim_end_matches(']'));
                    out.push(']');
                }
            }
            out.push('}');
        }
        out.push('}');
    }
    out.push(']');
    out
}

#[cfg(feature = "wado")]
fn escape_json_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
