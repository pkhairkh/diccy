// Auto-extracted from /home/z/diccy/crates/viewer-core/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::*;

    #[test]
    fn pixel_center_is_half_offset() {
        // REQ-UI-002
        let center = Viewport2D::pixel_center(0, 0);
        assert_eq!(center, ImagePoint { x: 0.5, y: 0.5 });
    }

    #[test]
    fn measurement_mode_defaults_to_pixel_domain() {
        // REQ-MEAS-001
        assert_eq!(MeasurementMode::default(), MeasurementMode::PixelDomain);
    }

    #[test]
    fn deterministic_event_processing_produces_same_state() {
        // REQ-UI-010, REQ-UI-012
        let mut model_a = ViewerModel::new(800, 600);
        let mut model_b = ViewerModel::new(800, 600);
        let events = [
            InputEvent::PointerDown {
                button: PointerButton::Primary,
                position: ScreenPoint { x: 10.0, y: 10.0 },
                modifiers: Modifiers::default(),
            },
            InputEvent::PointerMove {
                position: ScreenPoint { x: 20.0, y: 20.0 },
                modifiers: Modifiers::default(),
            },
            InputEvent::PointerUp {
                position: ScreenPoint { x: 20.0, y: 20.0 },
                modifiers: Modifiers::default(),
            },
        ];
        for event in events {
            model_a.apply_event(event);
            model_b.apply_event(event);
        }
        assert_eq!(model_a.interaction, model_b.interaction);
        assert_eq!(model_a.viewport, model_b.viewport);
    }

    #[test]
    fn tool_mode_is_explicit() {
        // REQ-UI-011
        let mut model = ViewerModel::new(640, 480);
        model.set_tool_mode(ToolMode::Distance);
        model.apply_event(InputEvent::PointerDown {
            button: PointerButton::Primary,
            position: ScreenPoint { x: 5.0, y: 5.0 },
            modifiers: Modifiers::default(),
        });
        assert!(matches!(
            model.interaction,
            InteractionState::MeasuringDistance { .. }
        ));
    }

    #[test]
    fn rotation_rejects_non_quadrants() {
        // REQ-UI-004
        let mut viewport = Viewport2D::new(100, 100);
        let err = viewport
            .set_rotation_degrees(45)
            .expect_err("reject 45 degrees");
        assert!(format!("{err}").contains("multiple of 90"));
    }

    #[test]
    fn viewport_transform_round_trip() {
        // REQ-UI-003
        let mut viewport = Viewport2D::new(800, 600);
        viewport.center_img = ImagePoint { x: 10.0, y: 20.0 };
        viewport.zoom = 2.0;
        viewport.rotation_quadrants = 0;
        let point = ImagePoint { x: 12.0, y: 23.0 };
        let screen = viewport.image_to_screen(point);
        let round_trip = viewport.screen_to_image(screen);
        let dx = (round_trip.x - point.x).abs();
        let dy = (round_trip.y - point.y).abs();
        assert!(dx < 1e-9 && dy < 1e-9);
    }

    #[test]
    fn angle_degrees_rejects_zero_length_segments() {
        // REQ-MEAS-060
        let vertex = ImagePoint { x: 0.0, y: 0.0 };
        let err = angle_degrees(vertex, vertex, ImagePoint { x: 1.0, y: 0.0 });
        assert!(err.is_none());
    }

    #[test]
    fn angle_degrees_clamps_rounding_overshoot() {
        // REQ-MEAS-061
        let vertex = ImagePoint { x: 0.0, y: 0.0 };
        let b = ImagePoint { x: 1.0, y: 0.0 };
        let c = ImagePoint { x: 1.0, y: 1e-14 };
        let angle = angle_degrees(vertex, b, c).expect("angle");
        assert!(angle.is_finite());
    }

    #[test]
    fn invalid_calibration_is_rejected() {
        // REQ-MEAS-020
        let mut model = ViewerModel::new(32, 32);
        assert!(!model.set_pixel_spacing_calibration((0.0, 1.0)));
        assert!(model.measurement_calibration.is_none());
    }

    #[test]
    fn tri_planar_plane_index_navigation_updates_crosshair() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([64, 32, 16]));
        assert!(model.set_mpr_plane_index(TriPlanarPlane::Axial, 7));
        let state = model.tri_planar_state();
        assert_eq!(state.axial_index, 7);
        assert_eq!(state.crosshair_voxel, [0, 0, 7]);
    }

    #[test]
    fn tri_planar_crosshair_synchronizes_all_views() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([64, 32, 16]));
        assert!(model.set_mpr_crosshair_voxel([11, 12, 13]));
        let state = model.tri_planar_state();
        assert_eq!(state.sagittal_index, 11);
        assert_eq!(state.coronal_index, 12);
        assert_eq!(state.axial_index, 13);
    }

    #[test]
    fn tri_planar_step_clamps_to_bounds() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([4, 3, 2]));
        assert!(model.step_mpr_plane(TriPlanarPlane::Sagittal, 10));
        assert!(!model.step_mpr_plane(TriPlanarPlane::Sagittal, 1));
        let state = model.tri_planar_state();
        assert_eq!(state.sagittal_index, 3);
        assert_eq!(state.crosshair_voxel[0], 3);
    }
