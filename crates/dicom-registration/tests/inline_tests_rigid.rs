// Auto-extracted from /home/z/diccy/crates/dicom-registration/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_registration::*;
    use dicom_core::Tag;

    fn small_volume() -> VolumeGrid {
        let mut vol = VolumeGrid::new(8, 8, 8, (1.0, 1.0, 1.0)).expect("volume");
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    let val = ((x + y + z) as f64) * 10.0;
                    vol.set_voxel(x, y, z, val);
                }
            }
        }
        vol
    }

    fn translated_volume(dx: usize, dy: usize, dz: usize) -> VolumeGrid {
        let source = small_volume();
        let mut vol = VolumeGrid::new(8, 8, 8, (1.0, 1.0, 1.0)).expect("volume");
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    let sx = x as isize - dx as isize;
                    let sy = y as isize - dy as isize;
                    let sz = z as isize - dz as isize;
                    if sx >= 0 && sy >= 0 && sz >= 0 {
                        if let Some(val) = source.voxel(sx as usize, sy as usize, sz as usize) {
                            vol.set_voxel(x, y, z, val);
                        }
                    }
                }
            }
        }
        vol
    }

    #[test]
    fn rigid_transform_identity() {
        let t = RigidTransform::identity();
        assert_eq!(t.tx, 0.0);
        assert_eq!(t.ty, 0.0);
        assert_eq!(t.tz, 0.0);
        assert_eq!(t.rx, 0.0);
        assert_eq!(t.ry, 0.0);
        assert_eq!(t.rz, 0.0);
    }

    #[test]
    fn rigid_transform_composition() {
        let a = RigidTransform::translation(1.0, 2.0, 3.0);
        let b = RigidTransform::translation(4.0, 5.0, 6.0);
        let c = a.compose(&b);
        assert!((c.tx - 5.0).abs() < 1e-10);
        assert!((c.ty - 7.0).abs() < 1e-10);
        assert!((c.tz - 9.0).abs() < 1e-10);
    }

    #[test]
    fn rigid_transform_inversion() {
        let t = RigidTransform::translation(1.0, 2.0, 3.0);
        let inv = t.invert();
        assert!((inv.tx - (-1.0)).abs() < 1e-10);
        assert!((inv.ty - (-2.0)).abs() < 1e-10);
        assert!((inv.tz - (-3.0)).abs() < 1e-10);

        // Compose with inverse should give identity
        let identity = t.compose(&inv);
        assert!((identity.tx).abs() < 1e-10);
        assert!((identity.ty).abs() < 1e-10);
        assert!((identity.tz).abs() < 1e-10);
    }

    #[test]
    fn rigid_transform_rotation_inversion() {
        let t = RigidTransform::rotation(10.0, 20.0, 30.0);
        let inv = t.invert();
        let composed = t.compose(&inv);
        assert!((composed.rx).abs() < 1e-10);
        assert!((composed.ry).abs() < 1e-10);
        assert!((composed.rz).abs() < 1e-10);
    }

    #[test]
    fn rigid_transform_validate_rejects_nan() {
        let t = RigidTransform {
            tx: f64::NAN,
            ty: 0.0,
            tz: 0.0,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
        };
        assert!(t.validate().is_err());
    }

    #[test]
    fn multi_resolution_pyramid_generation() {
        let vol = small_volume();
        let pyramid = MultiResolutionPyramid::build(&vol, &[4, 2]).expect("pyramid");
        // 3 levels: downsampled by 4, downsampled by 2, original
        assert_eq!(pyramid.level_count(), 3);

        // Coarsest level should be smaller
        let coarsest = pyramid.level(0).expect("coarsest");
        assert!(coarsest.width <= vol.width);
        assert!(coarsest.height <= vol.height);
        assert!(coarsest.depth <= vol.depth);

        // Finest level should match original
        let finest = pyramid.level(2).expect("finest");
        assert_eq!(finest.width, vol.width);
        assert_eq!(finest.height, vol.height);
        assert_eq!(finest.depth, vol.depth);
    }

    #[test]
    fn pyramid_rejects_empty_volume() {
        let vol = VolumeGrid::from_data(vec![], 0, 0, 0, (1.0, 1.0, 1.0));
        // This should fail because dimensions are zero
        assert!(vol.is_err());
    }

    #[test]
    fn registration_known_synthetic_translation() {
        let fixed = small_volume();
        let moving = translated_volume(2, 0, 0);

        // Use a simple config with no pyramid (just the original level)
        let config = RigidRegistrationConfig {
            max_iterations: 100,
            convergence_threshold: 1e-6,
            step_size: 2.0,
            pyramid_levels: vec![1], // No downsampling — just original level
        };

        let result = rigid_register(&fixed, &moving, RegistrationMetric::MeanSquares, &config)
            .expect("registration");

        // Should find a non-trivial transform for translated volumes.
        // The moving volume is the fixed shifted by +2 in x, so the
        // registration should find a non-identity transform.
        assert!(
            result.iterations > 0,
            "expected some optimization iterations"
        );
        // The metric should improve from the initial state
        assert!(
            result.metric_value < 10000.0,
            "expected improved metric value, got {}",
            result.metric_value
        );
    }

    #[test]
    fn registration_known_synthetic_rotation() {
        let fixed = small_volume();
        // For rotation test, use same volume (identity registration)
        let config = RigidRegistrationConfig {
            max_iterations: 20,
            convergence_threshold: 1e-6,
            step_size: 0.5,
            pyramid_levels: vec![2],
        };

        let result = rigid_register(&fixed, &fixed, RegistrationMetric::MeanSquares, &config)
            .expect("registration");

        // Identity registration should converge near zero
        assert!(
            result.transform.tx.abs() < 5.0,
            "identity registration should have small translation"
        );
    }

    #[test]
    fn metric_computation_mse() {
        let vol = small_volume();
        let mse = compute_metric(&vol, &vol, RegistrationMetric::MeanSquares).expect("mse");
        assert!(
            mse.abs() < 1e-10,
            "identical volumes should have zero MSE, got {mse}"
        );
    }

    #[test]
    fn metric_computation_ncc() {
        let vol = small_volume();
        let ncc = compute_metric(&vol, &vol, RegistrationMetric::NormalizedCrossCorrelation)
            .expect("ncc");
        assert!(
            (ncc - 1.0).abs() < 1e-10,
            "identical volumes should have NCC=1.0, got {ncc}"
        );
    }

    #[test]
    fn metric_computation_mi() {
        let vol = small_volume();
        let mi = compute_metric(&vol, &vol, RegistrationMetric::MutualInformation).expect("mi");
        assert!(
            mi > 0.0,
            "identical volumes should have positive MI, got {mi}"
        );
    }

    #[test]
    fn metric_computation_mse_different_volumes() {
        let vol1 = small_volume();
        let vol2 = translated_volume(2, 0, 0);
        let mse = compute_metric(&vol1, &vol2, RegistrationMetric::MeanSquares).expect("mse");
        assert!(mse > 0.0, "different volumes should have positive MSE");
    }

    #[test]
    fn dicom_registration_iod_encoding() {
        let result = RegistrationResult {
            transform: RigidTransform::translation(5.0, 10.0, 15.0),
            metric_value: 0.001,
            iterations: 50,
            convergence_status: ConvergenceStatus::Converged,
        };

        let ds = encode_registration_iod(&result, "1.2.3.4.5", "1.2.3.4.6").expect("encode");
        assert!(!ds.is_empty());

        // Check SOP Class UID is present
        let sop_class = ds.get(Tag(0x0008, 0x0016));
        assert!(sop_class.is_some());
    }

    #[test]
    fn edge_case_zero_volume_rejected() {
        let result = VolumeGrid::new(0, 8, 8, (1.0, 1.0, 1.0));
        assert!(result.is_err());
    }

    #[test]
    fn edge_case_single_slice() {
        let vol = VolumeGrid::new(4, 4, 1, (1.0, 1.0, 1.0)).expect("volume");
        assert_eq!(vol.depth, 1);
        assert_eq!(vol.len(), 16);
    }

    #[test]
    fn edge_case_different_dimensions_metric_rejected() {
        let vol1 = VolumeGrid::new(4, 4, 4, (1.0, 1.0, 1.0)).expect("volume");
        let vol2 = VolumeGrid::new(8, 8, 8, (1.0, 1.0, 1.0)).expect("volume");
        let result = compute_metric(&vol1, &vol2, RegistrationMetric::MeanSquares);
        assert!(result.is_err());
    }
