// Auto-extracted from /home/z/diccy/crates/dicom-test-util/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_test_util::*;
    use dicom_core::{Dataset, Error, ErrorKind, Tag};

    #[test]
    fn make_minimal_dataset_has_patient_id() {
        let ds = make_minimal_dataset();
        let patient_id = ds.get_str(Tag(0x0010, 0x0020));
        assert_eq!(patient_id, Some("TEST-PATIENT"));
    }

    #[test]
    fn make_minimal_dataset_has_study_uid() {
        let ds = make_minimal_dataset();
        let study_uid = ds.get_uid(Tag(0x0020, 0x000D));
        assert_eq!(study_uid, Some("1.2.3.4.5"));
    }

    #[test]
    fn make_ct_dataset_has_modality() {
        let ds = make_ct_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("CT"));
    }

    #[test]
    fn make_mr_dataset_has_modality() {
        let ds = make_mr_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("MR"));
    }

    #[test]
    fn make_sr_dataset_has_modality() {
        let ds = make_sr_dataset();
        let modality = ds.get_str(Tag(0x0008, 0x0060));
        assert_eq!(modality, Some("SR"));
    }

    #[test]
    fn minimal_p10_has_dicm_magic() {
        let bytes = minimal_p10_bytes();
        assert!(bytes.len() > 132, "P10 data must include preamble + magic");
        assert_eq!(&bytes[128..132], b"DICM", "P10 magic must be DICM");
    }

    #[test]
    fn minimal_p10_contains_meta_group() {
        let bytes = minimal_p10_bytes();
        // After the 132-byte preamble, the first element should be (0002,0000)
        assert!(bytes.len() > 140);
        let group = u16::from_le_bytes([bytes[132], bytes[133]]);
        let element = u16::from_le_bytes([bytes[134], bytes[135]]);
        assert_eq!(group, 0x0002);
        assert_eq!(element, 0x0000);
    }

    #[test]
    fn test_limits_are_valid() {
        let limits = test_limits();
        assert!(limits.validate().is_ok(), "test limits should validate");
    }

    #[test]
    fn assert_error_kind_macro_works() {
        let result: Result<Dataset, Box<Error>> = Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "test",
                observed: 100,
                allowed: 50,
            },
            "test error",
        )
        .into());
        assert_error_kind!(result, ErrorKind::LimitExceeded { .. });
    }

    #[test]
    fn assert_error_code_macro_works() {
        let result: Result<Dataset, Box<Error>> = Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "test",
                observed: 100,
                allowed: 50,
            },
            "test error",
        )
        .into());
        assert_error_code!(result, "DVF.SECURITY.LIMIT_EXCEEDED");
    }

    #[test]
    fn dicom_p10_preamble_is_all_zeros_then_dicm() {
        for i in 0..128 {
            assert_eq!(DICOM_P10_PREAMBLE[i], 0, "preamble byte {i} should be zero");
        }
        assert_eq!(&DICOM_P10_PREAMBLE[128..132], b"DICM");
    }
