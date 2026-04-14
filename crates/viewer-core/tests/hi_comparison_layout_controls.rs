use viewer_core::{
    apply_hanging_protocol_template, evaluate_side_by_side_sync, validate_prior_study_alignment,
    HangingProtocolTemplate, StudySeriesContext, ViewerModel,
};

#[test]
fn side_by_side_comparison_supports_pan_zoom_and_frame_lock() {
    // REQ-HI-162
    let mut left = ViewerModel::new(512, 512);
    let mut right = ViewerModel::new(512, 512);
    left.viewport.center_img.x = 40.0;
    left.viewport.center_img.y = 60.0;
    left.viewport.zoom = 1.5;
    right.viewport.center_img = left.viewport.center_img;
    right.viewport.zoom = left.viewport.zoom;

    let sync = evaluate_side_by_side_sync(&left.viewport, &right.viewport, 4, 4, true);
    assert!(sync.pan_zoom_locked);
    assert!(sync.frame_locked);
    assert!(sync.synchronized);
}

#[test]
fn hanging_protocol_templates_apply_deterministically_with_explicit_fallback() {
    // REQ-HI-163
    let templates = vec![
        HangingProtocolTemplate {
            template_id: "chest-2x1".to_string(),
            rows: 1,
            columns: 2,
        },
        HangingProtocolTemplate {
            template_id: "default-1x1".to_string(),
            rows: 1,
            columns: 1,
        },
    ];

    let direct = apply_hanging_protocol_template(&templates, "chest-2x1").expect("direct");
    assert_eq!(direct.applied_template_id, "chest-2x1");
    assert!(!direct.used_fallback);

    let fallback =
        apply_hanging_protocol_template(&templates, "missing-template").expect("fallback");
    assert_eq!(fallback.applied_template_id, "chest-2x1");
    assert!(fallback.used_fallback);
}

#[test]
fn prior_study_links_require_context_alignment_before_sync() {
    // REQ-HI-164
    let active = StudySeriesContext {
        study_uid: "1.2.840.30".to_string(),
        series_uid: "1.2.840.30.1".to_string(),
    };
    let prior = StudySeriesContext {
        study_uid: "1.2.840.29".to_string(),
        series_uid: "1.2.840.30.1".to_string(),
    };
    assert!(validate_prior_study_alignment(&active, &prior));

    let mismatched = StudySeriesContext {
        study_uid: "1.2.840.29".to_string(),
        series_uid: "1.2.840.88.1".to_string(),
    };
    assert!(!validate_prior_study_alignment(&active, &mismatched));
}
