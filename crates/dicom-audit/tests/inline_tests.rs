// Auto-extracted from /home/z/diccy/crates/dicom-audit/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_audit::*;
    use dicom_core::{ErrorKind};

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
