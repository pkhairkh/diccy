// Auto-extracted from /home/z/diccy/crates/pack-seg/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use pack_seg::*;
    use dicom_core::{Dataset, Element, Value, Vr};
    use dicom_pixel::{DisplayFrame, PixelFormat};

    const SEG_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        SEG_MANIFEST
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
    #[cfg(not(feature = "pack-seg"))]
    fn seg_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!SegPack::enabled());
        let err = SegPack::ensure_supported(SOP_CLASS_SEG).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-seg")]
    fn seg_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(SegPack::enabled());
        SegPack::ensure_supported(SOP_CLASS_SEG).expect("seg pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in SEG_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    // -----------------------------------------------------------------------
    // Binary segmentation parser tests (existing)
    // -----------------------------------------------------------------------

    #[test]
    fn binary_seg_overlay_is_deterministic() {
        // REQ-UI-063, REQ-VOL-923, REQ-VOL-927, REQ-SEG-300, REQ-SEG-303
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_0011])).unwrap());
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0, 0, 0, 0],
        };
        let reference = SegReference {
            rows: 2,
            cols: 2,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let overlay = seg.overlay_on(&base, reference).expect("overlay");
        let overlay_repeat = seg.overlay_on(&base, reference).expect("overlay repeat");
        assert_eq!(overlay.bytes, overlay_repeat.bytes);
        assert_eq!(overlay.bytes.len(), 16);
        assert!(overlay.bytes.iter().any(|&b| b != 0));
    }

    #[test]
    fn seg_reference_uid_mismatch_fails() {
        // REQ-CONF-086, REQ-SEG-300
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_00001])).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "9.9.9",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn seg_frame_of_reference_mismatch_fails() {
        // REQ-CONF-086, REQ-SEG-300
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_0001])).unwrap());
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "9.9.9",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    #[test]
    fn seg_multiframe_overlay_is_deterministic() {
        // REQ-CONF-086, REQ-SEG-301, REQ-SEG-303
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("2".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_0001])).unwrap());
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        assert_eq!(seg.frames, 2);
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let frame0 = seg
            .overlay_on_frame(&base, reference, 0)
            .expect("frame0 overlay");
        let frame1 = seg
            .overlay_on_frame(&base, reference, 1)
            .expect("frame1 overlay");
        let frame0_repeat = seg
            .overlay_on_frame(&base, reference, 0)
            .expect("frame0 repeat");
        assert_eq!(frame0.bytes, frame0_repeat.bytes);
        assert!(frame0.bytes.iter().any(|&value| value != 0));
        assert!(frame1.bytes.iter().all(|&value| value == 0));

        // Backward-compatible entrypoint uses frame 0.
        let default_overlay = seg.overlay_on(&base, reference).expect("default overlay");
        assert_eq!(default_overlay.bytes, frame0.bytes);
    }

    #[test]
    fn seg_multiframe_frame_index_out_of_range_fails_closed() {
        // REQ-CONF-086, REQ-SEG-301
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("2".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_0001])).unwrap());
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![0],
        };
        let reference = SegReference {
            rows: 1,
            cols: 1,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on_frame(&base, reference, 2).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn seg_grid_mismatch_fails_closed_without_resampling() {
        // REQ-VOL-924, REQ-SEG-302
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("BINARY".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0b0000_0001])).unwrap());
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0; 4],
        };
        let reference = SegReference {
            rows: 2,
            cols: 2,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let err = seg.overlay_on(&base, reference).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    // -----------------------------------------------------------------------
    // Fractional segmentation parser tests
    // -----------------------------------------------------------------------

    #[test]
    fn fractional_seg_parses_correctly() {
        // REQ-SEG-301
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("FRACTIONAL".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("7".to_string())).unwrap());
        // 4 pixels: 0, 128, 255, 0
        dataset.insert(
            Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 128, 255, 0])).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        assert_eq!(seg.seg_type, SegmentationType::Fractional);
        assert_eq!(seg.rows, 2);
        assert_eq!(seg.cols, 2);
        assert_eq!(seg.frames, 1);
        assert!(seg.mask.is_empty());
        assert_eq!(seg.fractional_probability.len(), 4);
        assert!((seg.fractional_probability[0] - 0.0).abs() < f32::EPSILON);
        assert!((seg.fractional_probability[1] - 128.0 / 255.0).abs() < 0.01);
        assert!((seg.fractional_probability[2] - 1.0).abs() < 0.01);
        assert!((seg.fractional_probability[3] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn fractional_seg_rejects_wrong_bits_allocated() {
        // REQ-SEG-301
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("FRACTIONAL".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("7".to_string())).unwrap());
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![1])).unwrap());

        let err = Segmentation::from_dataset(&dataset).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn fractional_seg_rejects_wrong_bits_stored() {
        // REQ-SEG-301
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("FRACTIONAL".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("7".to_string())).unwrap());
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![1])).unwrap());

        let err = Segmentation::from_dataset(&dataset).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn fractional_seg_rejects_wrong_high_bit() {
        // REQ-SEG-301
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("FRACTIONAL".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("0".to_string())).unwrap());
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![1])).unwrap());

        let err = Segmentation::from_dataset(&dataset).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn fractional_seg_overlay_is_deterministic() {
        // REQ-SEG-301, REQ-SEG-303
        let mut ref_instance = Dataset::new();
        ref_instance.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        let mut ref_series = Dataset::new();
        ref_series.insert(
            Element::new(
                TAG_REFERENCED_INSTANCE_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_instance]),
            )
            .unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("2".to_string())).unwrap());
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_SEGMENTATION_TYPE,
                Vr::Cs,
                Value::Str("FRACTIONAL".to_string()),
            )
            .unwrap(),
        );
        dataset
            .insert(Element::new(TAG_SEGMENT_NUMBER, Vr::Us, Value::Str("3".to_string())).unwrap());
        dataset
            .insert(Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_BITS_STORED, Vr::Us, Value::Str("8".to_string())).unwrap());
        dataset.insert(Element::new(TAG_HIGH_BIT, Vr::Us, Value::Str("7".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 200, 0, 0])).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_REFERENCED_SERIES_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_series]),
            )
            .unwrap(),
        );

        let seg = Segmentation::from_dataset(&dataset).expect("seg parse");
        let base = DisplayFrame {
            width: 2,
            height: 2,
            format: PixelFormat::Luma8,
            bytes: vec![0; 4],
        };
        let reference = SegReference {
            rows: 2,
            cols: 2,
            frame_of_reference_uid: "1.2.3",
            sop_instance_uid: "1.2.3.4",
        };
        let overlay1 = seg.overlay_on(&base, reference).expect("overlay1");
        let overlay2 = seg.overlay_on(&base, reference).expect("overlay2");
        assert_eq!(overlay1.bytes, overlay2.bytes);
        // Pixel 1 has probability 200/255 ≈ 0.78, so alpha > 0
        assert!(overlay1.bytes[4 * 1 + 3] > 0);
        // Pixel 0 has probability 0, so alpha = 0
        assert_eq!(overlay1.bytes[4 * 0 + 3], 0);
    }

    // -----------------------------------------------------------------------
    // SegmentationType enum tests
    // -----------------------------------------------------------------------

    #[test]
    fn segmentation_type_roundtrip() {
        // REQ-SEG-301
        assert_eq!(
            SegmentationType::from_cs_string(SegmentationType::Binary.to_cs_string()).unwrap(),
            SegmentationType::Binary
        );
        assert_eq!(
            SegmentationType::from_cs_string(SegmentationType::Fractional.to_cs_string()).unwrap(),
            SegmentationType::Fractional
        );
        assert!(SegmentationType::from_cs_string("UNKNOWN").is_err());
    }

    // -----------------------------------------------------------------------
    // Encoder tests
    // -----------------------------------------------------------------------

    #[test]
    fn encoder_binary_produces_valid_dataset() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0, 1, 0, 1]; // 2x2 binary mask
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3.4.5")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .expect("encode");

        assert_eq!(read_u16(&dataset, TAG_ROWS).unwrap(), 2);
        assert_eq!(read_u16(&dataset, TAG_COLUMNS).unwrap(), 2);
        assert_eq!(read_u16(&dataset, TAG_NUMBER_OF_FRAMES).unwrap(), 1);
        assert_eq!(
            dataset.get_uid(TAG_FRAME_OF_REFERENCE_UID).unwrap(),
            "1.2.3.4.5"
        );
        assert_eq!(
            read_str(&dataset, TAG_SEGMENTATION_TYPE).unwrap().unwrap(),
            "BINARY"
        );
        assert_eq!(read_u16(&dataset, TAG_SEGMENT_NUMBER).unwrap(), 1);
        assert_eq!(read_u16(&dataset, TAG_BITS_ALLOCATED).unwrap(), 1);
        assert_eq!(read_u16(&dataset, TAG_BITS_STORED).unwrap(), 1);
        assert_eq!(read_u16(&dataset, TAG_HIGH_BIT).unwrap(), 0);

        // Pixel data should be bit-packed: pixels 0,1,0,1 => byte 0b0000_1010
        let pixel_data = read_bytes(&dataset, TAG_PIXEL_DATA).unwrap();
        assert_eq!(pixel_data.len(), 1);
        assert_eq!(pixel_data[0], 0b0000_1010);
    }

    #[test]
    fn encoder_fractional_produces_valid_dataset() {
        // REQ-SEG-301
        let frac: Vec<f32> = vec![0.0, 0.5, 1.0, 0.25]; // 2x2 fractional
        let dataset = SegmentationEncoder::new(SegmentationType::Fractional)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3.4.5")
            .segment_number(2)
            .fractional_mask(frac)
            .encode_to_dataset()
            .expect("encode");

        assert_eq!(read_u16(&dataset, TAG_ROWS).unwrap(), 2);
        assert_eq!(read_u16(&dataset, TAG_COLUMNS).unwrap(), 2);
        assert_eq!(
            read_str(&dataset, TAG_SEGMENTATION_TYPE).unwrap().unwrap(),
            "FRACTIONAL"
        );
        assert_eq!(read_u16(&dataset, TAG_SEGMENT_NUMBER).unwrap(), 2);
        assert_eq!(read_u16(&dataset, TAG_BITS_ALLOCATED).unwrap(), 8);
        assert_eq!(read_u16(&dataset, TAG_BITS_STORED).unwrap(), 8);
        assert_eq!(read_u16(&dataset, TAG_HIGH_BIT).unwrap(), 7);

        let pixel_data = read_bytes(&dataset, TAG_PIXEL_DATA).unwrap();
        assert_eq!(pixel_data.len(), 4);
        assert_eq!(pixel_data[0], 0);
        assert_eq!(pixel_data[1], 128); // 0.5 * 255 ≈ 128
        assert_eq!(pixel_data[2], 255); // 1.0 * 255 = 255
        assert_eq!(pixel_data[3], 64); // 0.25 * 255 ≈ 64
    }

    #[test]
    fn encoder_includes_referenced_sop_instance_uid() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![1];
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(1)
            .cols(1)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .referenced_sop_instance_uid("1.2.3.4.5")
            .encode_to_dataset()
            .expect("encode");

        let parsed = Segmentation::from_dataset(&dataset).expect("roundtrip parse");
        assert_eq!(
            parsed.referenced_sop_instance_uid,
            Some("1.2.3.4.5".to_string())
        );
    }

    #[test]
    fn encoder_omits_referenced_series_when_no_ref_uid() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![1];
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(1)
            .cols(1)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .expect("encode");

        // No ReferencedSeriesSequence should be present
        assert!(dataset.get(TAG_REFERENCED_SERIES_SEQUENCE).is_none());
    }

    #[test]
    fn encoder_binary_roundtrip_matches_original() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0, 0xFF, 0, 0xFF, 0xFF, 0, 0, 0, 0xFF]; // 3x3
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(3)
            .cols(3)
            .frames(1)
            .frame_of_reference_uid("1.2.3.4")
            .segment_number(1)
            .binary_mask(mask.clone())
            .referenced_sop_instance_uid("1.2.3.4.5")
            .encode_to_dataset()
            .expect("encode");

        let parsed = Segmentation::from_dataset(&dataset).expect("parse roundtrip");
        assert_eq!(parsed.seg_type, SegmentationType::Binary);
        assert_eq!(parsed.rows, 3);
        assert_eq!(parsed.cols, 3);
        assert_eq!(parsed.frames, 1);
        assert_eq!(parsed.frame_of_reference_uid, "1.2.3.4");
        assert_eq!(parsed.segment_number, 1);
        assert_eq!(
            parsed.referenced_sop_instance_uid,
            Some("1.2.3.4.5".to_string())
        );
        // Compare mask (0xFF -> 0xFF, 0 -> 0x00)
        let expected_mask: Vec<u8> = mask
            .iter()
            .map(|&b| if b != 0 { 0xFF } else { 0x00 })
            .collect();
        assert_eq!(parsed.mask, expected_mask);
        assert!(parsed.fractional_probability.is_empty());
    }

    #[test]
    fn encoder_fractional_roundtrip_matches_original() {
        // REQ-SEG-301
        let frac: Vec<f32> = vec![0.0, 0.5, 1.0, 0.25]; // 2x2
        let dataset = SegmentationEncoder::new(SegmentationType::Fractional)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3.4")
            .segment_number(2)
            .fractional_mask(frac.clone())
            .referenced_sop_instance_uid("1.2.3.4.5")
            .encode_to_dataset()
            .expect("encode");

        let parsed = Segmentation::from_dataset(&dataset).expect("parse roundtrip");
        assert_eq!(parsed.seg_type, SegmentationType::Fractional);
        assert_eq!(parsed.rows, 2);
        assert_eq!(parsed.cols, 2);
        assert_eq!(parsed.frames, 1);
        assert!(parsed.mask.is_empty());
        assert_eq!(parsed.fractional_probability.len(), 4);
        // Allow 1/255 rounding tolerance
        for (orig, parsed_val) in frac.iter().zip(parsed.fractional_probability.iter()) {
            assert!(
                (orig - parsed_val).abs() < 1.0 / 255.0 + f32::EPSILON,
                "fractional roundtrip: orig={orig}, parsed={parsed_val}"
            );
        }
    }

    #[test]
    fn encoder_defaults_frames_to_one() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .expect("encode");

        assert_eq!(read_u16(&dataset, TAG_NUMBER_OF_FRAMES).unwrap(), 1);
    }

    #[test]
    fn encoder_rejects_zero_frames() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frames(0)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn encoder_rejects_missing_rows() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .cols(2)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn encoder_rejects_missing_cols() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn encoder_rejects_missing_frame_of_reference_uid() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn encoder_rejects_missing_segment_number() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 4];
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frame_of_reference_uid("1.2.3")
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn encoder_rejects_missing_binary_mask() {
        // REQ-SEG-301
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn encoder_rejects_missing_fractional_mask() {
        // REQ-SEG-301
        let err = SegmentationEncoder::new(SegmentationType::Fractional)
            .rows(2)
            .cols(2)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn encoder_rejects_binary_mask_too_short() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![0; 2]; // only 2 pixels but 2x2=4 expected
        let err = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn encoder_rejects_fractional_mask_too_short() {
        // REQ-SEG-301
        let frac: Vec<f32> = vec![0.5]; // only 1 pixel but 2x2=4 expected
        let err = SegmentationEncoder::new(SegmentationType::Fractional)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .fractional_mask(frac)
            .encode_to_dataset()
            .unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn encoder_multiframe_binary_roundtrip() {
        // REQ-SEG-301, REQ-SEG-303
        // 2 frames, 2x2 each = 8 pixels total
        let mask: Vec<u8> = vec![
            0, 1, 0, 1, // frame 0
            1, 0, 1, 0, // frame 1
        ];
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(2)
            .cols(2)
            .frames(2)
            .frame_of_reference_uid("1.2.3.4")
            .segment_number(1)
            .binary_mask(mask)
            .referenced_sop_instance_uid("1.2.3.4.5")
            .encode_to_dataset()
            .expect("encode");

        let parsed = Segmentation::from_dataset(&dataset).expect("parse roundtrip");
        assert_eq!(parsed.frames, 2);
        // Frame 0: pixels 0,1,0,1
        assert_eq!(parsed.mask[0], 0x00);
        assert_eq!(parsed.mask[1], 0xFF);
        assert_eq!(parsed.mask[2], 0x00);
        assert_eq!(parsed.mask[3], 0xFF);
        // Frame 1: pixels 1,0,1,0
        assert_eq!(parsed.mask[4], 0xFF);
        assert_eq!(parsed.mask[5], 0x00);
        assert_eq!(parsed.mask[6], 0xFF);
        assert_eq!(parsed.mask[7], 0x00);
    }

    #[test]
    fn encoder_dataset_is_deterministic() {
        // REQ-SEG-301
        let mask: Vec<u8> = vec![1, 0, 1, 0];
        let build = || {
            SegmentationEncoder::new(SegmentationType::Binary)
                .rows(2)
                .cols(2)
                .frames(1)
                .frame_of_reference_uid("1.2.3.4")
                .segment_number(1)
                .binary_mask(mask.clone())
                .encode_to_dataset()
                .expect("encode")
        };
        let d1 = build();
        let d2 = build();
        assert_eq!(d1, d2);
    }

    #[test]
    fn encoder_fractional_clamps_out_of_range() {
        // REQ-SEG-301: values outside [0.0, 1.0] are clamped
        let frac: Vec<f32> = vec![-0.5, 0.5, 1.5, 0.0];
        let dataset = SegmentationEncoder::new(SegmentationType::Fractional)
            .rows(2)
            .cols(2)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .fractional_mask(frac)
            .encode_to_dataset()
            .expect("encode");

        let pixel_data = read_bytes(&dataset, TAG_PIXEL_DATA).unwrap();
        assert_eq!(pixel_data[0], 0); // -0.5 clamped to 0
        assert_eq!(pixel_data[2], 255); // 1.5 clamped to 1.0 -> 255
    }

    #[test]
    fn encoder_binary_bit_packing_lsb_first() {
        // REQ-SEG-301: verify bit-packing order is LSB-first
        // 8 pixels: alternating 0,1 -> bit i = mask[i], so byte = 0b1010_1010 = 0xAA
        let mask: Vec<u8> = vec![0, 1, 0, 1, 0, 1, 0, 1];
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(8)
            .cols(1)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .expect("encode");

        let pixel_data = read_bytes(&dataset, TAG_PIXEL_DATA).unwrap();
        assert_eq!(pixel_data.len(), 1);
        assert_eq!(pixel_data[0], 0b1010_1010);
    }

    #[test]
    fn encoder_binary_non_byte_aligned_pixels() {
        // REQ-SEG-301: 5 pixels, not a full byte
        let mask: Vec<u8> = vec![1, 1, 1, 1, 1]; // 5 bits set
        let dataset = SegmentationEncoder::new(SegmentationType::Binary)
            .rows(5)
            .cols(1)
            .frames(1)
            .frame_of_reference_uid("1.2.3")
            .segment_number(1)
            .binary_mask(mask)
            .encode_to_dataset()
            .expect("encode");

        let pixel_data = read_bytes(&dataset, TAG_PIXEL_DATA).unwrap();
        assert_eq!(pixel_data.len(), 1); // ceil(5/8) = 1 byte
        assert_eq!(pixel_data[0], 0b0001_1111);
    }
