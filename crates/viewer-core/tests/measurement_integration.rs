use viewer_core::{
    InputEvent, MeasurementKind, MeasurementMode, MeasurementUnit, Modifiers, PointerButton,
    ScreenPoint, ToolMode, ViewerModel, ViewerWarning,
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
fn distance_uses_calibrated_mm_with_provenance() {
    // REQ-MEAS-010, REQ-MEAS-030, REQ-MEAS-070
    let mut model = ViewerModel::new(100, 100);
    model.set_tool_mode(ToolMode::Distance);
    model.set_measurement_mode(MeasurementMode::PhysicalUnits);
    assert!(model.set_pixel_spacing_calibration((0.5, 0.25)));

    model.apply_event(pointer_down(50.0, 50.0));
    model.apply_event(pointer_move(54.0, 58.0));
    model.apply_event(pointer_up(54.0, 58.0));

    let measurement = model.measurements.last().expect("measurement");
    assert_eq!(measurement.kind, MeasurementKind::Distance2D);
    assert_eq!(measurement.unit, MeasurementUnit::Millimeter);
    assert!(measurement.calibrated);
    let tags = measurement
        .provenance
        .as_ref()
        .map(|p| p.tags.clone())
        .expect("provenance tags");
    assert_eq!(tags, vec!["0028,0030".to_string()]);

    let value = measurement.value.expect("value");
    let expected = ((4.0_f64 * 0.5).powi(2) + (8.0_f64 * 0.25).powi(2)).sqrt();
    assert!((value - expected).abs() < 1e-9);
}

#[test]
fn distance_falls_back_to_pixel_when_uncalibrated() {
    // REQ-MEAS-020, REQ-MEAS-050
    let mut model = ViewerModel::new(100, 100);
    model.set_tool_mode(ToolMode::Distance);
    model.set_measurement_mode(MeasurementMode::PhysicalUnits);

    model.apply_event(pointer_down(50.0, 50.0));
    model.apply_event(pointer_move(53.0, 54.0));
    model.apply_event(pointer_up(53.0, 54.0));

    let measurement = model.measurements.last().expect("measurement");
    assert_eq!(measurement.kind, MeasurementKind::Distance2D);
    assert_eq!(measurement.unit, MeasurementUnit::Pixel);
    assert!(!measurement.calibrated);
    assert!(measurement.provenance.is_none());
    assert!(model
        .warnings
        .contains(&ViewerWarning::UncalibratedPhysicalUnits));
}

#[test]
fn angle_commits_after_two_stage_capture() {
    // REQ-MEAS-061
    let mut model = ViewerModel::new(100, 100);
    model.set_tool_mode(ToolMode::Angle);

    model.apply_event(pointer_down(50.0, 50.0));
    model.apply_event(pointer_move(60.0, 50.0));
    model.apply_event(pointer_up(60.0, 50.0));

    model.apply_event(pointer_move(50.0, 60.0));
    model.apply_event(pointer_up(50.0, 60.0));

    let measurement = model.measurements.last().expect("measurement");
    assert_eq!(measurement.kind, MeasurementKind::Angle2D);
    assert_eq!(measurement.unit, MeasurementUnit::Degree);
    let value = measurement.value.expect("angle value");
    assert!((value - 90.0).abs() < 1e-9);
}

#[test]
fn angle_zero_length_segment_fails_closed() {
    // REQ-MEAS-060
    let mut model = ViewerModel::new(100, 100);
    model.set_tool_mode(ToolMode::Angle);

    model.apply_event(pointer_down(50.0, 50.0));
    model.apply_event(pointer_move(60.0, 50.0));
    model.apply_event(pointer_up(60.0, 50.0));

    model.apply_event(pointer_move(50.0, 50.0));
    model.apply_event(pointer_up(50.0, 50.0));

    let measurement = model.measurements.last().expect("measurement");
    assert_eq!(measurement.kind, MeasurementKind::Angle2D);
    assert!(measurement.value.is_none());
    assert!(model
        .warnings
        .contains(&ViewerWarning::InvalidMeasurementInput));
}
