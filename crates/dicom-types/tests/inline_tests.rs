// Auto-extracted from /home/z/diccy/crates/dicom-types/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_types::*;

    #[test]
    fn window_level_explicit() {
        let wl = WindowLevel::explicit(40.0, 400.0);
        assert_eq!(wl.center(), Some(40.0));
        assert_eq!(wl.width(), Some(400.0));
        assert!(!wl.is_auto());
    }

    #[test]
    fn window_level_auto() {
        let wl = WindowLevel::auto_window();
        assert!(wl.is_auto());
        assert!(wl.center().is_none());
        assert_eq!(wl, WindowLevel::default());
    }

    #[test]
    fn window_level_voi_lut() {
        let wl = WindowLevel::voi_lut(2);
        assert!(!wl.is_auto());
        assert!(wl.center().is_none());
    }

    #[test]
    fn patient_position_round_trip() {
        for pos in [
            PatientPosition::HFP,
            PatientPosition::HFS,
            PatientPosition::HFDL,
            PatientPosition::HFDR,
            PatientPosition::FFP,
            PatientPosition::FFS,
            PatientPosition::FFDL,
            PatientPosition::FFDR,
        ] {
            assert_eq!(
                PatientPosition::from_dicom_str(pos.to_dicom_str()),
                Some(pos)
            );
        }
    }

    #[test]
    fn patient_position_predicates() {
        assert!(PatientPosition::HFS.is_head_first());
        assert!(PatientPosition::FFS.is_feet_first());
        assert!(PatientPosition::HFS.is_supine());
        assert!(PatientPosition::HFP.is_prone());
        assert!(PatientPosition::HFDL.is_decubitus());
        assert!(!PatientPosition::FFS.is_head_first());
    }

    #[test]
    fn patient_position_unknown_string() {
        assert_eq!(PatientPosition::from_dicom_str("UNKNOWN"), None);
    }
