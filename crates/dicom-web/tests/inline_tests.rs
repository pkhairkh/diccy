// Auto-extracted from /home/z/diccy/crates/dicom-web/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


use dicom_web::*;
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_audit::{AuditEvent, AuditValue};
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_auth::{AuthAction, AuthDecision, AuthRequest, Authorizer};
use dicom_auth::{AuthDenyReason, AuthResource, AuthScope, AuthSubject};
use dicom_core::{ErrorKind, Limits, Result};
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_core::Tag;
#[cfg(feature = "qido")]
use dicom_core::{Dataset, Element, Value, Vr};
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use dicom_storage::{IngestOutcome, Storage};
#[cfg(feature = "qido")]
use dicom_web::{TAG_ACCESSION_NUMBER, TAG_MODALITY, TAG_PATIENT_ID, TAG_STUDY_DATE};
#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
use std::sync::{Arc, Mutex};

fn limits() -> Limits {
    Limits::default()
}

fn policy() -> WebPolicy {
    WebPolicy::new(TlsPolicy::AllowInsecure, ThrottleDecision::Allow)
}

fn request(method: HttpMethod, path: &str, body: &[u8]) -> WebRequest {
    WebRequest {
        method,
        transport: TransportSecurity::Insecure,
        path: path.to_string(),
        query: Vec::new(),
        headers: Vec::new(),
        body: body.to_vec(),
    }
}

#[test]
fn default_config_is_secure() {
    // REQ-HTTP-303, REQ-AUTH-300: runtime defaults are secure by default.
    let config = DicomWebServiceConfig::default();
    assert_eq!(config.policy.tls, TlsPolicy::RequireTls);
    let decision = config
        .auth
        .authorizer
        .authorize(&dicom_auth::AuthRequest {
            scope: AuthScope::Dicomweb,
            action: dicom_auth::AuthAction::WebRequest,
            subject: AuthSubject::anonymous(),
            resource: AuthResource::none(),
        })
        .expect("decision");
    assert!(matches!(
        decision,
        dicom_auth::AuthDecision::Deny(AuthDenyReason::Policy)
    ));
}

#[test]
fn parse_http_request_basic() {
    // REQ-HTTP-300: DICOMweb supports GET/HEAD/POST and must parse deterministically.
    let data = b"GET /studies?PatientID=123\nHost: example\n\n";
    let req = parse_http_request(data, &limits(), TransportSecurity::Insecure).expect("parse");
    assert_eq!(req.method, HttpMethod::Get);
    assert_eq!(req.path, "/studies");
    assert_eq!(req.query.len(), 1);
    assert_eq!(req.query[0].key, "PatientID");
}

#[test]
fn parse_http_request_head_method() {
    // REQ-HTTP-300: HEAD requests are accepted for read-only DICOMweb routes.
    let data = b"HEAD /studies\nHost: example\n\n";
    let req = parse_http_request(data, &limits(), TransportSecurity::Insecure).expect("parse");
    assert_eq!(req.method, HttpMethod::Head);
    assert_eq!(req.path, "/studies");
}

#[test]
fn parse_http_rejects_unsupported_method() {
    // REQ-HTTP-300: Unsupported HTTP methods must fail closed.
    let data = b"PUT /studies\n\n";
    let err =
        parse_http_request(data, &limits(), TransportSecurity::Insecure).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn parse_http_rejects_long_query_keys() {
    // REQ-HTTP-301: URI and query parameter lengths are bounded by max_string_bytes.
    let mut limits = Limits::default();
    limits.set_max_string_bytes(4);
    let data = b"GET /studies?ABCDE=1\n\n";
    let err =
        parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
    match err.kind() {
        ErrorKind::LimitExceeded { limit_name, .. } => {
            assert_eq!(*limit_name, "max_string_bytes");
        }
        _ => panic!("expected limit exceeded"),
    }
}

#[test]
fn parse_http_rejects_excess_query_params() {
    // REQ-HTTP-301: Query parameter count is bounded by max_dataset_elements.
    let mut limits = Limits::default();
    limits.set_max_dataset_elements(1);
    let data = b"GET /studies?A=1&B=2\n\n";
    let err =
        parse_http_request(data, &limits, TransportSecurity::Insecure).expect_err("error");
    match err.kind() {
        ErrorKind::LimitExceeded { limit_name, .. } => {
            assert_eq!(*limit_name, "max_dataset_elements");
        }
        _ => panic!("expected limit exceeded"),
    }
}

#[test]
fn get_with_body_rejected() {
    // REQ-HTTP-302: GET/HEAD requests must not include a body.
    let req = request(HttpMethod::Get, "/studies", &[1, 2, 3]);
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn head_with_body_rejected() {
    // REQ-HTTP-302: GET/HEAD requests must not include a body.
    let req = request(HttpMethod::Head, "/studies", &[1, 2, 3]);
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn tls_policy_requires_tls() {
    // REQ-HTTP-303: TLS policy must fail closed on insecure transports.
    let req = request(HttpMethod::Get, "/studies", &[]);
    let policy = WebPolicy::new(TlsPolicy::RequireTls, ThrottleDecision::Allow);
    let err = parse_dicomweb_request(req, &limits(), policy).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn throttle_rejection_returns_limit_exceeded() {
    // REQ-HTTP-304: Throttling decisions must return LimitExceeded on rejection.
    let req = request(HttpMethod::Get, "/studies", &[]);
    let policy = WebPolicy::new(
        TlsPolicy::AllowInsecure,
        ThrottleDecision::Reject {
            limit_name: "max_web_requests_inflight",
            observed: 2,
            allowed: 1,
        },
    );
    let err = parse_dicomweb_request(req, &limits(), policy).expect_err("error");
    match err.kind() {
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        } => {
            assert_eq!(*limit_name, "max_web_requests_inflight");
            assert_eq!(*observed, 2);
            assert_eq!(*allowed, 1);
        }
        _ => panic!("expected limit exceeded"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_studies_parses() {
    // REQ-WEB-300: Supported QIDO-RS endpoints must parse deterministically.
    let req = request(HttpMethod::Get, "/studies", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::QidoStudies { .. } => {}
        _ => panic!("expected QIDO studies"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_studies_head_parses() {
    // REQ-WEB-300: HEAD is accepted for QIDO study queries.
    let req = request(HttpMethod::Head, "/studies", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::QidoStudies { .. } => {}
        _ => panic!("expected QIDO studies"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_all_series_parses() {
    // REQ-WEB-300: global series QIDO endpoint parses deterministically.
    let req = request(HttpMethod::Get, "/series", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::QidoAllSeries { .. } => {}
        _ => panic!("expected QIDO all series"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_all_instances_parses() {
    // REQ-WEB-300: global instances QIDO endpoint parses deterministically.
    let req = request(HttpMethod::Get, "/instances", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::QidoAllInstances { .. } => {}
        _ => panic!("expected QIDO all instances"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_study_instances_parses() {
    // REQ-WEB-300: study-level instance QIDO endpoint parses deterministically.
    let req = request(HttpMethod::Get, "/studies/1.2.3/instances", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::QidoStudyInstances { study_uid, .. } => assert_eq!(study_uid, "1.2.3"),
        _ => panic!("expected QIDO study instances"),
    }
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_study_uid() {
    // REQ-QR-300: only supported UID keys may be used for matching.
    // REQ-QR-301: query results are deterministically ordered by UID.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );
    let mut dataset_other = Dataset::new();
    dataset_other
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    dataset_other
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
    dataset_other.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
    );
    let datasets = vec![dataset, dataset_other];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies".to_string(),
        query: vec![QueryParam {
            key: "StudyInstanceUID".to_string(),
            value: "1.2.3".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_patient_and_modality() {
    // REQ-QR-300: QIDO query mapping supports PatientID and Modality keys.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );
    dataset.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());
    let mut dataset_other = Dataset::new();
    dataset_other
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset_other.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.9".to_string())).unwrap(),
    );
    dataset_other.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.9.1".to_string()),
        )
        .unwrap(),
    );
    dataset_other.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );
    dataset_other
        .insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("MR".to_string())).unwrap());
    let datasets = vec![dataset, dataset_other];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies/1.2.3/series".to_string(),
        query: vec![
            QueryParam {
                key: "PatientID".to_string(),
                value: "PATIENT_A".to_string(),
            },
            QueryParam {
                key: "Modality".to_string(),
                value: "CT".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_accession_and_study_date() {
    // REQ-QR-300: QIDO query mapping supports AccessionNumber and StudyDate keys.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_ACCESSION_NUMBER,
            Vr::Lo,
            Value::Str("ACC123".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20260211".to_string())).unwrap(),
    );

    let mut dataset_other = Dataset::new();
    dataset_other
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    dataset_other
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
    dataset_other.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
    );
    dataset_other.insert(
        Element::new(
            TAG_ACCESSION_NUMBER,
            Vr::Lo,
            Value::Str("ACC999".to_string()),
        )
        .unwrap(),
    );
    dataset_other.insert(
        Element::new(TAG_STUDY_DATE, Vr::Da, Value::Str("20260101".to_string())).unwrap(),
    );
    let datasets = vec![dataset, dataset_other];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies".to_string(),
        query: vec![
            QueryParam {
                key: "AccessionNumber".to_string(),
                value: "ACC123".to_string(),
            },
            QueryParam {
                key: "StudyDate".to_string(),
                value: "20260211".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_study_instances_path() {
    // REQ-WEB-300: /studies/{StudyUID}/instances maps to instance-level queries.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );

    let mut dataset_same_study = Dataset::new();
    dataset_same_study
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset_same_study.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.9".to_string())).unwrap(),
    );
    dataset_same_study.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.9.1".to_string()),
        )
        .unwrap(),
    );

    let mut dataset_other_study = Dataset::new();
    dataset_other_study
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    dataset_other_study
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
    dataset_other_study.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
    );

    let datasets = vec![dataset_same_study, dataset_other_study, dataset];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies/1.2.3/instances".to_string(),
        query: Vec::new(),
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].study_uid, "1.2.3");
    assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
    assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.3.4.5"));
    assert_eq!(matches[1].study_uid, "1.2.3");
    assert_eq!(matches[1].series_uid.as_deref(), Some("1.2.3.9"));
    assert_eq!(matches[1].instance_uid.as_deref(), Some("1.2.3.9.1"));
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_all_series_path() {
    // REQ-WEB-300: /series maps to series-level queries.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("CT".to_string())).unwrap());

    let mut dataset_other = Dataset::new();
    dataset_other
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    dataset_other
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
    dataset_other.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
    );
    dataset_other
        .insert(Element::new(TAG_MODALITY, Vr::Cs, Value::Str("MR".to_string())).unwrap());

    let datasets = vec![dataset_other, dataset];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/series".to_string(),
        query: vec![QueryParam {
            key: "Modality".to_string(),
            value: "CT".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
    assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
    assert_eq!(matches[0].instance_uid, None);
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_matches_all_instances_path() {
    // REQ-WEB-300: /instances maps to instance-level queries.
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.4".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.4.5".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );

    let mut dataset_other = Dataset::new();
    dataset_other
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("9.9".to_string())).unwrap());
    dataset_other
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("9.9.1".to_string())).unwrap());
    dataset_other.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid("9.9.1.1".to_string())).unwrap(),
    );
    dataset_other.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_B".to_string())).unwrap(),
    );

    let datasets = vec![dataset_other, dataset];

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/instances".to_string(),
        query: vec![QueryParam {
            key: "PatientID".to_string(),
            value: "PATIENT_A".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &datasets, &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
    assert_eq!(matches[0].series_uid.as_deref(), Some("1.2.3.4"));
    assert_eq!(matches[0].instance_uid.as_deref(), Some("1.2.3.4.5"));
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_rejects_unsupported_param() {
    // REQ-QR-300: unsupported query keys must fail closed.
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies".to_string(),
        query: vec![QueryParam {
            key: "StudyDescription".to_string(),
            value: "HEAD".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let err = qido_query_matches(&parsed, &[], &limits()).expect_err("expected error");
    assert!(matches!(
        err.kind(),
        ErrorKind::DecodeError { .. } | ErrorKind::InvalidTagValue { .. }
    ));
}

#[cfg(feature = "qido")]
#[test]
fn qido_query_rejects_path_conflict() {
    // REQ-QR-300: conflicting query keys must fail closed.
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies/1.2.3/series".to_string(),
        query: vec![QueryParam {
            key: "StudyInstanceUID".to_string(),
            value: "9.9".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let err = qido_query_matches(&parsed, &[], &limits()).expect_err("expected error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "qido")]
#[test]
fn qido_limit_and_offset_are_applied_deterministically() {
    let mut dataset_a = Dataset::new();
    dataset_a
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset_a.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.1".to_string())).unwrap(),
    );
    dataset_a.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.1.1".to_string()),
        )
        .unwrap(),
    );

    let mut dataset_b = Dataset::new();
    dataset_b
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.4".to_string())).unwrap());
    dataset_b.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.4.1".to_string())).unwrap(),
    );
    dataset_b.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.4.1.1".to_string()),
        )
        .unwrap(),
    );

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies".to_string(),
        query: vec![
            QueryParam {
                key: "offset".to_string(),
                value: "1".to_string(),
            },
            QueryParam {
                key: "limit".to_string(),
                value: "1".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches =
        qido_query_matches(&parsed, &[dataset_b, dataset_a], &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.4");
}

#[cfg(feature = "qido")]
#[test]
fn qido_accepts_normalized_keyword_and_tag_key_forms() {
    let mut dataset = Dataset::new();
    dataset
        .insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid("1.2.3".to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid("1.2.3.1".to_string())).unwrap(),
    );
    dataset.insert(
        Element::new(
            TAG_INSTANCE_UID,
            Vr::Ui,
            Value::Uid("1.2.3.1.1".to_string()),
        )
        .unwrap(),
    );
    dataset.insert(
        Element::new(TAG_PATIENT_ID, Vr::Lo, Value::Str("PATIENT_A".to_string())).unwrap(),
    );

    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies".to_string(),
        query: vec![
            QueryParam {
                key: "study_instance_uid".to_string(),
                value: "1.2.3".to_string(),
            },
            QueryParam {
                key: "(0010,0020)".to_string(),
                value: "PATIENT_A".to_string(),
            },
            QueryParam {
                key: "includefield".to_string(),
                value: "all".to_string(),
            },
            QueryParam {
                key: "fuzzy".to_string(),
                value: "false".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    let matches = qido_query_matches(&parsed, &[dataset], &limits()).expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].study_uid, "1.2.3");
}

#[cfg(not(feature = "qido"))]
#[test]
fn qido_feature_disabled_fails_closed() {
    // REQ-WEB-300: QIDO-RS endpoints fail closed when feature is disabled.
    let req = request(HttpMethod::Get, "/studies", &[]);
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(not(feature = "wado"))]
#[test]
fn wado_feature_disabled_fails_closed() {
    // REQ-WEB-300: WADO-RS endpoints fail closed when feature is disabled.
    let req = request(
        HttpMethod::Get,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5",
        &[],
    );
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn invalid_uid_fails_closed() {
    // REQ-WEB-301: UID segments must be valid UIDs.
    let req = request(
        HttpMethod::Get,
        "/studies/1.2.x/series/1.2.4/instances/1.2.5",
        &[],
    );
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::InvalidTagValue { .. }));
}

#[cfg(feature = "stow")]
#[test]
fn stow_requires_content_type() {
    // REQ-WEB-302: STOW-RS requires a valid content-type.
    let mut req = request(HttpMethod::Post, "/studies", &[1, 2]);
    req.headers.clear();
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn body_limit_enforced() {
    // REQ-WEB-303: Request bodies are bounded by max_input_bytes.
    let mut limits = Limits::default();
    limits.set_max_input_bytes(1);
    let req = request(HttpMethod::Post, "/studies", &[1, 2, 3]);
    let err = parse_dicomweb_request(req, &limits, policy()).expect_err("error");
    match err.kind() {
        ErrorKind::LimitExceeded { limit_name, .. } => {
            assert_eq!(*limit_name, "max_input_bytes");
        }
        _ => panic!("expected limit exceeded"),
    }
}

#[cfg(feature = "wado")]
#[test]
fn wado_rejects_query_params() {
    // REQ-WEB-300: Unsupported query parameters must fail closed.
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies/1.2.3/series/1.2.4/instances/1.2.5".to_string(),
        query: vec![QueryParam {
            key: "unexpected".to_string(),
            value: "x".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn wado_allows_supported_transfer_syntax_query_param() {
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/studies/1.2.3/series/1.2.4/instances/1.2.5".to_string(),
        query: vec![QueryParam {
            key: "transferSyntax".to_string(),
            value: "1.2.840.10008.1.2".to_string(),
        }],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::WadoInstance {
            transfer_syntax_uid,
            ..
        } => {
            assert_eq!(transfer_syntax_uid.as_deref(), Some("1.2.840.10008.1.2"));
        }
        _ => panic!("expected WADO instance"),
    }
}

#[cfg(feature = "wado")]
#[test]
fn wado_uri_compatibility_parses() {
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/wado".to_string(),
        query: vec![
            QueryParam {
                key: "requestType".to_string(),
                value: "WADO".to_string(),
            },
            QueryParam {
                key: "studyUID".to_string(),
                value: "1.2.3".to_string(),
            },
            QueryParam {
                key: "seriesUID".to_string(),
                value: "1.2.4".to_string(),
            },
            QueryParam {
                key: "objectUID".to_string(),
                value: "1.2.5".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::WadoInstance {
            study_uid,
            series_uid,
            instance_uid,
            ..
        } => {
            assert_eq!(study_uid, "1.2.3");
            assert_eq!(series_uid, "1.2.4");
            assert_eq!(instance_uid, "1.2.5");
        }
        _ => panic!("expected WADO instance"),
    }
}

#[cfg(feature = "wado")]
#[test]
fn wado_uri_rejects_unsupported_query_key() {
    let req = WebRequest {
        method: HttpMethod::Get,
        transport: TransportSecurity::Insecure,
        path: "/wado".to_string(),
        query: vec![
            QueryParam {
                key: "requestType".to_string(),
                value: "WADO".to_string(),
            },
            QueryParam {
                key: "studyUID".to_string(),
                value: "1.2.3".to_string(),
            },
            QueryParam {
                key: "seriesUID".to_string(),
                value: "1.2.4".to_string(),
            },
            QueryParam {
                key: "objectUID".to_string(),
                value: "1.2.5".to_string(),
            },
            QueryParam {
                key: "foo".to_string(),
                value: "bar".to_string(),
            },
        ],
        headers: Vec::new(),
        body: Vec::new(),
    };
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn wado_rejects_post_method() {
    // REQ-WEB-300: WADO routes accept GET/HEAD only; POST must fail closed.
    let req = request(
        HttpMethod::Post,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5",
        &[1, 2, 3],
    );
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn study_uid_route_get_parses_as_wado_study_retrieve() {
    // REQ-WEB-300: /studies/{StudyUID} supports WADO retrieve on GET/HEAD.
    let req = request(HttpMethod::Get, "/studies/1.2.3", &[]);
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    assert!(matches!(parsed, DicomWebRequest::WadoStudyRetrieve { .. }));
}

#[cfg(not(feature = "wado"))]
#[test]
fn study_uid_route_rejects_get_when_wado_disabled() {
    let req = request(HttpMethod::Get, "/studies/1.2.3", &[]);
    let err = parse_dicomweb_request(req, &limits(), policy()).expect_err("error");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn wado_head_parses() {
    // REQ-WEB-300: HEAD is accepted for WADO instance routes.
    let req = request(
        HttpMethod::Head,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5",
        &[],
    );
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::WadoInstance { .. } => {}
        _ => panic!("expected WADO instance"),
    }
}

#[cfg(feature = "wado")]
#[test]
fn study_and_series_retrieve_routes_parse_as_wado_requests() {
    let study = parse_dicomweb_request(
        request(HttpMethod::Get, "/studies/1.2.3", &[]),
        &limits(),
        policy(),
    )
    .expect("study retrieve");
    assert!(matches!(study, DicomWebRequest::WadoStudyRetrieve { .. }));

    let series = parse_dicomweb_request(
        request(HttpMethod::Get, "/studies/1.2.3/series/1.2.4", &[]),
        &limits(),
        policy(),
    )
    .expect("series retrieve");
    assert!(matches!(series, DicomWebRequest::WadoSeriesRetrieve { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn metadata_routes_parse_as_wado_requests() {
    let study = parse_dicomweb_request(
        request(HttpMethod::Get, "/studies/1.2.3/metadata", &[]),
        &limits(),
        policy(),
    )
    .expect("study metadata");
    assert!(matches!(study, DicomWebRequest::WadoStudyMetadata { .. }));

    let series = parse_dicomweb_request(
        request(HttpMethod::Get, "/studies/1.2.3/series/1.2.4/metadata", &[]),
        &limits(),
        policy(),
    )
    .expect("series metadata");
    assert!(matches!(series, DicomWebRequest::WadoSeriesMetadata { .. }));

    let instance = parse_dicomweb_request(
        request(
            HttpMethod::Get,
            "/studies/1.2.3/series/1.2.4/instances/1.2.5/metadata",
            &[],
        ),
        &limits(),
        policy(),
    )
    .expect("instance metadata");
    assert!(matches!(
        instance,
        DicomWebRequest::WadoInstanceMetadata { .. }
    ));
}

#[cfg(feature = "wado")]
#[test]
fn rendered_and_bulkdata_routes_parse_with_allowlisted_query() {
    let mut rendered_req = request(
        HttpMethod::Get,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5/rendered",
        &[],
    );
    rendered_req.query = vec![
        QueryParam {
            key: "accept".to_string(),
            value: "image/png".to_string(),
        },
        QueryParam {
            key: "transferSyntax".to_string(),
            value: "1.2.840.10008.1.2.1".to_string(),
        },
    ];
    let rendered =
        parse_dicomweb_request(rendered_req, &limits(), policy()).expect("rendered parse");
    assert!(matches!(rendered, DicomWebRequest::WadoRendered { .. }));

    let mut bulk_req = request(
        HttpMethod::Get,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5/bulkdata",
        &[],
    );
    bulk_req.query = vec![QueryParam {
        key: "accept".to_string(),
        value: "application/octet-stream".to_string(),
    }];
    let bulkdata =
        parse_dicomweb_request(bulk_req, &limits(), policy()).expect("bulkdata parse");
    assert!(matches!(bulkdata, DicomWebRequest::WadoBulkData { .. }));
}

#[cfg(feature = "wado")]
#[test]
fn frame_route_parses_with_frame_number() {
    let req = request(
        HttpMethod::Get,
        "/studies/1.2.3/series/1.2.4/instances/1.2.5/frames/1",
        &[],
    );
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::WadoInstance { frame_number, .. } => {
            assert_eq!(frame_number, Some(1));
        }
        _ => panic!("expected WADO instance"),
    }
}

#[cfg(feature = "stow")]
#[test]
fn stow_parses_multipart_content_type() {
    // REQ-WEB-302: multipart/related requires type=application/dicom and boundary.
    let mut req = request(HttpMethod::Post, "/studies", &[1, 2, 3]);
    req.headers.push(Header {
        name: "content-type".to_string(),
        value: "multipart/related; type=\"application/dicom\"; boundary=abc".to_string(),
    });
    let parsed = parse_dicomweb_request(req, &limits(), policy()).expect("parse");
    match parsed {
        DicomWebRequest::Stow { content_type, .. } => match content_type {
            DicomWebContentType::MultipartRelated { boundary } => {
                assert_eq!(boundary, "abc");
            }
            _ => panic!("expected multipart"),
        },
        _ => panic!("expected STOW"),
    }
}

#[cfg(feature = "stow")]
#[test]
fn stow_accepts_dicom_xml_and_json_content_types() {
    let mut xml_req = request(HttpMethod::Post, "/studies", b"<NativeDicomModel/>");
    xml_req.headers.push(Header {
        name: "content-type".to_string(),
        value: "application/dicom+xml".to_string(),
    });
    let xml_parsed = parse_dicomweb_request(xml_req, &limits(), policy()).expect("xml parse");
    match xml_parsed {
        DicomWebRequest::Stow { content_type, .. } => {
            assert!(matches!(
                content_type,
                DicomWebContentType::ApplicationDicomXml
            ));
        }
        _ => panic!("expected STOW"),
    }

    let mut json_req = request(HttpMethod::Post, "/studies", b"{}");
    json_req.headers.push(Header {
        name: "content-type".to_string(),
        value: "application/dicom+json".to_string(),
    });
    let json_parsed =
        parse_dicomweb_request(json_req, &limits(), policy()).expect("json parse");
    match json_parsed {
        DicomWebRequest::Stow { content_type, .. } => {
            assert!(matches!(
                content_type,
                DicomWebContentType::ApplicationDicomJson
            ));
        }
        _ => panic!("expected STOW"),
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
struct DenyRetrieveAllowOthers;

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
struct RequireHeaderSubject;

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
impl Authorizer for DenyRetrieveAllowOthers {
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
        if request.action == AuthAction::Retrieve {
            return Ok(AuthDecision::Deny(AuthDenyReason::Policy));
        }
        Ok(AuthDecision::Allow)
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
impl Authorizer for RequireHeaderSubject {
    fn authorize(&self, request: &AuthRequest<'_>) -> Result<AuthDecision> {
        if request.subject.principal == Some("alice")
            && request.subject.peer == Some("10.0.0.7")
        {
            return Ok(AuthDecision::Allow);
        }
        Ok(AuthDecision::Deny(AuthDenyReason::Unauthenticated))
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(b"UI");
    let mut bytes = value.as_bytes().to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(&bytes);
    buf
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&tag.0.to_le_bytes());
    buf.extend_from_slice(&tag.1.to_le_bytes());
    buf.extend_from_slice(&vr);
    let mut bytes = value.to_vec();
    if bytes.len() % 2 == 1 {
        bytes.push(0);
    }
    match &vr {
        b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
            buf.extend_from_slice(&0u16.to_le_bytes());
            buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        }
        _ => {
            buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        }
    }
    buf.extend_from_slice(&bytes);
    buf
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn sample_p10(study_uid: &str, series_uid: &str, instance_uid: &str) -> Vec<u8> {
    let mut dataset = Vec::new();
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0016),
        *b"UI",
        b"1.2.840.10008.5.1.4.1.1.7",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0008, 0x0018),
        *b"UI",
        instance_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000D),
        *b"UI",
        study_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0020, 0x000E),
        *b"UI",
        series_uid.as_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0002),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0004),
        *b"CS",
        b"MONOCHROME2",
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0010),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0011),
        *b"US",
        &1u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0100),
        *b"US",
        &16u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0101),
        *b"US",
        &12u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0102),
        *b"US",
        &11u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x0028, 0x0103),
        *b"US",
        &0u16.to_le_bytes(),
    ));
    dataset.extend_from_slice(&dataset_element_explicit(
        Tag(0x7FE0, 0x0010),
        *b"OB",
        &[0u8],
    ));

    let mut bytes = vec![0u8; 128];
    bytes.extend_from_slice(b"DICM");
    bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), "1.2.840.10008.1.2.1"));
    bytes.extend_from_slice(&dataset);
    bytes
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn multipart_body(boundary: &str, payloads: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for payload in payloads {
        out.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        out.extend_from_slice(b"content-type: application/dicom\r\n\r\n");
        out.extend_from_slice(payload);
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    out
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
fn service(
    authorizer: Arc<dyn Authorizer + Send + Sync>,
    events: Arc<Mutex<Vec<AuditEvent>>>,
) -> DicomWebService {
    let config = DicomWebServiceConfig {
        limits: Limits::default(),
        policy: policy(),
        auth: WebAuthConfig {
            authorizer,
            audit: Some(Arc::new(move |event| {
                let mut guard = events.lock().expect("audit lock");
                guard.push(event);
                Ok(())
            })),
        },
        cors: dicom_web::cors::default_dicomweb_cors(),
    };
    DicomWebService::new(config)
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_qido_allows_and_emits_audit() {
    // REQ-WEB-304, REQ-AUTH-300, REQ-AUDIT-350
    let mut storage = Storage::new(Limits::default());
    storage
        .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
        .expect("ingest");
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events.clone());

    let response = service
        .handle_http(
            b"GET /studies?StudyInstanceUID=1.2.3\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("service response");

    match response {
        DicomWebResponse::Qido { matches } => {
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].study_uid, "1.2.3");
        }
        _ => panic!("expected QIDO response"),
    }

    let events = events.lock().expect("audit lock");
    assert_eq!(events.len(), 1);
    assert!(events[0].fields.iter().any(|field| {
        field.key == "action" && matches!(field.value, AuditValue::Plain(ref v) if v == "query")
    }));
    assert!(events[0].fields.iter().any(|field| {
        field.key == "decision"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "allow")
    }));
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_wado_denied_records_audit() {
    // REQ-WEB-304, REQ-AUTH-302, REQ-AUDIT-350
    let mut storage = Storage::new(Limits::default());
    storage
        .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
        .expect("ingest");
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(DenyRetrieveAllowOthers), events.clone());

    let err = service
        .handle_http(
            b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect_err("expected deny");
    assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));

    let events = events.lock().expect("audit lock");
    assert_eq!(events.len(), 1);
    assert!(events[0].fields.iter().any(|field| {
        field.key == "decision"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "deny")
    }));
    assert!(events[0].fields.iter().any(|field| {
        field.key == "deny_reason"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "policy")
    }));
    assert!(events[0].fields.iter().any(|field| {
        field.key == "protocol_path"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "dicomweb")
    }));
    assert!(events[0].fields.iter().any(|field| {
        field.key == "operation"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "wado.instance")
    }));
    assert!(events[0].fields.iter().any(|field| {
        field.key == "event_code"
            && matches!(field.value, AuditValue::Plain(ref v) if v == "auth.deny")
    }));
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_stow_then_wado_round_trip() {
    // REQ-WEB-304, REQ-WEB-305
    let mut storage = Storage::new(Limits::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events);
    let body = sample_p10("1.2.3", "2.3.4", "3.4.5");

    let mut stow_http = b"POST /studies\r\ncontent-type: application/dicom\r\n\r\n".to_vec();
    stow_http.extend_from_slice(&body);
    let stow_response = service
        .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
        .expect("stow response");
    match stow_response {
        DicomWebResponse::Stow { outcomes } => {
            assert_eq!(outcomes.len(), 1);
            assert!(matches!(outcomes[0], IngestOutcome::Inserted { .. }));
        }
        _ => panic!("expected STOW response"),
    }

    let wado_response = service
        .handle_http(
            b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("wado response");
    match wado_response {
        DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, body),
        _ => panic!("expected WADO response"),
    }

    let err = service
        .handle_http(
            b"GET /studies/9.9.9/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect_err("expected not found");
    assert!(matches!(err.kind(), ErrorKind::NotFound { .. }));
    assert_eq!(err.code(), DICOM_WEB_NOT_FOUND_CODE);
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_wado_study_retrieve_returns_multipart() {
    let mut storage = Storage::new(Limits::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events);
    let first = sample_p10("1.2.3", "2.3.4", "3.4.5");
    let second = sample_p10("1.2.3", "2.3.4", "3.4.6");
    storage.ingest_bytes(first.clone()).expect("first ingest");
    storage.ingest_bytes(second.clone()).expect("second ingest");

    let response = service
        .handle_http(
            b"GET /studies/1.2.3\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("study retrieve");

    match response {
        DicomWebResponse::WadoMultipart { media_type, bytes } => {
            assert_eq!(
                media_type,
                "multipart/related; type=\"application/dicom\"; boundary=\"dicomweb-dataset\""
            );
            let body = String::from_utf8_lossy(&bytes);
            assert!(
                body.contains("--dicomweb-dataset\r\ncontent-type: application/dicom\r\n\r\n")
            );
            assert!(bytes
                .windows(first.len())
                .any(|window| window == first.as_slice()));
            assert!(bytes
                .windows(second.len())
                .any(|window| window == second.as_slice()));
            assert!(body.contains("--dicomweb-dataset--\r\n"));
        }
        _ => panic!("expected multipart WADO response"),
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_stow_multipart_ingests_each_part() {
    // REQ-WEB-302, REQ-WEB-305
    let mut storage = Storage::new(Limits::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events);
    let first = sample_p10("1.2.3", "2.3.4", "3.4.5");
    let second = sample_p10("1.2.3", "2.3.4", "3.4.6");
    let boundary = "batch-1";

    let mut stow_http = format!(
        "POST /studies\r\ncontent-type: multipart/related; type=\"application/dicom\"; boundary={boundary}\r\n\r\n"
    )
    .into_bytes();
    stow_http.extend_from_slice(&multipart_body(boundary, &[first.clone(), second.clone()]));

    let response = service
        .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
        .expect("multipart stow");
    match response {
        DicomWebResponse::Stow { outcomes } => {
            assert_eq!(outcomes.len(), 2);
            assert!(matches!(outcomes[0], IngestOutcome::Inserted { .. }));
            assert!(matches!(outcomes[1], IngestOutcome::Inserted { .. }));
        }
        _ => panic!("expected STOW response"),
    }

    let first_wado = service
        .handle_http(
            b"GET /studies/1.2.3/series/2.3.4/instances/3.4.5\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("first wado");
    match first_wado {
        DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, first),
        _ => panic!("expected first WADO"),
    }

    let second_wado = service
        .handle_http(
            b"GET /studies/1.2.3/series/2.3.4/instances/3.4.6\r\nHost: example\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("second wado");
    match second_wado {
        DicomWebResponse::WadoInstance { bytes } => assert_eq!(bytes, second),
        _ => panic!("expected second WADO"),
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_stow_rejects_study_uid_mismatch() {
    // REQ-WEB-305: /studies/{StudyUID} must enforce Study Instance UID consistency.
    let mut storage = Storage::new(Limits::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events);
    let body = sample_p10("9.9.9", "2.3.4", "3.4.5");
    let mut stow_http =
        b"POST /studies/1.2.3\r\ncontent-type: application/dicom\r\n\r\n".to_vec();
    stow_http.extend_from_slice(&body);

    let err = service
        .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
        .expect_err("expected study conflict");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
    assert_eq!(storage.index().total_instances(), 0);
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn request_requires_write_detects_mutating_routes() {
    // REQ-WEB-305: STOW and DELETE mutate storage state.
    let qido = DicomWebRequest::QidoStudies { params: Vec::new() };
    let wado = DicomWebRequest::WadoInstance {
        study_uid: "1.2.3".to_string(),
        series_uid: "2.3.4".to_string(),
        instance_uid: "3.4.5".to_string(),
        transfer_syntax_uid: None,
        frame_number: None,
    };
    let stow = DicomWebRequest::Stow {
        study_uid: None,
        content_type: DicomWebContentType::ApplicationDicom,
        body: vec![0u8; 1],
    };
    let delete_instance = DicomWebRequest::DeleteInstance {
        study_uid: "1.2.3".to_string(),
        series_uid: "2.3.4".to_string(),
        instance_uid: "3.4.5".to_string(),
        hard_delete: false,
    };
    assert!(!request_requires_write(&qido));
    assert!(!request_requires_write(&wado));
    assert!(request_requires_write(&stow));
    assert!(request_requires_write(&delete_instance));
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn execute_routed_read_only_rejects_stow() {
    // REQ-WEB-305: read-only execution path fails closed on mutating requests.
    let storage = Storage::new(Limits::default());
    let service = DicomWebService::new(DicomWebServiceConfig::default());
    let request = DicomWebRequest::Stow {
        study_uid: None,
        content_type: DicomWebContentType::ApplicationDicom,
        body: sample_p10("1.2.3", "2.3.4", "3.4.5"),
    };
    let err = service
        .execute_routed_read_only(&request, &storage)
        .expect_err("expected read-only rejection");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn delete_routes_are_policy_gated_and_soft_delete_by_default() {
    let disabled = parse_dicomweb_request(
        request(HttpMethod::Delete, "/studies/1.2.3", &[]),
        &limits(),
        policy(),
    )
    .expect_err("delete disabled");
    assert!(matches!(disabled.kind(), ErrorKind::DecodeError { .. }));

    let enabled = parse_dicomweb_request(
        request(HttpMethod::Delete, "/studies/1.2.3", &[]),
        &limits(),
        policy().with_delete_enabled(true),
    )
    .expect("delete enabled");
    match enabled {
        DicomWebRequest::DeleteStudy { hard_delete, .. } => assert!(!hard_delete),
        _ => panic!("expected delete study"),
    }
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn write_routes_fail_closed_on_cross_tenant_scope() {
    let mut storage = Storage::new(Limits::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(dicom_auth::AllowAll), events);

    let mut stow_http = b"POST /studies\r\ncontent-type: application/dicom\r\nx-tenant-id: tenant-a\r\nx-tenant-scope: tenant-b\r\n\r\n".to_vec();
    stow_http.extend_from_slice(&sample_p10("1.2.3", "2.3.4", "3.4.5"));

    let err = service
        .handle_http(&stow_http, TransportSecurity::Insecure, &mut storage)
        .expect_err("cross-tenant write should fail");
    assert!(matches!(err.kind(), ErrorKind::AuthorizationDenied { .. }));
    assert_eq!(storage.index().total_instances(), 0);
}

#[cfg(all(feature = "qido", feature = "wado", feature = "stow"))]
#[test]
fn service_uses_header_subject_for_authz() {
    // REQ-AUTH-300: DICOMweb auth context must preserve caller identity when provided.
    let mut storage = Storage::new(Limits::default());
    storage
        .ingest_bytes(sample_p10("1.2.3", "2.3.4", "3.4.5"))
        .expect("ingest");
    let events = Arc::new(Mutex::new(Vec::new()));
    let service = service(Arc::new(RequireHeaderSubject), events);
    let response = service
        .handle_http(
            b"GET /studies?StudyInstanceUID=1.2.3\r\nx-auth-principal: alice\r\nx-forwarded-for: 10.0.0.7, 10.0.0.8\r\n\r\n",
            TransportSecurity::Insecure,
            &mut storage,
        )
        .expect("authorized request");
    match response {
        DicomWebResponse::Qido { matches } => assert_eq!(matches.len(), 1),
        _ => panic!("expected QIDO response"),
    }
}
