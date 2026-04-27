//! Shared utility functions for the diccy workspace.
//!
//! Consolidates helper functions that were duplicated across 13+ crates.

use dicom_core::{Dataset, Error, ErrorKind, Tag, Value};

/// Create a decode error with a stage identifier.
pub fn decode_error(stage: &str, detail: &str) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::DecodeError {
            stage: stage.to_string(),
            detail: detail.to_string(),
        },
        detail.to_string(),
    ))
}

/// Enforce a limit, returning an error if exceeded.
pub fn enforce_limit(name: &'static str, current: u64, limit: u64) -> Result<(), Box<Error>> {
    if current > limit {
        Err(limit_exceeded(name, current, limit))
    } else {
        Ok(())
    }
}

/// Create a limit-exceeded error.
pub fn limit_exceeded(name: &'static str, current: u64, limit: u64) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name: name,
            observed: current,
            allowed: limit,
        },
        format!("{} limit exceeded: {} > {}", name, current, limit),
    ))
}

/// Create an error for a missing required DICOM tag.
pub fn missing_required_tag(tag: Tag) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::MissingRequiredTag { tag },
        format!("Missing required tag ({:04X},{:04X})", tag.0, tag.1),
    ))
}

/// Require a UID string from a dataset at the given tag.
pub fn require_uid(dataset: &Dataset, tag: Tag) -> Result<String, Box<Error>> {
    let element = dataset.get(tag).ok_or_else(|| missing_required_tag(tag))?;
    match element.value() {
        Value::Uid(uid) => {
            if uid.is_empty() {
                Err(missing_required_tag(tag))
            } else {
                Ok(uid.clone())
            }
        }
        _ => Err(missing_required_tag(tag)),
    }
}

/// Parse and validate a UID string.
pub fn parse_uid(s: &str) -> Result<String, Box<Error>> {
    if s.is_empty() || s.len() > 64 {
        return Err(decode_error("parse_uid", "UID length invalid"));
    }
    if !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(decode_error("parse_uid", "UID contains invalid characters"));
    }
    Ok(s.to_string())
}

/// Create an integrity error.
pub fn integrity_error(detail: &str) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::IntegrityError {
            detail: detail.to_string(),
        },
        detail.to_string(),
    ))
}

/// Create an IO error.
pub fn io_error(detail: &str) -> Box<Error> {
    Box::new(Error::from_kind(
        ErrorKind::IoError {
            detail: detail.to_string(),
        },
        detail.to_string(),
    ))
}
