use dicom_web::{
    evaluate_cache_freshness, evaluate_context_switch, render_workflow_timestamp,
    CacheFreshnessStatus, ContextSwitchDecision,
};

#[test]
fn mixed_patient_context_is_blocked_and_unsaved_changes_require_confirmation() {
    // REQ-HI-131, REQ-HI-132
    let blocked = evaluate_context_switch(Some("PAT-1"), Some("PAT-2"), false, true);
    assert_eq!(blocked, ContextSwitchDecision::BlockMixedPatient);

    let needs_confirm = evaluate_context_switch(Some("PAT-1"), Some("PAT-1"), true, false);
    assert_eq!(needs_confirm, ContextSwitchDecision::RequireConfirmation);

    let allowed = evaluate_context_switch(Some("PAT-1"), Some("PAT-1"), true, true);
    assert_eq!(allowed, ContextSwitchDecision::Allow);
}

#[test]
fn cache_freshness_is_explicit_for_reload_paths() {
    // REQ-HI-142
    assert_eq!(
        evaluate_cache_freshness(Some(1_000), Some(1_050)),
        CacheFreshnessStatus::Fresh
    );
    assert_eq!(
        evaluate_cache_freshness(Some(1_050), Some(1_000)),
        CacheFreshnessStatus::Stale
    );
    assert_eq!(
        evaluate_cache_freshness(Some(1_050), None),
        CacheFreshnessStatus::Unknown
    );
}

#[test]
fn workflow_timestamps_include_timezone_context() {
    // REQ-HI-143
    let rendered = render_workflow_timestamp("2026-02-14T10:30:00Z", -300).expect("timestamp");
    assert!(rendered.rendered.contains("UTC-05:00"));
    assert!(rendered.rendered.contains("2026-02-14T10:30:00Z"));
}
