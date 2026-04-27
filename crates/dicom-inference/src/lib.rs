#![deny(missing_docs)]

//! DICOM AI inference runtime, AI Results IOD encoding, and worklist prioritization.
//!
//! Provides:
//! - **S4-T3**: ONNX Runtime bindings (stub), preprocessing pipeline, postprocessing,
//!   model manifest, and inference result types.
//! - **S4-T4**: DICOM Supplement 228 (AI Results) IOD encoding with CADe/CADx findings.
//! - **S4-T5**: AI triage engine for worklist prioritization with notification hooks.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S4-T3: AI Inference Runtime
// ===========================================================================

/// Model manifest describing an AI model's input/output shape and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelManifest {
    /// Model input shape (e.g., [1, 1, 512, 512] for a 2D segmentation model).
    pub input_shape: Vec<usize>,
    /// Model output shape (e.g., [1, 1, 512, 512] for a segmentation model).
    pub output_shape: Vec<usize>,
    /// Modality constraint (e.g., "CT", "MR", "PT"). Empty string means no constraint.
    pub modality_constraint: String,
    /// Model version string.
    pub model_version: String,
    /// Model unique identifier (UID).
    pub model_uid: String,
}

impl ModelManifest {
    /// Create a new model manifest.
    pub fn new(
        input_shape: Vec<usize>,
        output_shape: Vec<usize>,
        modality_constraint: &str,
        model_version: &str,
        model_uid: &str,
    ) -> Self {
        Self {
            input_shape,
            output_shape,
            modality_constraint: modality_constraint.to_string(),
            model_version: model_version.to_string(),
            model_uid: model_uid.to_string(),
        }
    }

    /// Validate the model manifest.
    pub fn validate(&self) -> Result<()> {
        if self.input_shape.is_empty() {
            return Err(inference_error("model input shape must not be empty"));
        }
        if self.output_shape.is_empty() {
            return Err(inference_error("model output shape must not be empty"));
        }
        if self.model_uid.is_empty() {
            return Err(inference_error("model UID must not be empty"));
        }
        if self.model_version.is_empty() {
            return Err(inference_error("model version must not be empty"));
        }
        Ok(())
    }
}

/// Trait for AI inference runtime backends.
pub trait InferenceRuntime: fmt::Debug + Send + Sync {
    /// Load a model from the given path.
    fn load_model(&mut self, model_path: &str) -> Result<ModelManifest>;

    /// Run inference on the given input tensor.
    fn run_inference(&self, input: &[f32]) -> Result<InferenceOutput>;

    /// Get the names of model output tensors.
    fn get_output_names(&self) -> Vec<String>;

    /// Return `true` if this is a stub implementation not suitable for production.
    fn is_stub(&self) -> bool {
        false
    }
}

/// Output from an inference run.
#[derive(Debug, Clone, PartialEq)]
pub struct InferenceOutput {
    /// Output tensor data as flat f32 array.
    pub data: Vec<f32>,
    /// Output shape.
    pub shape: Vec<usize>,
    /// Output name.
    pub name: String,
}

/// ONNX Runtime stub implementation.
///
/// **STUB:** This implementation is not production-ready. It simulates
/// model loading and inference without any actual ONNX Runtime bindings.
/// The `load_model()` and `run_inference()` methods return synthetic data.
/// Do not use this for clinical decision support.
#[derive(Debug, Clone)]
pub struct OnnxRuntime {
    /// Whether a model is currently loaded.
    loaded: bool,
    /// Manifest of the loaded model.
    manifest: Option<ModelManifest>,
    /// Cached output names.
    output_names: Vec<String>,
}

impl Default for OnnxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl OnnxRuntime {
    /// Create a new ONNX Runtime stub.
    pub fn new() -> Self {
        Self {
            loaded: false,
            manifest: None,
            output_names: Vec::new(),
        }
    }
}

impl InferenceRuntime for OnnxRuntime {
    fn is_stub(&self) -> bool {
        true
    }
    fn load_model(&mut self, model_path: &str) -> Result<ModelManifest> {
        if model_path.is_empty() {
            return Err(inference_error("model path must not be empty"));
        }

        // STUB: This method returns a synthetic manifest. A real implementation
        // would load an ONNX model from disk. Callers that require real inference
        // should check `is_stub()` on the InferenceRuntime trait or use a
        // production-grade backend.
        let manifest = ModelManifest::new(
            vec![1, 1, 512, 512],
            vec![1, 1, 512, 512],
            "CT",
            "1.0.0",
            &format!("1.2.840.113619.6.{}.stub", model_path.len()),
        );

        self.output_names = vec!["output".to_string()];
        self.manifest = Some(manifest.clone());
        self.loaded = true;
        Ok(manifest)
    }

    fn run_inference(&self, input: &[f32]) -> Result<InferenceOutput> {
        if !self.loaded {
            return Err(inference_error("no model loaded"));
        }
        if input.is_empty() {
            return Err(inference_error("input tensor must not be empty"));
        }

        // Stub: return a synthetic output based on input statistics
        let mean = input.iter().sum::<f32>() / input.len() as f32;
        let output_len = self
            .manifest
            .as_ref()
            .map(|m| m.output_shape.iter().product())
            .unwrap_or(input.len());

        // Deterministic output: sigmoid-like transform of mean
        let output = vec![1.0 / (1.0 + (-mean / 100.0).exp()); output_len.min(input.len())];

        Ok(InferenceOutput {
            data: output,
            shape: self
                .manifest
                .as_ref()
                .map(|m| m.output_shape.clone())
                .unwrap_or_else(|| vec![output_len]),
            name: "output".to_string(),
        })
    }

    fn get_output_names(&self) -> Vec<String> {
        self.output_names.clone()
    }
}

/// Preprocessing pipeline for AI model input preparation.
#[derive(Debug, Clone, PartialEq)]
pub struct PreprocessingPipeline {
    /// Target spacing for resampling (x, y, z) in mm.
    pub target_spacing: (f64, f64, f64),
    /// HU window lower bound.
    pub window_min: f64,
    /// HU window upper bound.
    pub window_max: f64,
    /// Normalization method.
    pub normalization: NormalizationMethod,
}

/// Normalization method for preprocessing.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NormalizationMethod {
    /// Z-score normalization: (x - mean) / std.
    ZScore,
    /// Min-max normalization: (x - min) / (max - min).
    MinMax,
    /// No normalization.
    None,
}

impl Default for PreprocessingPipeline {
    fn default() -> Self {
        Self {
            target_spacing: (1.0, 1.0, 1.0),
            window_min: -1000.0,
            window_max: 1000.0,
            normalization: NormalizationMethod::ZScore,
        }
    }
}

impl PreprocessingPipeline {
    /// Create a new preprocessing pipeline.
    pub fn new(
        target_spacing: (f64, f64, f64),
        window_min: f64,
        window_max: f64,
        normalization: NormalizationMethod,
    ) -> Self {
        Self {
            target_spacing,
            window_min,
            window_max,
            normalization,
        }
    }

    /// Resample volume data to target spacing.
    ///
    /// For simplified synthetic data, this linearly interpolates voxel values
    /// to the target grid. Returns the resampled data as a flat f32 array.
    pub fn resample(
        &self,
        data: &[f64],
        source_spacing: (f64, f64, f64),
        dims: (usize, usize, usize),
    ) -> Vec<f32> {
        let (sx, sy, sz) = source_spacing;
        let (tx, ty, tz) = self.target_spacing;

        let new_x = ((dims.0 as f64 * sx / tx).round() as usize).max(1);
        let new_y = ((dims.1 as f64 * sy / ty).round() as usize).max(1);
        let new_z = ((dims.2 as f64 * sz / tz).round() as usize).max(1);

        let mut result = Vec::with_capacity(new_x * new_y * new_z);

        for nz in 0..new_z {
            for ny in 0..new_y {
                for nx in 0..new_x {
                    // Map back to source coordinates
                    let src_x = (nx as f64 * tx / sx).min((dims.0 - 1) as f64);
                    let src_y = (ny as f64 * ty / sy).min((dims.1 - 1) as f64);
                    let src_z = (nz as f64 * tz / sz).min((dims.2 - 1) as f64);

                    // Nearest neighbor
                    let ix = src_x.round() as usize;
                    let iy = src_y.round() as usize;
                    let iz = src_z.round() as usize;

                    let idx = iz * (dims.0 * dims.1) + iy * dims.0 + ix;
                    let val = data.get(idx).copied().unwrap_or(0.0);
                    result.push(val as f32);
                }
            }
        }

        result
    }

    /// Apply HU windowing (clip values to window range).
    pub fn window(&self, data: &mut [f32]) {
        for val in data.iter_mut() {
            *val = val.clamp(self.window_min as f32, self.window_max as f32);
        }
    }

    /// Apply normalization to the data.
    pub fn normalize(&self, data: &mut [f32]) {
        match self.normalization {
            NormalizationMethod::ZScore => {
                let n = data.len() as f32;
                if n == 0.0 {
                    return;
                }
                let mean = data.iter().sum::<f32>() / n;
                let variance = data.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n;
                let std = variance.sqrt().max(1e-8);
                for val in data.iter_mut() {
                    *val = (*val - mean) / std;
                }
            }
            NormalizationMethod::MinMax => {
                let min = data.iter().cloned().fold(f32::INFINITY, f32::min);
                let max = data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let range = (max - min).max(1e-8);
                for val in data.iter_mut() {
                    *val = (*val - min) / range;
                }
            }
            NormalizationMethod::None => {}
        }
    }

    /// Run the full preprocessing pipeline on volume data.
    pub fn process(
        &self,
        data: &[f64],
        source_spacing: (f64, f64, f64),
        dims: (usize, usize, usize),
    ) -> Vec<f32> {
        let mut result = self.resample(data, source_spacing, dims);
        self.window(&mut result);
        self.normalize(&mut result);
        result
    }
}

/// Postprocessing operations for AI model output.
#[derive(Debug, Clone)]
pub struct Postprocessing;

impl Postprocessing {
    /// Apply threshold to convert probability map to binary mask.
    pub fn threshold_to_binary(prob_map: &[f32], threshold: f32) -> Vec<u8> {
        prob_map
            .iter()
            .map(|&p| if p >= threshold { 1 } else { 0 })
            .collect()
    }

    /// Extract 2D contour from a binary mask using marching squares (simplified).
    ///
    /// Returns contour points as (x, y) pairs. Uses a simplified boundary-tracing
    /// algorithm suitable for testing.
    pub fn extract_contour_2d(mask: &[u8], width: usize, height: usize) -> Vec<(f64, f64)> {
        let mut contour = Vec::new();

        for y in 1..height.saturating_sub(1) {
            for x in 1..width.saturating_sub(1) {
                let idx = y * width + x;
                if mask.get(idx).copied().unwrap_or(0) == 1 {
                    // Check if this is a boundary pixel
                    let is_boundary = mask.get((y - 1) * width + x).copied().unwrap_or(0) == 0
                        || mask.get((y + 1) * width + x).copied().unwrap_or(0) == 0
                        || mask.get(y * width + (x - 1)).copied().unwrap_or(0) == 0
                        || mask.get(y * width + (x + 1)).copied().unwrap_or(0) == 0;

                    if is_boundary {
                        contour.push((x as f64 + 0.5, y as f64 + 0.5));
                    }
                }
            }
        }

        // Sort for deterministic output
        contour.sort_by(|a, b| {
            a.0.partial_cmp(&b.0)
                .unwrap()
                .then(a.1.partial_cmp(&b.1).unwrap())
        });
        contour
    }

    /// Apply Non-Maximum Suppression (NMS) to detection boxes.
    pub fn nms(boxes: &[DetectionBox], iou_threshold: f64) -> Vec<DetectionBox> {
        if boxes.is_empty() {
            return Vec::new();
        }

        let mut sorted: Vec<(usize, f64)> = boxes
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.probability))
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let mut selected = Vec::new();
        let mut suppressed = vec![false; boxes.len()];

        for &(idx, _) in &sorted {
            if suppressed[idx] {
                continue;
            }
            selected.push(boxes[idx].clone());

            for &(other_idx, _) in &sorted {
                if suppressed[other_idx] || other_idx == idx {
                    continue;
                }
                let iou = compute_iou(&boxes[idx], &boxes[other_idx]);
                if iou > iou_threshold {
                    suppressed[other_idx] = true;
                }
            }
        }

        selected
    }
}

/// Compute Intersection over Union between two detection boxes.
fn compute_iou(a: &DetectionBox, b: &DetectionBox) -> f64 {
    let x1 = a.x_min.max(b.x_min);
    let y1 = a.y_min.max(b.y_min);
    let x2 = a.x_max.min(b.x_max);
    let y2 = a.y_max.min(b.y_max);

    let intersection = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let area_a = (a.x_max - a.x_min) * (a.y_max - a.y_min);
    let area_b = (b.x_max - b.x_min) * (b.y_max - b.y_min);
    let union = area_a + area_b - intersection;

    if union < 1e-12 {
        0.0
    } else {
        intersection / union
    }
}

/// A detection bounding box.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionBox {
    /// Minimum x coordinate.
    pub x_min: f64,
    /// Minimum y coordinate.
    pub y_min: f64,
    /// Maximum x coordinate.
    pub x_max: f64,
    /// Maximum y coordinate.
    pub y_max: f64,
    /// Detection probability.
    pub probability: f64,
    /// Finding type code (e.g., SNOMED CT code).
    pub finding_type: String,
}

/// A class score for classification results.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassScore {
    /// Class label.
    pub label: String,
    /// Classification probability.
    pub probability: f64,
}

/// Inference result enum covering common AI output types.
#[derive(Debug, Clone, PartialEq)]
pub enum InferenceResult {
    /// Segmentation result compatible with LabelMap3D.
    Segmentation {
        /// Label map data (flat array in z-y-x order).
        labels: Vec<u16>,
        /// Width (x dimension).
        width: usize,
        /// Height (y dimension).
        height: usize,
        /// Depth (z dimension).
        depth: usize,
    },
    /// Detection result with bounding boxes.
    Detection(Vec<DetectionBox>),
    /// Classification result with class scores.
    Classification(Vec<ClassScore>),
}

// ===========================================================================
// S4-T4: DICOM AI Results (AIR) IOD
// ===========================================================================

/// Algorithm identification for AI Results IOD.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlgorithmIdentification {
    /// Algorithm name.
    pub name: String,
    /// Algorithm version.
    pub version: String,
    /// Algorithm UID.
    pub uid: String,
}

impl AlgorithmIdentification {
    /// Create a new algorithm identification.
    pub fn new(name: &str, version: &str, uid: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            uid: uid.to_string(),
        }
    }

    /// Validate the algorithm identification.
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(inference_error("algorithm name must not be empty"));
        }
        if self.version.is_empty() {
            return Err(inference_error("algorithm version must not be empty"));
        }
        if self.uid.is_empty() {
            return Err(inference_error("algorithm UID must not be empty"));
        }
        Ok(())
    }
}

/// CADe (Computer-Assisted Detection) finding.
#[derive(Debug, Clone, PartialEq)]
pub struct CADeFinding {
    /// Finding UID.
    pub finding_uid: String,
    /// Bounding box coordinates (x_min, y_min, x_max, y_max).
    pub bounding_box: (f64, f64, f64, f64),
    /// Detection probability.
    pub probability: f64,
    /// Finding type as coded terminology (SNOMED CT / DICOM code).
    pub finding_type_code: String,
    /// Finding type display name.
    pub finding_type_display: String,
    /// Coding scheme designator (e.g., "SCT" for SNOMED CT, "DCM" for DICOM).
    pub coding_scheme: String,
}

/// CADx (Computer-Assisted Diagnosis) finding.
#[derive(Debug, Clone, PartialEq)]
pub struct CADxFinding {
    /// Finding UID.
    pub finding_uid: String,
    /// Classification label.
    pub classification: String,
    /// Classification probability.
    pub probability: f64,
    /// Algorithm identity.
    pub algorithm: AlgorithmIdentification,
    /// Finding type as coded terminology.
    pub finding_type_code: String,
    /// Finding type display name.
    pub finding_type_display: String,
    /// Coding scheme designator.
    pub coding_scheme: String,
}

/// Encoder for DICOM Supplement 228 (AI Results) IOD.
#[derive(Debug, Clone)]
pub struct AirIodEncoder {
    /// Algorithm identification for the AI Results IOD.
    pub algorithm: AlgorithmIdentification,
    /// Series number for the AI Results IOD.
    pub series_number: i32,
    /// Instance number for the AI Results IOD.
    pub instance_number: i32,
}

impl AirIodEncoder {
    /// Create a new AIR IOD encoder with the given algorithm identification.
    pub fn new(algorithm: AlgorithmIdentification) -> Self {
        Self {
            algorithm,
            series_number: 1,
            instance_number: 1,
        }
    }

    /// Encode a CADe finding as a DICOM sequence item.
    pub fn encode_cade_finding(&self, finding: &CADeFinding) -> Dataset {
        let mut ds = Dataset::new();

        // Finding UID
        ds.insert(
            Element::new(
                Tag(0x0008, 0x0018),
                Vr::Ui,
                Value::Uid(finding.finding_uid.clone()),
            )
            .unwrap(),
        );

        // Finding type code sequence
        let mut type_item = Dataset::new();
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0100),
                Vr::Sh,
                Value::Str(finding.finding_type_code.clone()),
            )
            .unwrap(),
        );
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0102),
                Vr::Sh,
                Value::Str(finding.coding_scheme.clone()),
            )
            .unwrap(),
        );
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0104),
                Vr::Lo,
                Value::Str(finding.finding_type_display.clone()),
            )
            .unwrap(),
        );

        ds.insert(
            Element::new(
                Tag(0x0008, 0x0104),
                Vr::Sq,
                Value::Sequence(vec![type_item]),
            )
            .unwrap(),
        );

        // Bounding box (as measured value)
        ds.insert(
            Element::new(
                Tag(0x0040, 0xA300),
                Vr::Ds,
                Value::Str(format!(
                    "{:.6}\\\\{:.6}\\\\{:.6}\\\\{:.6}",
                    finding.bounding_box.0,
                    finding.bounding_box.1,
                    finding.bounding_box.2,
                    finding.bounding_box.3
                )),
            )
            .unwrap(),
        );

        // Probability
        ds.insert(
            Element::new(
                Tag(0x0040, 0xA353),
                Vr::Ds,
                Value::Str(format!("{:.6}", finding.probability)),
            )
            .unwrap(),
        );

        ds
    }

    /// Encode a CADx finding as a DICOM sequence item.
    pub fn encode_cadx_finding(&self, finding: &CADxFinding) -> Dataset {
        let mut ds = Dataset::new();

        // Finding UID
        ds.insert(
            Element::new(
                Tag(0x0008, 0x0018),
                Vr::Ui,
                Value::Uid(finding.finding_uid.clone()),
            )
            .unwrap(),
        );

        // Finding type code sequence
        let mut type_item = Dataset::new();
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0100),
                Vr::Sh,
                Value::Str(finding.finding_type_code.clone()),
            )
            .unwrap(),
        );
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0102),
                Vr::Sh,
                Value::Str(finding.coding_scheme.clone()),
            )
            .unwrap(),
        );
        type_item.insert(
            Element::new(
                Tag(0x0008, 0x0104),
                Vr::Lo,
                Value::Str(finding.finding_type_display.clone()),
            )
            .unwrap(),
        );

        ds.insert(
            Element::new(
                Tag(0x0008, 0x0104),
                Vr::Sq,
                Value::Sequence(vec![type_item]),
            )
            .unwrap(),
        );

        // Classification
        ds.insert(
            Element::new(
                Tag(0x0040, 0xA043),
                Vr::Cs,
                Value::Str(finding.classification.clone()),
            )
            .unwrap(),
        );

        // Probability
        ds.insert(
            Element::new(
                Tag(0x0040, 0xA353),
                Vr::Ds,
                Value::Str(format!("{:.6}", finding.probability)),
            )
            .unwrap(),
        );

        // Algorithm Identification
        let mut algo_item = Dataset::new();
        algo_item.insert(
            Element::new(
                Tag(0x0008, 0x0100),
                Vr::Sh,
                Value::Str(self.algorithm.name.clone()),
            )
            .unwrap(),
        );
        algo_item.insert(
            Element::new(
                Tag(0x0008, 0x0110),
                Vr::Lo,
                Value::Str(self.algorithm.version.clone()),
            )
            .unwrap(),
        );
        algo_item.insert(
            Element::new(
                Tag(0x0008, 0x0018),
                Vr::Ui,
                Value::Uid(self.algorithm.uid.clone()),
            )
            .unwrap(),
        );

        ds.insert(
            Element::new(
                Tag(0x0040, 0xA354),
                Vr::Sq,
                Value::Sequence(vec![algo_item]),
            )
            .unwrap(),
        );

        ds
    }
}

/// Encode a full AI Results IOD Dataset from inference results.
pub fn encode_air_iod(
    encoder: &AirIodEncoder,
    inference_result: &InferenceResult,
    study_uid: &str,
    series_uid: &str,
) -> Result<Dataset> {
    encoder.algorithm.validate()?;

    let mut ds = Dataset::new();

    // SOP Class UID for Enhanced SR (AI Results use TID 1500 in an Enhanced SR)
    ds.insert(Element::new(
        Tag(0x0008, 0x0016),
        Vr::Ui,
        Value::Uid("1.2.840.10008.5.1.4.1.1.88.22".to_string()),
    )?);

    // SOP Instance UID
    let sop_uid = format!(
        "1.2.840.113619.6.5.{}.{}.{}",
        study_uid.len(),
        series_uid.len(),
        encoder.algorithm.uid.len(),
    );
    ds.insert(Element::new(
        Tag(0x0008, 0x0018),
        Vr::Ui,
        Value::Uid(sop_uid),
    )?);

    // Study Instance UID
    ds.insert(Element::new(
        Tag(0x0020, 0x000D),
        Vr::Ui,
        Value::Uid(study_uid.to_string()),
    )?);

    // Series Instance UID
    ds.insert(Element::new(
        Tag(0x0020, 0x000E),
        Vr::Ui,
        Value::Uid(series_uid.to_string()),
    )?);

    // Series Number
    ds.insert(Element::new(
        Tag(0x0020, 0x0011),
        Vr::Is,
        Value::Str(encoder.series_number.to_string()),
    )?);

    // Instance Number
    ds.insert(Element::new(
        Tag(0x0020, 0x0013),
        Vr::Is,
        Value::Str(encoder.instance_number.to_string()),
    )?);

    // Content Sequence with findings
    let mut content_items = Vec::new();

    match inference_result {
        InferenceResult::Detection(boxes) => {
            for (i, det) in boxes.iter().enumerate() {
                let finding = CADeFinding {
                    finding_uid: format!("1.2.840.113619.6.5.f.{i}"),
                    bounding_box: (det.x_min, det.y_min, det.x_max, det.y_max),
                    probability: det.probability,
                    finding_type_code: det.finding_type.clone(),
                    finding_type_display: det.finding_type.clone(),
                    coding_scheme: "SCT".to_string(),
                };
                content_items.push(encoder.encode_cade_finding(&finding));
            }
        }
        InferenceResult::Classification(scores) => {
            for (i, score) in scores.iter().enumerate() {
                let finding = CADxFinding {
                    finding_uid: format!("1.2.840.113619.6.5.c.{i}"),
                    classification: score.label.clone(),
                    probability: score.probability,
                    algorithm: encoder.algorithm.clone(),
                    finding_type_code: "439569004".to_string(), // SNOMED CT: AI-assisted diagnosis
                    finding_type_display: "AI-assisted diagnosis".to_string(),
                    coding_scheme: "SCT".to_string(),
                };
                content_items.push(encoder.encode_cadx_finding(&finding));
            }
        }
        InferenceResult::Segmentation { .. } => {
            // Segmentation results use Segmentation IOD, not AIR IOD directly
            // Encode a reference finding
            let mut seg_item = Dataset::new();
            seg_item.insert(
                Element::new(
                    Tag(0x0008, 0x0100),
                    Vr::Sh,
                    Value::Str("126000".to_string()),
                )
                .unwrap(),
            );
            seg_item.insert(
                Element::new(Tag(0x0008, 0x0102), Vr::Sh, Value::Str("DCM".to_string())).unwrap(),
            );
            seg_item.insert(
                Element::new(
                    Tag(0x0008, 0x0104),
                    Vr::Lo,
                    Value::Str("Segmentation".to_string()),
                )
                .unwrap(),
            );
            content_items.push(seg_item);
        }
    }

    ds.insert(Element::new(
        Tag(0x0040, 0xA730),
        Vr::Sq,
        Value::Sequence(content_items),
    )?);

    Ok(ds)
}

// ===========================================================================
// S4-T5: AI Worklist Prioritization
// ===========================================================================

/// Triage flag severity levels.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TriageFlag {
    /// Critical finding requiring immediate attention.
    Critical,
    /// Urgent finding requiring prompt attention.
    Urgent,
    /// Routine finding — standard workflow.
    Routine,
    /// Low priority finding.
    Low,
}

impl TriageFlag {
    /// Get the numeric priority score for this flag.
    pub fn priority_score(&self) -> u32 {
        match self {
            TriageFlag::Critical => 100,
            TriageFlag::Urgent => 70,
            TriageFlag::Routine => 40,
            TriageFlag::Low => 10,
        }
    }

    /// Get a human-readable label for this flag.
    pub fn label(&self) -> &'static str {
        match self {
            TriageFlag::Critical => "Critical",
            TriageFlag::Urgent => "Urgent",
            TriageFlag::Routine => "Routine",
            TriageFlag::Low => "Low",
        }
    }
}

impl fmt::Display for TriageFlag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A triage rule that maps inference findings to triage flags.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageRule {
    /// Pattern to match against finding type codes.
    pub finding_pattern: String,
    /// Triage flag to apply when this rule matches.
    pub flag: TriageFlag,
    /// Minimum confidence threshold for this rule to trigger.
    pub confidence_threshold: f64,
    /// Notification message for this rule.
    pub notification_message: String,
}

impl TriageRule {
    /// Create a new triage rule.
    pub fn new(
        finding_pattern: &str,
        flag: TriageFlag,
        confidence_threshold: f64,
        notification_message: &str,
    ) -> Self {
        Self {
            finding_pattern: finding_pattern.to_string(),
            flag,
            confidence_threshold,
            notification_message: notification_message.to_string(),
        }
    }

    /// Check if this rule matches a given finding type and confidence.
    pub fn matches(&self, finding_type: &str, confidence: f64) -> bool {
        finding_type.contains(&self.finding_pattern) && confidence >= self.confidence_threshold
    }
}

/// Notification callback trait for critical findings.
pub trait NotificationCallback: fmt::Debug + Send + Sync {
    /// Called when a critical or urgent finding is detected.
    ///
    /// # S13-T7 — Interior Mutability Contract
    ///
    /// This method takes `&self`, so implementations that need to record
    /// notifications must use interior mutability (e.g., `Mutex<Vec<...>>`).
    fn notify(&self, flag: TriageFlag, message: &str, study_uid: &str);
}

/// A simple notification callback that collects notifications in memory.
///
/// Uses `Mutex<Vec<...>>` for interior mutability since the trait method
/// `notify` takes `&self` (S13-T7).
#[derive(Debug)]
pub struct InMemoryNotificationCallback {
    /// Collected notifications (interior mutability for `&self` trait method).
    notifications: std::sync::Mutex<Vec<(TriageFlag, String, String)>>,
}

impl Default for InMemoryNotificationCallback {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryNotificationCallback {
    /// Create a new in-memory notification callback.
    pub fn new() -> Self {
        Self {
            notifications: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Return the collected notifications.
    pub fn notifications(&self) -> Vec<(TriageFlag, String, String)> {
        self.notifications.lock().unwrap().clone()
    }
}

impl NotificationCallback for InMemoryNotificationCallback {
    fn notify(&self, flag: TriageFlag, message: &str, study_uid: &str) {
        let mut guard = self.notifications.lock().unwrap();
        guard.push((flag, message.to_string(), study_uid.to_string()));
    }
}

/// Study metadata for triage analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyMetadata {
    /// Study Instance UID.
    pub study_uid: String,
    /// Study modality (e.g., "CT", "MR").
    pub modality: String,
    /// Study description.
    pub description: String,
    /// Patient ID.
    pub patient_id: String,
}

/// Result of triage analysis for a study.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageResult {
    /// Study UID that was analyzed.
    pub study_uid: String,
    /// Triage flags that were triggered.
    pub flags: Vec<TriageFlag>,
    /// Overall priority score (maximum of all flag scores).
    pub priority_score: u32,
    /// Notification messages for triggered rules.
    pub notifications: Vec<String>,
}

/// AI triage engine for worklist prioritization.
pub struct AiTriageEngine {
    /// Active triage rules.
    rules: Vec<TriageRule>,
    /// Notification callback for critical/urgent findings.
    notification_callback: Option<Arc<dyn NotificationCallback>>,
    /// Audit callback.
    audit: Option<AuditCallback>,
}

impl fmt::Debug for AiTriageEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AiTriageEngine")
            .field("rules", &self.rules)
            .field(
                "notification_callback",
                &self.notification_callback.is_some(),
            )
            .field("audit", &self.audit.is_some())
            .finish()
    }
}

/// Audit callback for inference operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

impl Default for AiTriageEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AiTriageEngine {
    /// Create a new triage engine with built-in rules.
    pub fn new() -> Self {
        Self {
            rules: built_in_rules(),
            notification_callback: None,
            audit: None,
        }
    }

    /// Create a triage engine with custom rules.
    pub fn with_rules(rules: Vec<TriageRule>) -> Self {
        Self {
            rules,
            notification_callback: None,
            audit: None,
        }
    }

    /// Set the notification callback.
    pub fn set_notification_callback(&mut self, callback: Option<Arc<dyn NotificationCallback>>) {
        self.notification_callback = callback;
    }

    /// Set the audit callback.
    pub fn set_audit(&mut self, audit: Option<AuditCallback>) {
        self.audit = audit;
    }

    /// Add a triage rule.
    pub fn add_rule(&mut self, rule: TriageRule) {
        self.rules.push(rule);
    }

    /// Return the current triage rules.
    pub fn rules(&self) -> &[TriageRule] {
        &self.rules
    }

    /// Analyze a study with inference results and return triage flags.
    pub fn analyze_study(
        &self,
        study_metadata: &StudyMetadata,
        inference_results: &[InferenceResult],
    ) -> TriageResult {
        let mut flags = Vec::new();
        let mut notifications = Vec::new();
        let mut max_score: u32 = 0;

        for result in inference_results {
            match result {
                InferenceResult::Detection(boxes) => {
                    for det in boxes {
                        for rule in &self.rules {
                            if rule.matches(&det.finding_type, det.probability) {
                                flags.push(rule.flag);
                                notifications.push(rule.notification_message.clone());
                                max_score = max_score.max(rule.flag.priority_score());

                                // Notify for critical and urgent findings
                                if matches!(rule.flag, TriageFlag::Critical | TriageFlag::Urgent) {
                                    if let Some(callback) = &self.notification_callback {
                                        callback.notify(
                                            rule.flag,
                                            &rule.notification_message,
                                            &study_metadata.study_uid,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                InferenceResult::Classification(scores) => {
                    for score in scores {
                        for rule in &self.rules {
                            if rule.matches(&score.label, score.probability) {
                                flags.push(rule.flag);
                                notifications.push(rule.notification_message.clone());
                                max_score = max_score.max(rule.flag.priority_score());

                                if matches!(rule.flag, TriageFlag::Critical | TriageFlag::Urgent) {
                                    if let Some(callback) = &self.notification_callback {
                                        callback.notify(
                                            rule.flag,
                                            &rule.notification_message,
                                            &study_metadata.study_uid,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                InferenceResult::Segmentation { .. } => {
                    // Segmentation results don't directly trigger triage
                }
            }
        }

        // If no flags triggered, assign Routine
        if flags.is_empty() {
            flags.push(TriageFlag::Routine);
            max_score = TriageFlag::Routine.priority_score();
        }

        // Sort flags by severity (Critical first)
        flags.sort_by(|a, b| b.cmp(a));

        let _ = self.record_audit("analyze_study", &study_metadata.study_uid);

        TriageResult {
            study_uid: study_metadata.study_uid.clone(),
            flags,
            priority_score: max_score,
            notifications,
        }
    }

    fn record_audit(&self, operation: &'static str, subject_id: &str) -> Result<()> {
        let Some(callback) = &self.audit else {
            return Ok(());
        };
        callback(AuditEvent {
            kind: AuditEventKind::ServiceEvent,
            fields: vec![
                AuditField {
                    key: "operation",
                    value: AuditValue::Plain(operation.to_string()),
                },
                AuditField {
                    key: "study_uid",
                    value: AuditValue::Sensitive(subject_id.to_string()),
                },
            ],
        })
    }
}

/// Built-in triage rules for common critical findings.
/// Returns built-in triage rules.
pub fn built_in_rules() -> Vec<TriageRule> {
    vec![
        TriageRule::new(
            "pneumothorax",
            TriageFlag::Critical,
            0.5,
            "CRITICAL: Possible pneumothorax detected — immediate radiologist review required",
        ),
        TriageRule::new(
            "hemorrhage",
            TriageFlag::Critical,
            0.5,
            "CRITICAL: Possible hemorrhage detected — immediate radiologist review required",
        ),
        TriageRule::new(
            "fracture",
            TriageFlag::Urgent,
            0.5,
            "URGENT: Possible fracture detected — prompt radiologist review recommended",
        ),
        TriageRule::new(
            "mass",
            TriageFlag::Urgent,
            0.7,
            "URGENT: Possible mass detected — prompt radiologist review recommended",
        ),
        TriageRule::new(
            "nodule",
            TriageFlag::Routine,
            0.5,
            "ROUTINE: Possible nodule detected — standard workflow review",
        ),
    ]
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn inference_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-inference".to_string(),
            detail: detail.into(),
        },
        "inference error",
    )
    .into()
}

// ===========================================================================
// Tests: S4-T3 AI Inference Runtime (minimum 15)
// ===========================================================================

// ===========================================================================
// Tests: S4-T4 DICOM AI Results IOD (minimum 12)
// ===========================================================================

// ===========================================================================
// Tests: S4-T5 AI Worklist Prioritization (minimum 10)
// ===========================================================================
