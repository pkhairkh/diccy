use viewer_core::{
    ImagePoint, InputEvent, Modifiers, PointerButton, ScreenPoint, ViewerModel, WindowLevelState,
};

fn pointer_down_primary(x: f64, y: f64) -> InputEvent {
    InputEvent::PointerDown {
        button: PointerButton::Primary,
        position: ScreenPoint { x, y },
        modifiers: Modifiers::default(),
    }
}

fn pointer_down_secondary(x: f64, y: f64) -> InputEvent {
    InputEvent::PointerDown {
        button: PointerButton::Secondary,
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
fn replay_is_deterministic_for_pan_zoom_window_and_frame_navigation() {
    // REQ-HI-145, REQ-HI-150, REQ-HI-157, REQ-HI-158, REQ-HI-244
    // REQ-UI-010, REQ-UI-012
    let mut model_a = ViewerModel::new(1024, 768);
    let mut model_b = ViewerModel::new(1024, 768);

    assert!(model_a.set_total_frames(20));
    assert!(model_b.set_total_frames(20));

    let event_stream = [
        pointer_down_primary(100.0, 120.0),
        pointer_move(170.0, 200.0),
        pointer_up(170.0, 200.0),
        pointer_down_secondary(420.0, 360.0),
        pointer_move(440.0, 330.0),
        pointer_up(440.0, 330.0),
        InputEvent::Wheel {
            delta: 120.0,
            position: ScreenPoint { x: 512.0, y: 384.0 },
            modifiers: Modifiers::default(),
        },
        InputEvent::Wheel {
            delta: -15.0,
            position: ScreenPoint { x: 512.0, y: 384.0 },
            modifiers: Modifiers::default(),
        },
        InputEvent::EventComplete,
    ];

    for event in event_stream {
        model_a.apply_event(event);
        model_b.apply_event(event);
    }

    for delta in [1, 1, 5, -3, 100, -100, 7] {
        let changed_a = model_a.step_frame(delta);
        let changed_b = model_b.step_frame(delta);
        assert_eq!(changed_a, changed_b);
    }

    assert_eq!(model_a.viewport, model_b.viewport);
    assert_eq!(model_a.interaction, model_b.interaction);
    assert_eq!(model_a.frame_navigation, model_b.frame_navigation);
    assert_eq!(model_a.frame_navigation.frame_index, 7);
    assert_ne!(
        model_a.viewport.center_img,
        ImagePoint { x: 0.0, y: 0.0 },
        "pan replay should move viewport center"
    );
    assert!(matches!(
        model_a.viewport.window_level,
        WindowLevelState::Explicit { .. }
    ));
}

#[test]
fn frame_navigation_clamps_and_rejects_invalid_configuration() {
    // REQ-HI-157, REQ-HI-244
    let mut model = ViewerModel::new(256, 256);

    assert!(!model.set_total_frames(0));
    assert!(model.set_total_frames(4));
    assert!(!model.set_frame_index(4));
    assert!(model.set_frame_index(3));

    assert!(!model.step_frame(1));
    assert_eq!(model.frame_navigation.frame_index, 3);

    assert!(model.step_frame(-2));
    assert_eq!(model.frame_navigation.frame_index, 1);

    assert!(model.step_frame(-100));
    assert_eq!(model.frame_navigation.frame_index, 0);
}
