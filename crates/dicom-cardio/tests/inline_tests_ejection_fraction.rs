// Auto-extracted from /home/z/diccy/crates/dicom-cardio/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_cardio::*;

    #[test]
    fn ef_from_volumes() {
        // REQ-CARDIO-300: EF% = (EDV - ESV) / EDV * 100
        let result = EjectionFractionResult::from_volumes(120.0, 50.0, Some(72.0));
        assert!((result.ef_pct - 58.33).abs() < 1.0);
        assert!((result.sv_ml - 70.0).abs() < 1e-6);
        assert!(result.cardiac_output_lpm.is_some());
        assert!((result.cardiac_output_lpm.unwrap() - 5.04).abs() < 0.1);
    }

    #[test]
    fn ef_zero_edv() {
        let result = EjectionFractionResult::from_volumes(0.0, 0.0, None);
        assert_eq!(result.ef_pct, 0.0);
    }

    #[test]
    fn simpson_discs_volume() {
        let ed_contours = vec![
            VentricularContour {
                points: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
                slice_index: 0,
                phase: CardiacPhase::EndDiastole,
                area_cm2: 16.0,
            },
            VentricularContour {
                points: vec![(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)],
                slice_index: 1,
                phase: CardiacPhase::EndDiastole,
                area_cm2: 9.0,
            },
        ];

        let es_contours = vec![
            VentricularContour {
                points: vec![(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                slice_index: 0,
                phase: CardiacPhase::EndSystole,
                area_cm2: 4.0,
            },
            VentricularContour {
                points: vec![(0.0, 0.0), (1.5, 0.0), (1.5, 1.5), (0.0, 1.5)],
                slice_index: 1,
                phase: CardiacPhase::EndSystole,
                area_cm2: 2.25,
            },
        ];

        let result =
            EjectionFractionResult::from_simpson_discs(&ed_contours, &es_contours, 1.0, None);

        assert!(result.edv_ml > 0.0);
        assert!(result.esv_ml > 0.0);
        assert!(result.ef_pct > 0.0);
    }

    #[test]
    fn detect_ed_es_frames_test() {
        let areas = vec![10.0, 12.0, 15.0, 14.0, 8.0, 6.0, 7.0, 11.0];
        let (ed_idx, es_idx) = detect_ed_es_frames(&areas);
        assert_eq!(ed_idx, 2); // Max area at index 2
        assert!(es_idx > ed_idx);
    }

    #[test]
    fn polygon_area_calculation() {
        let points = vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let area = polygon_area(&points);
        assert!((area - 16.0).abs() < 1e-6);
    }

    #[test]
    fn polygon_area_triangle() {
        let points = vec![(0.0, 0.0), (3.0, 0.0), (0.0, 4.0)];
        let area = polygon_area(&points);
        assert!((area - 6.0).abs() < 1e-6);
    }

    #[test]
    fn ef_uncertainty_bounds() {
        let result = EjectionFractionResult::from_volumes(120.0, 50.0, None);
        assert_eq!(result.ef_uncertainty, 5.0);
    }
