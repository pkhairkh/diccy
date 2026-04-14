use viewer_core::{
    InputEvent, MeasurementMode, MeasurementUiState, Modifiers, PointerButton, ScreenPoint,
    ToolMode, ViewerIssueSeverity, ViewerModel,
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

fn emit_distance(model: &mut ViewerModel, start: (f64, f64), end: (f64, f64)) {
    model.apply_event(pointer_down(start.0, start.1));
    model.apply_event(pointer_move(end.0, end.1));
    model.apply_event(pointer_up(end.0, end.1));
}

#[test]
fn measurement_ui_state_is_explicit_for_calibration_gating() {
    // REQ-HI-140, REQ-HI-152, REQ-HI-153
    let mut model = ViewerModel::new(512, 512);

    assert_eq!(
        model.measurement_ui_state(),
        MeasurementUiState::PixelDomainOnly
    );

    model.set_measurement_mode(MeasurementMode::PhysicalUnits);
    assert_eq!(
        model.measurement_ui_state(),
        MeasurementUiState::PhysicalUnitsBlocked
    );

    assert!(!model.set_pixel_spacing_calibration((0.0, 0.5)));
    assert_eq!(
        model.measurement_ui_state(),
        MeasurementUiState::PhysicalUnitsBlocked
    );

    assert!(model.set_pixel_spacing_calibration((0.5, 0.5)));
    assert_eq!(
        model.measurement_ui_state(),
        MeasurementUiState::PhysicalUnitsReady
    );

    model.clear_measurement_calibration();
    assert_eq!(
        model.measurement_ui_state(),
        MeasurementUiState::PhysicalUnitsBlocked
    );
}

#[test]
fn rendered_issues_are_stable_for_identical_warning_sets() {
    // REQ-HI-180, REQ-HI-181, REQ-HI-182, REQ-HI-183, REQ-HI-194
    let mut model_a = ViewerModel::new(256, 256);
    let mut model_b = ViewerModel::new(256, 256);
    model_a.set_tool_mode(ToolMode::Distance);
    model_b.set_tool_mode(ToolMode::Distance);

    emit_distance(&mut model_a, (10.0, 10.0), (f64::NAN, 50.0));
    model_a.set_measurement_mode(MeasurementMode::PhysicalUnits);
    emit_distance(&mut model_a, (10.0, 10.0), (15.0, 20.0));

    model_b.set_measurement_mode(MeasurementMode::PhysicalUnits);
    emit_distance(&mut model_b, (10.0, 10.0), (15.0, 20.0));
    model_b.set_measurement_mode(MeasurementMode::PixelDomain);
    emit_distance(&mut model_b, (10.0, 10.0), (f64::NAN, 50.0));

    assert_ne!(model_a.warnings, model_b.warnings);

    let issues_a = model_a.rendered_issues();
    let issues_b = model_b.rendered_issues();
    assert_eq!(issues_a, issues_b);

    assert_eq!(issues_a.len(), 2);
    assert_eq!(issues_a[0].code, "DVF.HI.WARN.INVALID_MEASUREMENT_INPUT");
    assert_eq!(issues_a[0].severity, ViewerIssueSeverity::Blocking);
    assert_eq!(issues_a[1].code, "DVF.HI.WARN.UNCALIBRATED_PHYSICAL_UNITS");
    assert_eq!(issues_a[1].severity, ViewerIssueSeverity::Warning);

    for issue in issues_a {
        assert!(!issue.message.is_empty());
        assert!(!issue.guidance.is_empty());
    }
}
