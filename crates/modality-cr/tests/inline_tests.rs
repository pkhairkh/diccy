// Auto-extracted from /home/z/diccy/crates/modality-cr/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use modality_cr::*;

    #[test]
    #[cfg(not(feature = "modality-cr"))]
    fn cr_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301: CR pack requires explicit Cargo feature.
        assert!(!CrPack::enabled());
        let err = CrPack::ensure_cr_supported(SOP_CLASS_CR).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "modality-cr")]
    fn cr_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301: CR pack requires explicit Cargo feature.
        assert!(CrPack::enabled());
        CrPack::ensure_cr_supported(SOP_CLASS_CR).expect("cr pack enabled");
    }

    #[test]
    fn cr_measurements_gate() {
        // REQ-MEAS-010
        if CrPack::enabled() {
            CrPack::ensure_physical_measurements_enabled().expect("cr pack enabled");
        } else {
            let err = CrPack::ensure_physical_measurements_enabled().unwrap_err();
            assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
        }
    }
