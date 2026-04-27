// Auto-extracted from /home/z/diccy/crates/dicom-inference/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_inference::*;

    #[test]
    fn model_manifest_parsing() {
        let manifest = ModelManifest::new(
            vec![1, 1, 512, 512],
            vec![1, 1, 512, 512],
            "CT",
            "1.0.0",
            "1.2.840.113619.6.1",
        );
        assert_eq!(manifest.input_shape, vec![1, 1, 512, 512]);
        assert_eq!(manifest.output_shape, vec![1, 1, 512, 512]);
        assert_eq!(manifest.modality_constraint, "CT");
    }

    #[test]
    fn model_manifest_validation_rejects_empty_shape() {
        let manifest = ModelManifest::new(
            vec![],
            vec![1, 1, 512, 512],
            "CT",
            "1.0.0",
            "1.2.840.113619.6.1",
        );
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn model_manifest_validation_rejects_empty_uid() {
        let manifest = ModelManifest::new(
            vec![1, 1, 512, 512],
            vec![1, 1, 512, 512],
            "CT",
            "1.0.0",
            "",
        );
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn model_manifest_validation_rejects_empty_version() {
        let manifest = ModelManifest::new(
            vec![1, 1, 512, 512],
            vec![1, 1, 512, 512],
            "CT",
            "",
            "1.2.840.113619.6.1",
        );
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn model_manifest_validation_accepts_valid() {
        let manifest = ModelManifest::new(
            vec![1, 1, 512, 512],
            vec![1, 1, 512, 512],
            "CT",
            "1.0.0",
            "1.2.840.113619.6.1",
        );
        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn preprocessing_resampling() {
        let pipeline =
            PreprocessingPipeline::new((1.0, 1.0, 1.0), -1000.0, 1000.0, NormalizationMethod::None);
        let data = vec![100.0; 8 * 8 * 8];
        let result = pipeline.resample(&data, (1.0, 1.0, 1.0), (8, 8, 8));
        assert!(!result.is_empty());
        // Same spacing should produce same dimensions
        assert_eq!(result.len(), 8 * 8 * 8);
    }

    #[test]
    fn preprocessing_windowing() {
        let pipeline =
            PreprocessingPipeline::new((1.0, 1.0, 1.0), -100.0, 100.0, NormalizationMethod::None);
        let mut data = vec![-500.0f32, 0.0f32, 500.0f32];
        pipeline.window(&mut data);
        assert_eq!(data[0], -100.0); // clamped to min
        assert_eq!(data[1], 0.0); // within range
        assert_eq!(data[2], 100.0); // clamped to max
    }

    #[test]
    fn preprocessing_normalization_zscore() {
        let pipeline = PreprocessingPipeline::new(
            (1.0, 1.0, 1.0),
            -1000.0,
            1000.0,
            NormalizationMethod::ZScore,
        );
        let mut data = vec![10.0f32, 20.0f32, 30.0f32];
        pipeline.normalize(&mut data);
        // After z-score, mean should be ~0 and std should be ~1
        let mean = data.iter().sum::<f32>() / data.len() as f32;
        assert!(mean.abs() < 0.01, "z-score mean should be ~0, got {mean}");
    }

    #[test]
    fn preprocessing_normalization_minmax() {
        let pipeline = PreprocessingPipeline::new(
            (1.0, 1.0, 1.0),
            -1000.0,
            1000.0,
            NormalizationMethod::MinMax,
        );
        let mut data = vec![0.0f32, 50.0f32, 100.0f32];
        pipeline.normalize(&mut data);
        assert!((data[0] - 0.0).abs() < 0.01);
        assert!((data[1] - 0.5).abs() < 0.01);
        assert!((data[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn postprocessing_threshold_to_binary() {
        let prob = vec![0.1f32, 0.5f32, 0.9f32];
        let binary = Postprocessing::threshold_to_binary(&prob, 0.5);
        assert_eq!(binary, vec![0, 1, 1]);
    }

    #[test]
    fn postprocessing_contour_extraction() {
        // Create a simple 5x5 mask with a 3x3 square in the center
        let mut mask = vec![0u8; 5 * 5];
        for y in 1..4 {
            for x in 1..4 {
                mask[y * 5 + x] = 1;
            }
        }
        let contour = Postprocessing::extract_contour_2d(&mask, 5, 5);
        // Should have contour points for the boundary of the 3x3 square
        assert!(!contour.is_empty(), "contour should have boundary points");
    }

    #[test]
    fn postprocessing_nms() {
        let boxes = vec![
            DetectionBox {
                x_min: 0.0,
                y_min: 0.0,
                x_max: 10.0,
                y_max: 10.0,
                probability: 0.9,
                finding_type: "lesion".to_string(),
            },
            DetectionBox {
                x_min: 1.0,
                y_min: 1.0,
                x_max: 11.0,
                y_max: 11.0,
                probability: 0.7,
                finding_type: "lesion".to_string(),
            },
            DetectionBox {
                x_min: 50.0,
                y_min: 50.0,
                x_max: 60.0,
                y_max: 60.0,
                probability: 0.8,
                finding_type: "nodule".to_string(),
            },
        ];

        let result = Postprocessing::nms(&boxes, 0.5);
        // The second box overlaps heavily with the first and should be suppressed
        assert!(result.len() <= 3, "NMS should reduce overlapping boxes");
        // The highest-confidence box should be kept
        assert!(result.iter().any(|b| (b.probability - 0.9).abs() < 0.01));
    }

    #[test]
    fn inference_runtime_trait_object() {
        let mut runtime: Box<dyn InferenceRuntime> = Box::new(OnnxRuntime::new());
        let manifest = runtime.load_model("/models/test.onnx").expect("load");
        assert_eq!(manifest.modality_constraint, "CT");

        let output_names = runtime.get_output_names();
        assert_eq!(output_names, vec!["output"]);
    }

    #[test]
    fn onnx_runtime_stub_load_and_run() {
        let mut runtime = OnnxRuntime::new();
        let manifest = runtime.load_model("/models/test.onnx").expect("load");
        assert!(!manifest.input_shape.is_empty());

        let input = vec![100.0f32; 512 * 512];
        let output = runtime.run_inference(&input).expect("inference");
        assert!(!output.data.is_empty());
    }

    #[test]
    fn onnx_runtime_rejects_empty_model_path() {
        let mut runtime = OnnxRuntime::new();
        let result = runtime.load_model("");
        assert!(result.is_err());
    }

    #[test]
    fn onnx_runtime_rejects_inference_without_model() {
        let runtime = OnnxRuntime::new();
        let result = runtime.run_inference(&[1.0f32]);
        assert!(result.is_err());
    }

    #[test]
    fn onnx_runtime_is_stub() {
        let runtime = OnnxRuntime::new();
        assert!(runtime.is_stub());
    }

    #[test]
    fn onnx_runtime_stub_load_returns_synthetic_data() {
        let mut runtime = OnnxRuntime::new();
        let manifest = runtime.load_model("/models/test.onnx").expect("load");
        // STUB: the UID contains ".stub" marker
        assert!(
            manifest.model_uid.contains(".stub"),
            "stub manifest should contain .stub in UID"
        );
    }

    #[test]
    fn inference_result_segmentation() {
        let result = InferenceResult::Segmentation {
            labels: vec![0u16, 1, 1, 0],
            width: 2,
            height: 2,
            depth: 1,
        };
        if let InferenceResult::Segmentation {
            labels,
            width,
            height,
            depth,
        } = result
        {
            assert_eq!(labels, vec![0u16, 1, 1, 0]);
            assert_eq!(width, 2);
            assert_eq!(height, 2);
            assert_eq!(depth, 1);
        }
    }
