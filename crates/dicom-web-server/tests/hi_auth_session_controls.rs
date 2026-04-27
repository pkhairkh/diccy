use dicom_auth::{SessionPolicy, SessionState, SessionStatus};
use dicom_core::{ErrorKind, Limits};
use dicom_web::{
    parse_http_request, DicomWebService, DicomWebServiceConfig, TlsPolicy, TransportSecurity,
    WebAuthConfig, WebPolicy,
};
use std::sync::{Arc, Mutex};

fn service(policy: WebPolicy, auth: WebAuthConfig) -> DicomWebService {
    DicomWebService::new(DicomWebServiceConfig {
        limits: Limits::default(),
        policy,
        auth,
    })
}

#[test]
fn session_timeout_and_lock_transitions_are_deterministic() {
    // REQ-HI-119, REQ-HI-120, REQ-HI-127
    let policy = SessionPolicy::new(120, 30, 2).unwrap();
    let mut session = SessionStatus::new(1_000);

    assert_eq!(session.state_at(1_089, policy), SessionState::Active);
    assert_eq!(session.state_at(1_090, policy), SessionState::Warning);
    assert_eq!(session.state_at(1_120, policy), SessionState::Locked);

    session.unlock(2_000);
    assert_eq!(session.state_at(2_010, policy), SessionState::Active);
    assert_eq!(
        session.register_failure(2_020, policy),
        SessionState::Active
    );
    assert_eq!(
        session.register_failure(2_030, policy),
        SessionState::Locked
    );
}

#[test]
fn auth_denial_blocks_routed_requests() {
    // REQ-HI-116, REQ-HI-117, REQ-HI-129, REQ-HI-245
    let runtime = service(
        WebPolicy::new(TlsPolicy::RequireTls, dicom_web::ThrottleDecision::Allow),
        WebAuthConfig::deny_all(),
    );

    let request = parse_http_request(
        b"GET /studies HTTP/1.1\r\n\r\n",
        &Limits::default(),
        TransportSecurity::Tls,
    )
    .expect("request parse");

    let err = runtime
        .route_request(request)
        .expect_err("request must be denied");
    assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
}

#[test]
fn insecure_transport_is_blocked_before_authz_execution() {
    // REQ-HI-197, REQ-HI-198, REQ-HI-199
    let runtime = service(
        WebPolicy::new(TlsPolicy::RequireTls, dicom_web::ThrottleDecision::Allow),
        WebAuthConfig::allow_all(),
    );

    let request = parse_http_request(
        b"GET /studies HTTP/1.1\r\n\r\n",
        &Limits::default(),
        TransportSecurity::Insecure,
    )
    .expect("request parse");

    let err = runtime
        .route_request(request)
        .expect_err("insecure transport must fail");
    assert!(matches!(
        err.kind(),
        ErrorKind::DecodeError { ref stage, .. } if stage == "dicom-web"
    ));
}

#[test]
fn audit_event_marks_study_uid_as_sensitive() {
    // REQ-HI-103, REQ-HI-125, REQ-HI-195
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    let auth = WebAuthConfig {
        authorizer: Arc::new(dicom_auth::AllowAll),
        audit: Some(Arc::new(move |event| {
            let mut guard = sink.lock().expect("audit sink lock");
            guard.push(format!("{event:?}"));
            Ok(())
        })),
    };
    let runtime = service(
        WebPolicy::new(TlsPolicy::RequireTls, dicom_web::ThrottleDecision::Allow),
        auth,
    );

    let request = parse_http_request(
        b"GET /studies/1.2.840.100/series HTTP/1.1\r\nx-auth-principal: alice\r\n\r\n",
        &Limits::default(),
        TransportSecurity::Tls,
    )
    .expect("request parse");

    runtime
        .route_request(request)
        .expect("route should pass auth");

    let events = events.lock().expect("events lock");
    assert!(!events.is_empty());
    assert!(events
        .iter()
        .any(|line| line.contains("Sensitive(\"1.2.840.100\")")));
}
