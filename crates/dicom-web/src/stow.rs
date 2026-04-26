//! STOW-RS storage engine types, functions, and handlers.

#[cfg(feature = "stow")]
use dicom_core::{Limits, Result};
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_core::{Limits as LimitsType, Result as ResultType};

#[cfg(feature = "stow")]
use super::{
    decode_error, enforce_limit, ensure_ascii_printable, header_value,
    DicomWebContentType, WebRequest,
};

#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
use super::StowPreflight;

/// Preflight STOW payload bytes for SOP/transfer-syntax compatibility and study context.
#[cfg(any(feature = "qido", feature = "wado", feature = "stow"))]
pub fn stow_preflight_compatibility(bytes: &[u8], limits: &LimitsType) -> ResultType<StowPreflight> {
    let study_uid = dicom_storage::extract_study_uid(bytes, limits)?;
    Ok(StowPreflight { study_uid })
}

#[cfg(feature = "stow")]
pub(crate) fn parse_content_type(request: &WebRequest, limits: &Limits) -> Result<DicomWebContentType> {
    let value = header_value(&request.headers, "content-type")
        .ok_or_else(|| decode_error("missing content-type header"))?;
    enforce_limit(
        "max_string_bytes",
        value.len() as u64,
        limits.max_string_bytes(),
    )?;
    ensure_ascii_printable(value, "content-type")?;

    let mut parts = value.split(';');
    let base = parts
        .next()
        .ok_or_else(|| decode_error("invalid content-type"))?
        .trim()
        .to_ascii_lowercase();

    if base == "application/dicom" {
        return Ok(DicomWebContentType::ApplicationDicom);
    }
    if base == "application/dicom+xml" {
        return Ok(DicomWebContentType::ApplicationDicomXml);
    }
    if base == "application/dicom+json" {
        return Ok(DicomWebContentType::ApplicationDicomJson);
    }
    if base != "multipart/related" {
        return Err(decode_error("unsupported content-type"));
    }

    let mut boundary = None;
    let mut related_type = None;
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| decode_error("invalid content-type parameter"))?;
        let key = key.trim().to_ascii_lowercase();
        let mut value = value.trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = &value[1..value.len() - 1];
        }
        ensure_ascii_printable(value, "content-type parameter")?;
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
        )?;
        match key.as_str() {
            "boundary" => boundary = Some(value.to_string()),
            "type" => related_type = Some(value.to_string()),
            _ => {}
        }
    }

    let related_type = related_type
        .ok_or_else(|| decode_error("multipart/related missing type"))?
        .to_ascii_lowercase();
    if !matches!(
        related_type.as_str(),
        "application/dicom" | "application/dicom+xml" | "application/dicom+json"
    ) {
        return Err(decode_error(
            "multipart/related type must be application/dicom, application/dicom+xml, or application/dicom+json",
        ));
    }
    let boundary = boundary.ok_or_else(|| decode_error("multipart/related missing boundary"))?;
    if boundary.is_empty() {
        return Err(decode_error("multipart/related boundary is empty"));
    }

    Ok(DicomWebContentType::MultipartRelated { boundary })
}

#[cfg(feature = "stow")]
pub(crate) fn stow_payloads(
    content_type: &DicomWebContentType,
    body: &[u8],
    limits: &Limits,
) -> Result<Vec<Vec<u8>>> {
    match content_type {
        DicomWebContentType::ApplicationDicom => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes())?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::ApplicationDicomXml => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes())?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::ApplicationDicomJson => {
            enforce_limit("max_input_bytes", body.len() as u64, limits.max_input_bytes())?;
            Ok(vec![body.to_vec()])
        }
        DicomWebContentType::MultipartRelated { boundary } => {
            parse_stow_multipart_related(body, boundary, limits)
        }
    }
}

#[cfg(feature = "stow")]
fn parse_stow_multipart_related(
    body: &[u8],
    boundary: &str,
    limits: &Limits,
) -> Result<Vec<Vec<u8>>> {
    let delimiter = format!("--{boundary}").into_bytes();
    let mut cursor = skip_optional_line_ending(body, 0);
    if !body[cursor..].starts_with(&delimiter) {
        return Err(decode_error("multipart body missing opening boundary"));
    }
    cursor += delimiter.len();
    cursor = consume_required_line_ending(body, cursor, "multipart boundary line")?;

    let mut parts = Vec::new();
    loop {
        let (head_end, body_start) = part_header_offsets(body, cursor)?;
        validate_multipart_part_headers(&body[cursor..head_end], limits)?;

        let (part_end, boundary_marker_start) =
            find_next_multipart_boundary(body, body_start, &delimiter)
                .ok_or_else(|| decode_error("multipart body missing closing boundary"))?;
        enforce_limit(
            "max_input_bytes",
            (part_end - body_start) as u64,
            limits.max_input_bytes(),
        )?;
        if part_end == body_start {
            return Err(decode_error("multipart part payload is empty"));
        }
        parts.push(body[body_start..part_end].to_vec());
        if parts.len() as u64 > limits.max_dataset_elements() {
            return Err(super::limit_exceeded(
                "max_dataset_elements",
                parts.len() as u64,
                limits.max_dataset_elements(),
            ));
        }

        cursor = boundary_marker_start + delimiter.len();
        if body[cursor..].starts_with(b"--") {
            cursor += 2;
            let cursor = skip_optional_line_ending(body, cursor);
            if cursor != body.len() {
                return Err(decode_error(
                    "unexpected bytes after multipart closing boundary",
                ));
            }
            break;
        }
        cursor = consume_required_line_ending(body, cursor, "multipart boundary line")?;
    }

    if parts.is_empty() {
        return Err(decode_error("multipart body does not contain DICOM parts"));
    }
    Ok(parts)
}

#[cfg(feature = "stow")]
fn validate_multipart_part_headers(headers: &[u8], limits: &Limits) -> Result<()> {
    let head_str = std::str::from_utf8(headers)
        .map_err(|_| decode_error("multipart part headers are not valid UTF-8"))?;
    let mut content_type_seen = false;
    for line in head_str.lines() {
        let line = line.trim_end_matches('\r').trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| decode_error("invalid multipart part header line"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        enforce_limit(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes(),
        )?;
        if name == "content-type" {
            content_type_seen = true;
            let base = value
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if !matches!(
                base.as_str(),
                "application/dicom" | "application/dicom+xml" | "application/dicom+json"
            ) {
                return Err(decode_error(
                    "multipart part content-type must be application/dicom, application/dicom+xml, or application/dicom+json",
                ));
            }
        }
    }
    if !content_type_seen {
        return Err(decode_error("multipart part missing content-type header"));
    }
    Ok(())
}

#[cfg(feature = "stow")]
fn part_header_offsets(buffer: &[u8], start: usize) -> Result<(usize, usize)> {
    if let Some(pos) = find_subslice(buffer, start, b"\r\n\r\n") {
        return Ok((pos, pos + 4));
    }
    if let Some(pos) = find_subslice(buffer, start, b"\n\n") {
        return Ok((pos, pos + 2));
    }
    Err(decode_error("multipart part missing header terminator"))
}

#[cfg(feature = "stow")]
fn find_next_multipart_boundary(
    buffer: &[u8],
    start: usize,
    delimiter: &[u8],
) -> Option<(usize, usize)> {
    let mut crlf_pattern = Vec::with_capacity(delimiter.len() + 2);
    crlf_pattern.extend_from_slice(b"\r\n");
    crlf_pattern.extend_from_slice(delimiter);
    let crlf = find_subslice(buffer, start, &crlf_pattern).map(|pos| (pos, pos + 2));

    let mut lf_pattern = Vec::with_capacity(delimiter.len() + 1);
    lf_pattern.extend_from_slice(b"\n");
    lf_pattern.extend_from_slice(delimiter);
    let lf = find_subslice(buffer, start, &lf_pattern).map(|pos| (pos, pos + 1));

    match (crlf, lf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(feature = "stow")]
fn find_subslice(buffer: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || start >= buffer.len() || needle.len() > buffer.len() - start {
        return None;
    }
    buffer[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| start + offset)
}

#[cfg(feature = "stow")]
fn skip_optional_line_ending(buffer: &[u8], start: usize) -> usize {
    if buffer.get(start..start + 2) == Some(b"\r\n") {
        return start + 2;
    }
    if buffer.get(start) == Some(&b'\n') {
        return start + 1;
    }
    start
}

#[cfg(feature = "stow")]
fn consume_required_line_ending(buffer: &[u8], start: usize, context: &str) -> Result<usize> {
    if buffer.get(start..start + 2) == Some(b"\r\n") {
        return Ok(start + 2);
    }
    if buffer.get(start) == Some(&b'\n') {
        return Ok(start + 1);
    }
    Err(decode_error(format!(
        "expected line terminator after {context}"
    )))
}
