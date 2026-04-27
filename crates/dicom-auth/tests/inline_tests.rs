// Auto-extracted from /home/z/diccy/crates/dicom-auth/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_auth::*;
    use dicom_core::{ErrorKind};

    fn sample_request<'a>() -> AuthRequest<'a> {
        AuthRequest {
            scope: AuthScope::Dimse,
            action: AuthAction::Echo,
            subject: AuthSubject::anonymous(),
            resource: AuthResource::none(),
        }
    }

    #[test]
    fn allow_all_allows() {
        // REQ-AUTH-300
        let auth = AllowAll;
        let decision = auth.authorize(&sample_request()).expect("decision");
        assert!(decision.is_allowed());
        decision.enforce().expect("allowed");
    }

    #[test]
    fn deny_all_returns_error() {
        // REQ-AUTH-300, REQ-AUTH-302
        let auth = DenyAll::new(AuthDenyReason::Unauthorized);
        let decision = auth.authorize(&sample_request()).expect("decision");
        assert!(!decision.is_allowed());
        let err = decision.enforce().expect_err("denied");
        assert_eq!(err.code(), "DVF.AUTH.DENIED");
        assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
    }

    #[test]
    fn session_timeout_state_transitions_are_deterministic() {
        // REQ-HI-119, REQ-HI-127
        let policy = SessionPolicy::new(300, 60, 3).unwrap();
        let session = SessionStatus::new(1_000);
        assert_eq!(session.state_at(1_239, policy), SessionState::Active);
        assert_eq!(session.state_at(1_240, policy), SessionState::Warning);
        assert_eq!(session.state_at(1_300, policy), SessionState::Locked);
    }

    #[test]
    fn session_locks_after_failure_threshold() {
        // REQ-HI-120, REQ-HI-129
        let policy = SessionPolicy::new(300, 60, 2).unwrap();
        let mut session = SessionStatus::new(2_000);
        assert_eq!(
            session.register_failure(2_010, policy),
            SessionState::Active
        );
        assert_eq!(
            session.register_failure(2_020, policy),
            SessionState::Locked
        );
        assert_eq!(session.state_at(2_021, policy), SessionState::Locked);
    }

    #[test]
    fn tenant_scope_enforcement_denies_cross_tenant_write() {
        let err = enforce_tenant_scope(Some("tenant-a"), Some("tenant-b"))
            .expect_err("cross-tenant write must fail");
        assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
    }

    #[test]
    fn default_policy_keys_cover_viewer_and_protocol_mutations() {
        assert!(DEFAULT_POLICY_KEYS.iter().any(|row| {
            row.action == AuthAction::ViewerMeasurementWrite
                && row.resource == AuthResourceKey::ViewerMeasurement
        }));
        assert!(DEFAULT_POLICY_KEYS.iter().any(|row| {
            row.action == AuthAction::StorageCommitment
                && row.resource == AuthResourceKey::StorageCommitment
        }));
    }

    #[test]
    fn new_modules_reexport_types() {
        // Session module
        let _policy = SessionPolicy::default();
        let _status = SessionStatus::new(100);

        // Config control module
        let _journal = ConfigChangeJournal::default();

        // Break glass module
        let _bg_policy = BreakGlassPolicy::default();
        let _bg_request = BreakGlassRequest {
            principal: "test".to_string(),
            reason_code: "emergency".to_string(),
            scope: "elevated".to_string(),
            approved_by: "admin".to_string(),
            now_epoch_secs: 100,
            expires_epoch_secs: 200,
        };

        // Claim surface module
        let _surface = UiClaimSurface::new();

        // Interface control module
        let _rev = RequirementRevision {
            requirement_id: "REQ-001".to_string(),
            version: "1.0".to_string(),
            approver_signature: "sig".to_string(),
            effective_date: "2026-01-01".to_string(),
        };

        // Export policy module
        let _clip = evaluate_clipboard_policy(true);
        let _privacy = workspace_privacy_mode(false);

        // RBAC authorizer (S10-T4)
        let _rbac = RbacAuthorizer::new();
    }

    #[test]
    fn rbac_authorizer_allows_permitted_action() {
        let rbac = RbacAuthorizer::new();
        let request = AuthRequest {
            scope: AuthScope::Dimse,
            action: AuthAction::Echo,
            subject: AuthSubject::anonymous(),
            resource: AuthResource {
                study_uid: None,
                series_uid: None,
                instance_uid: None,
                key: AuthResourceKey::Study,
            },
        };
        let decision = rbac.authorize(&request).expect("decision");
        assert!(decision.is_allowed());
    }

    #[test]
    fn rbac_authorizer_denies_unknown_action() {
        let mut rbac = RbacAuthorizer::new();
        // Remove admin role to test deny path for non-existent permission
        rbac.policy.remove(&session::UserRole::Administrator);
        let request = AuthRequest {
            scope: AuthScope::Viewer,
            action: AuthAction::ViewerAnnotationWrite,
            subject: AuthSubject::anonymous(),
            resource: AuthResource {
                study_uid: None,
                series_uid: None,
                instance_uid: None,
                key: AuthResourceKey::ViewerAnnotation3d,
            },
        };
        let decision = rbac.authorize(&request).expect("decision");
        // Annotation3d is not in default policy, so should be denied
        assert!(!decision.is_allowed());
    }

    #[test]
    fn session_lockout_enforcement() {
        let policy = SessionPolicy::new(300, 60, 2).unwrap();
        let mut session = SessionStatus::new(100);
        session.register_failure(110, policy);
        // Not yet locked out
        assert!(session.enforce_lockout("sess1", policy).is_ok());
        session.register_failure(120, policy);
        // Now locked out
        assert!(session.enforce_lockout("sess1", policy).is_err());
    }
