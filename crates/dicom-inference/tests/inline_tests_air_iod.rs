// Auto-extracted from /home/z/diccy/crates/dicom-inference/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_inference::*;
    use dicom_core::{Tag};

    fn test_algorithm() -> AlgorithmIdentification {
        AlgorithmIdentification::new("TestAI", "1.0.0", "1.2.840.113619.6.1")
    }

    fn test_encoder() -> AirIodEncoder {
        AirIodEncoder::new(test_algorithm())
    }

    #[test]
    fn cade_finding_encoding() {
        let encoder = test_encoder();
        let finding = CADeFinding {
            finding_uid: "1.2.840.113619.6.5.f.0".to_string(),
            bounding_box: (10.0, 20.0, 50.0, 60.0),
            probability: 0.95,
            finding_type_code: "9540000".to_string(), // SNOMED CT: Pneumothorax
            finding_type_display: "Pneumothorax".to_string(),
            coding_scheme: "SCT".to_string(),
        };

        let ds = encoder.encode_cade_finding(&finding);
        assert!(!ds.is_empty());
    }

    #[test]
    fn cadx_finding_encoding() {
        let encoder = test_encoder();
        let finding = CADxFinding {
            finding_uid: "1.2.840.113619.6.5.c.0".to_string(),
            classification: "malignant".to_string(),
            probability: 0.85,
            algorithm: test_algorithm(),
            finding_type_code: "439569004".to_string(),
            finding_type_display: "AI-assisted diagnosis".to_string(),
            coding_scheme: "SCT".to_string(),
        };

        let ds = encoder.encode_cadx_finding(&finding);
        assert!(!ds.is_empty());
    }

    #[test]
    fn algorithm_identification_creation() {
        let algo = AlgorithmIdentification::new("LungAI", "2.0.0", "1.2.840.113619.6.2");
        assert_eq!(algo.name, "LungAI");
        assert_eq!(algo.version, "2.0.0");
        assert_eq!(algo.uid, "1.2.840.113619.6.2");
    }

    #[test]
    fn algorithm_identification_validation_rejects_empty_name() {
        let algo = AlgorithmIdentification::new("", "1.0.0", "1.2.3");
        assert!(algo.validate().is_err());
    }

    #[test]
    fn algorithm_identification_validation_rejects_empty_version() {
        let algo = AlgorithmIdentification::new("AI", "", "1.2.3");
        assert!(algo.validate().is_err());
    }

    #[test]
    fn algorithm_identification_validation_rejects_empty_uid() {
        let algo = AlgorithmIdentification::new("AI", "1.0.0", "");
        assert!(algo.validate().is_err());
    }

    #[test]
    fn full_air_iod_encoding_with_detection() {
        let encoder = test_encoder();
        let result = InferenceResult::Detection(vec![DetectionBox {
            x_min: 10.0,
            y_min: 20.0,
            x_max: 50.0,
            y_max: 60.0,
            probability: 0.95,
            finding_type: "pneumothorax".to_string(),
        }]);

        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.6").expect("encode");
        assert!(!ds.is_empty());

        // Check SOP Class UID
        let sop_class = ds.get(Tag(0x0008, 0x0016));
        assert!(sop_class.is_some());
    }

    #[test]
    fn full_air_iod_encoding_with_classification() {
        let encoder = test_encoder();
        let result = InferenceResult::Classification(vec![ClassScore {
            label: "malignant".to_string(),
            probability: 0.85,
        }]);

        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.6").expect("encode");
        assert!(!ds.is_empty());
    }

    #[test]
    fn full_air_iod_encoding_with_segmentation() {
        let encoder = test_encoder();
        let result = InferenceResult::Segmentation {
            labels: vec![0u16, 1, 1, 0],
            width: 2,
            height: 2,
            depth: 1,
        };

        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.6").expect("encode");
        assert!(!ds.is_empty());
    }

    #[test]
    fn air_iod_rejects_invalid_algorithm() {
        let algo = AlgorithmIdentification::new("", "1.0.0", "1.2.3");
        let encoder = AirIodEncoder::new(algo);
        let result = InferenceResult::Detection(vec![]);
        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.6");
        assert!(ds.is_err());
    }

    #[test]
    fn air_iod_roundtrip_preserves_study_uid() {
        let encoder = test_encoder();
        let result = InferenceResult::Detection(vec![DetectionBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 10.0,
            y_max: 10.0,
            probability: 0.9,
            finding_type: "nodule".to_string(),
        }]);

        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.6").expect("encode");
        let study_uid = ds.get_uid(Tag(0x0020, 0x000D));
        assert_eq!(study_uid, Some("1.2.3.4.5"));
    }

    #[test]
    fn air_iod_preserves_series_uid() {
        let encoder = test_encoder();
        let result = InferenceResult::Classification(vec![ClassScore {
            label: "benign".to_string(),
            probability: 0.3,
        }]);

        let ds = encode_air_iod(&encoder, &result, "1.2.3.4.5", "1.2.3.4.7").expect("encode");
        let series_uid = ds.get_uid(Tag(0x0020, 0x000E));
        assert_eq!(series_uid, Some("1.2.3.4.7"));
    }
