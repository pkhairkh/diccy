use dicom_core::ErrorKind;
use dicom_web::{validate_context_tuple, ContextTuple};

#[test]
fn context_tuple_validation_accepts_exact_match() {
    // REQ-HI-133, REQ-HI-134
    let expected = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: Some("1.2.840.0.2".to_string()),
        instance_uid: Some("1.2.840.0.3".to_string()),
        frame_index: Some(4),
    };
    let observed = expected.clone();

    validate_context_tuple(&expected, &observed).expect("tuple should match");
}

#[test]
fn context_tuple_validation_fails_closed_on_uid_mismatch() {
    // REQ-HI-134, REQ-HI-138, REQ-HI-144
    let expected = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: Some("1.2.840.0.2".to_string()),
        instance_uid: Some("1.2.840.0.3".to_string()),
        frame_index: Some(1),
    };
    let observed = ContextTuple {
        study_uid: Some("1.2.840.0.9".to_string()),
        series_uid: Some("1.2.840.0.2".to_string()),
        instance_uid: Some("1.2.840.0.3".to_string()),
        frame_index: Some(1),
    };

    let err = validate_context_tuple(&expected, &observed).expect_err("must fail closed");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}

#[test]
fn context_tuple_validation_requires_expected_frame_index() {
    // REQ-HI-133, REQ-HI-138
    let expected = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: None,
        instance_uid: None,
        frame_index: Some(7),
    };
    let observed = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: None,
        instance_uid: None,
        frame_index: None,
    };

    let err = validate_context_tuple(&expected, &observed).expect_err("frame index required");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}

#[test]
fn context_tuple_validation_allows_unconstrained_fields() {
    // REQ-HI-130, REQ-HI-139
    let expected = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: None,
        instance_uid: None,
        frame_index: None,
    };
    let observed = ContextTuple {
        study_uid: Some("1.2.840.0.1".to_string()),
        series_uid: Some("1.2.840.0.2".to_string()),
        instance_uid: Some("1.2.840.0.3".to_string()),
        frame_index: Some(9),
    };

    validate_context_tuple(&expected, &observed).expect("extra fields are allowed");
}
