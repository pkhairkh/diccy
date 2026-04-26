use dicom_core::{Dataset, Element, ErrorKind, Tag, Value, Vr};
use dicom_visualizer::{
    deterministic_import_diff, deterministic_measurement_ids, evaluate_bulk_export_state,
    evaluate_derived_object_layer, extract_export_context, finalize_report,
    validate_export_authorization, validate_export_plan, validate_import_acceptance,
    validate_remote_send_status, validate_report_template_record, BulkExportState,
    DataClassification, DerivedObjectKind, DerivedObjectLayerDecision, ExportAuthorizationRequest,
    ExportContextTuple, ExportPlan, ExportPreview, MeasurementBinding, RemoteSendItemStatus,
    ReportFinalizationRequest, ReportTemplateRecord,
};

const TAG_STUDY_UID: Tag = Tag(0x0020, 0x000D);
const TAG_SERIES_UID: Tag = Tag(0x0020, 0x000E);
const TAG_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);

fn dataset_with_context(study_uid: &str, series_uid: &str, instance_uid: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study_uid.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series_uid.to_string()),
    ).unwrap());
    dataset.insert(Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid(instance_uid.to_string()),
    ).unwrap());
    dataset
}

#[test]
fn extract_export_context_requires_source_object_ids() {
    // REQ-HI-169, REQ-HI-173
    let dataset = dataset_with_context("1.2.840.10", "1.2.840.10.1", "1.2.840.10.1.1");
    let context = extract_export_context(&dataset).expect("context extraction");

    assert_eq!(context.study_uid, "1.2.840.10");
    assert_eq!(context.series_uid, "1.2.840.10.1");
    assert_eq!(context.instance_uid, "1.2.840.10.1.1");
    assert_eq!(context.frame_index, Some(0));
}

#[test]
fn export_plan_rejects_mismatched_measurement_source_tuple() {
    // REQ-HI-133, REQ-HI-134, REQ-HI-169, REQ-HI-174
    let active = extract_export_context(&dataset_with_context(
        "1.2.840.11",
        "1.2.840.11.1",
        "1.2.840.11.1.1",
    ))
    .expect("active context");

    let mismatched = ExportContextTuple {
        study_uid: "1.2.840.99".to_string(),
        series_uid: "1.2.840.11.1".to_string(),
        instance_uid: "1.2.840.11.1.1".to_string(),
        frame_index: Some(0),
    };

    let plan = ExportPlan {
        active_context: active,
        bindings: vec![MeasurementBinding {
            measurement_id: "m-42".to_string(),
            source_context: mismatched,
        }],
    };

    let err = validate_export_plan(&plan).expect_err("tuple mismatch must fail closed");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}

#[test]
fn export_plan_rejects_empty_measurement_identifier() {
    // REQ-HI-167, REQ-HI-169
    let active = extract_export_context(&dataset_with_context(
        "1.2.840.12",
        "1.2.840.12.1",
        "1.2.840.12.1.1",
    ))
    .expect("active context");

    let plan = ExportPlan {
        active_context: active.clone(),
        bindings: vec![MeasurementBinding {
            measurement_id: "".to_string(),
            source_context: active,
        }],
    };

    let err = validate_export_plan(&plan).expect_err("empty measurement id must fail");
    assert!(matches!(err.kind(), ErrorKind::DecodeError { .. }));
}

#[test]
fn deterministic_measurement_ids_are_sorted_and_deduplicated() {
    // REQ-HI-173, REQ-HI-175
    let source = ExportContextTuple {
        study_uid: "1.2.840.13".to_string(),
        series_uid: "1.2.840.13.1".to_string(),
        instance_uid: "1.2.840.13.1.1".to_string(),
        frame_index: Some(0),
    };

    let ids = deterministic_measurement_ids(&[
        MeasurementBinding {
            measurement_id: "m-2".to_string(),
            source_context: source.clone(),
        },
        MeasurementBinding {
            measurement_id: "m-1".to_string(),
            source_context: source.clone(),
        },
        MeasurementBinding {
            measurement_id: "m-2".to_string(),
            source_context: source,
        },
    ]);

    assert_eq!(ids, vec!["m-1".to_string(), "m-2".to_string()]);
}

#[test]
fn derived_object_layers_require_provenance_and_fail_closed_for_unsupported_content() {
    // REQ-HI-165, REQ-HI-166
    let source = ExportContextTuple {
        study_uid: "1.2.840.20".to_string(),
        series_uid: "1.2.840.20.1".to_string(),
        instance_uid: "1.2.840.20.1.1".to_string(),
        frame_index: Some(0),
    };
    let supported =
        evaluate_derived_object_layer(DerivedObjectKind::Seg, source.clone(), "prov-seg-01")
            .expect("supported layer");
    assert!(matches!(
        supported,
        DerivedObjectLayerDecision::Supported(_)
    ));

    let blocked = evaluate_derived_object_layer(
        DerivedObjectKind::Unsupported("private-overlay".to_string()),
        source,
        "prov-private-01",
    )
    .expect("unsupported decision");
    assert!(matches!(
        blocked,
        DerivedObjectLayerDecision::UnsupportedBlocked { .. }
    ));
}

#[test]
fn report_finalization_requires_signer_timestamp_and_version() {
    // REQ-HI-168
    let finalized = finalize_report(&ReportFinalizationRequest {
        report_id: "report-1".to_string(),
        signer_identity: "radiologist-7".to_string(),
        finalized_epoch_secs: 1_700_005_000,
        application_version: "1.2.3".to_string(),
    })
    .expect("finalized report");
    assert_eq!(finalized.report_id, "report-1");
    assert_eq!(finalized.signer_identity, "radiologist-7");
    assert_eq!(finalized.application_version, "1.2.3");
}

#[test]
fn export_authorization_enforces_classification_and_policy_controls() {
    // REQ-HI-170, REQ-HI-171, REQ-HI-172
    validate_export_authorization(&ExportAuthorizationRequest {
        preview: ExportPreview {
            classification: DataClassification::DeIdentified,
            deidentification_profile: Some("basic-profile".to_string()),
        },
        has_elevated_privilege: false,
        operator_confirmed: false,
    })
    .expect("de-identified export profile");

    validate_export_authorization(&ExportAuthorizationRequest {
        preview: ExportPreview {
            classification: DataClassification::Identified,
            deidentification_profile: None,
        },
        has_elevated_privilege: false,
        operator_confirmed: true,
    })
    .expect_err("identified export requires elevated privilege");
}

#[test]
fn external_import_requires_deterministic_diff_review_before_acceptance() {
    // REQ-HI-176
    let diff = deterministic_import_diff(
        &[
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
        ],
        &[
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "4".to_string()),
        ],
    );
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].key, "B");
    validate_import_acceptance(false, &diff).expect_err("review required");
    validate_import_acceptance(true, &diff).expect("explicit review accepted");
}

#[test]
fn template_bulk_and_remote_status_controls_are_traceable() {
    // REQ-HI-177, REQ-HI-178, REQ-HI-179
    validate_report_template_record(&ReportTemplateRecord {
        template_id: "template-chest".to_string(),
        version: "2026.02".to_string(),
        approved_by: "qa-2".to_string(),
        checksum: "sha256:cafe".to_string(),
    })
    .expect("template metadata");

    evaluate_bulk_export_state(&BulkExportState {
        total_items: 3,
        completed_items: 3,
        retries: 1,
        max_retries: 3,
        cancellation_requested: false,
    })
    .expect("bulk export state");

    validate_remote_send_status(&[
        RemoteSendItemStatus {
            item_id: "exp-1".to_string(),
            endpoint: "dicom://pacs-a".to_string(),
            transport_mode: "TLS".to_string(),
            delivered: true,
        },
        RemoteSendItemStatus {
            item_id: "exp-2".to_string(),
            endpoint: "dicom://pacs-a".to_string(),
            transport_mode: "TLS".to_string(),
            delivered: false,
        },
    ])
    .expect("remote send statuses");
}
