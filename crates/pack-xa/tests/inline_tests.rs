// Auto-extracted from /home/z/diccy/crates/pack-xa/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use pack_xa::*;
    use dicom_core::{Dataset, Element, Limits, Value, Vr};

    const XA_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        XA_MANIFEST
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

    #[test]
    #[cfg(not(feature = "pack-xa"))]
    fn xa_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(!XaPack::enabled());
        let err = XaPack::ensure_supported(SOP_CLASS_XA).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-xa")]
    fn xa_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301
        assert!(XaPack::enabled());
        XaPack::ensure_supported(SOP_CLASS_XA).expect("xa pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083, REQ-SOP-300
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in XA_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn calibration_prefers_pixel_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("0.8\\0.9".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGER_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("1\\1".to_string()),
            )
            .unwrap(),
        );
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(
            ctx.calibration,
            Some((CalibrationSource::PixelSpacing, (0.8, 0.9)))
        );
    }

    #[test]
    fn calibration_falls_back_to_imager_spacing() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_IMAGER_PIXEL_SPACING,
                Vr::Ds,
                Value::Str("1\\2".to_string()),
            )
            .unwrap(),
        );
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(
            ctx.calibration,
            Some((CalibrationSource::ImagerPixelSpacing, (1.0, 2.0)))
        );
    }

    #[test]
    fn calibration_missing_warns() {
        // REQ-CONF-085
        let dataset = Dataset::new();
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert!(ctx.calibration.is_none());
        assert!(ctx
            .warnings
            .contains(&MeasurementWarning::MissingPixelSpacing));
        assert!(ctx
            .warnings
            .contains(&MeasurementWarning::MissingImagerPixelSpacing));
    }

    #[test]
    fn invalid_utf8_bytes_emit_invalid_warning() {
        // REQ-CONF-085: malformed string bytes must fail closed as invalid metadata.
        let mut dataset = Dataset::new();
        dataset
            .insert(Element::new(TAG_FRAME_TIME, Vr::Ds, Value::Bytes(vec![0xff, 0xfe])).unwrap());
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert!(ctx.warnings.contains(&MeasurementWarning::InvalidFrameTime));
    }

    #[test]
    fn frame_time_vector_fallback() {
        // REQ-CONF-085
        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_FRAME_TIME_VECTOR,
                Vr::Ds,
                Value::Str("33.3\\33.3".to_string()),
            )
            .unwrap(),
        );
        let ctx = extract_measurement_context(&dataset, &Limits::default()).expect("context");
        assert_eq!(ctx.frame_time_ms, Some(33.3));
    }
