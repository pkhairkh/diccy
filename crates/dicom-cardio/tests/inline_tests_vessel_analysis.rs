// Auto-extracted from /home/z/diccy/crates/dicom-cardio/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_cardio::*;

    #[test]
    fn centerline_point_addition() {
        let mut cl = VesselCenterline::new(CoronaryArtery::Lad);
        cl.add_point(0.0, 0.0, 0.0, 3.5);
        cl.add_point(10.0, 0.0, 0.0, 3.0);
        assert_eq!(cl.points.len(), 2);
        assert!((cl.points[1].distance_from_ostium - 10.0).abs() < 1e-6);
    }

    #[test]
    fn stenosis_detection() {
        let mut cl = VesselCenterline::new(CoronaryArtery::Lad);
        cl.add_point(0.0, 0.0, 0.0, 3.5); // Normal
        cl.add_point(10.0, 0.0, 0.0, 3.5); // Normal
        cl.add_point(20.0, 0.0, 0.0, 1.5); // 57% stenosis
        cl.add_point(30.0, 0.0, 0.0, 3.5); // Normal

        let stenoses = cl.find_stenoses(3.5);
        assert_eq!(stenoses.len(), 1);
        let (location, pct) = stenoses[0];
        assert!((pct - 57.14).abs() < 1.0);
    }

    #[test]
    fn curved_mpr_slices() {
        let mut cl = VesselCenterline::new(CoronaryArtery::Rca);
        for i in 0..10 {
            cl.add_point(i as f64 * 5.0, 0.0, 0.0, 3.5);
        }

        let slices = cl.curved_mpr_slices(5);
        assert_eq!(slices.len(), 5);
    }

    #[test]
    fn extract_centerline_stub() {
        let volume = vec![0.0; 100 * 100 * 50];
        let cl = extract_centerline(&volume, 100, 100, 50, (50, 50, 25), CoronaryArtery::Lad);
        assert_eq!(cl.artery, CoronaryArtery::Lad);
        assert!(!cl.points.is_empty());
        assert!(cl.total_length > 0.0);
    }

    #[test]
    fn no_stenosis_below_threshold() {
        let mut cl = VesselCenterline::new(CoronaryArtery::Lad);
        cl.add_point(0.0, 0.0, 0.0, 3.5);
        cl.add_point(10.0, 0.0, 0.0, 3.0); // Only 14% stenosis

        let stenoses = cl.find_stenoses(3.5);
        assert!(stenoses.is_empty()); // Below 25% threshold
    }

    #[test]
    fn vessel_tapering() {
        let cl = extract_centerline(
            &vec![0.0; 1000],
            10,
            10,
            10,
            (5, 5, 5),
            CoronaryArtery::LeftMain,
        );
        // First point should have larger diameter than last
        let first_d = cl.points.first().map(|p| p.diameter).unwrap_or(0.0);
        let last_d = cl.points.last().map(|p| p.diameter).unwrap_or(0.0);
        assert!(first_d > last_d, "Vessel should taper distally");
    }
