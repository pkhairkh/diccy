use viewer_core::{
    ImagePoint, InputEvent, MeasurementMode, MeasurementUncertainty, Modifiers, PointerButton,
    ScreenPoint, ToolMode, ViewerModel, WindowLevelState,
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
fn rotation_policy_blocks_arbitrary_angles_and_reset_is_one_step() {
    // REQ-HI-146, REQ-HI-147
    let mut model = ViewerModel::new(512, 512);
    assert!(model.viewport.set_rotation_degrees(90).is_ok());
    model.viewport.flip_x = true;
    model.viewport.flip_y = true;

    assert!(model.viewport.set_rotation_degrees(45).is_err());
    model.reset_to_canonical_orientation();
    assert_eq!(model.viewport.rotation_quadrants, 0);
    assert!(!model.viewport.flip_x);
    assert!(!model.viewport.flip_y);
}

#[test]
fn interpretation_layout_and_probe_readout_keep_context_visible() {
    // REQ-HI-148, REQ-HI-149
    let model = ViewerModel::new(256, 256);
    let indicators = model.interpretation_layout_indicators();
    assert!(indicators.orientation_markers_visible);
    assert!(indicators.scale_rulers_visible);

    let readout = model.probe_readout(
        ImagePoint { x: 12.5, y: 44.5 },
        Some(37.0),
        Some("HU"),
        "CT",
    );
    assert_eq!(readout.coordinate, ImagePoint { x: 12.5, y: 44.5 });
    assert_eq!(readout.value, Some(37.0));
    assert_eq!(readout.units.as_deref(), Some("HU"));
    assert_eq!(readout.modality, "CT");
}

#[test]
fn windowing_controls_expose_current_center_and_width() {
    // REQ-HI-151
    let mut model = ViewerModel::new(256, 256);
    let auto = model.window_level_display();
    assert_eq!(auto.preset, "auto");
    assert_eq!(auto.center, 0.0);
    assert_eq!(auto.width, 1.0);

    model.viewport.window_level = WindowLevelState::Explicit {
        center: 15.25,
        width: 340.5,
    };
    let explicit = model.window_level_display();
    assert_eq!(explicit.preset, "manual");
    assert_eq!(explicit.center, 15.25);
    assert_eq!(explicit.width, 340.5);
}

#[test]
fn quantitative_banner_carries_disclaimer_and_uncertainty() {
    // REQ-HI-109, REQ-HI-154
    let mut model = ViewerModel::new(256, 256);
    model.set_measurement_mode(MeasurementMode::PhysicalUnits);
    model.set_measurement_uncertainty(Some(MeasurementUncertainty {
        absolute_mm: Some(0.2),
        relative_fraction: Some(0.05),
        model_id: Some("ISO-GUM".to_string()),
    }));
    model.set_pixel_spacing_calibration((0.5, 0.5));

    let banner = model.quantitative_context_banner();
    assert_eq!(banner.mode, MeasurementMode::PhysicalUnits);
    assert!(banner.calibrated);
    assert!(banner.disclaimer.to_lowercase().contains("uncertainty"));
    assert!(banner.uncertainty.is_some());
}

#[test]
fn non_finite_measurements_fail_closed_with_edit_history() {
    // REQ-HI-155, REQ-HI-156
    let mut model = ViewerModel::new(256, 256);
    model.set_tool_mode(ToolMode::Distance);
    model.apply_event(pointer_down(30.0, 30.0));
    model.apply_event(pointer_move(f64::NAN, 31.0));
    model.apply_event(pointer_up(f64::NAN, 31.0));

    let measurement = model.measurements.last().expect("measurement");
    assert_eq!(measurement.id, "m0");
    assert!(measurement.value.is_none());
    assert_eq!(measurement.edit_history.len(), 1);
    assert_eq!(measurement.edit_history[0].revision, 0);
    assert_eq!(measurement.edit_history[0].action, "created");
}
