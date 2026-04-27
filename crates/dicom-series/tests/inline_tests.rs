// Auto-extracted from /home/z/diccy/crates/dicom-series/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_series::{
        assemble, FrameKey, InstanceHeader, SeriesWarningKind, SOP_CLASS_CT, SOP_CLASS_SC_MF_BYTE,
    };
    use dicom_core::ErrorKind;

    fn ct_instance(sop_uid: &str, z: f64, order: u64, instance_number: i32) -> InstanceHeader {
        InstanceHeader {
            study_instance_uid: Some("1.2.3".to_string()),
            series_instance_uid: Some("2.3.4".to_string()),
            sop_instance_uid: Some(sop_uid.to_string()),
            sop_class_uid: Some(SOP_CLASS_CT.to_string()),
            instance_number: Some(instance_number),
            image_position: Some([0.0, 0.0, z]),
            image_orientation: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            number_of_frames: Some(1),
            pixel_data_len: Some(128),
            source_order: order,
            in_envelope: true,
        }
    }

    #[test]
    fn assemble_orders_ct_by_slice_coord() {
        // REQ-GEOM-301: ordering by slice coord with stable tie-breaks.
        let instances = vec![
            ct_instance("a", 2.0, 2, 2),
            ct_instance("b", 0.0, 1, 1),
            ct_instance("c", 1.0, 3, 3),
        ];
        let studies = assemble(&instances).expect("assemble");
        let frames: Vec<&FrameKey> = studies[0].series[0].frames().map(|f| &f.key).collect();
        assert_eq!(frames[0].instance_uid, "b");
        assert_eq!(frames[1].instance_uid, "c");
        assert_eq!(frames[2].instance_uid, "a");
    }

    #[test]
    fn assemble_rejects_invalid_geometry() {
        // REQ-GEOM-302: invalid normal rejected.
        let mut instance = ct_instance("a", 0.0, 1, 1);
        instance.image_orientation = Some([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let err = assemble(&[instance]).expect_err("expected error");
        assert!(matches!(err.kind(), ErrorKind::InvalidGeometry { .. }));
    }

    #[test]
    fn assemble_marks_non_monotonic_order() {
        // REQ-GEOM-303: non-monotonic order yields warning and disables volume eligibility.
        let instances = vec![ct_instance("a", 0.0, 1, 1), ct_instance("b", -1.0, 2, 2)];
        let studies = assemble(&instances).expect("assemble");
        let series = &studies[0].series[0];
        assert!(!series.volume_eligible);
        assert!(series
            .warnings
            .iter()
            .any(|w| w.kind == SeriesWarningKind::NonMonotonicSliceOrder));
    }

    #[test]
    fn assemble_sc_multiframe_orders_by_frame_index() {
        // REQ-SER-230: frame order is natural frame index.
        let instance = InstanceHeader {
            study_instance_uid: Some("1.2.3".to_string()),
            series_instance_uid: Some("2.3.4".to_string()),
            sop_instance_uid: Some("sc".to_string()),
            sop_class_uid: Some(SOP_CLASS_SC_MF_BYTE.to_string()),
            instance_number: None,
            image_position: None,
            image_orientation: None,
            number_of_frames: Some(3),
            pixel_data_len: None,
            source_order: 1,
            in_envelope: true,
        };
        let studies = assemble(&[instance]).expect("assemble");
        let frames: Vec<&FrameKey> = studies[0].series[0].frames().map(|f| &f.key).collect();
        assert_eq!(frames[0].frame_index, 0);
        assert_eq!(frames[1].frame_index, 1);
        assert_eq!(frames[2].frame_index, 2);
    }

    #[test]
    fn assemble_records_duplicate_warning() {
        // REQ-SER-211: duplicate SOP warnings are attached to the series.
        let mut a = ct_instance("dup", 0.0, 2, 2);
        a.pixel_data_len = Some(10);
        let mut b = ct_instance("dup", 0.0, 1, 1);
        b.pixel_data_len = Some(20);
        let studies = assemble(&[a, b]).expect("assemble");
        let series = &studies[0].series[0];
        assert!(series
            .warnings
            .iter()
            .any(|w| w.kind == SeriesWarningKind::DuplicateSopInstanceUid));
    }
