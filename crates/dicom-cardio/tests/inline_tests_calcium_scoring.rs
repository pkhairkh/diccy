// Auto-extracted from /home/z/diccy/crates/dicom-cardio/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_cardio::*;
    use dicom_core::{Tag};

    #[test]
    fn agatston_weight_classification() {
        // REQ-CARDIO-100: Agatston weight must follow standard HU thresholds
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 150.0;
        assert_eq!(lesion.agatston_weight(), 1.0);

        lesion.peak_hu = 250.0;
        assert_eq!(lesion.agatston_weight(), 2.0);

        lesion.peak_hu = 350.0;
        assert_eq!(lesion.agatston_weight(), 3.0);

        lesion.peak_hu = 500.0;
        assert_eq!(lesion.agatston_weight(), 4.0);
    }

    #[test]
    fn agatston_score_calculation() {
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 250.0;
        lesion.area_mm2 = 10.0;

        let score = lesion.agatston_score(0.5, 3.0);
        // weight = 2.0, num_pixels = 10.0 / 0.5 = 20, score = 20 * 0.5 * 2.0 * 3.0 = 60.0
        assert_eq!(score, 60.0);
    }

    #[test]
    fn calcium_score_from_lesions() {
        let mut l1 = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        l1.peak_hu = 200.0;
        l1.area_mm2 = 5.0;
        l1.volume_mm3 = 15.0;
        l1.mass_mg = 3.0;

        let mut l2 = CalcifiedLesion::new("l2".to_string(), CoronaryArtery::Rca);
        l2.peak_hu = 400.0;
        l2.area_mm2 = 8.0;
        l2.volume_mm3 = 24.0;
        l2.mass_mg = 5.0;

        let result = CalciumScoreResult::from_lesions(vec![l1, l2], 0.5, 3.0);
        assert!(result.total_agatston > 0.0);
        assert_eq!(result.total_volume_mm3, 39.0);
        assert_eq!(result.total_mass_mg, 8.0);
        assert!(result.artery_scores.contains_key(&CoronaryArtery::Lad));
        assert!(result.artery_scores.contains_key(&CoronaryArtery::Rca));
    }

    #[test]
    fn calcium_detection_in_volume() {
        let mut volume = vec![0.0; 10 * 10 * 5];
        // Place calcium voxels
        volume[2 * (10 * 10) + 5 * 10 + 5] = 200.0;
        volume[2 * (10 * 10) + 5 * 10 + 6] = 250.0;
        volume[2 * (10 * 10) + 6 * 10 + 5] = 180.0;

        let artery_map = vec![CoronaryArtery::Lad; 5];
        let lesions = detect_calcium_lesions(&volume, 10, 10, 5, (0.5, 0.5), 3.0, &artery_map);
        assert!(!lesions.is_empty());
        assert!(lesions[0].peak_hu >= 180.0);
    }

    #[test]
    fn coronary_artery_dicom_codes() {
        assert_eq!(CoronaryArtery::LeftMain.dicom_code(), "74066000");
        assert_eq!(CoronaryArtery::Lad.dicom_code(), "74290005");
    }

    #[test]
    fn age_sex_percentile_range() {
        let result = CalciumScoreResult::from_lesions(vec![], 0.5, 3.0);
        let pct = result.age_sex_percentile(55, true);
        assert!(pct >= 1.0 && pct <= 99.0);
    }

    #[test]
    fn encode_to_dicom_sr() {
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 200.0;
        lesion.area_mm2 = 5.0;
        lesion.volume_mm3 = 15.0;
        lesion.mass_mg = 3.0;

        let result = CalciumScoreResult::from_lesions(vec![lesion], 0.5, 3.0);
        let ds = result.encode_to_dicom_sr("1.2.3", "4.5.6");
        assert_eq!(ds.get_uid(Tag(0x0020, 0x000D)), Some("1.2.3"));
    }

    #[test]
    fn hu_threshold_constant() {
        assert_eq!(CALCIUM_HU_THRESHOLD, 130.0);
    }
