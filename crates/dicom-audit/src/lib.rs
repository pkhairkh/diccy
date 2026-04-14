#![deny(missing_docs)]

//! Audit logging with deterministic redaction and retention.

use dicom_core::{Error, ErrorKind, Result};

/// Audit event category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventKind {
    /// Authorization decision event.
    AuthzDecision,
    /// Authentication failure event.
    AuthnFailure,
    /// Generic service event.
    ServiceEvent,
}

impl AuditEventKind {
    fn as_str(self) -> &'static str {
        match self {
            AuditEventKind::AuthzDecision => "authz_decision",
            AuditEventKind::AuthnFailure => "authn_failure",
            AuditEventKind::ServiceEvent => "service_event",
        }
    }
}

/// Audit field value classification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditValue {
    /// Plain value allowed in audit output.
    Plain(String),
    /// Sensitive value that requires redaction.
    Sensitive(String),
}

impl AuditValue {
    fn redacted(&self, redactor: &dyn AuditRedactor) -> String {
        match self {
            AuditValue::Plain(value) => value.clone(),
            AuditValue::Sensitive(value) => redactor.redact(value),
        }
    }
}

/// Audit field key-value pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditField {
    /// Stable field key.
    pub key: &'static str,
    /// Field value.
    pub value: AuditValue,
}

/// Input audit event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    /// Event kind.
    pub kind: AuditEventKind,
    /// Structured fields.
    pub fields: Vec<AuditField>,
}

/// Redactor interface for sensitive audit fields.
pub trait AuditRedactor {
    /// Redact a sensitive value into a safe representation.
    fn redact(&self, value: &str) -> String;
}

/// Audit configuration limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditConfig {
    /// Maximum number of retained events.
    pub max_events: usize,
    /// Maximum encoded bytes per event.
    pub max_event_bytes: usize,
}

impl AuditConfig {
    /// Create a new audit configuration.
    pub fn new(max_events: usize, max_event_bytes: usize) -> Self {
        Self {
            max_events,
            max_event_bytes,
        }
    }

    /// Return a disabled audit configuration (drops all events).
    pub const fn disabled() -> Self {
        Self {
            max_events: 0,
            max_event_bytes: 0,
        }
    }
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_events: 8192,
            max_event_bytes: 2048,
        }
    }
}

/// Redacted audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Monotonic sequence number.
    pub sequence: u64,
    /// Event kind.
    pub kind: AuditEventKind,
    /// Redacted fields.
    pub fields: Vec<AuditFieldRecord>,
}

/// Redacted audit field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFieldRecord {
    /// Field key.
    pub key: &'static str,
    /// Redacted value.
    pub value: String,
}

/// In-memory audit log with deterministic retention.
#[derive(Debug)]
pub struct AuditLog<R: AuditRedactor> {
    config: AuditConfig,
    redactor: R,
    records: Vec<AuditRecord>,
    dropped: u64,
    next_sequence: u64,
}

impl<R: AuditRedactor> AuditLog<R> {
    /// Create a new audit log.
    pub fn new(config: AuditConfig, redactor: R) -> Self {
        Self {
            config,
            redactor,
            records: Vec::new(),
            dropped: 0,
            next_sequence: 1,
        }
    }

    /// Return retained audit records.
    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    /// Return number of dropped records due to retention/disabled config.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Record an audit event.
    pub fn record(&mut self, event: AuditEvent) -> Result<()> {
        if self.config.max_events == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return Ok(());
        }
        let record = self.redact_event(event);
        let size = record_size_bytes(&record);
        if size > self.config.max_event_bytes {
            return Err(limit_error(
                "max_audit_event_bytes",
                size as u64,
                self.config.max_event_bytes as u64,
            ));
        }
        if self.records.len() >= self.config.max_events {
            let overflow = self
                .records
                .len()
                .saturating_add(1)
                .saturating_sub(self.config.max_events);
            for _ in 0..overflow {
                if !self.records.is_empty() {
                    self.records.remove(0);
                    self.dropped = self.dropped.saturating_add(1);
                }
            }
        }
        self.records.push(record);
        Ok(())
    }

    fn redact_event(&mut self, event: AuditEvent) -> AuditRecord {
        let mut fields = Vec::with_capacity(event.fields.len());
        for field in event.fields {
            fields.push(AuditFieldRecord {
                key: field.key,
                value: field.value.redacted(&self.redactor),
            });
        }
        let record = AuditRecord {
            sequence: self.next_sequence,
            kind: event.kind,
            fields,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        record
    }
}

fn record_size_bytes(record: &AuditRecord) -> usize {
    let mut size = record.kind.as_str().len();
    for field in &record.fields {
        size = size.saturating_add(field.key.len());
        size = size.saturating_add(field.value.len());
    }
    size
}

fn limit_error(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "audit limit exceeded",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRedactor;

    impl AuditRedactor for FixedRedactor {
        fn redact(&self, _value: &str) -> String {
            "redacted".to_string()
        }
    }

    fn event_with_value(value: AuditValue) -> AuditEvent {
        AuditEvent {
            kind: AuditEventKind::AuthzDecision,
            fields: vec![AuditField {
                key: "subject",
                value,
            }],
        }
    }

    #[test]
    fn audit_redacts_sensitive_fields() {
        // REQ-AUDIT-350
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Sensitive(
            "secret".to_string(),
        )))
        .expect("record");
        assert_eq!(log.records().len(), 1);
        assert_eq!(log.records()[0].fields[0].value, "redacted");
    }

    #[test]
    fn audit_rotates_when_max_events_exceeded() {
        // REQ-AUDIT-351
        let mut log = AuditLog::new(AuditConfig::new(1, 1024), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("a".to_string())))
            .expect("record");
        log.record(event_with_value(AuditValue::Plain("b".to_string())))
            .expect("record");
        assert_eq!(log.records().len(), 1);
        assert_eq!(log.dropped(), 1);
        assert_eq!(log.records()[0].fields[0].value, "b");
    }

    #[test]
    fn audit_rejects_oversized_event() {
        // REQ-AUDIT-351, REQ-AUDIT-352
        let mut log = AuditLog::new(AuditConfig::new(10, 8), FixedRedactor);
        let err = log
            .record(event_with_value(AuditValue::Plain("too_large".to_string())))
            .expect_err("expected error");
        assert_eq!(err.code, "DVF.SECURITY.LIMIT_EXCEEDED");
        assert!(matches!(err.kind, ErrorKind::LimitExceeded { .. }));
    }
}
