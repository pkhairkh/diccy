// Auto-extracted from /home/z/diccy/crates/pack-rt/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use pack_rt::*;
    use dicom_core::{Dataset, Element, Error, ErrorKind, Tag, Value, Vr};
    use dicom_pixel::{PixelFormat};

    const RT_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        RT_MANIFEST
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
    #[cfg(not(feature = "pack-rt"))]
    fn rt_pack_disabled_rejects() {
        // REQ-FEAT-302
        assert!(!RtPack::enabled());
        let err = RtPack::ensure_supported(SOP_CLASS_RT_DOSE).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "pack-rt")]
    fn rt_pack_enabled_allows() {
        // REQ-FEAT-302
        assert!(RtPack::enabled());
        RtPack::ensure_supported(SOP_CLASS_RT_DOSE).expect("rt pack enabled");
        RtPack::ensure_supported(SOP_CLASS_RT_STRUCTURE).expect("rt pack enabled");
        RtPack::ensure_supported(SOP_CLASS_RT_PLAN).expect("rt pack enabled");
    }

    #[test]
    fn manifest_matches_constants() {
        // REQ-CONF-083
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in RT_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn dose_grid_scaling_applied() {
        // REQ-VOL-925, REQ-RT-350, REQ-RT-352
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
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
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("0.5".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![2, 0])).unwrap());
        let dose = RtDoseGrid::from_dataset(&dataset).expect("dose parse");
        assert_eq!(dose.values, vec![1.0]);
        let reference = RtReferenceGeometry {
            rows: 1,
            cols: 1,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "1.2.3".to_string(),
        };
        let overlay = dose.overlay_on(&reference, 0).expect("overlay");
        let overlay_repeat = dose.overlay_on(&reference, 0).expect("overlay repeat");
        assert_eq!(overlay.bytes, overlay_repeat.bytes);
        assert_eq!(overlay.width, 1);
    }

    #[test]
    fn grid_frame_offset_vector_mismatch_fails() {
        // REQ-VOL-926, REQ-RT-351
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("2".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
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
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset
            .insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 0, 0, 0])).unwrap());
        let err = RtDoseGrid::from_dataset(&dataset).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn rt_alignment_mismatch_fails() {
        // REQ-VOL-926, REQ-RT-351
        let mut dataset = Dataset::new();
        dataset.insert(Element::new(TAG_ROWS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(Element::new(TAG_COLUMNS, Vr::Us, Value::Str("1".to_string())).unwrap());
        dataset.insert(
            Element::new(TAG_NUMBER_OF_FRAMES, Vr::Is, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_PIXEL_SPACING, Vr::Ds, Value::Str("1\\1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_POSITION,
                Vr::Ds,
                Value::Str("0\\0\\0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_IMAGE_ORIENTATION,
                Vr::Ds,
                Value::Str("1\\0\\0\\0\\1\\0".to_string()),
            )
            .unwrap(),
        );
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
                TAG_GRID_FRAME_OFFSET_VECTOR,
                Vr::Ds,
                Value::Str("0".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_DOSE_GRID_SCALING, Vr::Ds, Value::Str("1".to_string())).unwrap(),
        );
        dataset.insert(
            Element::new(TAG_BITS_ALLOCATED, Vr::Us, Value::Str("16".to_string())).unwrap(),
        );
        dataset.insert(Element::new(TAG_PIXEL_DATA, Vr::Ob, Value::Bytes(vec![0, 0])).unwrap());
        let dose = RtDoseGrid::from_dataset(&dataset).expect("dose parse");
        let reference = RtReferenceGeometry {
            rows: 1,
            cols: 1,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 1.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "1.2.3".to_string(),
        };
        let err = dose.overlay_on(&reference, 0).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    fn build_structure_dataset_with_type(points: &str, contour_type: &str) -> Dataset {
        let mut contour = Dataset::new();
        contour.insert(
            Element::new(
                TAG_CONTOUR_GEOMETRIC_TYPE,
                Vr::Cs,
                Value::Str(contour_type.to_string()),
            )
            .unwrap(),
        );
        contour.insert(
            Element::new(
                TAG_NUMBER_OF_CONTOUR_POINTS,
                Vr::Is,
                Value::Str("4".to_string()),
            )
            .unwrap(),
        );
        contour.insert(
            Element::new(TAG_CONTOUR_DATA, Vr::Ds, Value::Str(points.to_string())).unwrap(),
        );

        let mut roi = Dataset::new();
        roi.insert(
            Element::new(
                TAG_ROI_DISPLAY_COLOR,
                Vr::Is,
                Value::Str("255\\0\\0".to_string()),
            )
            .unwrap(),
        );
        roi.insert(
            Element::new(TAG_CONTOUR_SEQUENCE, Vr::Sq, Value::Sequence(vec![contour])).unwrap(),
        );

        let mut dataset = Dataset::new();
        dataset.insert(
            Element::new(
                TAG_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("9.8.7".to_string()),
            )
            .unwrap(),
        );
        dataset.insert(
            Element::new(TAG_ROI_CONTOUR_SEQUENCE, Vr::Sq, Value::Sequence(vec![roi])).unwrap(),
        );
        dataset
    }

    fn build_structure_dataset(points: &str) -> Dataset {
        build_structure_dataset_with_type(points, "CLOSED_PLANAR")
    }

    #[test]
    fn structure_set_overlay_applies() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset("0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0");
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let overlay = structure.overlay_on(&reference).expect("overlay");
        assert_eq!(overlay.format, PixelFormat::Rgba8);
        assert!(overlay.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn structure_set_closedplanar_xor_overlay_applies() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset_with_type(
            "0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0",
            "CLOSEDPLANAR_XOR",
        );
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let overlay = structure.overlay_on(&reference).expect("overlay");
        assert_eq!(overlay.format, PixelFormat::Rgba8);
        assert!(overlay.bytes.iter().any(|value| *value != 0));
    }

    #[test]
    fn structure_set_plane_mismatch_fails() {
        // REQ-VOL-929, REQ-RT-353
        let dataset = build_structure_dataset("0\\0\\1\\1\\0\\1\\1\\1\\1\\0\\1\\1");
        let structure = RtStructureSet::from_dataset(&dataset).expect("structure parse");
        let reference = RtReferenceGeometry {
            rows: 2,
            cols: 2,
            pixel_spacing: (1.0, 1.0),
            ipp: [0.0, 0.0, 0.0],
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            frame_of_reference_uid: "9.8.7".to_string(),
        };
        let err = structure.overlay_on(&reference).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    #[test]
    fn plan_summary_references_structure() {
        // REQ-CONF-090, REQ-RT-354
        let structure = RtStructureSet::from_dataset(&build_structure_dataset(
            "0\\0\\0\\1\\0\\0\\1\\1\\0\\0\\1\\0",
        ))
        .expect("structure parse");
        let mut plan = Dataset::new();
        plan.insert(
            Element::new(
                TAG_FRAME_OF_REFERENCE_UID,
                Vr::Ui,
                Value::Uid("9.8.7".to_string()),
            )
            .unwrap(),
        );
        plan.insert(
            Element::new(TAG_RT_PLAN_LABEL, Vr::Sh, Value::Str("PLAN".to_string())).unwrap(),
        );
        let mut ref_item = Dataset::new();
        ref_item.insert(
            Element::new(
                TAG_REFERENCED_SOP_INSTANCE_UID,
                Vr::Ui,
                Value::Uid("1.2.3.4".to_string()),
            )
            .unwrap(),
        );
        plan.insert(
            Element::new(
                TAG_REFERENCED_STRUCTURE_SET_SEQUENCE,
                Vr::Sq,
                Value::Sequence(vec![ref_item]),
            )
            .unwrap(),
        );
        let summary = RtPlanSummary::from_dataset(&plan).expect("plan parse");
        summary
            .validate_structure_set(&structure)
            .expect("reference valid");
    }
