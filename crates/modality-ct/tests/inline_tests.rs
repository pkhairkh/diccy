// Auto-extracted from /home/z/diccy/crates/modality-ct/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use modality_ct::*;

    fn base_slice(z: f64) -> CtGeometry {
        CtGeometry {
            rows: 512,
            cols: 512,
            pixel_spacing: (0.5, 0.5),
            iop: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            ipp: [0.0, 0.0, z],
        }
    }

    #[test]
    fn enhanced_ct_requires_pack() {
        // REQ-FEAT-302, REQ-SOP-301: enhanced CT requires explicit pack feature.
        if dicom_core::capabilities().pack_enhanced() {
            CtPack::ensure_enhanced_ct_supported().expect("pack-enhanced enabled");
        } else {
            let err = CtPack::ensure_enhanced_ct_supported().unwrap_err();
            assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
        }
    }

    #[test]
    fn ct_geometry_missing_required_tag() {
        // REQ-CONF-032: missing geometry tags must reject CT series assembly.
        let input = CtGeometryInput {
            rows: Some(512),
            cols: Some(512),
            pixel_spacing: Some((0.5, 0.5)),
            iop: Some([1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
            ipp: None,
        };
        let err = validate_ct_geometry(input, 1e-4).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    fn ct_geometry_rejects_non_orthonormal_iop() {
        // REQ-CONF-031: IOP must be approximately orthonormal.
        let input = CtGeometryInput {
            rows: Some(512),
            cols: Some(512),
            pixel_spacing: Some((0.5, 0.5)),
            iop: Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            ipp: Some([0.0, 0.0, 0.0]),
        };
        let err = validate_ct_geometry(input, 1e-4).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
    }

    #[test]
    fn ct_slice_ordering_by_projection() {
        // REQ-CONF-030: order slices by IPP projection onto normal.
        let slices = vec![base_slice(5.0), base_slice(1.0), base_slice(3.0)];
        let order = order_slices_by_ipp(&slices).expect("order slices");
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn ct_volume_geometry_checks() {
        // REQ-VOL-903/904/905: enforce geometry consistency and monotonic positions.
        let slices = vec![base_slice(0.0), base_slice(1.0), base_slice(2.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        validate_volume_geometry(&slices, tol).expect("volume geometry ok");
    }

    #[test]
    fn ct_volume_geometry_rejects_spacing_mismatch() {
        // REQ-VOL-903/905: mismatched spacing fails closed.
        let mut slices = vec![base_slice(0.0), base_slice(1.0)];
        slices[1].pixel_spacing = (0.8, 0.5);
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        let err = validate_volume_geometry(&slices, tol).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    #[test]
    fn ct_slice_spacing_single_slice() {
        // REQ-VOL-906: nz < 2 defaults to 1.0 mm and unknown.
        let slices = vec![base_slice(0.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 1e-4,
        };
        let spacing = compute_slice_spacing(&slices, tol, false).expect("spacing");
        assert!(spacing.is_unknown());
        assert_eq!(spacing.spacing_mm(), 1.0);
    }

    #[test]
    fn ct_slice_spacing_rejects_non_uniform() {
        // REQ-VOL-908: non-uniform spacing fails closed unless allowed.
        let slices = vec![base_slice(0.0), base_slice(1.0), base_slice(3.0)];
        let tol = GeometryTolerance {
            iop_epsilon: 1e-4,
            spacing_epsilon: 1e-4,
            slice_spacing_epsilon: 0.1,
        };
        let err = compute_slice_spacing(&slices, tol, false).unwrap_err();
        assert_eq!(err.code(), "DVF.GEOM.INVALID");
    }

    #[test]
    #[cfg(feature = "modality-ct")]
    fn ct_measure_distance_mm_requires_spacing() {
        // REQ-MEAS-020: missing pixel spacing must fail closed in mm mode.
        let err = measure_distance_mm((0.0, 0.0), (1.0, 1.0), None).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.MISSING_TAG");
    }

    #[test]
    #[cfg(not(feature = "modality-ct"))]
    fn ct_measure_distance_mm_rejects_when_pack_disabled() {
        // REQ-FEAT-302: physical measurements require CT pack feature.
        let err = measure_distance_mm((0.0, 0.0), (1.0, 1.0), None).unwrap_err();
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    #[cfg(feature = "modality-ct")]
    fn ct_measure_distance_mm_provenance() {
        // REQ-MEAS-050/070: calibrated mm outputs include provenance.
        let measurement =
            measure_distance_mm((0.0, 0.0), (2.0, 0.0), Some((0.5, 0.5))).expect("distance mm");
        assert_eq!(measurement.unit, MeasurementUnit::Millimeters);
        assert!(measurement.calibrated);
        assert_eq!(measurement.provenance, Some(TAG_PIXEL_SPACING));
    }

    #[test]
    fn ct_angle_zero_length_rejected() {
        // REQ-MEAS-060: zero-length segments fail closed.
        let err = measure_angle_degrees((0.0, 0.0), (0.0, 0.0), (1.0, 0.0)).unwrap_err();
        assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
    }

    #[test]
    fn ct_format_fixed_deterministic() {
        // REQ-MEAS-080: fixed precision formatting is deterministic.
        let formatted = format_fixed(1.2345, 2).expect("format");
        assert_eq!(formatted, "1.23");
    }
