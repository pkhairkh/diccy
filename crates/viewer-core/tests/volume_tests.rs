// Auto-extracted from /home/z/diccy/crates/viewer-core/src/volume.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::{LegacyVolumeGrid, SlicePlane, VolumeAssemblyConfig, VolumeError, VolumeGrid};

    fn slice(uid: &str, position_mm: i64, spacing_um: u64, pixels: [i32; 4]) -> SlicePlane {
        SlicePlane {
            instance_uid: uid.to_string(),
            position_mm,
            spacing_um,
            width: 2,
            height: 2,
            pixels: pixels.to_vec(),
        }
    }

    #[test]
    fn deterministic_slice_ordering_uses_position_then_uid() {
        let slices = vec![
            slice("1.2.3.b", 10, 1_000, [5, 6, 7, 8]),
            slice("1.2.3.a", 10, 1_000, [1, 2, 3, 4]),
            slice("1.2.3.c", 20, 1_000, [9, 10, 11, 12]),
        ];
        let volume =
            VolumeGrid::from_slices(&slices, VolumeAssemblyConfig::default()).expect("volume");
        assert_eq!(
            volume.source_instance_uids(),
            &[
                "1.2.3.a".to_string(),
                "1.2.3.b".to_string(),
                "1.2.3.c".to_string()
            ]
        );
        assert_eq!(volume.voxel(0, 0, 0), Some(1));
        assert_eq!(volume.voxel(1, 1, 2), Some(12));
    }

    #[test]
    fn non_uniform_spacing_fails_closed_by_default() {
        let slices = vec![
            slice("1.2.3.a", 0, 1_000, [1, 2, 3, 4]),
            slice("1.2.3.b", 1, 1_500, [5, 6, 7, 8]),
        ];
        let err = VolumeGrid::from_slices(&slices, VolumeAssemblyConfig::default())
            .expect_err("must fail");
        assert_eq!(err, VolumeError::NonUniformSpacing);
        assert_eq!(err.code(), "DVF.VOLUME.NON_UNIFORM_SPACING");
    }

    #[test]
    fn legacy_migration_adapter_adds_patient_geometry() {
        let legacy = LegacyVolumeGrid {
            dimensions: [2, 2, 1],
            spacing_um: [1000, 2000, 3000],
            orientation_ras: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            source_instance_uids: vec!["1.2.3".to_string()],
            voxels: vec![1, 2, 3, 4],
            non_uniform_spacing: false,
        };
        let migrated = VolumeGrid::migrated_from_legacy(legacy, Some("1.2.3.4".to_string()));
        let geometry = migrated.patient_geometry().expect("patient geometry");
        assert_eq!(geometry.origin_um, [0, 0, 0]);
        assert_eq!(geometry.basis_vectors_um[0], [1000, 0, 0]);
        assert_eq!(geometry.basis_vectors_um[1], [0, 2000, 0]);
        assert_eq!(geometry.basis_vectors_um[2], [0, 0, 3000]);
    }

    #[test]
    fn voxel_to_patient_mapping_is_deterministic() {
        let mut volume = VolumeGrid::new([3, 3, 3], [1000, 2000, 3000]).expect("volume");
        volume.migrate_patient_geometry_from_legacy(None);
        assert_eq!(volume.voxel_to_patient_um(0, 0, 0), Some([0, 0, 0]));
        assert_eq!(
            volume.voxel_to_patient_um(2, 1, 1),
            Some([2000, 2000, 3000])
        );
        let voxel = volume
            .patient_to_voxel_f64([2000.0, 2000.0, 3000.0])
            .expect("patient->voxel");
        assert!((voxel[0] - 2.0).abs() < 1e-9);
        assert!((voxel[1] - 1.0).abs() < 1e-9);
        assert!((voxel[2] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn serialization_compatibility_across_schema_versions() {
        let legacy = LegacyVolumeGrid {
            dimensions: [2, 2, 1],
            spacing_um: [1000, 1000, 1000],
            orientation_ras: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
            source_instance_uids: vec!["1.2.3".to_string()],
            voxels: vec![10, 20, 30, 40],
            non_uniform_spacing: false,
        };
        let legacy_json = serde_json::to_string(&legacy).expect("serialize legacy");
        let upgraded: VolumeGrid = serde_json::from_str(&legacy_json).expect("upgrade deserialize");
        assert!(upgraded.patient_geometry().is_none());

        let migrated = VolumeGrid::migrated_from_legacy(legacy.clone(), None);
        let current_json = serde_json::to_string(&migrated).expect("serialize current");
        let roundtrip: VolumeGrid = serde_json::from_str(&current_json).expect("roundtrip current");
        assert_eq!(roundtrip, migrated);

        let legacy_roundtrip: LegacyVolumeGrid =
            serde_json::from_str(&legacy_json).expect("legacy roundtrip");
        assert_eq!(legacy_roundtrip, legacy);
    }
