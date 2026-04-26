//! Per-operation payload size limits, resource caps, and rate-limiting middleware.

use crate::{decode_error, limit_exceeded, DimseRoleConfig};
use dicom_core::Result;
use dicom_dimse::DimseMessage;
use std::sync::{Arc, Mutex};

pub(crate) const DEFAULT_MAX_IN_FLIGHT_OPERATIONS: usize = 64;
pub(crate) const DEFAULT_MAX_QUERY_RESPONSE_COUNT: usize = 4_096;

/// Per-operation payload size limits for runtime role configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseOperationSizeConfig {
    /// Maximum accepted C-ECHO data-set bytes (normally zero for DICOM).
    pub c_echo_data_set_bytes: u64,
    /// Maximum accepted C-STORE data-set bytes.
    pub c_store_data_set_bytes: u64,
    /// Maximum accepted C-FIND data-set bytes.
    pub c_find_data_set_bytes: u64,
    /// Maximum accepted C-MOVE data-set bytes.
    pub c_move_data_set_bytes: u64,
    /// Maximum accepted C-GET data-set bytes.
    pub c_get_data_set_bytes: u64,
}

impl DimseOperationSizeConfig {
    /// Build config with identical payload size limits from a shared bound.
    pub(crate) fn with_default_bound(max_input_bytes: u64) -> Self {
        Self {
            c_echo_data_set_bytes: 0,
            c_store_data_set_bytes: max_input_bytes,
            c_find_data_set_bytes: max_input_bytes,
            c_move_data_set_bytes: max_input_bytes,
            c_get_data_set_bytes: max_input_bytes,
        }
    }
}

/// Per-operation resource caps for runtime governance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimseOperationResourceLimits {
    /// Maximum concurrent DIMSE operations across all associations.
    pub max_in_flight_operations: usize,
    /// Maximum number of responses a single query/retrieve operation may emit.
    pub max_query_responses: usize,
}

impl Default for DimseOperationResourceLimits {
    fn default() -> Self {
        Self {
            max_in_flight_operations: DEFAULT_MAX_IN_FLIGHT_OPERATIONS,
            max_query_responses: DEFAULT_MAX_QUERY_RESPONSE_COUNT,
        }
    }
}

#[derive(Clone)]
pub(crate) struct OperationLimiter {
    pub(crate) max_in_flight_operations: usize,
    pub(crate) active: Arc<Mutex<usize>>,
}

impl OperationLimiter {
    pub(crate) fn new(max_in_flight_operations: usize) -> Self {
        Self {
            max_in_flight_operations,
            active: Arc::new(Mutex::new(0)),
        }
    }

    pub(crate) fn try_acquire(&self) -> Result<OperationGuard> {
        let mut active = self
            .active
            .lock()
            .map_err(|_| decode_error("operation limiter lock poisoned"))?;
        let next = active.saturating_add(1);
        enforce_operation_limit(next, self.max_in_flight_operations)?;
        *active = next;
        Ok(OperationGuard {
            active: Arc::clone(&self.active),
        })
    }
}

pub(crate) struct OperationGuard {
    active: Arc<Mutex<usize>>,
}

impl Drop for OperationGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active.lock() {
            *active = active.saturating_sub(1);
        }
    }
}

pub(crate) fn enforce_connection_limit(observed: usize, allowed: usize) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(
            "max_connections",
            observed as u64,
            allowed as u64,
        ));
    }
    Ok(())
}

pub(crate) fn enforce_operation_limit(observed: usize, allowed: usize) -> Result<()> {
    if observed > allowed {
        return Err(limit_exceeded(
            "max_in_flight_operations",
            observed as u64,
            allowed as u64,
        ));
    }
    Ok(())
}

pub(crate) fn enforce_query_response_limit(observed: usize, max_query_responses: usize) -> Result<()> {
    if observed > max_query_responses {
        return Err(decode_error(format!(
            "query/retrieve response sequence exceeded {max_query_responses} responses",
        )));
    }
    Ok(())
}

pub(crate) fn message_needs_data(message: &DimseMessage) -> bool {
    match message {
        DimseMessage::CStoreRq { .. } => true,
        DimseMessage::NActionRq { .. } => true,
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => true,
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => true,
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => true,
        _ => false,
    }
}

pub(crate) fn message_is_enabled(message: &DimseMessage, roles: &DimseRoleConfig) -> bool {
    match message {
        DimseMessage::CEchoRq { .. } => roles.c_echo_enabled,
        DimseMessage::CStoreRq { .. } => roles.c_store_enabled,
        DimseMessage::NActionRq { .. } => roles.c_store_enabled,
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => roles.c_find_enabled,
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => roles.c_move_enabled,
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => roles.c_get_enabled,
        _ => false,
    }
}

pub(crate) fn message_data_set_limit(
    message: &DimseMessage,
    limits: &DimseOperationSizeConfig,
) -> Option<u64> {
    match message {
        DimseMessage::CStoreRq { .. } => Some(limits.c_store_data_set_bytes),
        DimseMessage::NActionRq { .. } => Some(limits.c_store_data_set_bytes),
        #[cfg(feature = "dimse-c-find")]
        DimseMessage::CFindRq { .. } => Some(limits.c_find_data_set_bytes),
        #[cfg(feature = "dimse-c-move")]
        DimseMessage::CMoveRq { .. } => Some(limits.c_move_data_set_bytes),
        #[cfg(feature = "dimse-c-get")]
        DimseMessage::CGetRq { .. } => Some(limits.c_get_data_set_bytes),
        DimseMessage::CEchoRq { .. } => Some(limits.c_echo_data_set_bytes),
        _ => None,
    }
}

pub(crate) fn validate_data_set_size(
    message: &DimseMessage,
    data_set: &[u8],
    limits: &DimseOperationSizeConfig,
) -> Result<()> {
    if !message_needs_data(message) {
        return Ok(());
    }
    let max_input_bytes = message_data_set_limit(message, limits)
        .ok_or_else(|| decode_error("unsupported operation size limit"))?;
    if data_set.len() as u64 > max_input_bytes {
        return Err(limit_exceeded(
            "max_input_bytes",
            data_set.len() as u64,
            max_input_bytes,
        ));
    }
    Ok(())
}
