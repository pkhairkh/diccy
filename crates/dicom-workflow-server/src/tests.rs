    use super::*;
    use dicom_workflow_server::{path_with_suffix, workflow_route_contract, WorkflowRouteContract};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("diccy_{name}_{nonce}.{ext}"))
    }

    fn cleanup_with_rotations(path: &Path, max_rotations: usize) {
        let _ = fs::remove_file(path);
        for index in 1..=max_rotations + 1 {
            let _ = fs::remove_file(path_with_suffix(path, index));
        }
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }

    fn with_env_var<F, R>(name: &str, value: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var(name).ok();
        match value {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        let result = f();
        match previous {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        result
    }

    fn with_env_vars<F, R>(pairs: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = pairs
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect::<Vec<_>>();
        for (name, value) in pairs {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        let result = f();
        for (name, value) in previous {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    #[test]
    fn parse_u64_uses_default_when_unset() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", None, || {
            let value = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("default");
            assert_eq!(value, 300);
        });
    }

    #[test]
    fn parse_u64_rejects_invalid_value() {
        with_env_var(
            "DICOM_WORKFLOW_TEST_RATE_LIMIT",
            Some("not-a-number"),
            || {
                let err = parse_u64(
                    WORKFLOW_SERVICE_NAME,
                    "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                    300,
                    NumericBounds::at_least(1),
                )
                .expect_err("invalid value must fail");
                assert_eq!(err.kind(), IoErrorKind::InvalidInput);
                assert!(err.to_string().contains("DICOM_WORKFLOW_TEST_RATE_LIMIT"));
            },
        );
    }

    #[test]
    fn parse_u64_accepts_positive_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("456"), || {
            let value = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("positive value");
            assert_eq!(value, 456);
        });
    }

    #[test]
    fn parse_u64_rejects_out_of_range_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("0"), || {
            let err = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_u64_rejects_negative_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("-1"), || {
            let err = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("invalid value"));
        });
    }

    #[test]
    fn parse_u64_is_idempotent_for_repeated_invocations() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("456"), || {
            let first = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("first parse");
            let second = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("second parse");
            assert_eq!(first, second);
        });
    }

    #[test]
    fn parse_usize_is_idempotent_for_repeated_invocations() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("6"), || {
            let first = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("first parse");
            let second = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("second parse");
            assert_eq!(first, second);
        });
    }

    #[test]
    fn parse_usize_uses_default_when_unset() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", None, || {
            let value = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("default");
            assert_eq!(value, 2);
        });
    }

    #[test]
    fn parse_usize_rejects_invalid_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("bad"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("invalid value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("DICOM_WORKFLOW_TEST_ROTATIONS"));
        });
    }

    #[test]
    fn parse_usize_accepts_positive_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("6"), || {
            let value = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("positive value");
            assert_eq!(value, 6);
        });
    }

    #[test]
    fn parse_usize_rejects_out_of_range_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("0"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_usize_rejects_negative_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("-1"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("invalid value"));
        });
    }

    #[test]
    fn workflow_runtime_config_from_env_is_idempotent() {
        with_env_vars(
            &[
                ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", Some("451")),
                ("DICOM_WORKFLOW_MUTATION_RATE_LIMIT", Some("129")),
                ("DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT", Some("64")),
                ("DICOM_WORKFLOW_DENYLIST_PATHS", Some("/test|/other")),
            ],
            || {
                let first = WorkflowRuntimeConfig::from_env().expect("first config");
                let second = WorkflowRuntimeConfig::from_env().expect("second config");
                assert_eq!(first.query_rate_limit, second.query_rate_limit);
                assert_eq!(first.mutation_rate_limit, second.mutation_rate_limit);
                assert_eq!(first.audit_export_limit, second.audit_export_limit);
                assert_eq!(first.denylist_routes, second.denylist_routes);
                assert_eq!(
                    first.hl7_transport.mllp_enabled,
                    second.hl7_transport.mllp_enabled
                );
                assert_eq!(
                    first.hl7_transport.mllp_bind,
                    second.hl7_transport.mllp_bind
                );
                assert_eq!(
                    first.hl7_transport.file_drop_dir,
                    second.hl7_transport.file_drop_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_done_dir,
                    second.hl7_transport.file_drop_done_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_error_dir,
                    second.hl7_transport.file_drop_error_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_poll_interval_ms,
                    second.hl7_transport.file_drop_poll_interval_ms
                );
            },
        );
    }

    #[test]
    fn parse_log_level_rejects_invalid_value_from_env() {
        let err = with_env_var("DICOM_WORKFLOW_LOG_LEVEL", Some("invalid"), || {
            parse_log_level("DICOM_WORKFLOW_LOG_LEVEL").expect_err("invalid log level must fail")
        });
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_WORKFLOW_LOG_LEVEL"));
    }

    #[test]
    fn test_only_env_vars_do_not_change_production_rate_defaults() {
        let config = with_env_vars(
            &[
                ("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("9999")),
                ("DICOM_WORKFLOW_TEST_ROTATIONS", Some("9999")),
                ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", None),
                ("DICOM_WORKFLOW_MUTATION_RATE_LIMIT", None),
            ],
            || WorkflowRuntimeConfig::from_env(),
        )
        .expect("runtime config defaults");
        assert_eq!(config.query_rate_limit, DEFAULT_QUERY_RATE_LIMIT);
        assert_eq!(config.mutation_rate_limit, DEFAULT_MUTATION_RATE_LIMIT);
    }

    #[test]
    fn parse_hl7_transport_config_requires_bind_when_enabled() {
        let err = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("true")),
                ("DICOM_WORKFLOW_HL7_MLLP_BIND", None),
            ],
            || parse_hl7_transport_config(),
        )
        .expect_err("MLLP enabled without bind must fail");
        assert!(err
            .to_string()
            .contains("DICOM_WORKFLOW_HL7_MLLP_ENABLED requires DICOM_WORKFLOW_HL7_MLLP_BIND"));
    }

    #[test]
    fn parse_hl7_transport_config_rejects_invalid_mllp_bind() {
        let err = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("true")),
                ("DICOM_WORKFLOW_HL7_MLLP_BIND", Some("bad:bind")),
            ],
            || parse_hl7_transport_config(),
        )
        .expect_err("invalid bind must fail");
        assert!(err.to_string().contains("requires a numeric TCP port"));
    }

    #[test]
    fn parse_hl7_transport_config_supports_file_drop_paths() {
        let drop_dir = temp_file_path("interop_drop", "in");
        let done_dir = temp_file_path("interop_drop", "done");
        let error_dir = temp_file_path("interop_drop", "err");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");
        let drop_dir = drop_dir.to_string_lossy().to_string();
        let done_dir = done_dir.to_string_lossy().to_string();
        let error_dir = error_dir.to_string_lossy().to_string();

        let config = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("false")),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_DIR", Some(&drop_dir)),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR", Some(&done_dir)),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR", Some(&error_dir)),
                (
                    "DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS",
                    Some("4000"),
                ),
            ],
            || parse_hl7_transport_config(),
        )
        .expect("valid file-drop config");
        assert!(config.file_drop_dir.is_some());
        assert!(config.file_drop_done_dir.is_some());
        assert!(config.file_drop_error_dir.is_some());
        assert_eq!(config.file_drop_poll_interval_ms, 4000);

        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn parse_hl7_raw_message_payload_defaults_status_to_scheduled() {
        let body = b"MSH|^~\\&|pre|his|rdr|rdr|20260201||x|ADT|MSG0001|P|2.3\rPID|1||PAT-1||||\r";
        let params =
            parse_hl7_raw_message_payload(body, &Limits::default()).expect("parse raw HL7 payload");
        assert_eq!(params.get("status"), Some(&"SCHEDULED".to_string()));
        assert_eq!(params.get("patient_id"), Some(&"PAT-1".to_string()));
        assert_eq!(params.get("source"), Some(&"his".to_string()));
    }

    #[test]
    fn take_next_hl7_mllp_frame_extracts_payloads() {
        let mut buffer = Vec::from(b"noise\x0bMSH|^~\\&|HIS||RDR|\x1c\x0dAFTER");
        let frame = take_next_hl7_mllp_frame(&mut buffer, 1024).expect("extract mllp frame");
        assert_eq!(frame, Some(b"MSH|^~\\&|HIS||RDR|".to_vec()));
        assert_eq!(buffer, b"AFTER");
    }

    #[test]
    fn build_mllp_ack_wraps_frame_with_control_bytes() {
        let frame = build_mllp_ack("AA", None);
        assert_eq!(frame.first(), Some(&HL7_MLLP_START_BYTE));
        assert_eq!(
            &frame[frame.len() - HL7_MLLP_END_BYTES.len()..],
            HL7_MLLP_END_BYTES.as_slice()
        );
        assert_eq!(
            std::str::from_utf8(&frame[1..frame.len() - HL7_MLLP_END_BYTES.len()]).expect("utf8"),
            "MSA|AA|ACK\r"
        );
    }

    #[test]
    fn build_mllp_nack_includes_deterministic_error_code() {
        let frame = build_mllp_ack("AE", Some("DVF.WORKFLOW.HTTP.DECODE_ERROR"));
        assert_eq!(
            std::str::from_utf8(&frame[1..frame.len() - HL7_MLLP_END_BYTES.len()]).expect("utf8"),
            "MSA|AE|ACK\rERR|DVF.WORKFLOW.HTTP.DECODE_ERROR\r"
        );
    }

    #[test]
    fn classify_hl7_mllp_nack_maps_error_classes_deterministically() {
        let auth = auth_denied_error("forbidden");
        let decode = decode_error("bad payload");
        let not_found = not_found_error("missing", "DVF.WORKFLOW.MPPS.NOT_FOUND");

        let (auth_ack, auth_code) = classify_hl7_mllp_nack(auth.as_ref());
        let (decode_ack, decode_code) = classify_hl7_mllp_nack(decode.as_ref());
        let (nf_ack, nf_code) = classify_hl7_mllp_nack(not_found.as_ref());

        assert_eq!(auth_ack, "AR");
        assert_eq!(auth_code, "DVF.DICOM.DECODE_ERROR");
        assert_eq!(decode_ack, "AE");
        assert_eq!(decode_code, "DVF.WORKFLOW.HTTP.DECODE_ERROR");
        assert_eq!(nf_ack, "AE");
        assert_eq!(nf_code, "DVF.WORKFLOW.MPPS.NOT_FOUND");
    }

    #[test]
    fn hl7_dual_mode_ingest_deduplicates_under_concurrent_delivery() {
        let worklist_path = temp_file_path("interop_dual_mode_dedupe_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_dual_mode_dedupe_mpps", "snapshot");
        let sr_path = temp_file_path("interop_dual_mode_dedupe_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_dual_mode_dedupe_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let drop_dir = temp_file_path("interop_dual_mode_dedupe", "drop");
        let done_dir = temp_file_path("interop_dual_mode_dedupe", "done");
        let error_dir = temp_file_path("interop_dual_mode_dedupe", "error");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let limits = Arc::new(Limits::default());
        let payload = b"source=his&message_type=ADT&message_control_id=MSG-DUAL-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-1".to_vec();
        fs::write(drop_dir.join("dedupe.hl7"), &payload).expect("write drop payload");

        let shared_for_mllp = Arc::clone(&shared);
        let limits_for_mllp = Arc::clone(&limits);
        let payload_for_mllp = payload.clone();
        let ingest_handle = thread::spawn(move || {
            run_hl7_ingest_payload(
                &payload_for_mllp,
                None,
                "corr-dual-mllp",
                &shared_for_mllp,
                limits_for_mllp.as_ref(),
            )
        });

        run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &shared, &limits)
            .expect("run file-drop once");
        let ingest_result = ingest_handle.join().expect("join mllp ingest thread");
        assert!(ingest_result.is_ok());

        {
            let state = shared.lock().expect("state lock after dual-mode dedupe");
            assert_eq!(state.hl7.lock().event_seq, 1);
            assert_eq!(state.hl7.lock().replay_cache.len(), 1);
            assert!(state.hl7.lock().failures.is_empty());
        }

        let done_count = fs::read_dir(&done_dir)
            .expect("read done directory")
            .count();
        let error_count = fs::read_dir(&error_dir)
            .expect("read error directory")
            .count();
        assert_eq!(done_count, 1);
        assert_eq!(error_count, 0);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn hl7_dual_mode_conflict_resolution_routes_divergent_payload_to_error_dir() {
        let worklist_path = temp_file_path("interop_dual_mode_conflict_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_dual_mode_conflict_mpps", "snapshot");
        let sr_path = temp_file_path("interop_dual_mode_conflict_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_dual_mode_conflict_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let drop_dir = temp_file_path("interop_dual_mode_conflict", "drop");
        let done_dir = temp_file_path("interop_dual_mode_conflict", "done");
        let error_dir = temp_file_path("interop_dual_mode_conflict", "error");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let limits = Arc::new(Limits::default());
        let accepted_payload = b"source=his&message_type=ADT&message_control_id=MSG-CONFLICT-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-1".to_vec();
        run_hl7_ingest_payload(
            &accepted_payload,
            None,
            "corr-conflict-primary",
            &shared,
            limits.as_ref(),
        )
        .expect("seed accepted replay payload");

        let divergent_payload = b"source=his&message_type=ADT&message_control_id=MSG-CONFLICT-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-2".to_vec();
        fs::write(drop_dir.join("conflict.hl7"), &divergent_payload)
            .expect("write conflict payload");
        run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &shared, &limits)
            .expect("run file-drop once");

        let done_count = fs::read_dir(&done_dir)
            .expect("read done directory")
            .count();
        let error_count = fs::read_dir(&error_dir)
            .expect("read error directory")
            .count();
        assert_eq!(done_count, 0);
        assert_eq!(error_count, 1);
        {
            let state = shared
                .lock()
                .expect("state lock after dual-mode conflict resolution");
            assert_eq!(state.hl7.lock().event_seq, 1);
            assert_eq!(state.hl7.lock().replay_cache.len(), 1);
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn parse_denylist_routes_falls_back_to_defaults_when_empty() {
        let denied = parse_denylist_routes(Some(" ".to_string()));
        assert_eq!(denied, *DEFAULT_WORKFLOW_DENYLIST_PATHS);
    }

    #[test]
    fn parse_denylist_routes_splits_multiple_delimiters_and_dedups() {
        let denied = parse_denylist_routes(Some(
            "/a,/b;/c\n/c; /d\t/e "
                .to_string()
                .replace("/e", "/d")
                .into(),
        ));
        assert_eq!(denied, vec!["/a", "/b", "/c", "/d"]);
    }

    #[test]
    fn denylist_precedence_applies_before_role_and_tenant_scope_checks() {
        let worklist_path = temp_file_path("denylist_precedence_worklist", "snapshot");
        let mpps_path = temp_file_path("denylist_precedence_mpps", "snapshot");
        let sr_path = temp_file_path("denylist_precedence_sr", "snapshot");
        let sr_audit_path = temp_file_path("denylist_precedence_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut store = shared.lock().expect("state lock");
            store.health.read().denylist_routes = vec!["/workflow/audit".to_string()];
        }

        let mut viewer_headers = BTreeMap::new();
        viewer_headers.insert("x-sr-principal".to_string(), "viewer-user".to_string());
        viewer_headers.insert("x-sr-role".to_string(), "viewer".to_string());
        viewer_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());
        let denied_viewer = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-b".to_string())]),
            headers: viewer_headers,
            body: Vec::new(),
        };
        let viewer_err = route_request(&denied_viewer, &shared, &limits)
            .expect_err("denylist should fail first");
        assert_eq!(viewer_err.code(), "DVF.DICOM.DECODE_ERROR");
        assert!(viewer_err
            .to_string()
            .contains("workflow operation deny-list"));

        let mut admin_headers = BTreeMap::new();
        admin_headers.insert("x-sr-principal".to_string(), "admin-user".to_string());
        admin_headers.insert("x-sr-role".to_string(), "admin".to_string());
        admin_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());
        let denied_admin = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-b".to_string())]),
            headers: admin_headers,
            body: Vec::new(),
        };
        let admin_err =
            route_request(&denied_admin, &shared, &limits).expect_err("denylist should fail first");
        assert_eq!(admin_err.code(), "DVF.DICOM.DECODE_ERROR");
        assert!(admin_err
            .to_string()
            .contains("workflow operation deny-list"));

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_callback_idempotency_prune_enforces_ttl() {
        let mut cache = BTreeMap::new();
        let _ = cache.insert("old".to_string(), 1_000);
        let _ = cache.insert("fresh".to_string(), 9_500);
        hl7_callback_idempotency_prune(&mut cache, 10_000, 1_000);
        assert!(!cache.contains_key("old"));
        assert!(cache.contains_key("fresh"));
    }

    #[test]
    fn hl7_callback_idempotency_cache_load_filters_stale_entries() {
        let path = temp_file_path("hl7_callback_idempotency", "cache");
        fs::write(&path, "1000\told\n9500\tfresh\nbad-line\n")
            .expect("write callback idempotency cache fixture");
        let loaded = load_hl7_callback_idempotency_cache(&path.to_string_lossy(), 10_000, 1_000);
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("fresh"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn webhook_auth_strategy_hmac_requires_shared_secret_for_third_party_destinations() {
        let _guard = ENV_LOCK.lock().expect("env lock for webhook auth");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        let previous_secret = env::var(WEBHOOK_AUTH_SECRET_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "hmac-sha256");
        env::remove_var(WEBHOOK_AUTH_SECRET_ENV);

        let subscription = Hl7Subscription {
            id: "sub-webhook".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()].into_iter().collect(),
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let err = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &BTreeMap::new(),
            "corr-1",
            1,
        )
        .expect_err("hmac strategy without shared secret must fail");
        assert_eq!(
            err,
            "webhook auth strategy hmac-sha256 requires shared secret"
        );

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
        if let Some(raw) = previous_secret {
            env::set_var(WEBHOOK_AUTH_SECRET_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_SECRET_ENV);
        }
    }

    #[test]
    fn webhook_auth_strategy_invalid_value_fails_closed() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for webhook auth invalid strategy");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "invalid-strategy");

        let subscription = Hl7Subscription {
            id: "sub-webhook-invalid".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()].into_iter().collect(),
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let err = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &BTreeMap::new(),
            "corr-1",
            1,
        )
        .expect_err("invalid strategy must fail closed");
        assert_eq!(err, "unsupported webhook auth strategy");

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
    }

    #[test]
    fn webhook_auth_strategy_hmac_generates_deterministic_signature() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for webhook auth deterministic signature");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        let previous_secret = env::var(WEBHOOK_AUTH_SECRET_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "hmac-sha256");
        env::set_var(WEBHOOK_AUTH_SECRET_ENV, "shared-secret");

        let subscription = Hl7Subscription {
            id: "sub-webhook".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()].into_iter().collect(),
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let mut payload = BTreeMap::new();
        let _ = payload.insert("task_id".to_string(), "TASK-001".to_string());
        let first = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &payload,
            "corr-1",
            1,
        )
        .expect("signature")
        .expect("hmac strategy returns signature");
        let second = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &payload,
            "corr-1",
            1,
        )
        .expect("signature")
        .expect("hmac strategy returns signature");
        assert_eq!(first, second);
        assert!(!first.is_empty());

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
        if let Some(raw) = previous_secret {
            env::set_var(WEBHOOK_AUTH_SECRET_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_SECRET_ENV);
        }
    }

    #[test]
    fn parse_hl7_connector_registry_supports_prefix_precedence() {
        let _guard = ENV_LOCK.lock().expect("env lock for connector registry");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR").ok(),
            ),
        ];
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP",
            "https://default.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A",
            "https://exact.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR",
            "https://fallback.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR",
            "https://a-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR",
            "https://ax-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR",
            "https://a1-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR",
            "https://east-prefix.corp/connect",
        );

        let registry = parse_hl7_connector_registry();
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_alice"),
            Some("https://a-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_axyz"),
            Some("https://ax-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_a1bravo"),
            Some("https://a1-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_east_lab"),
            Some("https://east-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_a"),
            Some("https://exact.corp/connect".to_string())
        );
        assert_eq!(resolve_hl7_connector_alias(&registry, "other"), None);

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn parse_hl7_connector_plugins_rejects_adapter_version_outside_compatibility_range() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector plugin compatibility");
        let plugin_path = temp_file_path("connector_plugin_compat", "wasm");
        fs::write(&plugin_path, "mock plugin").expect("write plugin fixture");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS").ok(),
            ),
        ];
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            plugin_path.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
            "2.0.0",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
            "2.1.0",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
            "3.0.0",
        );

        let err = parse_hl7_connector_plugins()
            .expect_err("version outside compatible range should fail");
        assert!(err
            .to_string()
            .contains("outside compatible range [2.1.0, 3.0.0]"));

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = fs::remove_file(plugin_path);
    }

    #[test]
    fn parse_hl7_connector_plugins_validates_compatibility_version_matrix() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector plugin matrix");
        let plugin_path = temp_file_path("connector_plugin_matrix", "wasm");
        fs::write(&plugin_path, "mock plugin").expect("write plugin fixture");

        let keys = [
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
        ];
        let mut prev = BTreeMap::new();
        for key in keys {
            let _ = prev.insert(key, std::env::var(key).ok());
        }
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            plugin_path.to_string_lossy().to_string(),
        );

        let cases = [
            ("2.1.0", "2.1.0", "3.0.0", true),
            ("2.5.4", "2.1.0", "3.0.0", true),
            ("3.0.0", "2.1.0", "3.0.0", true),
            ("2.0.9", "2.1.0", "3.0.0", false),
            ("3.0.1", "2.1.0", "3.0.0", false),
            ("2.5.0", "3.0.0", "2.1.0", false),
        ];
        for (version, min, max, should_pass) in cases {
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
                version,
            );
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
                min,
            );
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
                max,
            );

            let parsed = parse_hl7_connector_plugins();
            if should_pass {
                let plugins = parsed.expect("matrix case should pass compatibility validation");
                let plugin = plugins
                    .get("enterprisehis")
                    .expect("enterprisehis plugin should be present");
                assert_eq!(plugin.adapter_version, version);
                assert_eq!(plugin.compatible_min, min);
                assert_eq!(plugin.compatible_max, max);
            } else {
                assert!(
                    parsed.is_err(),
                    "matrix case should fail compatibility validation"
                );
            }
        }

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = fs::remove_file(plugin_path);
    }

    #[test]
    fn parse_hl7_connector_registry_skips_feature_and_rollout_env_vars() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector registry filtering");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS").ok(),
            ),
        ];

        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
            "https://connectors.enterprise-his.example/interop",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS",
            "false",
        );
        std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", "37");

        let registry = parse_hl7_connector_registry();
        let feature_flags = parse_hl7_connector_feature_flags().expect("parse feature flags");
        let rollout_percents =
            parse_hl7_connector_rollout_percents().expect("parse rollout percents");
        assert_eq!(
            registry.get("enterprise_his"),
            Some(&"https://connectors.enterprise-his.example/interop".to_string())
        );
        assert_eq!(
            feature_flags
                .get("enterprise_his")
                .expect("enterprise feature flag")
                .as_bool(),
            false
        );
        assert_eq!(rollout_percents.get("enterprise_his"), Some(&37));

        for (name, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(name, raw);
            } else {
                std::env::remove_var(name);
            }
        }
    }

    #[test]
    fn parse_hl7_connector_rollout_percent_rejects_values_above_100() {
        let _guard = ENV_LOCK.lock().expect("env lock for rollout bounds");
        let previous = std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS").ok();
        std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", "101");

        let err =
            parse_hl7_connector_rollout_percents().expect_err("rollout above 100 should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        match previous {
            Some(raw) => {
                std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", raw)
            }
            None => std::env::remove_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS"),
        }
    }

    #[test]
    fn parse_hl7_callback_policy_allows_operator_overrides() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for callback policy overrides");
        let keys = [
            HL7_CALLBACK_MAX_ATTEMPTS_ENV,
            HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV,
            HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV,
            HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV,
        ];
        let mut prev = BTreeMap::new();
        for key in keys {
            let _ = prev.insert(key, std::env::var(key).ok());
        }

        std::env::set_var(HL7_CALLBACK_MAX_ATTEMPTS_ENV, "5");
        std::env::set_var(HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV, "3");
        std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, "20000");
        std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, "240000");

        let policy =
            parse_hl7_callback_policy_from_env().expect("callback policy override should parse");
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.circuit_failure_threshold, 3);
        assert_eq!(policy.circuit_base_backoff_ms, 20000);
        assert_eq!(policy.circuit_max_backoff_ms, 240000);

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn parse_hl7_callback_policy_rejects_invalid_backoff_window() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for callback policy backoff");
        let prev_base = std::env::var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV).ok();
        let prev_max = std::env::var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV).ok();

        std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, "500000");
        std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, "100000");

        let err = parse_hl7_callback_policy_from_env()
            .expect_err("base backoff greater than max should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV));

        if let Some(raw) = prev_base {
            std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, raw);
        } else {
            std::env::remove_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV);
        }
        if let Some(raw) = prev_max {
            std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, raw);
        } else {
            std::env::remove_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV);
        }
    }

    #[test]
    fn hl7_failure_queue_round_trip_preserves_record_order() {
        let dlq_path = temp_file_path("hl7_dlq_order", "snapshot");
        let mut queue = VecDeque::new();
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000010".to_string(),
            source: "his".to_string(),
            message_type: "ADT".to_string(),
            reason: "parse_error".to_string(),
            payload_excerpt: "msh".to_string(),
            created_at_ms: 10,
            scope: "ingest".to_string(),
            subscription_id: String::new(),
            event_id: "HL7-000100".to_string(),
            correlation_id: "corr-10".to_string(),
            sequence: 100,
            attempt: 1,
            max_attempts: 3,
        });
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000009".to_string(),
            source: "his".to_string(),
            message_type: "ORM".to_string(),
            reason: "decode_error".to_string(),
            payload_excerpt: "pid".to_string(),
            created_at_ms: 9,
            scope: "callback".to_string(),
            subscription_id: "sub-1".to_string(),
            event_id: "HL7-000099".to_string(),
            correlation_id: "corr-9".to_string(),
            sequence: 99,
            attempt: 3,
            max_attempts: 3,
        });
        persist_hl7_failure_queue(&dlq_path.to_string_lossy(), &queue);

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        assert_eq!(
            loaded.front().map(|record| record.id.as_str()),
            Some("hl7-fail-000010")
        );
        assert_eq!(
            loaded.back().map(|record| record.id.as_str()),
            Some("hl7-fail-000009")
        );

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn hl7_failure_queue_migrates_legacy_v1_rows_to_current_schema_defaults() {
        let dlq_path = temp_file_path("hl7_dlq_legacy_v1", "snapshot");
        let legacy =
            "hl7-fail-000001\this\tADT\tparse_error\tmsh\t101\tingest\t\tHL7-000001\tcorr-1\t7\n";
        fs::write(&dlq_path, legacy).expect("write legacy v1 queue");

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        let record = loaded.front().expect("one legacy row should load");
        assert_eq!(record.id, "hl7_fail_000001");
        assert_eq!(record.sequence, 7);
        assert_eq!(record.attempt, 1);
        assert_eq!(record.max_attempts, MAX_WORKFLOW_CALLBACK_ATTEMPTS);

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn hl7_failure_queue_rollback_projection_preserves_core_fields() {
        let dlq_path = temp_file_path("hl7_dlq_rollback_projection", "snapshot");
        let mut queue = VecDeque::new();
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000021".to_string(),
            source: "his".to_string(),
            message_type: "ORU".to_string(),
            reason: "timeout".to_string(),
            payload_excerpt: "obr".to_string(),
            created_at_ms: 401,
            scope: "callback".to_string(),
            subscription_id: "sub-44".to_string(),
            event_id: "HL7-000021".to_string(),
            correlation_id: "corr-21".to_string(),
            sequence: 21,
            attempt: 4,
            max_attempts: 5,
        });
        let legacy_payload = render_hl7_failure_queue_legacy_v1(&queue);
        fs::write(&dlq_path, legacy_payload).expect("write rollback-projected queue");

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        let record = loaded.front().expect("one rollback row should load");
        assert_eq!(record.message_type, "oru");
        assert_eq!(record.scope, "callback");
        assert_eq!(record.sequence, 21);
        assert_eq!(record.attempt, 1);
        assert_eq!(record.max_attempts, MAX_WORKFLOW_CALLBACK_ATTEMPTS);

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn reconciliation_idempotency_snapshot_filters_non_reconciliation_entries() {
        let snapshot_path = temp_file_path("recon_idempotency_snapshot", "state");
        let mut cache = BTreeMap::new();
        let _ = cache.insert(
            "task:create:tenant-a:1".to_string(),
            CachedMppsRequest {
                signature: "task-signature".to_string(),
                response: "{\"task\":\"ok\"}".to_string(),
            },
        );
        let _ = cache.insert(
            "reconciliation-run:tenant-a:recon-1:key-1".to_string(),
            CachedMppsRequest {
                signature: "recon-signature".to_string(),
                response: "{\"recon\":\"ok\"}".to_string(),
            },
        );

        persist_reconciliation_run_idempotency_cache(&snapshot_path.to_string_lossy(), &cache);
        let loaded = load_reconciliation_run_idempotency_cache(&snapshot_path.to_string_lossy());

        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("reconciliation-run:tenant-a:recon-1:key-1"));
        assert!(!loaded.contains_key("task:create:tenant-a:1"));

        cleanup_with_rotations(&snapshot_path, 0);
    }

    fn build_sr_workflow_state(
        worklist_path: &Path,
        mpps_path: &Path,
        sr_path: &Path,
        sr_audit_path: &Path,
    ) -> Arc<Mutex<RuntimeState>> {
        let limits = Limits::default();
        set_tenant_rate_limit_override_cache(BTreeMap::new());
        let audit_path = sr_audit_path.to_string_lossy().to_string();
        let callback_idempotency_path = callback_idempotency_snapshot_path(&audit_path);
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState {
                subscriptions: BTreeMap::new(),
                failures: VecDeque::new(),
                ups: UpsCommandAdapter::new(),
                completion: CompletionWorkflowAdapter::new(),
                ian_events: BTreeMap::new(),
                storage_commitment_status: BTreeMap::new(),
                hl7_ups_correlation: BTreeMap::new(),
                subscription_seq: 0,
                failure_seq: 0,
                event_seq: 0,
                replay_cache: BTreeMap::new(),
                connector_registry: BTreeMap::new(),
                connector_feature_flags: BTreeMap::new(),
                connector_rollout_percent: BTreeMap::new(),
                connector_plugins: BTreeMap::new(),
                callback_delivery_idempotency: BTreeMap::new(),
                callback_idempotency_ttl_ms: DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS,
                callback_idempotency_path,
                callback_max_attempts: MAX_WORKFLOW_CALLBACK_ATTEMPTS,
                callback_circuit_breaker_failure_threshold:
                    CALLBACK_CIRCUIT_BREAKER_FAILURE_THRESHOLD,
                callback_circuit_breaker_base_backoff_ms: CALLBACK_CIRCUIT_BREAKER_BASE_BACKOFF_MS,
                callback_circuit_breaker_max_backoff_ms: CALLBACK_CIRCUIT_BREAKER_MAX_BACKOFF_MS,
                connector_callback_failure_streak: BTreeMap::new(),
                connector_circuit_open_until_ms: BTreeMap::new(),
                reconciliation_jobs: BTreeMap::new(),
                reconciliation_seq: 0,
            },
            audit_path,
            audit_rate_window_ms: DEFAULT_RATE_LIMIT_WINDOW_MS,
            query_rate_limit: DEFAULT_QUERY_RATE_LIMIT,
            mutation_rate_limit: DEFAULT_MUTATION_RATE_LIMIT,
            upload_cap_bytes: DEFAULT_UPLOAD_CAP_BYTES,
            audit_max_bytes: DEFAULT_AUDIT_MAX_BYTES,
            audit_max_rotated_files: DEFAULT_AUDIT_MAX_ROTATED_FILES,
            rate_windows: BTreeMap::new(),
            anomaly_alert_threshold: DEFAULT_ANOMALY_ALERT_THRESHOLD,
            audit_export_limit: DEFAULT_AUDIT_EXPORT_LIMIT,
            denylist_routes: DEFAULT_WORKFLOW_DENYLIST_PATHS
                .iter()
                .map(|path| path.to_string())
                .collect(),
            tenant_worklist: BTreeMap::new(),
            tenant_mpps: BTreeMap::new(),
            tenant_sr: BTreeMap::new(),
            tenant_tasks: BTreeMap::new(),
            metrics: BTreeMap::new(),
        };
        Arc::new(Mutex::new(state))
    }

    #[test]
    fn percent_decode_decodes_form_values() {
        // REQ-HTTP-301: query/body value decoding is deterministic.
        assert_eq!(percent_decode("A%20B+X"), Some("A B X".to_string()));
    }

    #[test]
    fn parse_pairs_rejects_non_ascii() {
        // REQ-HTTP-301: non-ASCII inputs fail closed.
        let err = parse_pairs("modality=%E2%98%83", &Limits::default()).expect_err("error");
        assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_pairs_enforces_workflow_pair_cap() {
        // REQ-HTTP-301: workflow query/body parsing is bounded by max_workflow_pair_count.
        let mut pairs = Vec::new();
        for index in 0..300 {
            pairs.push(format!("k{index}=v"));
        }
        let err = parse_pairs(&pairs.join("&"), &Limits::default()).expect_err("error");
        match err.kind() {
            ErrorKind::LimitExceeded {
                limit_name,
                observed,
                ..
            } => {
                assert_eq!(limit_name, "max_workflow_pair_count");
                assert_eq!(observed, 257);
            }
            _ => panic!("expected LimitExceeded"),
        }
    }

    #[test]
    fn sr_auth_context_accepts_operator_role_alias_header() {
        // REQ-AUTH-300: workflow auth role checks accept operator/admin writer-equivalent role aliases.
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: {
                let mut headers = BTreeMap::new();
                headers.insert("x-auth-principal".to_string(), "alice".to_string());
                headers.insert("x-auth-role".to_string(), "operator".to_string());
                headers
            },
            body: Vec::new(),
        };
        let auth = sr_auth_context(&request);
        assert!(auth.can_write);
        assert_eq!(auth.principal.as_deref(), Some("alice"));
    }

    #[test]
    fn workflow_audit_chain_detects_tampering() {
        // REQ-AUDIT-351: audit chain integrity is verifiable and tamper-evident.
        let path = temp_file_path("workflow_audit_chain", "log");
        let first = WorkflowAuditEvent {
            ts_ms: 100,
            tenant: "tenant-1".to_string(),
            principal_hash: "writer-a".to_string(),
            request_id_hash: "req-001".to_string(),
            previous_audit_hash: String::new(),
            audit_hash: String::new(),
            route: "/workflow/metrics".to_string(),
            method: "GET".to_string(),
            operation: "read".to_string(),
            outcome: "success".to_string(),
            scope: "tenant-1".to_string(),
            status: 200,
            anomaly: false,
        };
        let second = WorkflowAuditEvent {
            ts_ms: 101,
            tenant: "tenant-1".to_string(),
            principal_hash: "writer-b".to_string(),
            request_id_hash: "req-002".to_string(),
            previous_audit_hash: String::new(),
            audit_hash: String::new(),
            route: "/workflow/tasks".to_string(),
            method: "POST".to_string(),
            operation: "create".to_string(),
            outcome: "success".to_string(),
            scope: "tenant-1".to_string(),
            status: 201,
            anomaly: false,
        };
        append_workflow_audit_event(&path.to_string_lossy(), 1_048_576, 4, 128, &first);
        append_workflow_audit_event(&path.to_string_lossy(), 1_048_576, 4, 128, &second);

        let lines: Vec<String> = fs::read_to_string(&path)
            .expect("read audit")
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(lines.len(), 2);
        let parsed_first = parse_workflow_audit_line(&lines[0]).expect("parsed first");
        let parsed_second = parse_workflow_audit_line(&lines[1]).expect("parsed second");
        assert_eq!(parsed_first.previous_audit_hash, "");
        assert_eq!(
            parsed_second.previous_audit_hash, parsed_first.audit_hash,
            "chain should link previous hash"
        );
        assert!(
            verify_workflow_audit_chain(&lines).is_empty(),
            "chain should verify when untouched"
        );

        let mut tampered = lines.clone();
        tampered[1] = tampered[1].replace("tenant-1", "tenant-2");
        let failures = verify_workflow_audit_chain(&tampered);
        assert!(!failures.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn route_policy_enforcement_failure_log_includes_required_fields() {
        let actor = WorkflowActorContext {
            principal: Some("alice".to_string()),
            role: Some("writer".to_string()),
            tenant: "tenant-a".to_string(),
        };
        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "corr-123".to_string())]),
            body: Vec::new(),
        };
        let contract =
            workflow_contract_for_request("POST", "/workflow/tasks").expect("route contract");
        let log_line = render_route_policy_enforcement_failure_log(
            &actor,
            &request,
            &contract,
            "mutation_rate_limit",
            "rate limit exceeded",
        );

        assert!(log_line.contains("\"event\":\"workflow_route_policy_enforcement_failure\""));
        assert!(log_line.contains("\"tenant\":\"tenant-a\""));
        assert!(log_line.contains("\"route\":\"/workflow/tasks\""));
        assert!(log_line.contains("\"policy\":\"mutation_rate_limit\""));
        assert!(log_line.contains("\"correlation_id\":\"corr-123\""));
    }

    #[test]
    fn workflow_audit_payload_order_is_deterministic() {
        let lines = vec![
            "{\"ts_ms\":200,\"tenant\":\"tenant-b\"}".to_string(),
            "{\"ts_ms\":100,\"tenant\":\"tenant-a\"}".to_string(),
        ];
        let first = render_workflow_audit_payload(&lines);
        let second = render_workflow_audit_payload(&lines);

        assert_eq!(
            first,
            "[{\"ts_ms\":200,\"tenant\":\"tenant-b\"},{\"ts_ms\":100,\"tenant\":\"tenant-a\"}]"
        );
        assert_eq!(first, second);
    }

    #[test]
    fn workflow_route_error_invariants_are_unique_and_scoped() {
        let mut seen = BTreeMap::new();
        for invariant in WORKFLOW_ROUTE_ERROR_INVARIANTS {
            assert!(!invariant.route_group.is_empty());
            assert!(
                seen.insert(invariant.error_code.to_string(), true)
                    .is_none(),
                "duplicate workflow error-code invariant: {}",
                invariant.error_code
            );
        }
    }

    #[test]
    fn workflow_route_error_invariants_map_task_codes_to_expected_status() {
        let not_found = Error::from_kind(
            ErrorKind::NotFound {
                detail: "task not found".to_string(),
            },
            "task not found",
        );
        let invalid_transition = Error::from_kind(
            ErrorKind::IntegrityError {
                detail: "invalid transition".to_string(),
            },
            "invalid transition",
        );
        assert_eq!(status_for_error(&not_found), (404, "Not Found"));
        assert_eq!(status_for_error(&invalid_transition), (409, "Conflict"));
    }

    #[test]
    fn request_id_uses_x_request_id_header_when_present() {
        let mut headers = BTreeMap::new();
        headers.insert("x-request-id".to_string(), "trace-001".to_string());
        assert_eq!(request_id_from_headers(&headers), "trace-001");
    }

    #[test]
    fn request_id_falls_back_when_header_missing_or_empty() {
        assert!(request_id_from_headers(&BTreeMap::new()).starts_with("request-"));
        let mut headers = BTreeMap::new();
        headers.insert("x-request-id".to_string(), "   ".to_string());
        assert!(request_id_from_headers(&headers).starts_with("request-"));
    }

    #[test]
    fn auth_mode_defaults_to_deny_all() {
        // REQ-AUTH-300: runtime defaults fail closed.
        let mode = AuthMode::DenyAll;
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/healthz".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "tls"));
    }

    #[test]
    fn insecure_transport_rejected_even_when_auth_allows() {
        // REQ-HTTP-303 + REQ-AUTH-300: insecure transport must fail closed by default policy.
        let mode = AuthMode::AllowAll;
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/healthz".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "insecure"));
    }

    #[test]
    fn token_mode_requires_matching_header_on_tls() {
        // REQ-AUTH-300: token mode allows only exact token match.
        let mode = AuthMode::Token("secret-token".to_string());
        let mut headers = BTreeMap::new();
        headers.insert("x-diccy-token".to_string(), "wrong-token".to_string());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "tls"));

        headers.insert("x-diccy-token".to_string(), "secret-token".to_string());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers,
            body: Vec::new(),
        };
        assert!(authorize(&request, &mode, "tls"));
    }

    #[test]
    fn auth_mode_label_never_exposes_secret_values() {
        // REQ-AUTH-300: startup policy logging must not leak token material.
        let mode = AuthMode::Token("super-secret".to_string());
        assert_eq!(auth_mode_label(&mode), "token");
    }

    #[test]
    fn preflight_rejects_directory_path() {
        let path = temp_file_path("workflow_dir", "snapshot");
        fs::create_dir_all(&path).expect("mkdir");
        let err = prepare_persistence_file(&path.to_string_lossy(), 1024, 2, "worklist snapshot")
            .expect_err("expected error");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn preflight_rotates_oversized_snapshot_and_bounds_backup_count() {
        let path = temp_file_path("workflow_rotate", "snapshot");
        fs::write(&path, vec![7u8; 64]).expect("write snapshot");
        fs::write(path_with_suffix(&path, 1), vec![8u8; 8]).expect("write snapshot.1");
        prepare_persistence_file(&path.to_string_lossy(), 16, 2, "worklist snapshot")
            .expect("preflight");
        assert!(path.exists());
        assert!(path_with_suffix(&path, 1).exists());
        assert!(path_with_suffix(&path, 2).exists());
        assert!(!path_with_suffix(&path, 3).exists());
        cleanup_with_rotations(&path, 3);
    }

    #[test]
    fn workflow_audit_rotation_enforces_retention_cap() {
        let path = temp_file_path("workflow_audit_rotate", "log");
        fs::write(&path, vec![1u8; 64]).expect("write audit log");
        fs::write(path_with_suffix(&path, 1), vec![2u8; 8]).expect("write audit.1");
        fs::write(path_with_suffix(&path, 2), vec![3u8; 8]).expect("write audit.2");
        fs::write(path_with_suffix(&path, 3), vec![4u8; 8]).expect("write audit.3");

        rotate_audit_if_needed(&path.to_string_lossy(), 16, 2).expect("rotate audit");

        assert!(path_with_suffix(&path, 1).exists());
        assert!(path_with_suffix(&path, 2).exists());
        assert!(!path_with_suffix(&path, 3).exists());
        cleanup_with_rotations(&path, 3);
    }

    #[test]
    fn workflow_runtime_persistence_recovers_after_restart() {
        // REQ-WL-303 + REQ-MPPS-354: runtime-level restart must recover persisted workflow state.
        let worklist_path = temp_file_path("workflow_restart_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_restart_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_restart_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_restart_sr", "audit");
        prepare_persistence_file(
            &worklist_path.to_string_lossy(),
            1_048_576,
            2,
            "worklist snapshot",
        )
        .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps snapshot")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr snapshot")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");
        let limits = Limits::default();

        {
            let state = RuntimeState {
                worklist: WorklistStore::open(limits.clone(), &worklist_path)
                    .expect("open worklist"),
                mpps: MppsService::with_persistence(
                    MppsServiceConfig {
                        limits: limits.clone(),
                        audit: None,
                    },
                    &mpps_path,
                )
                .expect("open mpps"),
                sr: SrWorkflowStore::open(
                    limits.clone(),
                    &sr_path.to_string_lossy(),
                    &sr_audit_path.to_string_lossy(),
                )
                .expect("open sr"),
                mpps_idempotency: BTreeMap::new(),
                tasks: BTreeMap::new(),
                task_id_sequence: 1,
                task_idempotency: BTreeMap::new(),
                hl7: Hl7RuntimeState::default(),
            };
            let shared = Arc::new(Mutex::new(state));

            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: b"scheduled_step_id=STEP1&modality=CT&start_date=20260211&start_time=101010&patient_id=P001".to_vec(),
            };
            let worklist_response =
                route_request(&worklist_post, &shared, &limits).expect("worklist upsert");
            match worklist_response {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }

            let mpps_post = HttpRequest {
                method: "POST".to_string(),
                path: "/mpps/updates".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=111111".to_vec(),
            };
            let mpps_response = route_request(&mpps_post, &shared, &limits).expect("mpps upsert");
            match mpps_response {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let restarted = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("reopen worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("reopen mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("reopen sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(restarted));

        let worklist_get = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_get, &shared, &limits).expect("worklist get");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("STEP1"));
            }
        }

        let mpps_get = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let mpps_response = route_request(&mpps_get, &shared, &limits).expect("mpps get");
        match mpps_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_http_flow_create_update_retrieve_is_deterministic() {
        // REQ-SR-300, REQ-HI-165, REQ-HI-170
        let worklist_path = temp_file_path("sr_flow_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_flow_mpps", "snapshot");
        let sr_path = temp_file_path("sr_flow_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_flow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert("x-idempotency-key".to_string(), "create-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=num&concept_code_value=G-D7FE&concept_scheme=SRT&concept_meaning=Length&num_value=12.5&units_code_value=mm&units_scheme=UCUM&units_meaning=millimeter&referenced_sop_instance_uid=9.8.7&known_refs=9.8.7".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut update_headers = BTreeMap::new();
        update_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        update_headers.insert("x-sr-role".to_string(), "writer".to_string());
        update_headers.insert("x-idempotency-key".to_string(), "update-1".to_string());
        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: update_headers.clone(),
            body: b"expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=stable+finding&known_refs=9.8.7&referenced_sop_instance_uid=9.8.7".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("update");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
                assert!(body.contains("\"version\":2"));
            }
        }

        let replay_response = route_request(&update, &shared, &limits).expect("update replay");
        match replay_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"duplicate\""));
                assert!(body.contains("\"idempotency_replay\":true"));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get, &shared, &limits).expect("get");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"version\":2"));
                assert!(body.contains("stable finding"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_route_contract_is_valid_for_create_update_get_and_list_schemas() {
        // REQ-SR-300, REQ-HI-168, REQ-SR-201
        let worklist_path = temp_file_path("sr_contract_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_contract_mpps", "snapshot");
        let sr_path = temp_file_path("sr_contract_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_contract_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-contract-create".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=num&concept_code_value=G-D7FE&concept_scheme=SRT&concept_meaning=Length&num_value=12.5&units_code_value=mm&units_scheme=UCUM&units_meaning=millimeter&known_refs=9.8.7".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create request");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"sop_instance_uid\":\"1.2.3.4.5\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut update_headers = BTreeMap::new();
        update_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        update_headers.insert("x-sr-role".to_string(), "writer".to_string());
        update_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-contract-update".to_string(),
        );
        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: update_headers.clone(),
            body: b"expected_version=1&item_kind=code&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&value_code_value=F-001&value_scheme=DCM&value_meaning=Observation&known_refs=9.8.7".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("update request");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
                assert!(body.contains("\"version\":2"));
            }
        }

        let invalid_updates_method = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = route_request(&invalid_updates_method, &shared, &limits)
            .expect_err("updates path supports POST only");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get, &shared, &limits).expect("get request");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"provenance\""));
                assert!(body.contains("\"study_instance_uid\":\"1.2.3\""));
                assert!(body.contains("\"series_instance_uid\":\"1.2.3.4\""));
                assert!(body.contains("\"sop_instance_uid\":\"1.2.3.4.5\""));
                assert!(body.contains("\"version\":2"));
                assert!(body.contains("\"kind\":\"num\""));
                assert!(body.contains("\"kind\":\"code\""));
            }
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list request");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4.5"));
                assert!(body.contains("\"item_count\":2"));
                assert!(body.contains("\"observer\":\"alice\""));
            }
        }

        let invalid_collection_method = HttpRequest {
            method: "PATCH".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = route_request(&invalid_collection_method, &shared, &limits)
            .expect_err("collection path supports GET/HEAD/POST only");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    fn workflow_route_template_sample_path(template: &str) -> String {
        let mut path = template.to_string();
        let replacements = [
            ("{sop_instance_uid}", "1.2.3.4.5"),
            ("{task_id}", "TASK-0001"),
            ("{job_id}", "JOB-0001"),
            ("{StudyUID}", "1.2.3"),
            ("{SeriesUID}", "1.2.3.4"),
            ("{InstanceUID}", "1.2.3.4.5.6"),
        ];
        for (template_key, value) in replacements {
            path = path.replace(template_key, value);
        }
        assert!(
            !path.contains('{') && !path.contains('}'),
            "failed to expand route template: {template} => {path}"
        );
        path
    }

    fn workflow_route_template_request_headers(
        contract: &WorkflowRouteContract,
        unique_id: usize,
    ) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        if contract.requires_writer_role {
            headers.insert(
                "x-sr-principal".to_string(),
                "route-matrix-probe".to_string(),
            );
            headers.insert("x-sr-role".to_string(), "writer".to_string());
        }
        if contract.requires_idempotency_key {
            headers.insert(
                "x-idempotency-key".to_string(),
                format!("route-matrix-probe-{unique_id}"),
            );
        }
        headers
    }

    fn workflow_route_template_invalid_path(
        template: &str,
        placeholder: &str,
        value: &str,
    ) -> String {
        template.replace(placeholder, value)
    }

    #[test]
    fn workflow_route_contract_rows_are_dispatchable_from_request_router() {
        // REQ-HI-275, REQ-HI-326
        let worklist_path = temp_file_path("workflow_route_matrix_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_route_matrix_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_route_matrix_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_route_matrix_sr_audit", "audit");

        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let contracts = workflow_route_contract();
        for (index, contract) in contracts.iter().enumerate() {
            let sample_path = workflow_route_template_sample_path(contract.path_template);
            for method in contract.method.split('|') {
                let headers = workflow_route_template_request_headers(contract, index);
                let request = HttpRequest {
                    method: method.to_string(),
                    path: sample_path.clone(),
                    query: BTreeMap::new(),
                    headers,
                    body: Vec::new(),
                };
                let outcome = route_request(&request, &state, &limits);
                if let Err(err) = outcome {
                    if let dicom_core::ErrorKind::DecodeError { detail, .. } = &err.kind() {
                        assert_ne!(
                            detail.as_str(),
                            "unsupported route",
                            "route contract row is not wired to routing: {} {}",
                            method,
                            contract.path_template
                        );
                    }
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_route_contract_path_parameters_are_validated_by_contract() {
        // REQ-HI-327
        let worklist_path = temp_file_path("workflow_route_param_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_route_param_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_route_param_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_route_param_sr_audit", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let invalid_values = [
            ("{sop_instance_uid}", "uid with spaces"),
            ("{task_id}", "task with spaces"),
            ("{job_id}", "job with spaces"),
        ];

        for contract in workflow_route_contract() {
            for (placeholder, value) in invalid_values {
                if !contract.path_template.contains(placeholder) {
                    continue;
                }
                let invalid_path = workflow_route_template_invalid_path(
                    contract.path_template,
                    placeholder,
                    value,
                );
                for method in contract.method.split('|') {
                    let headers = workflow_route_template_request_headers(contract, 100);
                    let request = HttpRequest {
                        method: method.to_string(),
                        path: invalid_path.clone(),
                        query: BTreeMap::new(),
                        headers,
                        body: Vec::new(),
                    };
                    let err = route_request(&request, &state, &limits)
                        .expect_err("invalid path parameter should fail");
                    assert_eq!(err.code(), "DVF.HTTP.DECODE");
                    assert!(
                        matches!(err.kind(), ErrorKind::DecodeError { .. }),
                        "route validation must fail as decode error for {}/{}",
                        method,
                        contract.path_template
                    );
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_identifier_aliases_and_normalization_are_supported() {
        let worklist_path = temp_file_path("id_alias_worklist", "snapshot");
        let mpps_path = temp_file_path("id_alias_mpps", "snapshot");
        let sr_path = temp_file_path("id_alias_sr", "snapshot");
        let sr_audit_path = temp_file_path("id_alias_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let alias_patient = "  P-ALIAS  ";
        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: format!(
                "scheduled_step_id=STEP-A&modality=CT&start_date=20260211&start_time=101010&patient_id={alias_patient}"
            )
            .into_bytes(),
        };
        let worklist_response =
            route_request(&worklist_post, &shared, &limits).expect("worklist upsert");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("PatientID=%20P-ALIAS%20", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_list =
            route_request(&worklist_query, &shared, &limits).expect("query by patient");
        match worklist_list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("P-ALIAS"));
            }
        }

        let mut sr_headers = BTreeMap::new();
        sr_headers.insert("x-sr-role".to_string(), "writer".to_string());
        sr_headers.insert("x-idempotency-key".to_string(), "id-alias-1".to_string());
        let sr_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers,
            body: b"study_uid=1.2.3&series_uid=1.2.3.4&sop_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=normalized".to_vec(),
        };
        let sr_create_response = route_request(&sr_create, &shared, &limits).expect("sr create");
        match sr_create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let sr_list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map("StudyInstanceUID=%201.2.3%20", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let sr_list_response = route_request(&sr_list, &shared, &limits).expect("sr list");
        match sr_list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4.5"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_http_lifecycle_endpoints_enforce_state_transitions_and_immutable_history() {
        // REQ-SR-300, REQ-SR-315, REQ-HI-170
        let worklist_path = temp_file_path("sr_lifecycle_workflow_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_lifecycle_workflow_mpps", "snapshot");
        let sr_path = temp_file_path("sr_lifecycle_workflow_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_lifecycle_workflow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-lifecycle-create-1".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=baseline&known_refs=9.8.7".to_vec(),
        };
        match route_request(&create, &shared, &limits).expect("sr create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        review_headers.insert("x-sr-role".to_string(), "writer".to_string());
        review_headers.insert("x-idempotency-key".to_string(), "sr-review-1".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/review".to_string(),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("sr review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let mut finalize_headers = BTreeMap::new();
        finalize_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        finalize_headers.insert("x-sr-role".to_string(), "writer".to_string());
        finalize_headers.insert("x-idempotency-key".to_string(), "sr-finalize-1".to_string());
        let finalize = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/finalize".to_string(),
            query: BTreeMap::new(),
            headers: finalize_headers,
            body: Vec::new(),
        };
        match route_request(&finalize, &shared, &limits).expect("sr finalize") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"FINALIZED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let mut commit_headers = BTreeMap::new();
        commit_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        commit_headers.insert("x-sr-role".to_string(), "writer".to_string());
        commit_headers.insert("x-idempotency-key".to_string(), "sr-commit-1".to_string());
        let commit = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/commit".to_string(),
            query: BTreeMap::new(),
            headers: commit_headers,
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("sr commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("sr get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"lifecycle_status\":\"COMMITTED\""));
            }
        }

        let history = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5/history".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&history, &shared, &limits).expect("sr history") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"action\":\"create\""));
                assert!(body.contains("\"action\":\"review\""));
                assert!(body.contains("\"action\":\"finalize\""));
                assert!(body.contains("\"action\":\"commit\""));
            }
        }

        let mut create_second_headers = BTreeMap::new();
        create_second_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_second_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_second_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-lifecycle-create-2".to_string(),
        );
        let second_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_second_headers,
            body: b"study_instance_uid=2.3.4&series_instance_uid=2.3.4.6&sop_instance_uid=2.3.4.6.7&observer=alice&authored_epoch_ms=50&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=draft".to_vec(),
        };
        match route_request(&second_create, &shared, &limits).expect("sr create second") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let mut invalid_headers = BTreeMap::new();
        invalid_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        invalid_headers.insert("x-sr-role".to_string(), "writer".to_string());
        invalid_headers.insert("x-idempotency-key".to_string(), "sr-invalid-1".to_string());
        let invalid_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/2.3.4.6.7/commit".to_string(),
            query: BTreeMap::new(),
            headers: invalid_headers,
            body: Vec::new(),
        };
        let err = match route_request(&invalid_transition, &shared, &limits) {
            Ok(_) => panic!("invalid lifecycle transition must fail"),
            Err(err) => err,
        };
        assert_eq!(err.code(), "DVF.INTEGRITY.ERROR");
        assert_eq!(status_for_error(&err).0, 409);

        let restarted =
            build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        match route_request(&history, &restarted, &limits).expect("history after restart") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"action\":\"commit\""));
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_route_shims_accept_legacy_paths_with_normalized_ids() {
        let worklist_path = temp_file_path("id_compat_worklist", "snapshot");
        let mpps_path = temp_file_path("id_compat_mpps", "snapshot");
        let sr_path = temp_file_path("id_compat_sr", "snapshot");
        let sr_audit_path = temp_file_path("id_compat_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-COMPAT&requested_procedure_id=RP-COMPAT&worker=compat"
                .to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("task create request") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body.find(prefix).expect("task id field") + prefix.len();
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                let raw_id = &remainder[..end];
                assert!(!raw_id.trim().is_empty());
                raw_id.to_string()
            }
            _ => panic!("unexpected create response"),
        };
        assert!(!task_id.is_empty());
        let normalized_task_id = task_id.trim();

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("legacy task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&normalized_task_id));
            }
            _ => panic!("unexpected list response"),
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("legacy task get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&format!("\"task_id\":\"{normalized_task_id}\"")));
            }
            _ => panic!("unexpected get response"),
        }

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "compat-worker".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/tasks/{task_id}/start/"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("legacy task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
            _ => panic!("unexpected start response"),
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/tasks/{task_id}/complete/"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("legacy task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
            _ => panic!("unexpected complete response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_supports_filter_sort_and_pagination() {
        // REQ-HI-201, REQ-WF-300
        let worklist_path = temp_file_path("workflow_list_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_list_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut posts = vec![
            (
                "scheduled_step_id=STEP1&modality=CT&start_date=20260211&start_time=101010&patient_id=P1",
                "accession_number=ACC1",
            ),
            (
                "scheduled_step_id=STEP2&modality=MR&start_date=20260210&start_time=095959&patient_id=P2",
                "accession_number=ACC2",
            ),
            (
                "scheduled_step_id=STEP3&modality=CT&start_date=20260212&start_time=111111&patient_id=P3",
                "accession_number=ACC3",
            ),
        ];
        for post in posts.drain(..) {
            let body = format!("{}&{}", post.0, post.1);
            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: body.into_bytes(),
            };
            match route_request(&worklist_post, &shared, &limits).expect("insert worklist") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=1&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP3\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP1\""));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=2&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP1\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP3\""));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=2&page_size=2",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_deterministic_defaults_apply_sort_and_pagination() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_default_sort_pagination_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_default_sort_pagination_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_default_sort_pagination_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_default_sort_pagination_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        for i in (0..51).rev() {
            let scheduled_step_id = format!("PS-{i:03}");
            let start_time = format!("{:06}", 100_000 + i);
            let body = format!(
                "scheduled_step_id={scheduled_step_id}&modality=CT&start_date=20260210&start_time={start_time}&patient_id=PAT-{i}&accession_number=ACC-{i}"
            );
            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: body.into_bytes(),
            };
            match route_request(&worklist_post, &shared, &limits).expect("insert worklist") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body.matches("\"scheduled_step_id\":\"").count(), 50);
                let first = body
                    .find("\"scheduled_step_id\":\"PS-000\"")
                    .expect("first row");
                let second = body
                    .find("\"scheduled_step_id\":\"PS-001\"")
                    .expect("second row");
                let third = body
                    .find("\"scheduled_step_id\":\"PS-002\"")
                    .expect("third row");
                assert!(first < second);
                assert!(second < third);
                assert!(body.contains("\"scheduled_step_id\":\"PS-049\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS-050\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_language_expression_filters_and_sorts() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let ct_item = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-CT&modality=CT&start_date=20260211&start_time=101010&patient_id=PAT-1&accession_number=ACC-1".to_vec(),
        };
        let mr_item = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-MR&modality=MR&start_date=20260211&start_time=101020&patient_id=PAT-2&accession_number=ACC-2".to_vec(),
        };
        assert!(matches!(
            route_request(&ct_item, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));
        assert!(matches!(
            route_request(&mr_item, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "query=modality%3DCT%26PatientID%3DPAT-1%26sort%3Dscheduled_step_id%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("query worklist");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP-CT\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP-MR\""));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_supports_status_filter_sort_and_pagination() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_list_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_list_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let in_progress = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=101010".to_vec(),
        };
        let in_progress_next = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.5&status=IN+PROGRESS&performed_step_id=PS2&start_date=20260211&start_time=102010".to_vec(),
        };
        let _ = route_request(&in_progress, &shared, &limits).expect("mpps in progress");
        let _ = route_request(&in_progress_next, &shared, &limits).expect("mpps in progress");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map(
                "status=IN+PROGRESS&sort=sop_instance_uid&page_size=1&page=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("mpps list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
                assert!(!body.contains("1.2.3.5"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_language_expression_filters_and_sorts() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let pending = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-IN_PROGRESS&start_date=20260211&start_time=101010".to_vec(),
        };
        let completed = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.5&status=COMPLETED&performed_step_id=PS-COMPLETED&start_date=20260211&start_time=102010&end_date=20260211&end_time=102010".to_vec(),
        };
        let _ = route_request(&pending, &shared, &limits).expect("mpps in progress");
        let _ = route_request(&completed, &shared, &limits).expect("mpps completed");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map(
                "query=status%3DCOMPLETED%26performed_step_id%3DPS-COMPLETED%26sort%3Dsop_instance_uid%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("query mpps");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.5"));
                assert!(!body.contains("1.2.3.4"));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_language_rejects_malformed_nested_query_expression() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let in_progress = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map("query=status%3DCOMPLETED%26badpair", &limits)
                .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&in_progress, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_status_filter_aliases_and_reproducible_defaults() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_status_filter_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_status_filter_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_status_filter_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_status_filter_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS1&modality=CT&start_date=20260211&start_time=101010&patient_id=P1&accession_number=ACC1".to_vec(),
        };
        assert!(matches!(
            route_request(&worklist_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS2&modality=MR&start_date=20260211&start_time=101020&patient_id=P2&accession_number=ACC2".to_vec(),
        };
        assert!(matches!(
            route_request(&worklist_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let mpps_post = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=COMPLETED&performed_step_id=PS1&start_date=20260211&start_time=101010&end_date=20260211&end_time=101100".to_vec(),
        };
        assert!(matches!(
            route_request(&mpps_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let completed_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&status=COMPLETED&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let completed_response =
            route_request(&completed_query, &shared, &limits).expect("query completed");
        match completed_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS2\""));
            }
        }

        let scheduled_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=MR&status=SCHEDULED&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let scheduled_response =
            route_request(&scheduled_query, &shared, &limits).expect("query scheduled");
        match scheduled_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS2\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let scheduled_alias_response = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=MR&status=pending&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let scheduled_alias_response = route_request(&scheduled_alias_response, &shared, &limits)
            .expect("query scheduled alias");
        match scheduled_alias_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS2\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_rejects_invalid_sort() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_sort_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_sort_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_sort_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_sort_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let invalid_sort = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("sort=urgency&order=asc", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let invalid_sort_err = match route_request(&invalid_sort, &shared, &limits) {
            Ok(_) => panic!("invalid sort must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&invalid_sort_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_rejects_invalid_status_filter() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_status_filter_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_status_filter_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_status_filter_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_status_filter_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let invalid_status = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("status=unknown", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let invalid_status_err = match route_request(&invalid_status, &shared, &limits) {
            Ok(_) => panic!("invalid status filter must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&invalid_status_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_enforces_pagination_limits() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_pagination_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_pagination_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_pagination_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_pagination_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let page_zero = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page=0", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_zero_err = match route_request(&page_zero, &shared, &limits) {
            Ok(_) => panic!("page=0 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_zero_err).0, 400);

        let page_size_zero = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page_size=0", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_size_zero_err = match route_request(&page_size_zero, &shared, &limits) {
            Ok(_) => panic!("page_size=0 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_size_zero_err).0, 400);

        let page_size_over_limit = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page_size=501", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_size_over_limit_err = match route_request(&page_size_over_limit, &shared, &limits)
        {
            Ok(_) => panic!("page_size>500 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_size_over_limit_err).0, 413);
        match page_size_over_limit_err.kind() {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "page_size");
            }
            _ => panic!("expected limit_exceeded for page_size=501"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_status_endpoint_enforces_transition_rules_and_idempotency() {
        // REQ-WF-305 + REQ-WF-306
        let worklist_path = temp_file_path("mpps_status_endpoints_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_status_endpoints_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_status_endpoints_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_status_endpoints_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=101010".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create mpps");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let get_status = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get_status, &shared, &limits).expect("read mpps status");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
                assert!(body.contains("\"performed_step_id\":\"PS1\""));
            }
        }

        let mut headers = BTreeMap::new();
        headers.insert(
            "x-idempotency-key".to_string(),
            "mpps-status-update-1".to_string(),
        );
        let complete = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: b"status=COMPLETED&end_date=20260211&end_time=101212".to_vec(),
        };
        let complete_response = route_request(&complete, &shared, &limits).expect("complete mpps");
        match complete_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }

        let complete_replay = route_request(&complete, &shared, &limits).expect("replay complete");
        match complete_replay {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }

        let terminal_transition_setup = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=2.3.4.5&status=IN+PROGRESS&performed_step_id=PS2&start_date=20260211&start_time=111111".to_vec(),
        };
        let terminal_transition_setup_response =
            route_request(&terminal_transition_setup, &shared, &limits)
                .expect("create terminal transition mpps");
        match terminal_transition_setup_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }
        let terminal_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/2.3.4.5/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=COMPLETED&end_date=20260211&end_time=131313".to_vec(),
        };
        let terminal_transition_response =
            route_request(&terminal_transition, &shared, &limits).expect("terminal transition");
        match terminal_transition_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }
        let invalid_terminal_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/2.3.4.5/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=IN+PROGRESS".to_vec(),
        };
        let invalid_terminal_transition =
            match route_request(&invalid_terminal_transition, &shared, &limits) {
                Ok(_) => panic!("terminal status transition must fail"),
                Err(err) => err,
            };
        assert_eq!(invalid_terminal_transition.code, "DVF.INTEGRITY.ERROR");
        assert_eq!(status_for_error(&invalid_terminal_transition).0, 409);

        let invalid_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers,
            body: b"status=DISCONTINUED".to_vec(),
        };
        let invalid = match route_request(&invalid_transition, &shared, &limits) {
            Ok(_) => panic!("invalid transition must fail"),
            Err(err) => err,
        };
        assert_eq!(invalid.code, "DVF.INTEGRITY.ERROR");
        assert_eq!(status_for_error(&invalid).0, 409);

        let get_completed = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_completed_response =
            route_request(&get_completed, &shared, &limits).expect("read mpps completed");
        match get_completed_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_tenant_context_blocks_cross_tenant_leakage() {
        // REQ-WF-401, REQ-WF-402
        let worklist_path = temp_file_path("tenant_scope_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_scope_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_scope_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_scope_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut tenant_a_headers = BTreeMap::new();
        tenant_a_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        tenant_a_headers.insert("x-sr-role".to_string(), "writer".to_string());
        tenant_a_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());

        let mut tenant_b_headers = BTreeMap::new();
        tenant_b_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        tenant_b_headers.insert("x-sr-role".to_string(), "writer".to_string());
        tenant_b_headers.insert("x-workflow-tenant-id".to_string(), "tenant-b".to_string());

        let worklist_create = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: b"scheduled_step_id=STEP-TENANT-A&modality=CT&start_date=20260211&start_time=101010&patient_id=PA".to_vec(),
        };
        match route_request(&worklist_create, &shared, &limits).expect("create tenant-a worklist") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let worklist_cross_tenant_upsert = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: b"scheduled_step_id=STEP-TENANT-A&modality=MR&start_date=20260211&start_time=102020&patient_id=PB".to_vec(),
        };
        let denied = match route_request(&worklist_cross_tenant_upsert, &shared, &limits) {
            Ok(_) => panic!("cross-tenant worklist upsert must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied.code, "DVF.DICOM.DECODE_ERROR");

        let tenant_a_worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: Vec::new(),
        };
        match route_request(&tenant_a_worklist_query, &shared, &limits)
            .expect("tenant-a worklist query")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("STEP-TENANT-A"));
            }
        }

        let tenant_b_worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: Vec::new(),
        };
        match route_request(&tenant_b_worklist_query, &shared, &limits)
            .expect("tenant-b worklist query")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
        }

        let mpps_create = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-A&start_date=20260211&start_time=101010".to_vec(),
        };
        match route_request(&mpps_create, &shared, &limits).expect("create tenant-a mpps") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let mpps_cross_tenant_update = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: b"sop_instance_uid=1.2.3.4&status=COMPLETED&performed_step_id=PS-B&start_date=20260211&start_time=101020&end_date=20260211&end_time=101121".to_vec(),
        };
        let denied_mpps = match route_request(&mpps_cross_tenant_update, &shared, &limits) {
            Ok(_) => panic!("cross-tenant mpps update must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied_mpps.code, "DVF.DICOM.DECODE_ERROR");

        let mpps_cross_tenant_get = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: Vec::new(),
        };
        let denied_mpps_read = match route_request(&mpps_cross_tenant_get, &shared, &limits) {
            Ok(_) => panic!("cross-tenant mpps read must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied_mpps_read.code, "DVF.DICOM.DECODE_ERROR");

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_http_flow_creates_lists_gets_and_controls_lifecycle() {
        // REQ-WF-400: task lifecycle endpoints must support planned procedure execution flow control.
        let worklist_path = temp_file_path("task_flow_worklist", "snapshot");
        let mpps_path = temp_file_path("task_flow_mpps", "snapshot");
        let sr_path = temp_file_path("task_flow_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_flow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-idempotency-key".to_string(), "task-create-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-100&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("task create request") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map("status=SCHEDULED&sort=task_id", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id));
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/workflow/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("task get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&format!("\"task_id\":\"{task_id}\"")));
            }
        }

        let mut pause_reject_headers = BTreeMap::new();
        pause_reject_headers.insert("x-task-worker".to_string(), "nurse".to_string());
        let pause = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/pause"),
            query: BTreeMap::new(),
            headers: pause_reject_headers,
            body: Vec::new(),
        };
        let pause_err = match route_request(&pause, &shared, &limits) {
            Ok(_) => panic!("pause before start must fail"),
            Err(err) => err,
        };
        assert_eq!(pause_err.code(), "DVF.INTEGRITY.ERROR");

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/complete"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        let commit_without_review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let commit_without_review_err =
            match route_request(&commit_without_review, &shared, &limits) {
                Ok(_) => panic!("commit must follow review"),
                Err(err) => err,
            };
        assert_eq!(
            commit_without_review_err.code(),
            "DVF.INTEGRITY.ERROR"
        );

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-task-worker".to_string(), "quality-reviewer".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/review"),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("task review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
            }
        }

        let commit = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("task commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_http_flow_supports_ups_aliases() {
        // REQ-WF-400: /ups aliases must provide the same task lifecycle behavior as /workflow/tasks.
        let worklist_path = temp_file_path("ups_alias_task_worklist", "snapshot");
        let mpps_path = temp_file_path("ups_alias_task_mpps", "snapshot");
        let sr_path = temp_file_path("ups_alias_task_sr", "snapshot");
        let sr_audit_path = temp_file_path("ups_alias_task_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "ups-task-create-1".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/ups".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-101&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("ups task create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/ups".to_string(),
            query: parse_query_map("status=SCHEDULED&sort=task_id", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("ups task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id));
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("ups task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/complete"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("ups task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-task-worker".to_string(), "quality-reviewer".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/review"),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("ups task review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
            }
        }

        let commit = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("ups task commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_query_language_expression_filters_and_sorts() {
        // REQ-WF-400
        let worklist_path = temp_file_path("task_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("task_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("task_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-100&worker=planner".to_vec(),
        };
        let task_id1 = match route_request(&create, &shared, &limits).expect("task create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
            _ => panic!("unexpected task create response"),
        };
        let create_in_progress = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS2&requested_procedure_id=RP-101&worker=planner".to_vec(),
        };
        let task_id2 =
            match route_request(&create_in_progress, &shared, &limits).expect("task create") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("\"outcome\":\"inserted\""));
                    let prefix = "\"task_id\":\"";
                    let start = body
                        .find(prefix)
                        .expect("task id field")
                        .saturating_add(prefix.len());
                    let remainder = &body[start..];
                    let end = remainder.find('"').expect("task id close");
                    remainder[..end].to_string()
                }
                _ => panic!("unexpected task create response"),
            };
        assert_ne!(task_id1, task_id2);

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id2}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        let _ = route_request(&start, &shared, &limits).expect("task start");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map(
                "query=scheduled_step_id%3DPS2%26status%3DIN+PROGRESS%26sort%3Dtask_id%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("task list query language") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id2));
                assert!(!body.contains(&task_id1));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_query_language_rejects_malformed_nested_query_expression() {
        // REQ-WF-400
        let worklist_path = temp_file_path("task_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("task_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("task_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map("query=scheduled_step_id%3DPS2%26badpair", &limits)
                .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&list, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_supports_observer_filter_sort_and_pagination() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_list_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_list_mpps", "snapshot");
        let sr_path = temp_file_path("sr_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut alice_headers = BTreeMap::new();
        alice_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        alice_headers.insert("x-sr-role".to_string(), "writer".to_string());
        alice_headers.insert("x-idempotency-key".to_string(), "sr-list-1".to_string());
        let alice_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: alice_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1000&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=alice".to_vec(),
        };
        let _ = route_request(&alice_create, &shared, &limits).expect("create alice");

        let mut bob_headers = BTreeMap::new();
        bob_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        bob_headers.insert("x-sr-role".to_string(), "writer".to_string());
        bob_headers.insert("x-idempotency-key".to_string(), "sr-list-2".to_string());
        let bob_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: bob_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.6&observer=bob&authored_epoch_ms=1001&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob".to_vec(),
        };
        let _ = route_request(&bob_create, &shared, &limits).expect("create bob");

        let mut bob_update_headers = bob_headers;
        bob_update_headers.insert("x-idempotency-key".to_string(), "sr-list-3".to_string());
        let bob_update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.6/updates".to_string(),
            query: BTreeMap::new(),
            headers: bob_update_headers,
            body: b"expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob-v2".to_vec(),
        };
        let _ = route_request(&bob_update, &shared, &limits).expect("update bob");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map(
                "observer=bob&sort=version&order=desc&page=1&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("sr list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"observer\":\"bob\""));
                assert!(body.contains("\"version\":2"));
                assert!(!body.contains("\"observer\":\"alice\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_language_expression_filters_and_sorts() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("sr_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut alice_headers = BTreeMap::new();
        alice_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        alice_headers.insert("x-sr-role".to_string(), "writer".to_string());
        alice_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-language-alice".to_string(),
        );
        let alice_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: alice_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1000&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=alice".to_vec(),
        };
        let _ = route_request(&alice_create, &shared, &limits).expect("create alice");

        let mut bob_headers = BTreeMap::new();
        bob_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        bob_headers.insert("x-sr-role".to_string(), "writer".to_string());
        bob_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-language-bob".to_string(),
        );
        let bob_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: bob_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.6&observer=bob&authored_epoch_ms=1001&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob".to_vec(),
        };
        let _ = route_request(&bob_create, &shared, &limits).expect("create bob");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map(
                "query=observer%3Dbob%26sort%3Dversion%26order%3Ddesc&page=1&page_size=1",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("sr list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"observer\":\"bob\""));
                assert!(!body.contains("\"observer\":\"alice\""));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_language_rejects_malformed_nested_query_expression() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("sr_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map("query=observer%3Dbob%26badpair", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&list, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_write_fails_closed_without_write_role() {
        // REQ-HI-168, REQ-AUTH-300
        let worklist_path = temp_file_path("sr_auth_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_auth_mpps", "snapshot");
        let sr_path = temp_file_path("sr_auth_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_auth_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut headers = BTreeMap::new();
        headers.insert("x-sr-principal".to_string(), "alice".to_string());
        headers.insert("x-idempotency-key".to_string(), "auth-fail-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=unauthorized".to_vec(),
        };
        match route_request(&create, &shared, &limits) {
            Ok(_) => panic!("must fail"),
            Err(err) => assert_eq!(err.code(), "DVF.DICOM.DECODE_ERROR"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn diagnostics_redact_uid_path_and_host_tokens() {
        // REQ-HI-188, REQ-HI-195, REQ-HI-245
        let raw = "uid=1.2.840.10008 path=/var/state/workflow.snapshot peer=workflow.local:8082";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("1.2.840.10008"));
        assert!(!redacted.contains("/var/state/workflow.snapshot"));
        assert!(!redacted.contains("workflow.local:8082"));
        assert!(redacted.contains("[REDACTED_UID]"));
        assert!(redacted.contains("[REDACTED_PATH]"));
        assert!(redacted.contains("[REDACTED_HOST]"));
    }

    #[test]
    fn diagnostics_redact_phi_pii_keyed_and_email_tokens() {
        let raw = "patient_id=PX-7788 patient_name=Bob email=bob@example.org";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("PX-7788"));
        assert!(!redacted.contains("Bob"));
        assert!(!redacted.contains("bob@example.org"));
        assert!(redacted.contains("patient_id=[REDACTED_PII]"));
        assert!(redacted.contains("patient_name=[REDACTED_PII]"));
        assert!(redacted.contains("email=[REDACTED_PII]"));
    }

    #[test]
    fn hl7_adt_creates_task_and_orm_updates_mpps_when_supplied() {
        let worklist_path = temp_file_path("interop_hl7_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_hl7_mpps", "snapshot");
        let sr_path = temp_file_path("interop_hl7_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_hl7_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let adt = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&scheduled_step_id=STEP-101&task_id=TASK-101&requested_procedure_id=REQ-77&status=SCHEDULED&worker=agent"
                .to_vec(),
        };
        let adt_response = route_request(&adt, &shared, &limits).expect("interop adt");
        match adt_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"outcome\":\"inserted\""));
            }
        }

        let orm = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORM&scheduled_step_id=STEP-101&task_id=TASK-101&status=IN_PROGRESS&sop_instance_uid=1.2.3.4&performed_step_id=PS-1&start_date=20260222&start_time=101010&mpps_status=IN_PROGRESS"
                .to_vec(),
        };
        let orm_response = route_request(&orm, &shared, &limits).expect("interop orm");
        match orm_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORM\""));
                assert!(body.contains("\"task_status\":\"IN PROGRESS\""));
            }
        }

        let get_task = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks/TASK-101".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let task_response = route_request(&get_task, &shared, &limits).expect("get task");
        match task_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"task_id\":\"TASK-101\""));
            }
        }

        let list_mpps = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let mpps_response = route_request(&list_mpps, &shared, &limits).expect("mpps list");
        match mpps_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_oru_updates_sr_and_subscriptions_and_failures() {
        let worklist_path = temp_file_path("interop_oru_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_oru_mpps", "snapshot");
        let sr_path = temp_file_path("interop_oru_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_oru_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
            "http://connector.example/interop",
        );

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&event_filter=oru&sink_kind=webhook&sink_target=https%3A%2F%2Finterop.example%2Fevents"
                .to_vec(),
        };
        let create_sub_response =
            route_request(&create_sub, &shared, &limits).expect("create subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
            }
        }

        let create_custom_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&event_filter=oru&sink_kind=custom&sink_connector=enterprise_his&sink_target=https%3A%2F%2Fconnector.example%2Forg%2Foru"
                .to_vec(),
        };
        let create_custom_sub_response = route_request(&create_custom_sub, &shared, &limits)
            .expect("create custom subscription");
        match create_custom_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"sink_kind\":\"custom\""));
            }
        }

        let list_sub_before = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_before =
            route_request(&list_sub_before, &shared, &limits).expect("list subscriptions");
        match list_before {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
            }
        }

        let oru = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORU&study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&report_text=report+from+his&authored_epoch_ms=170000&known_refs=1.2.3.4.5"
                .to_vec(),
        };
        let oru_response = route_request(&oru, &shared, &limits).expect("interop oru");
        match oru_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORU\""));
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let list_sub_after =
            route_request(&list_sub_before, &shared, &limits).expect("list subscriptions");
        match list_sub_after {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":1"));
            }
        }

        let bad_adt = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-adt-bad-1".to_string())]),
            body: b"source=his&message_type=ADT&status=SCHEDULED".to_vec(),
        };
        assert!(route_request(&bad_adt, &shared, &limits).is_err());

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
                assert!(body.contains("\"correlation_id\":\"hlt-adt-bad-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
        }

        std::env::remove_var("DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS");
        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_malformed_payload_is_rejected_with_decode_error() {
        let worklist_path = temp_file_path("interop_malformed_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_malformed_mpps", "snapshot");
        let sr_path = temp_file_path("interop_malformed_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_malformed_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let malformed = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-malformed-1".to_string())]),
            body: b"source=his&message_type=ADT&task=TASK-MALFORMED".to_vec(),
        };
        let err = route_request(&malformed, &shared, &limits)
            .expect_err("malformed hl7 payload should fail");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");
        assert!(err
            .message
            .contains("missing required identifier: scheduled_step_id"));

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"scope\":\"ingest\""));
                assert!(body.contains("\"correlation_id\":\"hlt-malformed-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_interop_failure_mapping_preserves_correlation_across_audit_and_dlq() {
        let worklist_path = temp_file_path("interop_e2e_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_e2e_mpps", "snapshot");
        let sr_path = temp_file_path("interop_e2e_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_e2e_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let correlation_id = "e2e-corr-bridge-1";
        let writer_headers = BTreeMap::from([
            ("x-sr-principal".to_string(), "interop-e2e".to_string()),
            ("x-sr-role".to_string(), "writer".to_string()),
            (
                "x-idempotency-key".to_string(),
                "interop-e2e-task-1".to_string(),
            ),
            ("x-request-id".to_string(), correlation_id.to_string()),
        ]);

        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: writer_headers,
            body: b"scheduled_step_id=STEP-E2E&requested_procedure_id=RP-E2E".to_vec(),
        };
        let _ = route_request(&create_task, &shared, &limits).expect("seed workflow task");

        let bad_hl7 = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-request-id".to_string(), correlation_id.to_string()),
            ]),
            body: b"source=his&message_type=ADT&status=SCHEDULED".to_vec(),
        };
        let err = route_request(&bad_hl7, &shared, &limits)
            .expect_err("interop failure should be captured in DLQ");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let failures_response =
            route_request(&failures, &shared, &limits).expect("list interop failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scope\":\"ingest\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"correlation_id\":\"e2e-corr-bridge-1\""));
            }
        }

        let audit = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let audit_response = route_request(&audit, &shared, &limits).expect("workflow audit");
        match audit_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"route\":\"/interop/hl7\""));
                assert!(body.contains(&hash_text(correlation_id)));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_status_dashboard_reports_connector_and_failure_summary() {
        let worklist_path = temp_file_path("interop_connectors_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_connectors_mpps", "snapshot");
        let sr_path = temp_file_path("interop_connectors_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_connectors_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        {
            let mut state = shared
                .lock()
                .expect("shared state lock for dashboard fixture");
            state.hl7.lock().connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.lock().connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );

            let _ = state.hl7.lock().subscriptions.insert(
                "sub-enterprise".to_string(),
                Hl7Subscription {
                    id: "sub-enterprise".to_string(),
                    source: "his".to_string(),
                    event_filter: vec!["adt".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("enterprise_his".to_string()),
                        target: "https://connectors.enterprise-his.example/interop".to_string(),
                    },
                    delivered_events: 7,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.lock().subscriptions.insert(
                "sub-lab".to_string(),
                Hl7Subscription {
                    id: "sub-lab".to_string(),
                    source: "*".to_string(),
                    event_filter: vec!["oru".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("lab".to_string()),
                        target: "https://connectors.lab.example/interop".to_string(),
                    },
                    delivered_events: 4,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.lock().subscriptions.insert(
                "sub-webhook".to_string(),
                Hl7Subscription {
                    id: "sub-webhook".to_string(),
                    source: "lab".to_string(),
                    event_filter: vec!["orf".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Webhook,
                        target: "https://notify.example/events".to_string(),
                    },
                    delivered_events: 11,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.lock().subscriptions.insert(
                "sub-bus".to_string(),
                Hl7Subscription {
                    id: "sub-bus".to_string(),
                    source: "ris".to_string(),
                    event_filter: vec!["mpps".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::MessageBus,
                        target: "bus://workflow/connectivity".to_string(),
                    },
                    delivered_events: 2,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );

            state.hl7.lock().failures.push_back(Hl7FailureRecord {
                id: "failure-ingest-1".to_string(),
                source: "his".to_string(),
                message_type: "ADT".to_string(),
                reason: "validation failure".to_string(),
                payload_excerpt: "source=his&message_type=ADT".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "ingest".to_string(),
                subscription_id: String::new(),
                event_id: "evt-ingest-1".to_string(),
                correlation_id: "corr-ingest-1".to_string(),
                sequence: 1,
                attempt: 3,
                max_attempts: 5,
            });
            state.hl7.lock().failures.push_back(Hl7FailureRecord {
                id: "failure-callback-1".to_string(),
                source: "his".to_string(),
                message_type: "ORU".to_string(),
                reason: "callback timeout".to_string(),
                payload_excerpt: "callback timeout".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "callback".to_string(),
                subscription_id: "sub-enterprise".to_string(),
                event_id: "evt-callback-1".to_string(),
                correlation_id: "corr-callback-1".to_string(),
                sequence: 2,
                attempt: 3,
                max_attempts: 5,
            });
        }

        let status = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-test".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&status, &shared, &limits).expect("connector status");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"subscription_count\":4"));
                assert!(body.contains("\"custom_connector_subscriptions\":2"));
                assert!(body.contains("\"webhook_subscriptions\":1"));
                assert!(body.contains("\"message_bus_subscriptions\":1"));
                assert!(body.contains("\"wildcard_subscriptions\":1"));
                assert!(body.contains("\"failures\":{\"total\":2,\"ingest\":1,\"callback\":1"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"delivered_events\":7"));
                assert!(body.contains("\"callback_failures\":1"));
                assert!(body.contains("\"generated_at_ms\":"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_status_payload_orders_aliases_deterministically() {
        let worklist_path = temp_file_path("interop_connectors_order_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_connectors_order_mpps", "snapshot");
        let sr_path = temp_file_path("interop_connectors_order_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_connectors_order_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector ordering fixture");
            state.hl7.lock().connector_registry.insert(
                "zeta".to_string(),
                "https://connectors.example/zeta".to_string(),
            );
            state.hl7.lock().connector_registry.insert(
                "alpha".to_string(),
                "https://connectors.example/alpha".to_string(),
            );
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-order".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };

        let first = route_request(&request, &shared, &limits).expect("connector status first");
        let second = route_request(&request, &shared, &limits).expect("connector status second");

        let (first_body, second_body) = match (first, second) {
            (WorkflowResponse::Json(200, first_body), WorkflowResponse::Json(200, second_body)) => {
                (first_body, second_body)
            }
            _ => panic!("unexpected response type"),
        };

        let first_alpha = first_body.find("\"alias\":\"alpha\"").expect("alpha alias");
        let first_zeta = first_body.find("\"alias\":\"zeta\"").expect("zeta alias");
        let second_alpha = second_body
            .find("\"alias\":\"alpha\"")
            .expect("alpha alias");
        let second_zeta = second_body.find("\"alias\":\"zeta\"").expect("zeta alias");
        assert!(first_alpha < first_zeta);
        assert!(second_alpha < second_zeta);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_features_endpoint_reports_feature_flag_and_rollout() {
        let worklist_path = temp_file_path("interop_features_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_features_mpps", "snapshot");
        let sr_path = temp_file_path("interop_features_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_features_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for features dashboard fixture");
            state.hl7.lock().connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.lock().connector_registry.insert(
                "corp*".to_string(),
                "https://connectors.example/corp/{*}".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("enterprise_his".to_string(), false);
            state
                .hl7
                .connector_rollout_percent
                .insert("enterprise_his".to_string(), 42);
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/features".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-test".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector features");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"feature_enabled\":false"));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"alias\":\"corp*\""));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_rollout_admin_api_updates_with_bounds_validation() {
        let worklist_path = temp_file_path("interop_rollout_admin_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_rollout_admin_mpps", "snapshot");
        let sr_path = temp_file_path("interop_rollout_admin_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_rollout_admin_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for rollout admin fixture");
            state.hl7.lock().connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
        }

        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&rollout_percent=42".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("rollout update");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"updated\""));
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"percent\":42"));
            }
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("rollout list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"percent\":42"));
            }
        }

        let legacy_update = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&percent=64".to_vec(),
        };
        let legacy_response =
            route_request(&legacy_update, &shared, &limits).expect("legacy percent update");
        match legacy_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":64"));
                assert!(body.contains("\"percent\":64"));
            }
        }

        let invalid = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&rollout_percent=101".to_vec(),
        };
        let invalid_err =
            route_request(&invalid, &shared, &limits).expect_err("rollout >100 should fail");
        assert_eq!(status_for_error(&invalid_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    #[test]
    fn workflow_admin_api_versioning_accepts_v1_alias_and_rejects_unknown_versions() {
        let worklist_path = temp_file_path("admin_api_version_worklist", "snapshot");
        let mpps_path = temp_file_path("admin_api_version_mpps", "snapshot");
        let sr_path = temp_file_path("admin_api_version_sr", "snapshot");
        let sr_audit_path = temp_file_path("admin_api_version_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for admin api versioning fixture");
            let _ = state.hl7.lock().connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connector.example/interop".to_string(),
            );
        }

        let headers = BTreeMap::from([
            ("x-sr-principal".to_string(), "admin-api".to_string()),
            ("x-sr-role".to_string(), "admin".to_string()),
        ]);

        let no_version = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: b"alias=enterprise_his&rollout_percent=40".to_vec(),
        };
        let default_response =
            route_request(&no_version, &shared, &limits).expect("default admin api version");
        match default_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":40"));
            }
        }

        let legacy_v1_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::from([("api_version".to_string(), "1".to_string())]),
            headers: headers.clone(),
            body: b"alias=enterprise_his&rollout_percent=55".to_vec(),
        };
        let legacy_response =
            route_request(&legacy_v1_alias, &shared, &limits).expect("legacy v1 alias support");
        match legacy_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":55"));
            }
        }

        let unsupported = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::from([("api_version".to_string(), "v2".to_string())]),
            headers,
            body: b"alias=enterprise_his&rollout_percent=60".to_vec(),
        };
        let err = route_request(&unsupported, &shared, &limits)
            .expect_err("unknown admin api version should fail closed");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");
        assert!(err.message().contains("unsupported admin api_version"));

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_capabilities_endpoint_reports_version_and_message_classes() {
        let worklist_path = temp_file_path("interop_capabilities_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_capabilities_mpps", "snapshot");
        let sr_path = temp_file_path("interop_capabilities_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_capabilities_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for capabilities fixture");
            state.hl7.lock().connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.lock().connector_registry.insert(
                "dimse_bridge".to_string(),
                "dimse://bridge.example/ingest".to_string(),
            );
            state.hl7.lock().connector_plugins.insert(
                "enterprise_his".to_string(),
                ConnectorPluginMetadata {
                    plugin_path: "/opt/connectors/enterprise_his.wasm".to_string(),
                    adapter_version: "2.4.0".to_string(),
                    compatible_min: "2.0.0".to_string(),
                    compatible_max: "3.0.0".to_string(),
                },
            );
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/capabilities".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-capabilities".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector capabilities");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"version\":\"2.4.0\""));
                assert!(body.contains("\"adapter_version\":\"2.4.0\""));
                assert!(body
                    .contains("\"supported_message_classes\":[\"ADT\",\"ORM\",\"ORU\",\"SIU\"]"));
                assert!(body.contains("\"alias\":\"dimse_bridge\""));
                assert!(body
                    .contains("\"supported_message_classes\":[\"C-FIND\",\"N-CREATE\",\"N-SET\"]"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_health_endpoint_reports_degraded_and_downstream_timeout_states() {
        let worklist_path = temp_file_path("interop_health_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_health_mpps", "snapshot");
        let sr_path = temp_file_path("interop_health_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_health_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector health fixture");
            state
                .hl7
                .connector_registry
                .insert("lab".to_string(), String::new());
            state.hl7.lock().connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            let _ = state.hl7.lock().subscriptions.insert(
                "sub-enterprise".to_string(),
                Hl7Subscription {
                    id: "sub-enterprise".to_string(),
                    source: "his".to_string(),
                    event_filter: vec!["oru".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("enterprise_his".to_string()),
                        target: "https://connectors.enterprise-his.example/interop".to_string(),
                    },
                    delivered_events: 0,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: 0,
                },
            );
            state.hl7.lock().failures.push_back(Hl7FailureRecord {
                id: "failure-timeout-1".to_string(),
                source: "his".to_string(),
                message_type: "ORU".to_string(),
                reason: "callback timeout".to_string(),
                payload_excerpt: "timeout".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "callback".to_string(),
                subscription_id: "sub-enterprise".to_string(),
                event_id: "evt-timeout-1".to_string(),
                correlation_id: "corr-timeout-1".to_string(),
                sequence: 1,
                attempt: 3,
                max_attempts: 3,
            });
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/health".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-health-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector health");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"status\":\"degraded\""));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"status\":\"downstream_timeout\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn fhir_ingest_returns_typed_contract_when_enabled() {
        let _guard = ENV_LOCK.lock().expect("env lock for fhir ingest");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=Observation&source_system=ehr-core&observation_code=LOINC-1234&subject_id=patient-1&observed_at=2026-02-24T10%3A00%3A00Z&content_sha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&dry_run=true"
                .to_vec(),
        };
        let response = handle_fhir_ingest(&request, &Limits::default()).expect("typed fhir ingest");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 202);
                assert!(body.contains("\"mode\":\"typed\""));
                assert!(body.contains("\"resource_type\":\"Observation\""));
                assert!(body.contains("\"source_system\":\"ehr-core\""));
                assert!(body.contains("\"dry_run\":true"));
                assert!(body.contains("\"content_sha256_present\":true"));
            }
        }

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn fhir_ingest_rejects_unsupported_resource_type() {
        let _guard = ENV_LOCK.lock().expect("env lock for fhir ingest rejection");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=MedicationRequest".to_vec(),
        };
        let err = handle_fhir_ingest(&request, &Limits::default())
            .expect_err("unsupported fhir resource_type should fail");
        assert_eq!(status_for_error(&err).0, 400);
        assert!(err.message().contains("unsupported fhir resource_type"));

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn fhir_ingest_rejects_missing_resource_specific_required_fields() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for fhir required field validation");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=Observation&source_system=ehr-core".to_vec(),
        };
        let err = handle_fhir_ingest(&request, &Limits::default())
            .expect_err("missing observation required fields must fail");
        assert_eq!(status_for_error(&err).0, 400);
        assert!(err.message().contains("missing required fhir field"));

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn hl7_connector_delivery_is_skipped_when_feature_disabled() {
        let worklist_path = temp_file_path("interop_feature_disabled_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_feature_disabled_mpps", "snapshot");
        let sr_path = temp_file_path("interop_feature_disabled_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_feature_disabled_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for feature gate disabled fixture");
            state.hl7.lock().connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("lab".to_string(), false);
        }

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&event_filter=all&sink_kind=custom&sink_connector=lab&sink_target=https://callback.example/his".to_vec(),
        };
        let create_response =
            route_request(&create, &shared, &limits).expect("create subscription");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("sub-00001"));
            }
            _ => panic!("unexpected response type"),
        }

        let ingest = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&message_type=ADT&scheduled_step_id=STEP-100&patient_id=P-1&status=SCHEDULED".to_vec(),
        };
        let _ = route_request(&ingest, &shared, &limits).expect("deliverable event");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list subscriptions");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_delivery_is_skipped_when_rollout_zero() {
        let worklist_path = temp_file_path("interop_rollout_zero_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_rollout_zero_mpps", "snapshot");
        let sr_path = temp_file_path("interop_rollout_zero_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_rollout_zero_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for feature gate rollout fixture");
            state.hl7.lock().connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("lab".to_string(), true);
            state
                .hl7
                .connector_rollout_percent
                .insert("lab".to_string(), 0);
        }

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&event_filter=all&sink_kind=custom&sink_connector=lab&sink_target=https://callback.example/his".to_vec(),
        };
        let create_response =
            route_request(&create, &shared, &limits).expect("create subscription");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("sub-00001"));
            }
            _ => panic!("unexpected response type"),
        }

        let ingest = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&message_type=ADT&scheduled_step_id=STEP-200&patient_id=P-2&status=SCHEDULED".to_vec(),
        };
        let _ = route_request(&ingest, &shared, &limits).expect("deliverable event");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list subscriptions");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_schema_evolution_aliases_are_accepted() {
        let worklist_path = temp_file_path("interop_alias_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_alias_mpps", "snapshot");
        let sr_path = temp_file_path("interop_alias_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_alias_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let adt_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&task=TASK-ALIAS&ScheduledStepID=STEP-ALIAS&status=IN_PROGRESS"
                .to_vec(),
        };
        let adt_response = route_request(&adt_alias, &shared, &limits).expect("interop adt alias");
        match adt_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"task_id\":\"TASK-ALIAS\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
            }
            _ => panic!("unexpected response type"),
        }

        let oru_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORU&StudyInstanceUID=1.2.3&SeriesInstanceUID=1.2.3.4&SOPInstanceUID=1.2.3.4.5&text=report+from+legacy"
                .to_vec(),
        };
        let oru_response = route_request(&oru_alias, &shared, &limits).expect("interop oru alias");
        match oru_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORU\""));
                assert!(body.contains("\"outcome\":\"created\""));
            }
            _ => panic!("unexpected response type"),
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_replay_payload_is_detected_and_cached() {
        let worklist_path = temp_file_path("interop_hl7_replay_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_hl7_replay_mpps", "snapshot");
        let sr_path = temp_file_path("interop_hl7_replay_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_hl7_replay_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let body = b"source=his&message_type=ADT&message_control_id=MSG-HL7-REPLAY&task_id=TASK-REPLAY&scheduled_step_id=STEP-REPLAY&status=SCHEDULED"
            .to_vec();

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: body.clone(),
        };

        let first = match route_request(&request, &shared, &limits).expect("interop adt first") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ADT\""));
                body
            }
            _ => panic!("unexpected response type"),
        };

        let second = match route_request(&request, &shared, &limits).expect("interop adt replay") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                body
            }
            _ => panic!("unexpected response type"),
        };
        assert_eq!(first, second);

        let mutated = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&message_control_id=MSG-HL7-REPLAY&task_id=TASK-REPLAY&scheduled_step_id=STEP-REPLAY&status=IN_PROGRESS"
                .to_vec(),
        };
        let err = route_request(&mutated, &shared, &limits).expect_err("replay mismatch must fail");
        assert_eq!(err.code(), "DVF.HTTP.DECODE");
        assert!(
            err.message().contains("hl7 replay payload mismatch")
                || err.message().contains("decode error")
                || err.message().contains("DVF.HTTP.DECODE")
        );

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_task_transition_callbacks_are_delivered_and_counted() {
        let worklist_path = temp_file_path("interop_cb_task_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_task_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_task_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_task_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=task&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback.example%2Fevents"
                .to_vec(),
        };
        let create_sub_response = route_request(&create_sub, &shared, &limits)
            .expect("create task callback subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"workflow\""));
                assert!(body.contains("\"event_filter\":[\"task\"]"));
            }
        }

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-idempotency-key".to_string(), "task-cb-1".to_string());
        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS-CB&requested_procedure_id=RP-CB&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create_task, &shared, &limits)
            .expect("create task for callback")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let marker = "\"task_id\":\"";
                let start = body
                    .find(marker)
                    .expect("task id field")
                    .saturating_add(marker.len());
                let end = body[start..].find('"').expect("task id terminator");
                body[start..start + end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let start_task = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-cb-success-1".to_string())]),
            body: Vec::new(),
        };
        match route_request(&start_task, &shared, &limits).expect("start task") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":1"));
            }
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(!body.contains("\"scope\":\"callback\""));
                assert!(!body.contains("\"attempt\":3"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_workflow_transition_callbacks_record_dlq_metadata_after_retries() {
        let worklist_path = temp_file_path("interop_cb_dlq_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_dlq_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_dlq_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_dlq_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=task&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-fail.local%2Fevents"
                .to_vec(),
        };
        let create_sub_response = route_request(&create_sub, &shared, &limits)
            .expect("create failing callback subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"workflow\""));
                assert!(body.contains("\"event_filter\":[\"task\"]"));
            }
        }

        let mut create_headers = BTreeMap::new();
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "task-cb-fail-1".to_string(),
        );
        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS-CB-F&requested_procedure_id=RP-CB-F&worker=planner"
                .to_vec(),
        };
        let task_id = match route_request(&create_task, &shared, &limits)
            .expect("create task for callback failure path")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let marker = "\"task_id\":\"";
                let start = body
                    .find(marker)
                    .expect("task id field")
                    .saturating_add(marker.len());
                let end = body[start..].find('"').expect("task id terminator");
                body[start..start + end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let start_task = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-cb-dlq-1".to_string())]),
            body: Vec::new(),
        };
        match route_request(&start_task, &shared, &limits).expect("start task") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let task_get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/workflow/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let task_after =
            route_request(&task_get, &shared, &limits).expect("get task after callback failure");
        match task_after {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scope\":\"callback\""));
                assert!(body.contains("\"attempt\":3"));
                assert!(body.contains("\"max_attempts\":3"));
                assert!(body.contains("\"event_id\":\"HL7-000001\""));
                assert!(body.contains("\"subscription_id\":\"sub-00001\""));
                assert!(body.contains("\"correlation_id\":\"hlt-cb-dlq-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn connector_callback_circuit_breaker_opens_after_repeated_failures() {
        let worklist_path = temp_file_path("interop_cb_circuit_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_circuit_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_circuit_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_circuit_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector circuit fixture");
            state.hl7.lock().connector_registry.insert(
                "lab".to_string(),
                "https://callback-fail.local/events".to_string(),
            );
            let _ = state.hl7.lock().subscriptions.insert(
                "sub-00001".to_string(),
                Hl7Subscription {
                    id: "sub-00001".to_string(),
                    source: "workflow".to_string(),
                    event_filter: vec!["task".to_string()].into_iter().collect(),
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("lab".to_string()),
                        target: "https://callback-fail.local/events".to_string(),
                    },
                    delivered_events: 0,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: 0,
                },
            );

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-1",
                1,
                "corr-circuit-1",
            );
            assert_eq!(state.hl7.lock().failures.len(), 1);

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-2",
                2,
                "corr-circuit-2",
            );
            assert_eq!(state.hl7.lock().failures.len(), 2);
            let open_until = state
                .hl7
                .connector_circuit_open_until_ms
                .get("lab")
                .copied()
                .expect("circuit should open after repeated callback failures");
            assert!(open_until > now_epoch_millis());

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-3",
                3,
                "corr-circuit-3",
            );
            assert_eq!(
                state.hl7.lock().failures.len(),
                2,
                "circuit-open connector should not emit additional callback failures until backoff elapses"
            );
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_tenant_quota_readonly_api_and_snapshot_export_override_values() {
        let _guard = ENV_LOCK.lock().expect("env lock for tenant quota api");
        let prev_task = std::env::var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A").ok();
        let prev_sub = std::env::var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A").ok();
        std::env::set_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A", "17");
        std::env::set_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A", "5");

        let worklist_path = temp_file_path("tenant_quota_api_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_quota_api_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_quota_api_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_quota_api_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let read_override = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/policy/quotas".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-a".to_string())]),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "policy-admin".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: Vec::new(),
        };
        let read_response =
            route_request(&read_override, &shared, &limits).expect("tenant quota read api");
        match read_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"tenant\":\"tenant-a\""));
                assert!(body.contains("\"task_quota\":17"));
                assert!(body.contains("\"subscription_quota\":5"));
            }
        }

        let snapshot = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/policy/quotas/snapshot".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "policy-admin".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let snapshot_response =
            route_request(&snapshot, &shared, &limits).expect("tenant quota snapshot api");
        match snapshot_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"generated_at_ms\":"));
                assert!(body.contains("\"tenant_a\":{\"task_quota\":17,\"subscription_quota\":5}"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);

        if let Some(value) = prev_task {
            std::env::set_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A");
        }
        if let Some(value) = prev_sub {
            std::env::set_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A");
        }
    }

    #[test]
    fn tenant_rate_limit_override_snapshot_round_trip() {
        let snapshot_path = temp_file_path("tenant_rate_limit_overrides", "state");
        let mut overrides = BTreeMap::new();
        let _ = overrides.insert(
            "tenant_a".to_string(),
            TenantRateLimitOverride {
                query_rate_limit: 45,
                mutation_rate_limit: 21,
                upload_cap_bytes: 2 * 1024 * 1024,
            },
        );
        persist_tenant_rate_limit_overrides(&snapshot_path.to_string_lossy(), &overrides);

        let loaded = load_tenant_rate_limit_overrides(&snapshot_path.to_string_lossy());
        let tenant_a = loaded
            .get("tenant_a")
            .expect("tenant_a override should round-trip");
        assert_eq!(tenant_a.query_rate_limit, 45);
        assert_eq!(tenant_a.mutation_rate_limit, 21);
        assert_eq!(tenant_a.upload_cap_bytes, 2 * 1024 * 1024);

        cleanup_with_rotations(&snapshot_path, 0);
    }

    #[test]
    fn tenant_rate_limit_overrides_apply_and_fallback_to_defaults() {
        let worklist_path = temp_file_path("tenant_rate_policy_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_rate_policy_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_rate_policy_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_rate_policy_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let mut overrides = BTreeMap::new();
        let _ = overrides.insert(
            tenant_env_suffix("tenant-a").to_ascii_lowercase(),
            TenantRateLimitOverride {
                query_rate_limit: 61,
                mutation_rate_limit: 17,
                upload_cap_bytes: 1_024,
            },
        );
        set_tenant_rate_limit_override_cache(overrides);

        {
            let state = shared
                .lock()
                .expect("shared state lock for tenant rate policy assertions");
            let tenant_a_policy = tenant_policy(&state, "tenant-a");
            assert_eq!(tenant_a_policy.query_rate_limit, 61);
            assert_eq!(tenant_a_policy.mutation_rate_limit, 17);
            assert_eq!(tenant_a_policy.upload_cap_bytes, 1_024);

            let tenant_b_policy = tenant_policy(&state, "tenant-b");
            assert_eq!(tenant_b_policy.query_rate_limit, DEFAULT_QUERY_RATE_LIMIT);
            assert_eq!(
                tenant_b_policy.mutation_rate_limit,
                DEFAULT_MUTATION_RATE_LIMIT
            );
            assert_eq!(tenant_b_policy.upload_cap_bytes, DEFAULT_UPLOAD_CAP_BYTES);
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        set_tenant_rate_limit_override_cache(BTreeMap::new());
    }

    #[test]
    fn reconciliation_jobs_enforce_interval_bounds_max_jobs_and_tenant_isolation() {
        let worklist_path = temp_file_path("recon_guardrails_worklist", "snapshot");
        let mpps_path = temp_file_path("recon_guardrails_mpps", "snapshot");
        let sr_path = temp_file_path("recon_guardrails_sr", "snapshot");
        let sr_audit_path = temp_file_path("recon_guardrails_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let invalid_interval = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/reconciliation/jobs".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-admin".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: b"source=his&target_endpoint=https://recon.example/jobs&interval_seconds=10"
                .to_vec(),
        };
        let invalid_interval_err = route_request(&invalid_interval, &shared, &limits)
            .expect_err("interval below floor should fail");
        assert_eq!(status_for_error(&invalid_interval_err).0, 400);

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/reconciliation/jobs".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-admin".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: b"source=his&target_endpoint=https://recon.example/jobs&interval_seconds=120"
                .to_vec(),
        };
        let created = route_request(&create, &shared, &limits).expect("create reconciliation job");
        let created_job_id = match created {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"tenant\":\"tenant-a\""));
                let marker = "\"id\":\"";
                let start = body
                    .find(marker)
                    .expect("reconciliation id field")
                    .saturating_add(marker.len());
                let end = body[start..]
                    .find('"')
                    .expect("reconciliation id terminator");
                body[start..start + end].to_string()
            }
        };

        let cross_tenant_run = HttpRequest {
            method: "POST".to_string(),
            path: format!("/interop/reconciliation/jobs/{created_job_id}/run"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-other".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-b".to_string()),
                (
                    "x-idempotency-key".to_string(),
                    "recon-run-cross-1".to_string(),
                ),
            ]),
            body: Vec::new(),
        };
        let cross_tenant_err = route_request(&cross_tenant_run, &shared, &limits)
            .expect_err("cross-tenant run should fail");
        assert_eq!(cross_tenant_err.code(), "DVF.DICOM.DECODE_ERROR");

        {
            let mut state = shared
                .lock()
                .expect("shared state lock for reconciliation cap fixture");
            while state.hl7.lock().reconciliation_jobs.len() < MAX_RECONCILIATION_JOBS {
                state.hl7.lock().reconciliation_seq = state.hl7.lock().reconciliation_seq.saturating_add(1);
                let id = format!("recon-cap-{0:05}", state.hl7.reconciliation_seq);
                let _ = state.hl7.lock().reconciliation_jobs.insert(
                    id.clone(),
                    StudyReconciliationJob {
                        id,
                        tenant: "tenant-a".to_string(),
                        source: "his".to_string(),
                        target_endpoint: "https://recon.example/jobs".to_string(),
                        interval_seconds: 120,
                        runs_enqueued: 0,
                        runs_completed: 0,
                        last_run_at_ms: 0,
                        created_at_ms: now_epoch_millis(),
                        enabled: true,
                    },
                );
            }
        }

        let over_cap = route_request(&create, &shared, &limits)
            .expect_err("reconciliation max jobs guardrail should fail");
        assert_eq!(status_for_error(&over_cap).0, 413);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn parse_hl7_event_filter_supports_workflow_transition_aliases() {
        let parsed = parse_hl7_event_filter("workflow,task,mpps,sr")
            .expect("parse workflow transition filters");
        assert!(parsed.contains("mpps"));
        assert!(parsed.contains("sr"));
        assert!(parsed.contains("task"));
        assert!(parsed.contains("workflow"));
        let parsed_workflow_only =
            parse_hl7_event_filter("workflow").expect("parse workflow filter");
        assert!(parsed_workflow_only.contains("workflow"));
        let err = parse_hl7_event_filter("badfilter")
            .expect_err("unsupported hl7 event filter must fail");
        assert_eq!(err.code(), "DVF.WORKFLOW.HTTP.DECODE_ERROR");
    }

    #[test]
    fn hl7_mpps_and_sr_transition_callbacks_use_workflow_filters() {
        let worklist_path = temp_file_path("interop_cb_mpps_sr_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_mpps_sr_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_mpps_sr_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_mpps_sr_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_mpps_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=workflow&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-mpps.example%2Fevents"
                .to_vec(),
        };
        let _ = route_request(&create_mpps_sub, &shared, &limits)
            .expect("create mpps/sub transition subscription");

        let create_sr_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=workflow&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-sr.example%2Fevents"
                .to_vec(),
        };
        let _ = route_request(&create_sr_sub, &shared, &limits)
            .expect("create sr transition subscription");

        let create_mpps = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-1&start_date=20260222&start_time=101010".to_vec(),
        };
        let _ = route_request(&create_mpps, &shared, &limits).expect("create mpps");

        let update_mpps = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=COMPLETED".to_vec(),
        };
        let _ = route_request(&update_mpps, &shared, &limits).expect("update mpps");

        let mut sr_headers = BTreeMap::new();
        sr_headers.insert("x-sr-role".to_string(), "writer".to_string());
        let create_sr = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1700&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=transition"
                .to_vec(),
        };
        let _ = route_request(&create_sr, &shared, &limits).expect("create sr");
        let review_sr = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/review".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers,
            body: Vec::new(),
        };
        let _ = route_request(&review_sr, &shared, &limits).expect("review sr");

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(!body.contains("\"scope\":\"callback\""));
                assert!(!body.contains("\"attempt\":3"));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"event_filter\":[\"workflow\"]"));
                assert!(body.contains("\"delivered_events\":2"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }
}

#[cfg(all(test, feature = "workflow-main-tests"))]
mod parallel_tests {
    use super::*;
    use dicom_workflow_server::path_with_suffix;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("diccy_{name}_{nonce}.{ext}"))
    }

    fn cleanup_with_rotations(path: &Path, max_rotations: usize) {
        let _ = std::fs::remove_file(path);
        for index in 1..=max_rotations + 1 {
            let _ = std::fs::remove_file(path_with_suffix(path, index));
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }

    #[test]
    fn sr_http_parallel_create_update_is_deterministic() {
        // REQ-SR-300, REQ-TEST-742
        let worklist_path = temp_file_path("sr_parallel_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_parallel_mpps", "snapshot");
        let sr_path = temp_file_path("sr_parallel_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_parallel_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Arc::new(Limits::default());
        let state = RuntimeState {
            worklist: WorklistStore::open((*limits).clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: (*limits).clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                (*limits).clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut handles = Vec::new();
        for index in 0..8u32 {
            let shared = Arc::clone(&shared);
            let limits = Arc::clone(&limits);
            handles.push(thread::spawn(move || {
                let sop_uid = format!("1.2.840.10008.5.1.{}", index + 1);

                let mut create_headers = BTreeMap::new();
                create_headers.insert("x-sr-principal".to_string(), "parallel-user".to_string());
                create_headers.insert("x-sr-role".to_string(), "writer".to_string());
                create_headers.insert(
                    "x-idempotency-key".to_string(),
                    format!("parallel-create-{index}"),
                );
                let create_body = format!(
                    "study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid={sop_uid}&observer=parallel-user&authored_epoch_ms={}&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=create-{index}",
                    1000 + index
                );
                let create = HttpRequest {
                    method: "POST".to_string(),
                    path: "/sr/documents".to_string(),
                    query: BTreeMap::new(),
                    headers: create_headers,
                    body: create_body.into_bytes(),
                };
                let create_response = route_request(&create, &shared, &limits).expect("parallel create");
                match create_response {
                    WorkflowResponse::Json(code, body) => {
                        assert_eq!(code, 200);
                        assert!(body.contains("\"version\":1"));
                    }
                }

                let mut update_headers = BTreeMap::new();
                update_headers.insert("x-sr-principal".to_string(), "parallel-user".to_string());
                update_headers.insert("x-sr-role".to_string(), "writer".to_string());
                update_headers.insert(
                    "x-idempotency-key".to_string(),
                    format!("parallel-update-{index}"),
                );
                let update_body =
                    format!("expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=update-{index}");
                let update = HttpRequest {
                    method: "POST".to_string(),
                    path: format!("/sr/documents/{sop_uid}/updates"),
                    query: BTreeMap::new(),
                    headers: update_headers,
                    body: update_body.into_bytes(),
                };
                let update_response = route_request(&update, &shared, &limits).expect("parallel update");
                match update_response {
                    WorkflowResponse::Json(code, body) => {
                        assert_eq!(code, 200);
                        assert!(body.contains("\"version\":2"));
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread join");
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                for index in 0..8u32 {
                    let sop_uid = format!("1.2.840.10008.5.1.{}", index + 1);
                    assert!(body.contains(&sop_uid));
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }
}
}
