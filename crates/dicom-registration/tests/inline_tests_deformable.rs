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

    #[test]
    fn bspline_control_point_initialization() {
        let bspline = BSplineTransform::new((4, 4, 4), (2.0, 2.0, 2.0)).expect("bspline");
        assert_eq!(bspline.grid_dimensions, (4, 4, 4));

        // All control points should be zero-initialized
        for iz in 0..4 {
            for iy in 0..4 {
                for ix in 0..4 {
                    let cp = bspline.control_point(ix, iy, iz).expect("cp");
                    assert_eq!(cp, [0.0, 0.0, 0.0]);
                }
            }
        }
    }

    #[test]
    fn bspline_rejects_zero_dimensions() {
        let result = BSplineTransform::new((0, 4, 4), (2.0, 2.0, 2.0));
        assert!(result.is_err());
    }

    #[test]
    fn bspline_rejects_non_positive_spacing() {
        let result = BSplineTransform::new((4, 4, 4), (-1.0, 2.0, 2.0));
        assert!(result.is_err());
    }

    #[test]
    fn dvf_creation_and_identity() {
        let dvf = DeformationVectorField::identity(4, 4, 4).expect("dvf");
        assert_eq!(dvf.width, 4);
        assert_eq!(dvf.height, 4);
        assert_eq!(dvf.depth, 4);

        // All displacements should be zero
        for z in 0..4 {
            for y in 0..4 {
                for x in 0..4 {
                    let d = dvf.displacement(x, y, z).expect("displacement");
                    assert_eq!(d, [0.0, 0.0, 0.0]);
                }
            }
        }
    }

    #[test]
    fn identity_dvf_produces_same_image() {
        let vol = small_volume();
        let dvf = DeformationVectorField::identity(8, 8, 8).expect("dvf");
        let result = dvf.apply_to_volume(&vol).expect("apply");

        // Identity DVF should produce the same image
        for z in 0..8 {
            for y in 0..8 {
                for x in 0..8 {
                    let orig = vol.voxel(x, y, z).unwrap();
                    let resampled = result.voxel(x, y, z).unwrap();
                    assert!(
                        (orig - resampled).abs() < 1e-10,
                        "mismatch at ({x},{y},{z}): {orig} vs {resampled}"
                    );
                }
            }
        }
    }

    #[test]
    fn known_translation_dvf() {
        let vol = small_volume();
        let dvf = DeformationVectorField::uniform_translation(8, 8, 8, 2.0, 0.0, 0.0).expect("dvf");

        // Check displacements
        let d = dvf.displacement(0, 0, 0).expect("displacement");
        assert!((d[0] - 2.0).abs() < 1e-10);
        assert!((d[1]).abs() < 1e-10);
        assert!((d[2]).abs() < 1e-10);

        // Apply should shift the image: at output position (0,0,0), the DVF
        // adds displacement 2.0mm to the source lookup, so source voxel (2,0,0)
        let result = dvf.apply_to_volume(&vol).expect("apply");
        let expected = vol.voxel(2, 0, 0).unwrap();
        let actual = result.voxel(0, 0, 0).unwrap();
        assert!(
            (expected - actual).abs() < 1e-10,
            "expected {expected} got {actual}"
        );
    }

    #[test]
    fn dvf_compose() {
        let dvf1 =
            DeformationVectorField::uniform_translation(8, 8, 8, 2.0, 0.0, 0.0).expect("dvf1");
        let dvf2 =
            DeformationVectorField::uniform_translation(8, 8, 8, 3.0, 0.0, 0.0).expect("dvf2");

        let composed = dvf1.compose(&dvf2).expect("compose");

        // Composed displacement at any voxel should be [5.0, 0.0, 0.0]
        let d = composed.displacement(4, 4, 4).expect("displacement");
        assert!((d[0] - 5.0).abs() < 1e-10, "expected dx=5.0, got {}", d[0]);
    }

    #[test]
    fn dvf_rejects_dimension_mismatch() {
        let dvf = DeformationVectorField::identity(4, 4, 4).expect("dvf");
        let vol = VolumeGrid::new(8, 8, 8, (1.0, 1.0, 1.0)).expect("volume");
        let result = dvf.apply_to_volume(&vol);
        assert!(result.is_err());
    }

    #[test]
    fn dicom_deformable_registration_iod_encoding() {
        let dvf = DeformationVectorField::identity(8, 8, 8).expect("dvf");
        let bspline = BSplineTransform::new((4, 4, 4), (2.0, 2.0, 2.0)).expect("bspline");

        let ds = encode_deformable_registration_iod(&dvf, &bspline, "1.2.3.4.5", "1.2.3.4.6")
            .expect("encode");
        assert!(!ds.is_empty());

        // Check SOP Class UID is present
        let sop_class = ds.get(Tag(0x0008, 0x0016));
        assert!(sop_class.is_some());
    }

    #[test]
    fn dvf_from_bspline_identity() {
        let bspline = BSplineTransform::new((4, 4, 4), (2.0, 2.0, 2.0)).expect("bspline");
        let vol = small_volume();
        let dvf = DeformationVectorField::from_bspline(&bspline, &vol).expect("dvf");

        // Identity bspline should produce zero displacements
        let d = dvf.displacement(4, 4, 4).expect("displacement");
        assert!(
            d[0].abs() < 1e-10 && d[1].abs() < 1e-10 && d[2].abs() < 1e-10,
            "identity bspline should produce zero displacement, got {:?}",
            d
        );
    }

    #[test]
    fn dvf_from_bspline_with_displacement() {
        let mut bspline = BSplineTransform::new((4, 4, 4), (8.0, 8.0, 8.0)).expect("bspline");
        bspline.set_control_point(1, 1, 1, [5.0, 3.0, 0.0]);

        let vol = VolumeGrid::new(8, 8, 8, (1.0, 1.0, 1.0)).expect("volume");
        let dvf = DeformationVectorField::from_bspline(&bspline, &vol).expect("dvf");

        // At least some displacements should be non-zero
        let d = dvf.displacement(4, 4, 4).expect("displacement");
        assert!(
            d[0].abs() > 0.0 || d[1].abs() > 0.0,
            "expected non-zero displacement near control point, got {:?}",
            d
        );
    }

    #[test]
    fn deformable_register_on_identical_volumes() {
        let vol = small_volume();
        let config = DeformableRegistrationConfig {
            grid_spacing: (8.0, 8.0, 8.0),
            max_iterations: 5,
            similarity_threshold: 1e-5,
        };

        let (bspline, dvf, metric) =
            deformable_register(&vol, &vol, &config).expect("deformable register");

        // MSE for identical volumes should be very low
        assert!(
            metric < 1.0,
            "expected low MSE for identical volumes, got {metric}"
        );
    }
