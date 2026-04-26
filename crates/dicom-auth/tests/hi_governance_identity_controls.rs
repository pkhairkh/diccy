use dicom_auth::{
    activate_break_glass_access, activate_secure_default_override, commit_secret_entry,
    evaluate_clipboard_policy, force_reauthentication_for_suspicious_session,
    permission_denied_view, validate_interface_change_control_record, validate_release_text_input,
    validate_removable_media_export, validate_requirement_revision_record,
    validate_screenshot_export_policy, validate_startup_fail_closed_controls,
    validate_system_metadata_view, validate_ui_string_change_review, workspace_privacy_mode,
    BreakGlassRequest, ClipboardPolicyDecision, ConfigChangeJournal, ConfigChangeRequest,
    ControlledWordingPolicy, FeatureProfile, HighRiskToggleSummary, IntegrityStatus,
    InterfaceChangeControlRecord, InterfaceImpactClass, PackActivationState,
    RemovableMediaExportRequest, RequirementRevisionRecord, RuntimeLimitIndicators,
    ScreenshotExportPolicy, SecureDefaultOverrideRequest, SecurityPostureIndicators,
    SessionDirectory, SessionOpenRequest, SessionPolicy, SessionState, SessionStatus,
    StartupFailClosedControls, SystemMetadataView, UiStringChangeReview, UserRole,
};

fn valid_metadata_view() -> SystemMetadataView {
    SystemMetadataView {
        intended_purpose: ControlledWordingPolicy::default().approved_intended_purpose,
        envelope_version: "envelope-v1".to_string(),
        application_version: "1.2.3".to_string(),
        build_id: "build-2026-02-14".to_string(),
        feature_profile: FeatureProfile::Workstation,
        pack_activation: vec![
            PackActivationState {
                pack_id: "pack-seg".to_string(),
                enabled: true,
            },
            PackActivationState {
                pack_id: "pack-sr".to_string(),
                enabled: false,
            },
        ],
        security_posture: SecurityPostureIndicators {
            tls_policy: "require_tls".to_string(),
            auth_mode: "token".to_string(),
            audit_mode: "strict".to_string(),
        },
        runtime_limits: RuntimeLimitIndicators {
            max_input_bytes: 268_435_456,
            max_cache_entries: 4_096,
            max_gpu_bytes: 536_870_912,
            max_transport_connections: 64,
        },
        integrity_status: IntegrityStatus::Valid {
            checksum: "sha256:abc123".to_string(),
        },
        unsupported_workflows: vec![dicom_auth::UnsupportedWorkflowState {
            workflow_id: "out_of_envelope_rotation".to_string(),
            visible: true,
            enabled: false,
            rationale: "deterministic resampling policy inactive".to_string(),
        }],
        high_risk_toggles: vec![HighRiskToggleSummary {
            toggle_id: "privileged_export".to_string(),
            enabled: true,
            risk_summary: "identified export increases data exposure risk".to_string(),
        }],
    }
}

#[test]
fn metadata_view_is_controlled_and_claim_safe() {
    // REQ-HI-100, REQ-HI-101, REQ-HI-102, REQ-HI-104, REQ-HI-108, REQ-HI-114
    let policy = ControlledWordingPolicy::default();
    let mut view = valid_metadata_view();
    validate_system_metadata_view(&view, &policy).expect("valid controlled view");

    view.intended_purpose = "Diagnostic workstation - FDA cleared".to_string();
    let err = validate_system_metadata_view(&view, &policy).expect_err("claim text must fail");
    assert_eq!(err.code(), "DVF.DICOM.DECODE_ERROR");

    let claim_err =
        validate_release_text_input("ce marked diagnostic mode").expect_err("claim must fail");
    assert_eq!(claim_err.code(), "DVF.DICOM.DECODE_ERROR");
}

#[test]
fn metadata_view_enforces_unsupported_and_risk_toggle_rules() {
    // REQ-HI-110, REQ-HI-112
    let policy = ControlledWordingPolicy::default();
    let mut view = valid_metadata_view();
    view.unsupported_workflows[0].enabled = true;
    validate_system_metadata_view(&view, &policy).expect_err("unsupported path cannot be enabled");

    let mut view = valid_metadata_view();
    view.high_risk_toggles[0].risk_summary.clear();
    validate_system_metadata_view(&view, &policy)
        .expect_err("enabled high-risk toggle requires summary");
}

#[test]
fn config_change_journal_requires_authenticated_actor_and_reason() {
    // REQ-HI-105, REQ-HI-106
    let mut journal = ConfigChangeJournal::default();
    let unauthenticated = ConfigChangeRequest {
        actor_user_id: "admin-1".to_string(),
        authenticated: false,
        reason_code: "SEC-ROTATION".to_string(),
        key: "tls_policy".to_string(),
        before: "allow_insecure".to_string(),
        after: "require_tls".to_string(),
        timestamp_epoch_secs: 1_700_000_000,
    };
    journal
        .record_change(unauthenticated)
        .expect_err("unauthenticated config change must fail");

    let entry = journal
        .record_change(ConfigChangeRequest {
            actor_user_id: "admin-1".to_string(),
            authenticated: true,
            reason_code: "SEC-ROTATION".to_string(),
            key: "tls_policy".to_string(),
            before: "allow_insecure".to_string(),
            after: "require_tls".to_string(),
            timestamp_epoch_secs: 1_700_000_010,
        })
        .expect("authenticated change should be journaled");
    assert_eq!(entry.before, "allow_insecure");
    assert_eq!(entry.after, "require_tls");
    assert_eq!(journal.entries.len(), 1);
}

#[test]
fn startup_controls_and_override_are_fail_closed_and_time_bounded() {
    // REQ-HI-107, REQ-HI-111, REQ-HI-113
    validate_startup_fail_closed_controls(StartupFailClosedControls {
        tls_policy_loaded: true,
        auth_policy_loaded: true,
        audit_policy_loaded: true,
        claim_surface_policy_loaded: true,
        integrity_checks_valid: true,
    })
    .expect("all controls loaded");

    validate_startup_fail_closed_controls(StartupFailClosedControls {
        tls_policy_loaded: true,
        auth_policy_loaded: false,
        audit_policy_loaded: true,
        claim_surface_policy_loaded: true,
        integrity_checks_valid: true,
    })
    .expect_err("missing control must block startup");

    activate_secure_default_override(
        &SecureDefaultOverrideRequest {
            requested_by: "admin-1".to_string(),
            approved_by: "supervisor-1".to_string(),
            reason_code: "TEMP-DEBUG".to_string(),
            now_epoch_secs: 1_700_000_100,
            expires_epoch_secs: 1_700_000_460,
        },
        600,
    )
    .expect("bounded approved override");
}

#[test]
fn sessions_enforce_unique_identity_listing_revocation_and_token_binding() {
    // REQ-HI-115, REQ-HI-121, REQ-HI-122
    let mut directory = SessionDirectory::default();
    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-shared".to_string(),
            principal: "shared-tech".to_string(),
            role: UserRole::Viewer,
            security_context_hash: "ctx-1".to_string(),
            issued_epoch_secs: 1_700_001_000,
            expires_epoch_secs: 1_700_001_900,
        })
        .expect_err("shared identity must fail");

    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-1".to_string(),
            principal: "alice".to_string(),
            role: UserRole::Reporter,
            security_context_hash: "ctx-1".to_string(),
            issued_epoch_secs: 1_700_001_000,
            expires_epoch_secs: 1_700_001_900,
        })
        .expect("session 1");
    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-2".to_string(),
            principal: "alice".to_string(),
            role: UserRole::Reporter,
            security_context_hash: "ctx-1".to_string(),
            issued_epoch_secs: 1_700_001_050,
            expires_epoch_secs: 1_700_001_950,
        })
        .expect("session 2");

    let listed = directory.list_sessions("alice");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].session_id, "sess-1");
    assert_eq!(listed[1].session_id, "sess-2");

    directory
        .validate_session_token("alice", "sess-1", "ctx-2", 1_700_001_100)
        .expect_err("context mismatch must fail");
    directory
        .validate_session_token("alice", "sess-1", "ctx-1", 1_700_001_100)
        .expect("bound token should validate");

    directory
        .revoke_session("alice", "sess-1", 1_700_001_200)
        .expect("revoke session");
    directory
        .validate_session_token("alice", "sess-1", "ctx-1", 1_700_001_300)
        .expect_err("revoked token must fail");
}

#[test]
fn break_glass_and_permission_denied_views_are_explicit() {
    // REQ-HI-118, REQ-HI-123, REQ-HI-124
    let grant = activate_break_glass_access(
        &BreakGlassRequest {
            principal: "alice".to_string(),
            reason_code: "EMERG-ACCESS".to_string(),
            scope: "export-identified".to_string(),
            approved_by: "supervisor-1".to_string(),
            now_epoch_secs: 1_700_002_000,
            expires_epoch_secs: 1_700_002_300,
        },
        600,
    )
    .expect("break-glass grant should be accepted");
    assert!(grant.post_event_audit_review_required);

    let mut directory = SessionDirectory::default();
    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-3".to_string(),
            principal: "alice".to_string(),
            role: UserRole::Reporter,
            security_context_hash: "ctx-3".to_string(),
            issued_epoch_secs: 1_700_002_010,
            expires_epoch_secs: 1_700_002_910,
        })
        .expect("session for banner");
    let banner = directory
        .identity_banner("sess-3")
        .expect("identity banner");
    assert_eq!(banner.principal, "alice");
    assert_eq!(banner.effective_role, UserRole::Reporter);

    let denied = permission_denied_view("identified_export", UserRole::Administrator);
    assert_eq!(denied.code, "DVF.HI.AUTH.PERMISSION_DENIED");
    assert!(denied.message.contains("identified_export"));
    assert!(denied.next_step.contains("Administrator"));
}

#[test]
fn secret_entry_and_role_transition_controls_are_deterministic() {
    // REQ-HI-126, REQ-HI-128, REQ-HI-207
    let mut secret = "super-secret-value".to_string();
    let receipt = commit_secret_entry(&mut secret).expect("secret commit");
    assert_eq!(receipt.length, 18);
    assert!(secret.is_empty());
    assert_eq!(receipt.masked_preview, "****************");

    let mut directory = SessionDirectory::default();
    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-4".to_string(),
            principal: "alice".to_string(),
            role: UserRole::Viewer,
            security_context_hash: "ctx-4".to_string(),
            issued_epoch_secs: 1_700_003_000,
            expires_epoch_secs: 1_700_003_900,
        })
        .expect("session 4");
    directory
        .open_session(SessionOpenRequest {
            session_id: "sess-5".to_string(),
            principal: "alice".to_string(),
            role: UserRole::Viewer,
            security_context_hash: "ctx-5".to_string(),
            issued_epoch_secs: 1_700_003_010,
            expires_epoch_secs: 1_700_003_910,
        })
        .expect("session 5");

    let events = directory.change_role_for_principal("alice", UserRole::Exporter, 1_700_003_100);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].session_id, "sess-4");
    assert_eq!(events[1].session_id, "sess-5");
    assert_eq!(events[0].next_role, UserRole::Exporter);
}

#[test]
fn privacy_copy_export_and_suspicious_session_controls_are_enforced() {
    // REQ-HI-196, REQ-HI-202, REQ-HI-203, REQ-HI-204, REQ-HI-208
    let non_clinical = workspace_privacy_mode(false);
    assert!(non_clinical.mask_identifiers);

    let denied: ClipboardPolicyDecision = evaluate_clipboard_policy(false);
    assert!(!denied.allowed);
    assert!(denied.audited);

    validate_screenshot_export_policy(&ScreenshotExportPolicy {
        watermark_required: true,
        identifiers_allowed: false,
    })
    .expect("watermark policy");

    validate_removable_media_export(&RemovableMediaExportRequest {
        requested_by: "alice".to_string(),
        approved_by: "security-admin".to_string(),
        policy_authorization_code: "USB-ALLOW-2026".to_string(),
    })
    .expect("removable media authorization");

    let _policy = SessionPolicy {
        inactivity_timeout_secs: 300,
        warning_window_secs: 60,
        max_failures: 3,
    };
    let mut session = SessionStatus::new(1_700_006_000);
    let state = force_reauthentication_for_suspicious_session(&mut session);
    assert_eq!(state, SessionState::Locked);
}

#[test]
fn interface_change_and_revision_governance_requirements_are_enforced() {
    // REQ-HI-247, REQ-HI-248, REQ-HI-249, REQ-HI-254
    validate_ui_string_change_review(&UiStringChangeReview {
        change_id: "CHG-001".to_string(),
        claim_surface_lint_passed: true,
        controlled_wording_approved_by: "qa-1".to_string(),
    })
    .expect("ui string change review");

    let disposition = validate_interface_change_control_record(&InterfaceChangeControlRecord {
        change_id: "CHG-002".to_string(),
        impact_class: InterfaceImpactClass::C2,
        affects_claim_boundary: true,
        affects_conformance_boundary: false,
        evidence_review_ticket: Some("EVID-442".to_string()),
    })
    .expect("change-control disposition");
    assert!(disposition.envelope_review_required);

    validate_requirement_revision_record(&RequirementRevisionRecord {
        requirement_id: "REQ-HI-254".to_string(),
        version: "v1.1".to_string(),
        approver_signature: "SIG-QA-77".to_string(),
        effective_date: "2026-02-14".to_string(),
    })
    .expect("revision record");
}
