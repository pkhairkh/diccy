// Auto-extracted from /home/z/diccy/crates/modality-mg/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use modality_mg::*;
    use dicom_core::{Error, ErrorKind, Tag};
    const MG_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        MG_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
        Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag,
                detail: detail.into(),
            },
            "invalid tag value",
        )
        .into()
    }

    #[test]
    #[cfg(not(feature = "modality-mg"))]
    fn mg_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(!MgPack::enabled());
        let err = MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "modality-mg")]
    fn mg_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(MgPack::enabled());
        MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).expect("mg pack enabled");
    }

    #[test]
    fn mg_manifest_matches_constants() {
        // REQ-CONF-012: MG pack includes a SOP class manifest.
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in MG_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn mg_manifest_uids_look_like_uids() {
        // REQ-CONF-012: manifest entries must be valid UID-like strings.
        for uid in MG_SOP_CLASS_UIDS {
            if uid.is_empty() || uid.starts_with('.') || uid.ends_with('.') {
                let err = invalid_tag_value(TAG_SOP_CLASS_UID, "invalid UID format");
                assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
            }
            assert!(uid.chars().all(|ch| ch.is_ascii_digit() || ch == '.'));
        }
    }

    #[test]
    fn mg_measurements_gate() {
        // REQ-MEAS-010
        if MgPack::enabled() {
            MgPack::ensure_physical_measurements_enabled().expect("mg pack enabled");
        } else {
            let err = MgPack::ensure_physical_measurements_enabled().unwrap_err();
            assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
        }
    }

    // -----------------------------------------------------------------------
    // S7-T4: Tomosynthesis Tests
    // -----------------------------------------------------------------------

    #[test]
    fn tomo_navigation_slice_navigation() {
        let mut nav = TomoNavigation::new(50, 1.0, 1.0);
        assert_eq!(nav.current_slice, 0);

        nav.next_slice();
        assert_eq!(nav.current_slice, 1);

        nav.go_to_slice(25);
        assert_eq!(nav.current_slice, 25);

        nav.prev_slice();
        assert_eq!(nav.current_slice, 24);
    }

    #[test]
    fn tomo_navigation_bounds_check() {
        let mut nav = TomoNavigation::new(10, 1.0, 1.0);
        nav.go_to_slice(100); // Beyond bounds
        assert_eq!(nav.current_slice, 9); // Clamped to last slice
    }

    #[test]
    fn tomo_cine_bounce() {
        let mut nav = TomoNavigation::new(5, 1.0, 1.0);
        nav.start_cine();
        nav.go_to_slice(4); // Last slice

        nav.advance_cine_frame(); // Should bounce back
        assert_eq!(nav.current_slice, 3);
        assert_eq!(nav.cine, CineState::PlayingBackward);
    }

    #[test]
    fn tomo_z_position() {
        let nav = TomoNavigation::new(100, 1.0, 2.0);
        let mut nav2 = nav.clone();
        nav2.go_to_slice(10);
        assert!((nav2.current_z_mm() - 20.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // S7-T4: CADe Tests
    // -----------------------------------------------------------------------

    #[test]
    fn cade_add_and_retrieve_findings() {
        let mut hook = MammographyCadeHook::new();
        hook.add_finding(MammographyCadeFinding {
            finding_id: "f1".to_string(),
            laterality: BreastLaterality::Left,
            view: MammographyView::CC,
            bounding_box: (100.0, 100.0, 200.0, 200.0),
            confidence: 0.8,
            finding_type: "mass".to_string(),
            birads_category: 4,
        });
        assert_eq!(hook.finding_count(), 1);

        let visible = hook.visible_findings();
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn cade_confidence_filtering() {
        let mut hook = MammographyCadeHook::new();
        hook.add_finding(MammographyCadeFinding {
            finding_id: "f1".to_string(),
            laterality: BreastLaterality::Right,
            view: MammographyView::MLO,
            bounding_box: (50.0, 50.0, 150.0, 150.0),
            confidence: 0.3,
            finding_type: "calcification".to_string(),
            birads_category: 3,
        });

        hook.set_confidence_threshold(0.5);
        let visible = hook.visible_findings();
        assert!(visible.is_empty());
    }

    #[test]
    fn cade_overlay_toggle() {
        let mut hook = MammographyCadeHook::new();
        assert!(hook.is_overlay_visible());
        hook.toggle_overlay();
        assert!(!hook.is_overlay_visible());
    }

    // -----------------------------------------------------------------------
    // S7-T4: Dual-Monitor Hanging Protocol Tests
    // -----------------------------------------------------------------------

    #[test]
    fn dual_monitor_protocol_default() {
        let protocol = DualMonitorHangingProtocol::new();
        assert_eq!(protocol.current_slots.len(), 4); // LCC, LMLO, RCC, RMLO
        assert!(!protocol.prior_display.is_visible());
    }

    #[test]
    fn dual_monitor_protocol_with_priors() {
        let protocol = DualMonitorHangingProtocol::with_priors();
        assert!(protocol.prior_display.is_visible());
        assert_eq!(protocol.prior_slots.len(), 4);
    }

    #[test]
    fn dual_monitor_slots_for_monitor() {
        let protocol = DualMonitorHangingProtocol::new();
        let left_monitor = protocol.slots_for_monitor(0);
        let right_monitor = protocol.slots_for_monitor(1);
        assert_eq!(left_monitor.len(), 2);
        assert_eq!(right_monitor.len(), 2);
    }

    // -----------------------------------------------------------------------
    // S7-T4: MQSA Display Controls Tests
    // -----------------------------------------------------------------------

    #[test]
    fn mqsa_compliance_check() {
        let mut controls = MqsaDisplayControls::new(500.0, 0.5, 10);
        controls.apply_gsdf_calibration();
        assert!(controls.mqsa_compliant);
    }

    #[test]
    fn mqsa_non_compliant_no_gsdf() {
        let mut controls = MqsaDisplayControls::new(500.0, 0.5, 10);
        assert!(!controls.check_mqsa_compliance());
    }

    #[test]
    fn mqsa_luminance_ratio() {
        let controls = MqsaDisplayControls::new(450.0, 1.0, 10);
        assert!((controls.luminance_ratio() - 450.0).abs() < 1e-6);
    }
