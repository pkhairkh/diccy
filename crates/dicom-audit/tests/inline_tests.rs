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

// ===========================================================================
// S15-T5: ATNA, tamper-evident, and RBAC audit tests
// ===========================================================================

    use dicom_audit::atna::{AtnaExporter, AtnaEventType};
    use dicom_audit::rbac_audit::{audit_rbac_check, RbacDecision};
    use dicom_audit::tamper_evident::TamperEvidentLog;

    // ---- ATNA export tests ----

    #[test]
    fn atna_export_produces_valid_xml() {
        // REQ-AUDIT-004: ATNA export produces valid IHE ATNA messages
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("test".to_string())))
            .expect("record");

        let record = &log.records()[0];
        let xml = AtnaExporter::export_as_atna_xml(record);
        assert!(xml.contains("<?xml"), "ATNA XML must have XML declaration");
        assert!(xml.contains("<AuditMessage>"), "ATNA XML must have AuditMessage root");
        assert!(xml.contains("<EventIdentification"), "ATNA XML must have EventIdentification");
        assert!(xml.contains("<ActiveParticipant"), "ATNA XML must have ActiveParticipant");
        assert!(xml.contains("<ParticipantObjectIdentification"), "ATNA XML must have ParticipantObjectIdentification");
    }

    #[test]
    fn atna_export_batch_all_records() {
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        for i in 0..3 {
            log.record(event_with_value(AuditValue::Plain(format!("ev{i}"))))
                .expect("record");
        }
        let batch = AtnaExporter::export_batch(log.records());
        assert_eq!(batch.len(), 3);
        for xml in &batch {
            assert!(xml.contains("<AuditMessage>"));
        }
    }

    #[test]
    fn atna_event_type_mapping() {
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::AuthzDecision),
            AtnaEventType::SecurityAlert
        );
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::AuthnFailure),
            AtnaEventType::SecurityAlert
        );
        assert_eq!(
            AtnaEventType::from_audit_kind(AuditEventKind::ServiceEvent),
            AtnaEventType::ApplicationActivity
        );
    }

    #[test]
    fn atna_dicom_export_format() {
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        log.record(event_with_value(AuditValue::Plain("test".to_string())))
            .expect("record");

        let binary = AtnaExporter::export_as_atna_dicom(&log.records()[0]);
        assert!(binary.len() > 4, "binary must have 4-byte header + payload");
        let len = u32::from_be_bytes([binary[0], binary[1], binary[2], binary[3]]);
        assert_eq!(len as usize, binary.len() - 4, "header length must match payload");
    }

    // ---- Tamper-evident log tests ----

    #[test]
    fn tamper_evident_clean_chain_validates() {
        // REQ-AUDIT-006: tamper-evident log validates clean chain
        let mut tel = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"key".to_vec());
        for i in 0..5 {
            tel.record(event_with_value(AuditValue::Plain(format!("ev{i}"))))
                .expect("record");
        }
        let result = tel.verify_tamper_evidence();
        assert!(result.is_valid, "clean tamper-evident chain should validate: {}", result.details);
    }

    #[test]
    fn tamper_evident_detects_tampered_record() {
        // REQ-AUDIT-006: tamper-evident log detects tampering
        let mut tel = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"key".to_vec());
        tel.record(event_with_value(AuditValue::Plain("original".to_string())))
            .expect("record");
        tel.record(event_with_value(AuditValue::Plain("second".to_string())))
            .expect("record");

        // Tamper with the inner log's record
        tel.inner.records[0].fields[0].value = "tampered".to_string();

        let result = tel.verify_tamper_evidence();
        assert!(!result.is_valid, "tampered chain should not validate");
        assert!(result.broken_at.is_some(), "should report where chain is broken");
    }

    #[test]
    fn tamper_evident_empty_log_is_valid() {
        let tel = TamperEvidentLog::new(AuditConfig::default(), FixedRedactor, b"key".to_vec());
        assert!(tel.verify_tamper_evidence().is_valid);
    }

    // ---- RBAC audit event tests ----

    #[test]
    fn rbac_audit_allowed_generates_event() {
        // REQ-AUDIT-005: audit events for RBAC allowed decisions
        let event = audit_rbac_check(
            "dr.smith",
            "Radiologist",
            "ReadStudy",
            "1.2.840.113619",
            RbacDecision::Allowed,
        );
        assert_eq!(event.kind, AuditEventKind::AuthzDecision);

        let decision = event.fields.iter().find(|f| f.key == "decision").unwrap();
        assert_eq!(decision.value, AuditValue::Plain("Allowed".to_string()));
    }

    #[test]
    fn rbac_audit_denied_generates_event() {
        // REQ-AUDIT-005: audit events for RBAC denied decisions
        let event = audit_rbac_check(
            "tech.jones",
            "Technologist",
            "DeleteStudy",
            "1.2.840.113619",
            RbacDecision::Denied,
        );
        let decision = event.fields.iter().find(|f| f.key == "decision").unwrap();
        assert_eq!(decision.value, AuditValue::Plain("Denied".to_string()));

        // Denied events must have a reason
        let reason = event.fields.iter().find(|f| f.key == "reason");
        assert!(reason.is_some(), "denied events must have reason");
    }

    #[test]
    fn rbac_audit_event_can_be_recorded() {
        // RBAC audit events must be recordable in an AuditLog
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        let allowed_event = audit_rbac_check(
            "dr.smith",
            "Radiologist",
            "ReadStudy",
            "1.2.840.113619",
            RbacDecision::Allowed,
        );
        log.record(allowed_event).expect("allowed event should record");

        let denied_event = audit_rbac_check(
            "tech.jones",
            "Technologist",
            "DeleteStudy",
            "1.2.840.113619",
            RbacDecision::Denied,
        );
        log.record(denied_event).expect("denied event should record");

        assert_eq!(log.records().len(), 2, "both RBAC events should be recorded");
    }

    #[test]
    fn rbac_audit_sensitive_fields_are_redacted() {
        let mut log = AuditLog::new(AuditConfig::default(), FixedRedactor);
        let event = audit_rbac_check(
            "dr.smith",
            "Radiologist",
            "ReadStudy",
            "1.2.840.113619",
            RbacDecision::Allowed,
        );
        log.record(event).expect("record");

        let record = &log.records()[0];
        // principal and resource should be redacted
        let principal = record.fields.iter().find(|f| f.key == "principal").unwrap();
        assert_eq!(principal.value, "redacted", "principal should be redacted");
        let resource = record.fields.iter().find(|f| f.key == "resource").unwrap();
        assert_eq!(resource.value, "redacted", "resource should be redacted");
    }
