//! Configuration change control bounded-context module.
//!
//! Contains `ConfigChangeJournalEntry`, `ConfigChangeJournal`, and `ConfigChangeRequest`.

use dicom_core::{Error, ErrorKind, Result};

/// Administrative configuration change request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChangeRequest {
    /// Authenticated actor identity.
    pub actor_user_id: String,
    /// Authentication state at request time.
    pub authenticated: bool,
    /// Structured reason code.
    pub reason_code: String,
    /// Configuration key.
    pub key: String,
    /// Previous value.
    pub before: String,
    /// Next value.
    pub after: String,
    /// Timestamp (seconds since epoch).
    pub timestamp_epoch_secs: u64,
}

/// Journal entry for a configuration delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChangeJournalEntry {
    /// Authenticated actor identity.
    pub actor_user_id: String,
    /// Structured reason code.
    pub reason_code: String,
    /// Configuration key.
    pub key: String,
    /// Previous value.
    pub before: String,
    /// Next value.
    pub after: String,
    /// Timestamp (seconds since epoch).
    pub timestamp_epoch_secs: u64,
}

/// Deterministic in-memory configuration journal.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigChangeJournal {
    /// Append-only entry list.
    pub entries: Vec<ConfigChangeJournalEntry>,
}

impl ConfigChangeJournal {
    /// Record a configuration change after authentication/reason checks.
    pub fn record_change(
        &mut self,
        request: ConfigChangeRequest,
    ) -> Result<ConfigChangeJournalEntry> {
        if !request.authenticated {
            return Err(hi_decode_error(
                "administrative configuration changes require authenticated identity",
            ));
        }
        if request.actor_user_id.trim().is_empty() || request.reason_code.trim().is_empty() {
            return Err(hi_decode_error(
                "administrative configuration changes require actor identity and reason code",
            ));
        }
        if request.key.trim().is_empty() {
            return Err(hi_decode_error(
                "configuration change key must not be empty",
            ));
        }

        let entry = ConfigChangeJournalEntry {
            actor_user_id: request.actor_user_id,
            reason_code: request.reason_code,
            key: request.key,
            before: request.before,
            after: request.after,
            timestamp_epoch_secs: request.timestamp_epoch_secs,
        };
        self.entries.push(entry.clone());
        Ok(entry)
    }
}

fn hi_decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-auth".to_string(),
            detail: detail.into(),
        },
        "human-interface policy error",
    )
    .into()
}
