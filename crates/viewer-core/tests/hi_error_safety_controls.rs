use viewer_core::{
    control_availability, destructive_action_allowed, escalation_pathway, incident_correlation_id,
    link_error_to_audit, progression_allowed_for_critical_warning, redact_support_bundle_text,
    runtime_security_banner, validate_clinical_numeric_input, InputEvent, Modifiers, PointerButton,
    ScreenPoint, ToolMode, ViewerModel,
};

fn pointer_down(x: f64, y: f64) -> InputEvent {
    InputEvent::PointerDown {
        button: PointerButton::Primary,
        position: ScreenPoint { x, y },
        modifiers: Modifiers::default(),
    }
}

fn pointer_move(x: f64, y: f64) -> InputEvent {
    InputEvent::PointerMove {
        position: ScreenPoint { x, y },
        modifiers: Modifiers::default(),
    }
}

fn pointer_up(x: f64, y: f64) -> InputEvent {
    InputEvent::PointerUp {
        position: ScreenPoint { x, y },
        modifiers: Modifiers::default(),
    }
}

#[test]
fn unresolved_blocking_errors_prevent_signoff_and_controlled_export() {
    // REQ-HI-184
    let mut model = ViewerModel::new(256, 256);
    model.set_tool_mode(ToolMode::Distance);
    model.apply_event(pointer_down(20.0, 20.0));
    model.apply_event(pointer_move(f64::NAN, 21.0));
    model.apply_event(pointer_up(f64::NAN, 21.0));

    let gate = model.blocking_action_gate();
    assert!(!gate.can_finalize_signoff);
    assert!(!gate.can_controlled_export);
}

#[test]
fn malformed_numeric_values_fail_closed_without_auto_correction() {
    // REQ-HI-185
    assert_eq!(validate_clinical_numeric_input(42.0), Some(42.0));
    assert_eq!(validate_clinical_numeric_input(f64::NAN), None);
    assert_eq!(validate_clinical_numeric_input(f64::INFINITY), None);
}

#[test]
fn disabled_controls_and_error_views_include_rationale_and_audit_links() {
    // REQ-HI-186, REQ-HI-187
    let disabled = control_availability(false, Some("requires calibrated context"));
    assert!(!disabled.enabled);
    assert_eq!(
        disabled.rationale.as_deref(),
        Some("requires calibrated context")
    );

    let link = link_error_to_audit("DVF.HI.WARN.INVALID_MEASUREMENT_INPUT", "AUD-431")
        .expect("audit link");
    assert_eq!(link.audit_event_id, "AUD-431");
}

#[test]
fn support_bundle_and_warning_controls_are_deterministic_and_explicit() {
    // REQ-HI-188, REQ-HI-189, REQ-HI-190, REQ-HI-191
    let redacted = redact_support_bundle_text(
        "uid=1.2.840.100 path=/var/tmp/dicom host=pacs.local:104 reason=timeout",
    );
    assert!(redacted.contains("[redacted]"));
    assert!(!redacted.contains("1.2.840.100"));
    assert!(!redacted.contains("/var/tmp/dicom"));

    let incident = incident_correlation_id("DVF.HI.WARN.INVALID_MEASUREMENT_INPUT", 1_700_100_000);
    assert!(incident.starts_with("INC-"));
    assert!(incident.contains("DVF_HI_WARN_INVALID_MEASUREMENT_INPUT"));

    assert!(!progression_allowed_for_critical_warning(true, false));
    assert!(progression_allowed_for_critical_warning(true, true));

    let escalation = escalation_pathway("supervisor-review", true).expect("pathway");
    assert!(escalation.required);
}

#[test]
fn insecure_runtime_and_destructive_actions_require_explicit_controls() {
    // REQ-HI-192, REQ-HI-193
    let insecure = runtime_security_banner(true);
    assert!(insecure.visible);
    assert!(insecure.persistent);

    assert!(!destructive_action_allowed(false, "delete-case-artifacts"));
    assert!(!destructive_action_allowed(true, ""));
    assert!(destructive_action_allowed(true, "delete-case-artifacts"));
}
