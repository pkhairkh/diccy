//! Tamper-evident audit log with cryptographic signatures.
//!
//! Provides a wrapper around [`AuditLog`] that adds cryptographic signing
//! to each record and supports full-chain tamper detection. This is a
//! critical regulatory requirement under IEC 62304 and ISO 14971 for
//! medical device audit trails.

use crate::{AuditConfig, AuditEvent, AuditLog, AuditRedactor, ChainVerification};
use sha2::{Digest, Sha256};

/// Result of a tamper-evidence check on the audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamperCheckResult {
    /// Whether the entire chain is valid.
    pub is_valid: bool,
    /// If invalid, the index at which the chain is broken.
    pub broken_at: Option<usize>,
    /// Human-readable details about the check result.
    pub details: String,
}

/// Tamper-evident audit log with cryptographic signatures.
///
/// Wraps an [`AuditLog`] and adds a SHA-256 signature over each record's
/// integrity hash plus a signing key, producing a per-entry signature
/// that makes the log append-only and tamper-evident.
///
/// The signature chain works as follows:
/// 1. The inner `AuditLog` computes the SHA-256 integrity hash chain.
/// 2. This wrapper computes an additional signature over
///    `integrity_hash || signing_key` for each record.
/// 3. Verification checks both the inner integrity chain and the
///    outer signature chain.
#[derive(Debug)]
pub struct TamperEvidentLog<R: AuditRedactor> {
    /// The underlying audit log with SHA-256 integrity chain.
    pub inner: AuditLog<R>,
    /// Signing key for per-entry signatures.
    signing_key: Vec<u8>,
    /// Stored signatures for each record.
    signatures: Vec<Vec<u8>>,
}

impl<R: AuditRedactor> TamperEvidentLog<R> {
    /// Create a new tamper-evident log with the given config, redactor, and signing key.
    pub fn new(config: AuditConfig, redactor: R, signing_key: Vec<u8>) -> Self {
        Self {
            inner: AuditLog::new(config, redactor),
            signing_key,
            signatures: Vec::new(),
        }
    }

    /// Record an audit event with an additional cryptographic signature.
    ///
    /// The event is first recorded in the inner audit log (which computes
    /// the SHA-256 integrity chain), then a signature is computed over
    /// the record's integrity hash and the signing key.
    pub fn record(&mut self, event: AuditEvent) -> Result<(), Box<dicom_core::Error>> {
        self.inner.record(event)?;
        if let Some(record) = self.inner.records().last() {
            let sig = self.sign_entry(record.integrity_hash.as_bytes());
            self.signatures.push(sig);
        }
        Ok(())
    }

    /// Verify the entire chain has not been tampered with.
    ///
    /// This performs two checks:
    /// 1. The inner SHA-256 integrity hash chain is valid.
    /// 2. Each record's signature matches the computed signature.
    pub fn verify_tamper_evidence(&self) -> TamperCheckResult {
        // Check 1: Inner integrity hash chain
        match self.inner.verify_chain() {
            ChainVerification::Broken {
                sequence,
                expected,
                actual,
            } => {
                let idx = usize::try_from(sequence)
                    .ok()
                    .and_then(|s| if s > 0 { Some(s - 1) } else { None });
                return TamperCheckResult {
                    is_valid: false,
                    broken_at: idx,
                    details: format!(
                        "integrity hash chain broken at sequence {}: expected={}, actual={}",
                        sequence, expected, actual
                    ),
                };
            }
            ChainVerification::Valid => {}
        }

        // Check 2: Signature verification for each record
        let records = self.inner.records();
        if records.len() != self.signatures.len() {
            return TamperCheckResult {
                is_valid: false,
                broken_at: None,
                details: format!(
                    "record/signature count mismatch: {} records vs {} signatures",
                    records.len(),
                    self.signatures.len()
                ),
            };
        }

        for (i, record) in records.iter().enumerate() {
            let expected_sig = self.sign_entry(record.integrity_hash.as_bytes());
            if self.signatures[i] != expected_sig {
                return TamperCheckResult {
                    is_valid: false,
                    broken_at: Some(i),
                    details: format!(
                        "signature mismatch at record {} (sequence {})",
                        i, record.sequence
                    ),
                };
            }
        }

        TamperCheckResult {
            is_valid: true,
            broken_at: None,
            details: format!(
                "all {} records verified: integrity chain and signatures valid",
                records.len()
            ),
        }
    }

    /// Return a reference to the inner audit log records.
    pub fn records(&self) -> &[crate::AuditRecord] {
        self.inner.records()
    }

    /// Return the number of dropped records.
    pub fn dropped(&self) -> u64 {
        self.inner.dropped()
    }

    /// Sign an entry by computing SHA-256(integrity_hash || signing_key).
    fn sign_entry(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(&self.signing_key);
        hasher.finalize().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuditEventKind, AuditField, AuditValue};

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
    fn tamper_evident_log_clean_chain_validates() {
        let mut log =
            TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"test-key".to_vec());
        for i in 0..5 {
            log.record(event_with_value(AuditValue::Plain(format!("event{i}"))))
                .expect("record");
        }
        let result = log.verify_tamper_evidence();
        assert!(result.is_valid, "clean chain should validate: {}", result.details);
        assert!(result.broken_at.is_none());
    }

    #[test]
    fn tamper_evident_log_detects_tampered_record() {
        let mut log =
            TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"test-key".to_vec());
        log.record(event_with_value(AuditValue::Plain("original".to_string())))
            .expect("record");
        log.record(event_with_value(AuditValue::Plain("second".to_string())))
            .expect("record");

        // Tamper with the first record's integrity hash
        log.inner.records[0].integrity_hash = "0".repeat(64);

        let result = log.verify_tamper_evidence();
        assert!(!result.is_valid, "tampered chain should not validate");
        assert!(result.broken_at.is_some(), "should report where chain is broken");
    }

    #[test]
    fn tamper_evident_log_empty_log_is_valid() {
        let log = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"key".to_vec());
        let result = log.verify_tamper_evidence();
        assert!(result.is_valid, "empty log should be valid");
    }

    #[test]
    fn tamper_evident_log_signature_count_mismatch_detected() {
        let mut log =
            TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"key".to_vec());
        log.record(event_with_value(AuditValue::Plain("test".to_string())))
            .expect("record");

        // Artificially add a bogus signature to create a mismatch
        log.signatures.push(vec![0u8; 32]);

        let result = log.verify_tamper_evidence();
        assert!(!result.is_valid, "signature count mismatch should be detected");
        assert!(result.details.contains("mismatch"));
    }

    #[test]
    fn tamper_evident_log_signs_with_key() {
        let key1 = b"key-1".to_vec();
        let key2 = b"key-2".to_vec();

        let mut log1 = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, key1);
        let mut log2 = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, key2);

        // Record the same event in both logs
        let event = event_with_value(AuditValue::Plain("same".to_string()));
        log1.record(event.clone()).expect("record");
        log2.record(event).expect("record");

        // Both should validate with their own keys
        assert!(log1.verify_tamper_evidence().is_valid);
        assert!(log2.verify_tamper_evidence().is_valid);

        // Signatures should differ because the keys differ
        assert_ne!(log1.signatures[0], log2.signatures[0]);
    }
}
