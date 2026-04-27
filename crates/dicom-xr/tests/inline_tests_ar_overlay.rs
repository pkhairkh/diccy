// Auto-extracted from /home/z/diccy/crates/dicom-xr/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_xr::*;

    #[test]
    fn coordinate_registration_identity() {
        let reg = CoordinateRegistration::identity();
        let patient = [100.0, 200.0, 300.0];
        let world = reg.patient_to_world(patient);
        assert!((world[0] - 0.1).abs() < 1e-10);
        assert!((world[1] - 0.2).abs() < 1e-10);
        assert!((world[2] - 0.3).abs() < 1e-10);
    }

    #[test]
    fn coordinate_registration_roundtrip() {
        let reg = CoordinateRegistration::lps_to_stage();
        let patient = [50.0, -30.0, 100.0];
        let world = reg.patient_to_world(patient);
        let back = reg.world_to_patient(world);
        assert!((back[0] - patient[0]).abs() < 1e-8);
        assert!((back[1] - patient[1]).abs() < 1e-8);
        assert!((back[2] - patient[2]).abs() < 1e-8);
    }

    #[test]
    fn coordinate_registration_lps_to_stage() {
        let reg = CoordinateRegistration::lps_to_stage();
        // Patient LPS (100, 0, 0) → world (-0.1, 0, 0)
        let world = reg.patient_to_world([100.0, 0.0, 0.0]);
        assert!((world[0] - (-0.1)).abs() < 1e-10);
        assert!((world[1] - 0.0).abs() < 1e-10);
        assert!((world[2] - 0.0).abs() < 1e-10);

        // Patient LPS (0, 0, 100) → world (0, 0.1, 0) (Superior → Up)
        let world2 = reg.patient_to_world([0.0, 0.0, 100.0]);
        assert!((world2[0] - 0.0).abs() < 1e-10);
        assert!((world2[1] - 0.1).abs() < 1e-10);
        assert!((world2[2] - 0.0).abs() < 1e-10);
    }

    #[test]
    fn coordinate_registration_validation() {
        let valid = CoordinateRegistration::default();
        assert!(valid.validate().is_ok());

        let bad_scale = CoordinateRegistration {
            scale: -1.0,
            ..Default::default()
        };
        assert!(bad_scale.validate().is_err());
    }

    #[test]
    fn ar_overlay_session_creation() {
        let session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        assert_eq!(session.platform, ArPlatform::HoloLens2);
        assert!(!session.visible);
        assert!((session.opacity - 0.7).abs() < 1e-10);
    }

    #[test]
    fn ar_overlay_add_fiducial() {
        let mut session = ArOverlaySession::new(ArPlatform::VisionPro, "1.2.3");
        let id = session.add_fiducial([10.0, 20.0, 30.0], [0.01, 0.02, 0.03]);
        assert_eq!(id, "marker0");
        assert_eq!(session.markers.len(), 1);
        assert_eq!(session.markers[0].marker_type, MarkerType::Fiducial);
    }

    #[test]
    fn ar_overlay_add_instrument() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        let id = session.add_instrument("scalpel", [0.5, 1.0, 0.5]);
        assert!(id.starts_with("inst"));
        assert_eq!(session.markers.len(), 1);
        assert_eq!(session.markers[0].marker_type, MarkerType::Instrument);
    }

    #[test]
    fn ar_overlay_show_hide() {
        let mut session = ArOverlaySession::new(ArPlatform::Generic, "1.2.3");
        assert!(!session.visible);
        session.show();
        assert!(session.visible);
        session.hide();
        assert!(!session.visible);
    }

    #[test]
    fn ar_overlay_opacity_validation() {
        let mut session = ArOverlaySession::new(ArPlatform::Generic, "1.2.3");
        assert!(session.set_opacity(0.5).is_ok());
        assert!(session.set_opacity(0.0).is_ok());
        assert!(session.set_opacity(1.0).is_ok());
        assert!(session.set_opacity(-0.1).is_err());
        assert!(session.set_opacity(1.1).is_err());
    }

    #[test]
    fn ar_overlay_registration_from_fiducials() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");

        // Add 3 non-collinear fiducials
        session.add_fiducial([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        session.add_fiducial([100.0, 0.0, 0.0], [-0.1, 0.0, 0.0]);
        session.add_fiducial([0.0, 100.0, 0.0], [0.0, 0.0, 0.1]);

        let result = session
            .compute_registration_from_fiducials()
            .expect("registration");
        assert!(result.rms_error_mm >= 0.0);
        assert_eq!(result.fiducial_count, 3);
        assert!(session.registration.verified);
    }

    #[test]
    fn ar_overlay_registration_requires_3_fiducials() {
        let mut session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        session.add_fiducial([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        session.add_fiducial([100.0, 0.0, 0.0], [-0.1, 0.0, 0.0]);
        // Only 2 fiducials — should fail
        assert!(session.compute_registration_from_fiducials().is_err());
    }

    #[test]
    fn ar_overlay_instrument_navigation() {
        let mut session = ArOverlaySession::new(ArPlatform::VisionPro, "1.2.3");
        let inst_id = session.add_instrument("probe", [0.5, 1.0, 0.5]);
        let pos = session.instrument_patient_position(&inst_id);
        assert!(pos.is_some());
        // Patient position should be computed from world position via inverse registration
        let p = pos.unwrap();
        // All values should be finite
        assert!(p[0].is_finite());
        assert!(p[1].is_finite());
        assert!(p[2].is_finite());
    }
