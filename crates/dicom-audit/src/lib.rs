#![deny(missing_docs)]

//! Audit logging with deterministic redaction, retention, and SHA-256
//! integrity chain verification.
//!
//! Each `AuditRecord` carries an `integrity_hash` computed over its contents
//! and the previous record's hash, forming a tamper-evident chain.  Because
//! this crate is pre-1.0, SHA-256 is the only accepted hash algorithm — no
//! FNV migration path is provided.

use dicom_core::{Error, ErrorKind, Result};
use sha2::{Digest, Sha256};

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

impl AuditEvent {
    /// Validate that this audit event has the required fields for recording.
    ///
    /// Returns an error if the event has no fields or if any field has an
    /// empty key, which would produce an ambiguous audit record.
    pub fn validate(&self) -> Result<()> {
        if self.fields.is_empty() {
            return Err(limit_error("audit_event_fields", 0, 1));
        }
        for field in &self.fields {
            if field.key.is_empty() {
                return Err(dicom_core::Error::from_kind(
                    dicom_core::ErrorKind::PolicyViolation {
                        policy: "audit_field_key".to_string(),
                        detail: "audit field key must not be empty".to_string(),
                    },
                    "audit event field has empty key",
                )
                .into());
            }
        }
        Ok(())
    }
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

/// SHA-256 integrity hash (hex-encoded, 64 characters).
pub type IntegrityHash = String;

/// Redacted audit record with SHA-256 integrity hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    /// Monotonic sequence number.
    pub sequence: u64,
    /// Event kind.
    pub kind: AuditEventKind,
    /// Redacted fields.
    pub fields: Vec<AuditFieldRecord>,
    /// SHA-256 integrity hash covering this record and the previous hash.
    pub integrity_hash: IntegrityHash,
}

/// Redacted audit field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFieldRecord {
    /// Field key.
    pub key: &'static str,
    /// Redacted value.
    pub value: String,
}

/// Outcome of chain verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainVerification {
    /// All records have valid integrity hashes and the chain is intact.
    Valid,
    /// The chain is broken at the indicated sequence number.
    Broken {
        /// Sequence number of the first invalid record.
        sequence: u64,
        /// Expected integrity hash.
        expected: IntegrityHash,
        /// Actual integrity hash stored in the record.
        actual: IntegrityHash,
    },
}

/// In-memory audit log with deterministic retention and SHA-256 integrity chain.
#[derive(Debug)]
pub struct AuditLog<R: AuditRedactor> {
    config: AuditConfig,
    redactor: R,
    records: Vec<AuditRecord>,
    dropped: u64,
    next_sequence: u64,
    /// Hash of the most recently appended record (used to chain the next one).
    prev_hash: IntegrityHash,
}

/// The hash used for the genesis (first) record when no prior hash exists.
const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

impl<R: AuditRedactor> AuditLog<R> {
    /// Create a new audit log.
    pub fn new(config: AuditConfig, redactor: R) -> Self {
        Self {
            config,
            redactor,
            records: Vec::new(),
            dropped: 0,
            next_sequence: 1,
            prev_hash: GENESIS_PREV_HASH.to_string(),
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
        let mut rec = self.redact_event(event);
        let size = record_size_bytes(&rec);
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
        // Compute SHA-256 integrity hash over record content + previous hash.
        rec.integrity_hash = compute_record_hash(&rec, &self.prev_hash);
        self.prev_hash = rec.integrity_hash.clone();
        self.records.push(rec);
        Ok(())
    }

    /// Verify the integrity of the entire audit chain.
    ///
    /// Recomputes each record's SHA-256 hash from its content and the
    /// preceding record's hash, then compares against the stored hash.
    /// Returns [`ChainVerification::Valid`] if all hashes match, or
    /// [`ChainVerification::Broken`] at the first mismatch.
    pub fn verify_chain(&self) -> ChainVerification {
        let mut prev = GENESIS_PREV_HASH.to_string();
        for rec in &self.records {
            let expected = compute_record_hash(rec, &prev);
            if rec.integrity_hash != expected {
                return ChainVerification::Broken {
                    sequence: rec.sequence,
                    expected,
                    actual: rec.integrity_hash.clone(),
                };
            }
            prev = rec.integrity_hash.clone();
        }
        ChainVerification::Valid
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
            integrity_hash: String::new(), // filled in by caller
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        record
    }
}

/// Compute the SHA-256 integrity hash for an `AuditRecord`.
///
/// The preimage is a deterministic string encoding of the record's sequence,
/// kind, and fields, concatenated with the previous record's hash.
fn compute_record_hash(record: &AuditRecord, prev_hash: &str) -> IntegrityHash {
    // Deterministic encoding: sequence|kind|key1=val1|key2=val2|...|prev_hash
    let mut preimage = String::new();
    preimage.push_str(&record.sequence.to_string());
    preimage.push('|');
    preimage.push_str(record.kind.as_str());
    for field in &record.fields {
        preimage.push('|');
        preimage.push_str(field.key);
        preimage.push('=');
        preimage.push_str(&field.value);
    }
    preimage.push('|');
    preimage.push_str(prev_hash);

    let mut hasher = Sha256::new();
    hasher.update(preimage.as_bytes());
    let result = hasher.finalize();
    format!("{result:02x}")
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
        assert_eq!(err.code(), "DVF.SECURITY.LIMIT_EXCEEDED");
        assert!(matches!(err.kind(), ErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn audit_record_has_sha256_integrity_hash() {
        // S11-T7: each record carries a 64-char hex SHA-256 hash
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("test".to_string())))
            .expect("record");
        let hash = &log.records()[0].integrity_hash;
        assert_eq!(hash.len(), 64, "SHA-256 hex digest must be 64 chars");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn audit_chain_verification_valid() {
        // S11-T7: chain verification succeeds on untampered records
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        for i in 0..5 {
            log.record(event_with_value(AuditValue::Plain(format!("event{i}"))))
                .expect("record");
        }
        assert_eq!(log.verify_chain(), ChainVerification::Valid);
    }

    #[test]
    fn audit_chain_detects_tampered_record() {
        // S11-T7: tampering with a record's field value breaks the chain
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("original".to_string())))
            .expect("record");
        log.record(event_with_value(AuditValue::Plain("second".to_string())))
            .expect("record");

        // Tamper with the first record's field value
        log.records[0].fields[0].value = "tampered".to_string();

        let result = log.verify_chain();
        match result {
            ChainVerification::Broken {
                sequence,
                expected: _,
                actual: _,
            } => {
                assert_eq!(sequence, 1, "broken at the tampered record");
            }
            ChainVerification::Valid => {
                panic!("chain verification should detect tampered record");
            }
        }
    }

    #[test]
    fn audit_chain_detects_tampered_hash() {
        // S11-T7: tampering with the integrity hash itself is also detected
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("data".to_string())))
            .expect("record");
        log.record(event_with_value(AuditValue::Plain("more".to_string())))
            .expect("record");

        // Tamper with the first record's hash
        log.records[0].integrity_hash = "0".repeat(64);

        let result = log.verify_chain();
        match result {
            ChainVerification::Broken { sequence, .. } => {
                assert_eq!(sequence, 1);
            }
            ChainVerification::Valid => {
                panic!("chain verification should detect tampered hash");
            }
        }
    }

    #[test]
    fn audit_chain_deterministic() {
        // S11-T7: same inputs produce same hashes
        let mut log1 = AuditLog::new(AuditConfig::default(), FixedRedactor);
        let mut log2 = AuditLog::new(AuditConfig::default(), FixedRedactor);
        for i in 0..3 {
            let ev = event_with_value(AuditValue::Plain(format!("v{i}")));
            log1.record(ev.clone()).expect("record");
            log2.record(ev).expect("record");
        }
        for (r1, r2) in log1.records().iter().zip(log2.records().iter()) {
            assert_eq!(r1.integrity_hash, r2.integrity_hash);
        }
    }

    #[test]
    fn audit_chain_empty_log_is_valid() {
        let log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        assert_eq!(log.verify_chain(), ChainVerification::Valid);
    }
}
