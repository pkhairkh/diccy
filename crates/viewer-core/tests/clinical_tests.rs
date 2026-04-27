// Auto-extracted from /home/z/diccy/crates/viewer-core/src/clinical.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::*;

    #[test]
    fn measurement_store_crud_undo_redo_and_exports_are_deterministic() {
        let mut store = MeasurementStore::new();
        let id = store
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![
                    ImagePoint { x: 10.0, y: 11.0 },
                    ImagePoint { x: 14.0, y: 15.0 },
                ],
                Some(5.0),
                MeasurementUnit::Millimeter,
            )
            .expect("create measurement");
        store
            .update_measurement(
                &id,
                vec![
                    ImagePoint { x: 10.0, y: 11.0 },
                    ImagePoint { x: 16.0, y: 19.0 },
                ],
                Some(9.0),
                MeasurementUnit::Millimeter,
            )
            .expect("update measurement");
        assert_eq!(
            store.jump_target(&id),
            Some(ImagePoint { x: 10.0, y: 11.0 })
        );
        let bundle = store.export_bundle();
        assert!(bundle
            .csv
            .contains("id,kind,unit,value,revision,updated_at_tick"));
        assert!(bundle.json.contains("\"id\":\"m0\""));
        let sr = store.sr_payload();
        assert_eq!(sr.source, "viewer-core.measurements");
        assert_eq!(sr.measurement_count, 1);
        store.delete_measurement(&id).expect("delete");
        assert_eq!(store.active_measurements().len(), 0);
        assert!(store.undo());
        assert_eq!(store.active_measurements().len(), 1);
        assert!(store.redo());
        assert_eq!(store.active_measurements().len(), 0);
    }

    #[test]
    fn segmentation_store_tracks_revisions_and_style_diffs() {
        let mut store = SegmentationStore::new();
        let id = store.import_seg("Tumor").expect("import");
        let diff = store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.8,
                    color_rgb: [10, 20, 30],
                    active: true,
                },
            )
            .expect("style update");
        assert_eq!(diff.segment_id, id);
        assert_eq!(diff.revision, 1);
        assert!(diff.changed_fields.contains(&"opacity".to_string()));
        store.set_locked(&id, true).expect("lock");
        let locked_err = store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.4,
                    color_rgb: [1, 2, 3],
                    active: false,
                },
            )
            .expect_err("locked segment should fail");
        assert!(matches!(locked_err, ClinicalError::Locked { .. }));
    }

    #[test]
    fn volumetric_fusion_and_rt_controls_fail_closed() {
        let mut volume = VolumeWorkflowState::new(VolumeWorkflowCapabilities {
            mpr: true,
            mip: false,
            volume_3d: false,
        });
        assert!(volume.set_mpr_enabled(true).is_ok());
        let mip_err = volume
            .set_mip_mode(Some(MipProjectionMode::MaxIntensity))
            .expect_err("mip disabled");
        assert!(matches!(
            mip_err,
            ClinicalError::CapabilityDisabled { capability: "mip" }
        ));
        let volume_err = volume.set_volume_3d_enabled(true).expect_err("3d disabled");
        assert!(matches!(
            volume_err,
            ClinicalError::CapabilityDisabled {
                capability: "volume_3d"
            }
        ));
        assert_eq!(
            volume.status().fallback_reason.as_deref(),
            Some("3d volume capability disabled")
        );

        let mut fusion = FusionOverlayState::new("CT", "PET");
        fusion.mark_geometry_mismatch("frame of reference mismatch");
        assert!(matches!(
            fusion.registration,
            FusionRegistrationState::GeometryMismatch { .. }
        ));

        let mut dose = RtDoseOverlayState::default();
        dose.set_window(0.1, 3.5).expect("valid dose window");
        let err = dose.set_window(2.0, 1.0).expect_err("invalid window");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn annotation_store_round_trips_snapshot_and_json_export() {
        let mut store = Annotation3dStore::new();
        let id = store
            .add("Apex", [1.0, 2.0, 3.0])
            .expect("create annotation");
        store
            .update(&id, "Apex-1", [4.0, 5.0, 6.0])
            .expect("update annotation");
        let snapshot = store.snapshot();
        let mut restored = Annotation3dStore::new();
        restored.restore_from_snapshot(snapshot);
        let json = restored.export_json();
        assert!(json.contains("\"id\":\"a0\""));
        assert!(json.contains("\"label\":\"Apex-1\""));
    }

    #[test]
    fn regression_path_covers_measurement_segmentation_and_rt_overlay_artifacts() {
        let mut measurements = MeasurementStore::new();
        let _ = measurements
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![ImagePoint { x: 2.0, y: 3.0 }, ImagePoint { x: 6.0, y: 9.0 }],
                Some(7.2),
                MeasurementUnit::Millimeter,
            )
            .expect("measurement create");
        let export = measurements.export_bundle();
        assert!(export.csv.contains("Distance2D"));

        let mut segments = SegmentationStore::new();
        let seg_id = segments.create_labelmap("Lesion").expect("segment create");
        let _ = segments
            .update_style(
                &seg_id,
                SegmentationStyle {
                    visible: true,
                    opacity: 0.7,
                    color_rgb: [200, 10, 10],
                    active: true,
                },
            )
            .expect("segment style update");
        assert_eq!(segments.active_segments().len(), 1);

        let mut dose = RtDoseOverlayState::default();
        dose.visible = true;
        dose.set_window(0.2, 2.6).expect("rt dose window");
        assert!(dose.visible);
    }

    // =======================================================================
    // Sprint 1 Extended Tests: Volume Rendering & Capabilities
    // =======================================================================

    #[test]
    fn volume_workflow_capabilities_default_enables_all() {
        let caps = VolumeWorkflowCapabilities::default();
        assert!(caps.mpr, "MPR should be enabled by default after Sprint 1");
        assert!(caps.mip, "MIP should be enabled by default after Sprint 1");
        assert!(
            caps.volume_3d,
            "3D volume should be enabled by default after Sprint 1"
        );
    }

    #[test]
    fn volume_workflow_state_allows_enabling_all_capabilities() {
        let mut state = VolumeWorkflowState::new(VolumeWorkflowCapabilities::default());
        assert!(state.set_mpr_enabled(true).is_ok());
        assert!(state
            .set_mip_mode(Some(MipProjectionMode::MaxIntensity))
            .is_ok());
        assert!(state.set_volume_3d_enabled(true).is_ok());
        let status = state.status();
        assert!(status.mpr_enabled);
        assert_eq!(status.mip_mode, Some(MipProjectionMode::MaxIntensity));
        assert!(status.volume_3d_enabled);
    }

    #[test]
    fn volume_workflow_state_toggle_mip_modes() {
        let mut state = VolumeWorkflowState::new(VolumeWorkflowCapabilities::default());
        assert!(state
            .set_mip_mode(Some(MipProjectionMode::MaxIntensity))
            .is_ok());
        assert_eq!(
            state.status().mip_mode,
            Some(MipProjectionMode::MaxIntensity)
        );
        assert!(state
            .set_mip_mode(Some(MipProjectionMode::MinIntensity))
            .is_ok());
        assert_eq!(
            state.status().mip_mode,
            Some(MipProjectionMode::MinIntensity)
        );
        assert!(state.set_mip_mode(None).is_ok());
        assert_eq!(state.status().mip_mode, None);
    }

    #[test]
    fn volume_workflow_state_disabled_capabilities_reject() {
        let caps = VolumeWorkflowCapabilities {
            mpr: false,
            mip: false,
            volume_3d: false,
        };
        let mut state = VolumeWorkflowState::new(caps);
        let mpr_err = state.set_mpr_enabled(true).expect_err("mpr disabled");
        assert!(matches!(
            mpr_err,
            ClinicalError::CapabilityDisabled { capability: "mpr" }
        ));
        let mip_err = state
            .set_mip_mode(Some(MipProjectionMode::MaxIntensity))
            .expect_err("mip disabled");
        assert!(matches!(
            mip_err,
            ClinicalError::CapabilityDisabled { capability: "mip" }
        ));
        let vol_err = state.set_volume_3d_enabled(true).expect_err("3d disabled");
        assert!(matches!(
            vol_err,
            ClinicalError::CapabilityDisabled {
                capability: "volume_3d"
            }
        ));
    }

    #[test]
    fn fusion_overlay_blend_validation() {
        let mut fusion = FusionOverlayState::new("CT", "PET");
        fusion.set_blend(0.0);
        assert!((fusion.blend() - 0.0).abs() < f64::EPSILON);
        fusion.set_blend(0.5);
        assert!((fusion.blend() - 0.5).abs() < f64::EPSILON);
        fusion.set_blend(1.0);
        assert!((fusion.blend() - 1.0).abs() < f64::EPSILON);
        // Out-of-range values are clamped
        fusion.set_blend(-0.1);
        assert!((fusion.blend() - 0.0).abs() < f64::EPSILON);
        fusion.set_blend(1.5);
        assert!((fusion.blend() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fusion_overlay_registration_lifecycle() {
        let mut fusion = FusionOverlayState::new("CT", "MR");
        // Starts unregistered
        assert!(matches!(
            fusion.registration,
            FusionRegistrationState::Unregistered
        ));
        // Mark as registered
        fusion.mark_registered();
        assert!(matches!(
            fusion.registration,
            FusionRegistrationState::Registered
        ));
        // Mark as geometry mismatch
        fusion.mark_geometry_mismatch("frame of reference UID differs");
        assert!(matches!(
            fusion.registration,
            FusionRegistrationState::GeometryMismatch { .. }
        ));
    }

    #[test]
    fn rt_dose_window_rejects_inverted_range() {
        let mut dose = RtDoseOverlayState::default();
        let err = dose.set_window(5.0, 1.0).expect_err("inverted window");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn rt_dose_window_accepts_valid_range() {
        let mut dose = RtDoseOverlayState::default();
        dose.set_window(0.0, 10.0).expect("valid window");
        dose.set_window(-5.0, 5.0).expect("negative min ok");
    }

    // =======================================================================
    // Sprint 2 Extended Tests: Segmentation Brush, Interpolation, ROI
    // =======================================================================

    #[test]
    fn labelmap_new_rejects_zero_dimensions() {
        assert!(LabelMap3D::new(0, 10, 10).is_err());
        assert!(LabelMap3D::new(10, 0, 10).is_err());
        assert!(LabelMap3D::new(10, 10, 0).is_err());
    }

    #[test]
    fn labelmap_new_creates_zero_initialized_labelmap() {
        let lm = LabelMap3D::new(5, 5, 3).expect("valid dimensions");
        assert_eq!(lm.width, 5);
        assert_eq!(lm.height, 5);
        assert_eq!(lm.depth, 3);
        assert_eq!(lm.labels.len(), 5 * 5 * 3);
        assert!(
            lm.labels.iter().all(|&l| l == 0),
            "all labels should be 0 (background)"
        );
    }

    #[test]
    fn labelmap_set_and_get_label() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        assert_eq!(lm.label(3, 4, 2), Some(0));
        assert!(lm.set_label(3, 4, 2, 5));
        assert_eq!(lm.label(3, 4, 2), Some(5));
        // Out of bounds
        assert_eq!(lm.label(10, 0, 0), None);
        assert!(!lm.set_label(10, 0, 0, 1));
    }

    #[test]
    fn brush_config_validation() {
        let valid = BrushConfig {
            radius_voxels: 5,
            mode: BrushMode::Paint,
        };
        assert!(valid.validate().is_ok());
        let zero_radius = BrushConfig {
            radius_voxels: 0,
            mode: BrushMode::Paint,
        };
        assert!(zero_radius.validate().is_err());
        let too_large = BrushConfig {
            radius_voxels: 65,
            mode: BrushMode::Erase,
        };
        assert!(too_large.validate().is_err());
    }

    #[test]
    fn brush_stroke_paint_fills_circular_region() {
        let mut lm = LabelMap3D::new(20, 20, 5).expect("valid");
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 2,
            center_col: 10,
            center_row: 10,
            radius_voxels: 3,
            mode: BrushMode::Paint,
        };
        let modified = lm.apply_brush_stroke(&stroke, 1).expect("paint stroke");
        assert!(modified > 0, "should modify some voxels");
        // Center should be painted
        assert_eq!(lm.label(10, 10, 2), Some(1));
        // Count voxels with label 1
        let count = lm.count_voxels_with_label(1);
        assert_eq!(count, modified, "all modified voxels should have label 1");
        // Approximate: circle with radius 3 should have about π*9 ≈ 28 voxels
        assert!(
            count > 20 && count < 40,
            "circle of radius 3 should have ~28 voxels, got {count}"
        );
    }

    #[test]
    fn brush_stroke_erase_removes_segment() {
        let mut lm = LabelMap3D::new(20, 20, 5).expect("valid");
        // Paint first
        let paint = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 2,
            center_col: 10,
            center_row: 10,
            radius_voxels: 3,
            mode: BrushMode::Paint,
        };
        let painted = lm.apply_brush_stroke(&paint, 1).expect("paint");
        assert!(painted > 0);
        // Now erase the same region
        let erase = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 2,
            center_col: 10,
            center_row: 10,
            radius_voxels: 3,
            mode: BrushMode::Erase,
        };
        let erased = lm.apply_brush_stroke(&erase, 1).expect("erase");
        assert_eq!(
            erased, painted,
            "erasing same region should remove same count"
        );
        assert_eq!(lm.count_voxels_with_label(1), 0, "no voxels should remain");
    }

    #[test]
    fn brush_stroke_rejects_zero_label() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 0,
            center_col: 5,
            center_row: 5,
            radius_voxels: 2,
            mode: BrushMode::Paint,
        };
        let err = lm.apply_brush_stroke(&stroke, 0).expect_err("zero label");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn brush_stroke_rejects_out_of_range_slice() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 10,
            center_col: 5,
            center_row: 5,
            radius_voxels: 2,
            mode: BrushMode::Paint,
        };
        let err = lm
            .apply_brush_stroke(&stroke, 1)
            .expect_err("slice out of range");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn brush_stroke_rejects_zero_radius() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 0,
            center_col: 5,
            center_row: 5,
            radius_voxels: 0,
            mode: BrushMode::Paint,
        };
        let err = lm.apply_brush_stroke(&stroke, 1).expect_err("zero radius");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn brush_stroke_clips_to_labelmap_bounds() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        // Brush centered at (0,0) with radius 3 should only paint the in-bounds portion
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 0,
            center_col: 0,
            center_row: 0,
            radius_voxels: 3,
            mode: BrushMode::Paint,
        };
        let modified = lm.apply_brush_stroke(&stroke, 1).expect("clip paint");
        assert!(modified > 0, "should paint some voxels at corner");
        assert!(
            modified < 30,
            "corner clip should reduce voxel count from full circle"
        );
        assert_eq!(lm.label(0, 0, 0), Some(1), "center should be painted");
    }

    #[test]
    fn labelmap_snapshot_and_restore() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let stroke = BrushStroke {
            segment_id: "seg0".to_string(),
            slice_index: 1,
            center_col: 5,
            center_row: 5,
            radius_voxels: 2,
            mode: BrushMode::Paint,
        };
        lm.apply_brush_stroke(&stroke, 1).expect("paint");
        let snap = lm.snapshot();
        let count_before = lm.count_voxels_with_label(1);
        // Clear all
        lm.restore_from_snapshot(vec![0u16; lm.labels.len()]);
        assert_eq!(lm.count_voxels_with_label(1), 0);
        // Restore
        lm.restore_from_snapshot(snap);
        assert_eq!(lm.count_voxels_with_label(1), count_before);
    }

    #[test]
    fn labelmap_count_voxels_with_label() {
        let mut lm = LabelMap3D::new(5, 5, 2).expect("valid");
        assert_eq!(lm.count_voxels_with_label(0), 50);
        lm.set_label(0, 0, 0, 3);
        lm.set_label(1, 0, 0, 3);
        lm.set_label(2, 0, 0, 3);
        assert_eq!(lm.count_voxels_with_label(3), 3);
        assert_eq!(lm.count_voxels_with_label(0), 47);
    }

    #[test]
    fn morphological_interpolation_fills_between_slices() {
        let mut lm = LabelMap3D::new(10, 10, 10).expect("valid");
        // Paint slice 2 and slice 7 with label 1 at same positions
        for y in 3..7 {
            for x in 3..7 {
                lm.set_label(x, y, 2, 1);
                lm.set_label(x, y, 7, 1);
            }
        }
        let modified = interpolate_slices_morphological(&mut lm, 2, 7, 1).expect("interp");
        assert!(modified > 0, "should fill intermediate slices");
        // Intermediate slices should now have the intersection
        assert_eq!(
            lm.label(5, 5, 4),
            Some(1),
            "middle slice should be interpolated"
        );
    }

    #[test]
    fn morphological_interpolation_only_fills_intersection() {
        let mut lm = LabelMap3D::new(10, 10, 10).expect("valid");
        // Paint different regions on slice 0 and slice 4
        for y in 0..5 {
            for x in 0..5 {
                lm.set_label(x, y, 0, 1);
            }
        }
        for y in 5..10 {
            for x in 5..10 {
                lm.set_label(x, y, 4, 1);
            }
        }
        let modified = interpolate_slices_morphological(&mut lm, 0, 4, 1).expect("interp");
        // No overlap between slices, so morphological interpolation fills nothing
        assert_eq!(
            modified, 0,
            "no intersection means no morphological interpolation"
        );
    }

    #[test]
    fn linear_interpolation_fills_with_distance_weighting() {
        let mut lm = LabelMap3D::new(10, 10, 10).expect("valid");
        // Paint same region on slice 0 and slice 4
        for y in 3..7 {
            for x in 3..7 {
                lm.set_label(x, y, 0, 1);
                lm.set_label(x, y, 4, 1);
            }
        }
        let modified = interpolate_slices_linear(&mut lm, 0, 4, 1).expect("linear interp");
        assert!(modified > 0, "should fill intermediate slices");
        // Slices closer to the boundary should be filled (weight > 0.5)
        assert_eq!(
            lm.label(5, 5, 1),
            Some(1),
            "slice 1 should be filled (close to slice 0)"
        );
    }

    #[test]
    fn interpolation_rejects_out_of_range_slices() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        let err = interpolate_slices_morphological(&mut lm, 0, 10, 1).expect_err("z out of range");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
        let err = interpolate_slices_linear(&mut lm, 10, 0, 1).expect_err("z out of range");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn interpolation_rejects_zero_label() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        let err = interpolate_slices_morphological(&mut lm, 0, 4, 0).expect_err("zero label");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
        let err = interpolate_slices_linear(&mut lm, 0, 4, 0).expect_err("zero label");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn interpolation_same_slice_returns_zero() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        let result = interpolate_slices_morphological(&mut lm, 2, 2, 1).expect("same slice");
        assert_eq!(result, 0);
        let result = interpolate_slices_linear(&mut lm, 2, 2, 1).expect("same slice");
        assert_eq!(result, 0);
    }

    #[test]
    fn threshold_segment_volume_segments_hu_range() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        // Create volume with bone-like HU values (200-1000) in some voxels
        let mut voxels = vec![0i32; 10 * 10 * 5];
        for y in 3..7 {
            for x in 3..7 {
                voxels[2 * 100 + y * 10 + x] = 400; // Bone range on slice 2
            }
        }
        let config = ThresholdConfig {
            hu_min: 200,
            hu_max: 1000,
            slice_start: 0,
            slice_end: 5,
        };
        let modified = threshold_segment_volume(&mut lm, &voxels, &config, 1).expect("threshold");
        assert_eq!(modified, 16, "4x4 region on one slice = 16 voxels");
        assert_eq!(lm.label(5, 5, 2), Some(1));
    }

    #[test]
    fn threshold_segment_volume_respects_slice_range() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        let voxels = vec![500i32; 10 * 10 * 5]; // All bone HU
        let config = ThresholdConfig {
            hu_min: 200,
            hu_max: 1000,
            slice_start: 1,
            slice_end: 3,
        };
        let modified = threshold_segment_volume(&mut lm, &voxels, &config, 1).expect("threshold");
        // Only slices 1 and 2 should be segmented
        assert_eq!(modified, 2 * 10 * 10);
        assert_eq!(
            lm.label(0, 0, 0),
            Some(0),
            "slice 0 should not be segmented"
        );
        assert_eq!(lm.label(0, 0, 1), Some(1), "slice 1 should be segmented");
        assert_eq!(
            lm.label(0, 0, 3),
            Some(0),
            "slice 3 should not be segmented"
        );
    }

    #[test]
    fn threshold_config_validation() {
        let valid = ThresholdConfig {
            hu_min: -1000,
            hu_max: 3000,
            slice_start: 0,
            slice_end: 5,
        };
        assert!(valid.validate().is_ok());
        let inverted_hu = ThresholdConfig {
            hu_min: 500,
            hu_max: 200,
            slice_start: 0,
            slice_end: 5,
        };
        assert!(inverted_hu.validate().is_err());
        let inverted_slice = ThresholdConfig {
            hu_min: 0,
            hu_max: 100,
            slice_start: 5,
            slice_end: 3,
        };
        assert!(inverted_slice.validate().is_err());
        let equal_slices = ThresholdConfig {
            hu_min: 0,
            hu_max: 100,
            slice_start: 3,
            slice_end: 3,
        };
        assert!(equal_slices.validate().is_err());
    }

    #[test]
    fn threshold_segment_rejects_wrong_voxel_buffer_size() {
        let mut lm = LabelMap3D::new(10, 10, 5).expect("valid");
        let wrong_size = vec![0i32; 100]; // Way too small
        let config = ThresholdConfig {
            hu_min: 0,
            hu_max: 100,
            slice_start: 0,
            slice_end: 5,
        };
        let err =
            threshold_segment_volume(&mut lm, &wrong_size, &config, 1).expect_err("buffer size");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn region_grow_from_seed_expands_connected_region() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        // Create a 6x6 block of uniform HU in the middle of slice 1
        let mut voxels = vec![0i32; 10 * 10 * 3];
        for y in 2..8 {
            for x in 2..8 {
                voxels[1 * 100 + y * 10 + x] = 50;
            }
        }
        let seed = RegionGrowSeed {
            x: 5,
            y: 5,
            z: 1,
            hu_min: 40,
            hu_max: 60,
        };
        let modified = region_grow(&mut lm, &voxels, &seed, 2).expect("region grow");
        assert_eq!(modified, 36, "6x6 block = 36 voxels");
        assert_eq!(lm.label(5, 5, 1), Some(2));
    }

    #[test]
    fn region_grow_stops_at_hu_boundary() {
        let mut lm = LabelMap3D::new(10, 10, 1).expect("valid"); // Single slice
                                                                 // Create a small island of target HU in the center, surrounded by different HU
        let mut voxels = vec![0i32; 10 * 10];
        // 4x4 block of target HU in the center
        for y in 3..7 {
            for x in 3..7 {
                voxels[y * 10 + x] = 50;
            }
        }
        let seed = RegionGrowSeed {
            x: 5,
            y: 5,
            z: 0,
            hu_min: 40,
            hu_max: 60,
        };
        let modified = region_grow(&mut lm, &voxels, &seed, 2).expect("region grow");
        // Should only grow within the 4x4 block = 16 voxels
        assert_eq!(
            modified, 16,
            "should be limited to the 4x4 island, got {modified}"
        );
    }

    #[test]
    fn region_grow_seed_outside_hu_range_returns_zero() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let voxels = vec![0i32; 10 * 10 * 3];
        let seed = RegionGrowSeed {
            x: 5,
            y: 5,
            z: 1,
            hu_min: 100,
            hu_max: 200,
        };
        let modified = region_grow(&mut lm, &voxels, &seed, 1).expect("region grow");
        assert_eq!(modified, 0, "seed HU outside range should return 0 voxels");
    }

    #[test]
    fn region_grow_rejects_out_of_bounds_seed() {
        let mut lm = LabelMap3D::new(10, 10, 3).expect("valid");
        let voxels = vec![0i32; 10 * 10 * 3];
        let seed = RegionGrowSeed {
            x: 20,
            y: 5,
            z: 1,
            hu_min: -1000,
            hu_max: 3000,
        };
        let err = region_grow(&mut lm, &voxels, &seed, 1).expect_err("seed out of bounds");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    // =======================================================================
    // Sprint 2 Extended Tests: ROI Statistics
    // =======================================================================

    #[test]
    fn roi_statistics_rect_computes_mean_and_stddev() {
        // 10x10 image with uniform value 100 on slice 0
        let voxels = vec![100i32; 10 * 10 * 3];
        let roi = RoiShape::Rect {
            left: 2,
            top: 2,
            right: 7,
            bottom: 7,
        };
        let stats = compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect("rect stats");
        assert_eq!(stats.voxel_count, 36, "6x6 rect = 36 voxels");
        assert!((stats.mean - 100.0).abs() < 0.01, "mean should be 100");
        assert!(
            stats.std_dev < 0.01,
            "uniform values should have ~0 std dev"
        );
        assert_eq!(stats.min, 100.0);
        assert_eq!(stats.max, 100.0);
    }

    #[test]
    fn roi_statistics_rect_with_varying_values() {
        let mut voxels = vec![0i32; 10 * 10 * 3];
        // Set a gradient in the ROI region
        for y in 2..8 {
            for x in 2..8 {
                voxels[y * 10 + x] = (x + y) as i32;
            }
        }
        let roi = RoiShape::Rect {
            left: 2,
            top: 2,
            right: 7,
            bottom: 7,
        };
        let stats = compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect("stats");
        assert_eq!(stats.voxel_count, 36);
        assert!(stats.mean > 0.0, "mean should be positive");
        assert!(
            stats.std_dev > 0.0,
            "varying values should have positive std dev"
        );
        assert!(stats.min < stats.max, "min should be less than max");
    }

    #[test]
    fn roi_statistics_ellipse_computes_correctly() {
        let voxels = vec![50i32; 20 * 20 * 3];
        let roi = RoiShape::Ellipse {
            center_col: 10.0,
            center_row: 10.0,
            semi_col: 5.0,
            semi_row: 5.0,
        };
        let stats =
            compute_roi_statistics(&voxels, 20, 20, 3, 0, &roi, None).expect("ellipse stats");
        assert!(stats.voxel_count > 0, "should include voxels in ellipse");
        assert!((stats.mean - 50.0).abs() < 0.01);
        // Approximate area: π * 5 * 5 ≈ 78.5
        assert!(
            stats.voxel_count > 60 && stats.voxel_count < 100,
            "area should be approximately π*25, got {}",
            stats.voxel_count
        );
    }

    #[test]
    fn roi_statistics_freehand_triangle() {
        let voxels = vec![42i32; 20 * 20 * 3];
        // Simple triangle
        let roi = RoiShape::Freehand {
            points: vec![(10.0, 5.0), (5.0, 15.0), (15.0, 15.0)],
        };
        let stats =
            compute_roi_statistics(&voxels, 20, 20, 3, 0, &roi, None).expect("freehand stats");
        assert!(stats.voxel_count > 0, "triangle should contain voxels");
        assert!((stats.mean - 42.0).abs() < 0.01);
    }

    #[test]
    fn roi_statistics_with_pixel_spacing() {
        let voxels = vec![100i32; 10 * 10 * 3];
        let roi = RoiShape::Rect {
            left: 0,
            top: 0,
            right: 4,
            bottom: 4,
        };
        let stats = compute_roi_statistics(
            &voxels,
            10,
            10,
            3,
            0,
            &roi,
            Some((0.5, 0.5)), // 0.5mm x 0.5mm pixel spacing
        )
        .expect("stats with spacing");
        assert_eq!(stats.voxel_count, 25);
        let expected_mm2 = 25.0 * 0.5 * 0.5;
        assert!(stats.area_mm2.is_some());
        assert!((stats.area_mm2.unwrap() - expected_mm2).abs() < 0.01);
    }

    #[test]
    fn roi_statistics_histogram_is_256_bins() {
        let voxels = vec![100i32; 10 * 10 * 3];
        let roi = RoiShape::Rect {
            left: 0,
            top: 0,
            right: 4,
            bottom: 4,
        };
        let stats = compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect("stats");
        assert_eq!(stats.histogram.len(), 256, "histogram should have 256 bins");
        assert!(
            stats.histogram.iter().sum::<u64>() > 0,
            "histogram should contain counts"
        );
    }

    #[test]
    fn roi_statistics_rejects_invalid_rect_bounds() {
        let voxels = vec![0i32; 10 * 10 * 3];
        let roi = RoiShape::Rect {
            left: 0,
            top: 0,
            right: 20,
            bottom: 20,
        }; // Exceeds dimensions
        let err =
            compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect_err("invalid rect");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn roi_statistics_rejects_invalid_freehand_too_few_points() {
        let voxels = vec![0i32; 10 * 10 * 3];
        let roi = RoiShape::Freehand {
            points: vec![(1.0, 1.0), (2.0, 2.0)],
        }; // Only 2 points
        let err =
            compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect_err("too few points");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn roi_statistics_rejects_invalid_ellipse_semi_axes() {
        let voxels = vec![0i32; 10 * 10 * 3];
        let roi = RoiShape::Ellipse {
            center_col: 5.0,
            center_row: 5.0,
            semi_col: 0.0,
            semi_row: 5.0,
        };
        let err =
            compute_roi_statistics(&voxels, 10, 10, 3, 0, &roi, None).expect_err("zero semi axis");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    #[test]
    fn roi_statistics_rejects_invalid_slice_index() {
        let voxels = vec![0i32; 10 * 10 * 3];
        let roi = RoiShape::Rect {
            left: 0,
            top: 0,
            right: 5,
            bottom: 5,
        };
        let err =
            compute_roi_statistics(&voxels, 10, 10, 3, 10, &roi, None).expect_err("invalid slice");
        assert!(matches!(err, ClinicalError::InvalidInput { .. }));
    }

    // =======================================================================
    // Sprint 2 Extended Tests: Annotation Store
    // =======================================================================

    #[test]
    fn annotation_store_add_multiple_and_remove() {
        let mut store = Annotation3dStore::new();
        let id1 = store.add("Nodule", [1.0, 2.0, 3.0]).expect("add 1");
        let id2 = store.add("Calcification", [4.0, 5.0, 6.0]).expect("add 2");
        let id3 = store.add("Scar", [7.0, 8.0, 9.0]).expect("add 3");
        assert_eq!(store.annotations().len(), 3);
        store.remove(&id2).expect("remove 2");
        assert_eq!(store.annotations().len(), 2);
        assert!(store.annotations().iter().all(|a| a.id != id2));
    }

    #[test]
    fn annotation_store_remove_nonexistent_fails() {
        let mut store = Annotation3dStore::new();
        let err = store.remove("nonexistent").expect_err("remove missing");
        assert!(matches!(err, ClinicalError::NotFound { .. }));
    }

    #[test]
    fn annotation_store_snapshot_restore_roundtrip() {
        let mut store = Annotation3dStore::new();
        store.add("A", [1.0, 2.0, 3.0]).expect("add");
        store.add("B", [4.0, 5.0, 6.0]).expect("add");
        let snap = store.snapshot();
        let mut restored = Annotation3dStore::new();
        restored.restore_from_snapshot(snap);
        assert_eq!(restored.annotations().len(), 2);
        assert!(restored.export_json().contains("\"label\":\"A\""));
        assert!(restored.export_json().contains("\"label\":\"B\""));
    }

    // =======================================================================
    // Sprint 2 Extended Tests: Segmentation Store lifecycle
    // =======================================================================

    #[test]
    fn segmentation_store_import_and_remove() {
        let mut store = SegmentationStore::new();
        let id1 = store.import_seg("Liver").expect("import");
        let id2 = store.create_labelmap("Kidney").expect("create");
        assert_eq!(store.active_segments().len(), 2);
        store.remove(&id1).expect("remove liver");
        assert_eq!(store.active_segments().len(), 1);
        assert!(store.segment(&id2).is_some());
    }

    #[test]
    fn segmentation_store_remove_nonexistent_fails() {
        let mut store = SegmentationStore::new();
        let err = store.remove("nonexistent").expect_err("remove missing");
        assert!(matches!(err, ClinicalError::NotFound { .. }));
    }

    #[test]
    fn segmentation_store_locked_segment_rejects_style_update() {
        let mut store = SegmentationStore::new();
        let id = store.create_labelmap("Tumor").expect("create");
        store.set_locked(&id, true).expect("lock");
        let err = store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: false,
                    opacity: 0.1,
                    color_rgb: [0, 0, 0],
                    active: false,
                },
            )
            .expect_err("update locked");
        assert!(matches!(err, ClinicalError::Locked { .. }));
    }

    #[test]
    fn segmentation_store_unlock_allows_mutations() {
        let mut store = SegmentationStore::new();
        let id = store.create_labelmap("Tumor").expect("create");
        store.set_locked(&id, true).expect("lock");
        store.set_locked(&id, false).expect("unlock");
        store
            .update_style(
                &id,
                SegmentationStyle {
                    visible: false,
                    opacity: 0.1,
                    color_rgb: [0, 0, 0],
                    active: false,
                },
            )
            .expect("update after unlock should work");
    }

    // =======================================================================
    // Sprint 2 Extended Tests: Measurement Store extended
    // =======================================================================

    #[test]
    fn measurement_store_multiple_measurements() {
        let mut store = MeasurementStore::new();
        let id1 = store
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![
                    ImagePoint { x: 0.0, y: 0.0 },
                    ImagePoint { x: 10.0, y: 0.0 },
                ],
                Some(10.0),
                MeasurementUnit::Millimeter,
            )
            .expect("create dist");
        let id2 = store
            .create_measurement(
                MeasurementKind::Angle2D,
                vec![
                    ImagePoint { x: 0.0, y: 0.0 },
                    ImagePoint { x: 5.0, y: 5.0 },
                    ImagePoint { x: 10.0, y: 0.0 },
                ],
                Some(45.0),
                MeasurementUnit::Degree,
            )
            .expect("create angle");
        assert_eq!(store.active_measurements().len(), 2);
        assert!(store.measurement(&id1).is_some());
        assert!(store.measurement(&id2).is_some());
    }

    #[test]
    fn measurement_store_delete_nonexistent_fails() {
        let mut store = MeasurementStore::new();
        let err = store
            .delete_measurement("nonexistent")
            .expect_err("delete missing");
        assert!(matches!(err, ClinicalError::NotFound { .. }));
    }

    #[test]
    fn measurement_store_undo_redo_exhaustion() {
        let mut store = MeasurementStore::new();
        // No undo when empty
        assert!(!store.undo());
        assert!(!store.redo());
        let id = store
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![ImagePoint { x: 0.0, y: 0.0 }, ImagePoint { x: 5.0, y: 5.0 }],
                Some(7.07),
                MeasurementUnit::Millimeter,
            )
            .expect("create");
        store.delete_measurement(&id).expect("delete");
        assert!(store.undo()); // undo delete
        assert!(store.redo()); // redo delete
        assert!(!store.redo()); // nothing more to redo
    }

    #[test]
    fn measurement_store_sr_payload_reports_correct_count() {
        let mut store = MeasurementStore::new();
        store
            .create_measurement(
                MeasurementKind::Distance2D,
                vec![ImagePoint { x: 0.0, y: 0.0 }, ImagePoint { x: 5.0, y: 5.0 }],
                Some(7.07),
                MeasurementUnit::Millimeter,
            )
            .expect("create 1");
        store
            .create_measurement(
                MeasurementKind::Probe,
                vec![ImagePoint { x: 3.0, y: 3.0 }],
                Some(45.0),
                MeasurementUnit::Pixel,
            )
            .expect("create 2");
        let payload = store.sr_payload();
        assert_eq!(payload.measurement_count, 2);
    }
