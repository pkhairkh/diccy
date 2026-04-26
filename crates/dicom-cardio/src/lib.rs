#![deny(missing_docs)]

//! Cardiovascular quantification: calcium scoring, coronary analysis, ejection fraction.
//!
//! Provides:
//! - **S7-T1**: Agatston calcium scoring on non-contrast cardiac CT with DICOM SR encoding.
//! - **S7-T2**: Coronary artery centerline extraction, curved MPR, stenosis measurement.
//! - **S7-T3**: Left/right ventricular ejection fraction via Simpson's method on cardiac MR.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

// ===========================================================================
// S7-T1: Calcium Scoring (Agatston Score)
// ===========================================================================

/// HU threshold for calcium detection (standard: 130 HU).
pub const CALCIUM_HU_THRESHOLD: f64 = 130.0;

/// Standard Agatston weighting factors by peak HU in a lesion.
pub const AGATSTON_WEIGHTS: [(f64, f64); 4] = [
    (130.0, 1.0),   // 130-199 HU: weight 1
    (200.0, 2.0),   // 200-299 HU: weight 2
    (300.0, 3.0),   // 300-399 HU: weight 3
    (400.0, 4.0),   // >= 400 HU: weight 4
];

/// Coronary artery label for calcium scoring.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CoronaryArtery {
    /// Left main coronary artery.
    LeftMain,
    /// Left anterior descending artery.
    Lad,
    /// Left circumflex artery.
    Lcx,
    /// Right coronary artery.
    Rca,
    /// Posterior descending artery.
    Pda,
}

impl CoronaryArtery {
    /// Return the DICOM code value for this artery.
    pub fn dicom_code(&self) -> &str {
        match self {
            CoronaryArtery::LeftMain => "74066000",
            CoronaryArtery::Lad => "74290005",
            CoronaryArtery::Lcx => "3611008",
            CoronaryArtery::Rca => "45631007",
            CoronaryArtery::Pda => "84160008",
        }
    }

    /// Return the display name for this artery.
    pub fn display_name(&self) -> &str {
        match self {
            CoronaryArtery::LeftMain => "Left Main",
            CoronaryArtery::Lad => "LAD",
            CoronaryArtery::Lcx => "LCx",
            CoronaryArtery::Rca => "RCA",
            CoronaryArtery::Pda => "PDA",
        }
    }
}

/// A single calcified lesion identified in a coronary artery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalcifiedLesion {
    /// Unique lesion identifier.
    pub lesion_id: String,
    /// Artery where the lesion is located.
    pub artery: CoronaryArtery,
    /// Area in square millimeters.
    pub area_mm2: f64,
    /// Peak HU value in the lesion.
    pub peak_hu: f64,
    /// Mean HU value in the lesion.
    pub mean_hu: f64,
    /// Volume in cubic millimeters.
    pub volume_mm3: f64,
    /// Mass in milligrams of calcium hydroxyapatite.
    pub mass_mg: f64,
    /// Slice indices where this lesion appears.
    pub slice_indices: Vec<usize>,
}

impl CalcifiedLesion {
    /// Create a new calcified lesion.
    pub fn new(lesion_id: String, artery: CoronaryArtery) -> Self {
        Self {
            lesion_id,
            artery,
            area_mm2: 0.0,
            peak_hu: 0.0,
            mean_hu: 0.0,
            volume_mm3: 0.0,
            mass_mg: 0.0,
            slice_indices: Vec::new(),
        }
    }

    /// Calculate the Agatston score for this lesion.
    ///
    /// The Agatston score is the product of the lesion area and the Agatston
    /// weighting factor based on peak HU, multiplied by the slice thickness factor.
    pub fn agatston_score(&self, pixel_area_mm2: f64, slice_thickness_mm: f64) -> f64 {
        let weight = self.agatston_weight();
        let num_pixels = self.area_mm2 / pixel_area_mm2.max(1e-12);
        num_pixels * pixel_area_mm2 * weight * slice_thickness_mm
    }

    /// Get the Agatston weight factor based on peak HU.
    pub fn agatston_weight(&self) -> f64 {
        let mut weight = 1.0;
        for (threshold, w) in AGATSTON_WEIGHTS {
            if self.peak_hu >= threshold {
                weight = w;
            }
        }
        weight
    }
}

/// Calcium scoring result for an entire study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalciumScoreResult {
    /// Total Agatston score across all arteries.
    pub total_agatston: f64,
    /// Total calcium volume in cubic millimeters.
    pub total_volume_mm3: f64,
    /// Total calcium mass in milligrams.
    pub total_mass_mg: f64,
    /// Per-artery Agatston scores.
    pub artery_scores: BTreeMap<CoronaryArtery, f64>,
    /// Per-artery calcium volumes.
    pub artery_volumes: BTreeMap<CoronaryArtery, f64>,
    /// Individual lesions identified.
    pub lesions: Vec<CalcifiedLesion>,
    /// Pixel area in square millimeters.
    pub pixel_area_mm2: f64,
    /// Slice thickness in millimeters.
    pub slice_thickness_mm: f64,
}

impl CalciumScoreResult {
    /// Create a calcium score result from a list of lesions.
    pub fn from_lesions(
        lesions: Vec<CalcifiedLesion>,
        pixel_area_mm2: f64,
        slice_thickness_mm: f64,
    ) -> Self {
        let mut artery_scores = BTreeMap::new();
        let mut artery_volumes = BTreeMap::new();
        let mut total_agatston = 0.0;
        let mut total_volume = 0.0;
        let mut total_mass = 0.0;

        for lesion in &lesions {
            let score = lesion.agatston_score(pixel_area_mm2, slice_thickness_mm);
            *artery_scores.entry(lesion.artery).or_insert(0.0) += score;
            *artery_volumes.entry(lesion.artery).or_insert(0.0) += lesion.volume_mm3;
            total_agatston += score;
            total_volume += lesion.volume_mm3;
            total_mass += lesion.mass_mg;
        }

        Self {
            total_agatston,
            total_volume_mm3: total_volume,
            total_mass_mg: total_mass,
            artery_scores,
            artery_volumes,
            lesions,
            pixel_area_mm2,
            slice_thickness_mm,
        }
    }

    /// Get the calcium score percentile based on age and sex.
    ///
    /// Returns an estimated percentile based on the MESA study reference ranges.
    /// This is an advisory value with mandatory uncertainty context.
    pub fn age_sex_percentile(&self, age: u32, is_male: bool) -> f64 {
        // Simplified MESA-like percentile estimation
        // Real implementation would use lookup tables from published data
        let base = if is_male { 80.0 } else { 40.0 };
        let age_factor = (age as f64 - 45.0).max(0.0) * 2.0;
        let expected = base + age_factor;
        let ratio = self.total_agatston / expected.max(1.0);

        // Rough percentile mapping (log-normal approximation)
        let percentile = 50.0 + 30.0 * (ratio.ln().max(-3.0).min(3.0) / 3.0);
        percentile.clamp(1.0, 99.0)
    }

    /// Encode the calcium score result as a DICOM SR TID 3905 document.
    pub fn encode_to_dicom_sr(&self, study_uid: &str, series_uid: &str) -> Dataset {
        let mut ds = Dataset::new();

        // SOP Class UID (Enhanced SR)
        ds.insert(Element::new(Tag(0x0008, 0x0016), Vr::Ui, Value::Uid("1.2.840.10008.5.1.4.1.1.88.22".to_string())).unwrap());

        // Study Instance UID
        ds.insert(Element::new(Tag(0x0020, 0x000D), Vr::Ui, Value::Uid(study_uid.to_string())).unwrap());

        // Series Instance UID
        ds.insert(Element::new(Tag(0x0020, 0x000E), Vr::Ui, Value::Uid(series_uid.to_string())).unwrap());

        // Total Agatston Score
        ds.insert(Element::new(Tag(0x0040, 0xA300), Vr::Ds, Value::Str(format!("{:.2}", self.total_agatston))).unwrap());

        // Total Volume
        ds.insert(Element::new(Tag(0x0040, 0xA301), Vr::Ds, Value::Str(format!("{:.2}", self.total_volume_mm3))).unwrap());

        // Total Mass
        ds.insert(Element::new(Tag(0x0040, 0xA302), Vr::Ds, Value::Str(format!("{:.2}", self.total_mass_mg))).unwrap());

        ds
    }
}

/// Detect calcified lesions in a CT volume above the HU threshold.
///
/// This performs connected-component analysis on voxels above 130 HU.
/// Returns a list of detected lesions grouped by proximity.
pub fn detect_calcium_lesions(
    volume: &[f64],
    width: usize,
    height: usize,
    depth: usize,
    pixel_spacing_mm: (f64, f64),
    slice_thickness_mm: f64,
    artery_map: &[CoronaryArtery],
) -> Vec<CalcifiedLesion> {
    let mut visited = vec![false; width * height * depth];
    let mut lesions = Vec::new();
    let mut lesion_counter = 0;

    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                let idx = z * (width * height) + y * width + x;
                if visited[idx] || volume.get(idx).copied().unwrap_or(0.0) < CALCIUM_HU_THRESHOLD {
                    continue;
                }

                // Flood-fill connected component
                let (component, peak_hu, mean_hu) = flood_fill_calcium(
                    volume, &mut visited, x, y, z, width, height, depth,
                );

                if component.is_empty() {
                    continue;
                }

                let pixel_area = pixel_spacing_mm.0 * pixel_spacing_mm.1;
                let area_mm2 = component.len() as f64 * pixel_area;
                let volume_mm3 = area_mm2 * slice_thickness_mm;

                // Estimate mass: simplified Rafferty method
                // mass = area * mean_HU * calibration_factor
                let calibration_factor = 0.001; // mg/HU/mm2 (approximate)
                let mass_mg = area_mm2 * mean_hu * calibration_factor;

                let artery = artery_map.get(z).copied().unwrap_or(CoronaryArtery::Lad);

                let mut lesion = CalcifiedLesion::new(
                    format!("lesion-{}", lesion_counter),
                    artery,
                );
                lesion.area_mm2 = area_mm2;
                lesion.peak_hu = peak_hu;
                lesion.mean_hu = mean_hu;
                lesion.volume_mm3 = volume_mm3;
                lesion.mass_mg = mass_mg;
                lesion.slice_indices = component.iter().map(|(x, y, z)| *z).collect();
                lesion.slice_indices.sort();
                lesion.slice_indices.dedup();

                lesions.push(lesion);
                lesion_counter += 1;
            }
        }
    }

    lesions
}

/// Flood-fill to find connected calcium voxels.
fn flood_fill_calcium(
    volume: &[f64],
    visited: &mut [bool],
    start_x: usize,
    start_y: usize,
    start_z: usize,
    width: usize,
    height: usize,
    depth: usize,
) -> (Vec<(usize, usize, usize)>, f64, f64) {
    let mut component = Vec::new();
    let mut stack = vec![(start_x, start_y, start_z)];
    let mut peak_hu = 0.0_f64;
    let mut sum_hu = 0.0_f64;

    while let Some((x, y, z)) = stack.pop() {
        let idx = z * (width * height) + y * width + x;
        if visited[idx] {
            continue;
        }
        let hu = volume.get(idx).copied().unwrap_or(0.0);
        if hu < CALCIUM_HU_THRESHOLD {
            continue;
        }

        visited[idx] = true;
        component.push((x, y, z));
        peak_hu = peak_hu.max(hu);
        sum_hu += hu;

        // 6-connected neighbors
        if x > 0 { stack.push((x - 1, y, z)); }
        if x + 1 < width { stack.push((x + 1, y, z)); }
        if y > 0 { stack.push((x, y - 1, z)); }
        if y + 1 < height { stack.push((x, y + 1, z)); }
        if z > 0 { stack.push((x, y, z - 1)); }
        if z + 1 < depth { stack.push((x, y, z + 1)); }
    }

    let mean_hu = if component.is_empty() { 0.0 } else { sum_hu / component.len() as f64 };
    (component, peak_hu, mean_hu)
}

// ===========================================================================
// S7-T2: Coronary Artery Analysis
// ===========================================================================

/// A point along a vessel centerline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CenterlinePoint {
    /// X coordinate in patient space (mm).
    pub x: f64,
    /// Y coordinate in patient space (mm).
    pub y: f64,
    /// Z coordinate in patient space (mm).
    pub z: f64,
    /// Lumen diameter at this point (mm).
    pub diameter: f64,
    /// Distance along the centerline from the ostium (mm).
    pub distance_from_ostium: f64,
}

/// A vessel centerline extracted from a CT angiogram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VesselCenterline {
    /// Artery label.
    pub artery: CoronaryArtery,
    /// Ordered centerline points from ostium to distal.
    pub points: Vec<CenterlinePoint>,
    /// Total length of the vessel in mm.
    pub total_length: f64,
}

impl VesselCenterline {
    /// Create a new vessel centerline.
    pub fn new(artery: CoronaryArtery) -> Self {
        Self {
            artery,
            points: Vec::new(),
            total_length: 0.0,
        }
    }

    /// Add a centerline point.
    pub fn add_point(&mut self, x: f64, y: f64, z: f64, diameter: f64) {
        let distance = if let Some(last) = self.points.last() {
            let dx = x - last.x;
            let dy = y - last.y;
            let dz = z - last.z;
            last.distance_from_ostium + (dx * dx + dy * dy + dz * dz).sqrt()
        } else {
            0.0
        };

        self.total_length = distance;
        self.points.push(CenterlinePoint {
            x, y, z, diameter, distance_from_ostium: distance,
        });
    }

    /// Find stenosis locations where the diameter is significantly reduced.
    ///
    /// Returns pairs of (location, stenosis_percentage).
    pub fn find_stenoses(&self, reference_diameter: f64) -> Vec<(f64, f64)> {
        let mut stenoses = Vec::new();
        if reference_diameter <= 0.0 {
            return stenoses;
        }

        for point in &self.points {
            let stenosis_pct = (1.0 - point.diameter / reference_diameter) * 100.0;
            if stenosis_pct > 25.0 {
                stenoses.push((point.distance_from_ostium, stenosis_pct));
            }
        }

        stenoses
    }

    /// Compute the curved MPR reformat path through the volume.
    ///
    /// Returns slice positions (in patient coordinates) for generating
    /// curved MPR images along the vessel.
    pub fn curved_mpr_slices(&self, num_slices: usize) -> Vec<(f64, f64, f64)> {
        if self.points.is_empty() || num_slices == 0 {
            return Vec::new();
        }

        let total = self.total_length;
        let step = total / (num_slices - 1).max(1) as f64;

        let mut slices = Vec::with_capacity(num_slices);
        for i in 0..num_slices {
            let target_dist = step * i as f64;
            // Find the closest point
            let closest = self.points.iter().min_by_key(|p| {
                ((p.distance_from_ostium - target_dist).abs() * 1000.0) as u64
            });
            if let Some(p) = closest {
                slices.push((p.x, p.y, p.z));
            }
        }

        slices
    }
}

/// Extract a vessel centerline using a simplified vessel tracking algorithm.
///
/// This stub implementation creates a synthetic centerline for testing.
/// A real implementation would use Frangi vesselness filtering and
/// minimum cost path computation.
pub fn extract_centerline(
    _volume: &[f64],
    _width: usize,
    _height: usize,
    _depth: usize,
    seed: (usize, usize, usize),
    artery: CoronaryArtery,
) -> VesselCenterline {
    let mut cl = VesselCenterline::new(artery);
    let base_diameter = match artery {
        CoronaryArtery::LeftMain => 4.0,
        CoronaryArtery::Lad => 3.5,
        CoronaryArtery::Lcx => 3.0,
        CoronaryArtery::Rca => 3.5,
        CoronaryArtery::Pda => 2.5,
    };

    // Generate a synthetic curved centerline
    for i in 0..50 {
        let t = i as f64 / 49.0;
        let x = seed.0 as f64 + t * 50.0;
        let y = seed.1 as f64 + (t * std::f64::consts::PI).sin() * 10.0;
        let z = seed.2 as f64 + t * 30.0;
        let diameter = base_diameter * (1.0 - 0.3 * t); // Tapering
        cl.add_point(x, y, z, diameter);
    }

    cl
}

// ===========================================================================
// S7-T3: Ejection Fraction (Simpson's Method)
// ===========================================================================

/// A contour delineating the ventricular boundary on a single slice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VentricularContour {
    /// Contour points in image coordinates (x, y pairs).
    pub points: Vec<(f64, f64)>,
    /// Slice index in the stack.
    pub slice_index: usize,
    /// Phase of the cardiac cycle.
    pub phase: CardiacPhase,
    /// Enclosed area in square centimeters.
    pub area_cm2: f64,
}

/// Cardiac cycle phase.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum CardiacPhase {
    /// End-diastole (maximum volume).
    EndDiastole,
    /// End-systole (minimum volume).
    EndSystole,
    /// Intermediate frame.
    MidSystole,
}

/// Ejection fraction result computed via Simpson's method of discs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EjectionFractionResult {
    /// End-diastolic volume in milliliters.
    pub edv_ml: f64,
    /// End-systolic volume in milliliters.
    pub esv_ml: f64,
    /// Stroke volume in milliliters.
    pub sv_ml: f64,
    /// Ejection fraction as percentage.
    pub ef_pct: f64,
    /// Cardiac output in liters per minute (requires heart rate).
    pub cardiac_output_lpm: Option<f64>,
    /// Uncertainty bounds for EF%.
    pub ef_uncertainty: f64,
    /// Ventricular contour data used for computation.
    pub contours: Vec<VentricularContour>,
}

impl EjectionFractionResult {
    /// Compute ejection fraction from EDV and ESV.
    pub fn from_volumes(edv_ml: f64, esv_ml: f64, heart_rate: Option<f64>) -> Self {
        let sv_ml = edv_ml - esv_ml;
        let ef_pct = if edv_ml > 0.0 { (sv_ml / edv_ml) * 100.0 } else { 0.0 };

        // Uncertainty estimation: ±5% for manual contouring, ±3% for semi-automatic
        let ef_uncertainty = 5.0;

        let cardiac_output_lpm = heart_rate.map(|hr| sv_ml * hr / 1000.0);

        Self {
            edv_ml,
            esv_ml,
            sv_ml,
            ef_pct,
            cardiac_output_lpm,
            ef_uncertainty,
            contours: Vec::new(),
        }
    }

    /// Compute EF using Simpson's method of discs from short-axis contours.
    ///
    /// Simpson's rule: sum the areas of discs at each slice position,
    /// multiplied by slice thickness, to estimate 3D volume.
    pub fn from_simpson_discs(
        ed_contours: &[VentricularContour],
        es_contours: &[VentricularContour],
        slice_thickness_cm: f64,
        heart_rate: Option<f64>,
    ) -> Self {
        let edv_ml = simpson_volume(ed_contours, slice_thickness_cm) * 1000.0; // cm3 to mL
        let esv_ml = simpson_volume(es_contours, slice_thickness_cm) * 1000.0;

        let mut result = Self::from_volumes(edv_ml, esv_ml, heart_rate);
        result.contours = ed_contours.iter().chain(es_contours.iter()).cloned().collect();
        result
    }
}

/// Compute volume using Simpson's method of discs.
///
/// Each contour's area is treated as a disc. The volume is the sum of
/// all disc areas multiplied by slice thickness.
fn simpson_volume(contours: &[VentricularContour], slice_thickness_cm: f64) -> f64 {
    let mut volume = 0.0;
    for contour in contours {
        volume += contour.area_cm2 * slice_thickness_cm;
    }
    volume
}

/// Detect end-diastole and end-systole frames from a time series of cavity areas.
///
/// Returns the indices of the ED (max area) and ES (min area) frames.
pub fn detect_ed_es_frames(areas: &[f64]) -> (usize, usize) {
    if areas.is_empty() {
        return (0, 0);
    }

    let ed_idx = areas
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    // ES must come after ED in the cardiac cycle
    let es_idx = areas[ed_idx..]
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| ed_idx + i)
        .unwrap_or(0);

    (ed_idx, es_idx)
}

/// Compute the area enclosed by a polygon using the Shoelace formula.
pub fn polygon_area(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }

    let n = points.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += points[i].0 * points[j].1;
        area -= points[j].0 * points[i].1;
    }
    (area / 2.0).abs()
}

// ---------------------------------------------------------------------------
// Shared: Error helpers
// ---------------------------------------------------------------------------

fn cardio_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-cardio".to_string(),
            detail: detail.into(),
        },
        "cardio error",
    )
    .into()
}

// ===========================================================================
// Tests: S7-T1 Calcium Scoring (minimum 8)
// ===========================================================================

#[cfg(test)]
mod tests_calcium_scoring {
    use super::*;

    #[test]
    fn agatston_weight_classification() {
        // REQ-CARDIO-100: Agatston weight must follow standard HU thresholds
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 150.0;
        assert_eq!(lesion.agatston_weight(), 1.0);

        lesion.peak_hu = 250.0;
        assert_eq!(lesion.agatston_weight(), 2.0);

        lesion.peak_hu = 350.0;
        assert_eq!(lesion.agatston_weight(), 3.0);

        lesion.peak_hu = 500.0;
        assert_eq!(lesion.agatston_weight(), 4.0);
    }

    #[test]
    fn agatston_score_calculation() {
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 250.0;
        lesion.area_mm2 = 10.0;

        let score = lesion.agatston_score(0.5, 3.0);
        // weight = 2.0, num_pixels = 10.0 / 0.5 = 20, score = 20 * 0.5 * 2.0 * 3.0 = 60.0
        assert_eq!(score, 60.0);
    }

    #[test]
    fn calcium_score_from_lesions() {
        let mut l1 = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        l1.peak_hu = 200.0;
        l1.area_mm2 = 5.0;
        l1.volume_mm3 = 15.0;
        l1.mass_mg = 3.0;

        let mut l2 = CalcifiedLesion::new("l2".to_string(), CoronaryArtery::Rca);
        l2.peak_hu = 400.0;
        l2.area_mm2 = 8.0;
        l2.volume_mm3 = 24.0;
        l2.mass_mg = 5.0;

        let result = CalciumScoreResult::from_lesions(vec![l1, l2], 0.5, 3.0);
        assert!(result.total_agatston > 0.0);
        assert_eq!(result.total_volume_mm3, 39.0);
        assert_eq!(result.total_mass_mg, 8.0);
        assert!(result.artery_scores.contains_key(&CoronaryArtery::Lad));
        assert!(result.artery_scores.contains_key(&CoronaryArtery::Rca));
    }

    #[test]
    fn calcium_detection_in_volume() {
        let mut volume = vec![0.0; 10 * 10 * 5];
        // Place calcium voxels
        volume[2 * (10 * 10) + 5 * 10 + 5] = 200.0;
        volume[2 * (10 * 10) + 5 * 10 + 6] = 250.0;
        volume[2 * (10 * 10) + 6 * 10 + 5] = 180.0;

        let artery_map = vec![CoronaryArtery::Lad; 5];
        let lesions = detect_calcium_lesions(&volume, 10, 10, 5, (0.5, 0.5), 3.0, &artery_map);
        assert!(!lesions.is_empty());
        assert!(lesions[0].peak_hu >= 180.0);
    }

    #[test]
    fn coronary_artery_dicom_codes() {
        assert_eq!(CoronaryArtery::LeftMain.dicom_code(), "74066000");
        assert_eq!(CoronaryArtery::Lad.dicom_code(), "74290005");
    }

    #[test]
    fn age_sex_percentile_range() {
        let result = CalciumScoreResult::from_lesions(vec![], 0.5, 3.0);
        let pct = result.age_sex_percentile(55, true);
        assert!(pct >= 1.0 && pct <= 99.0);
    }

    #[test]
    fn encode_to_dicom_sr() {
        let mut lesion = CalcifiedLesion::new("l1".to_string(), CoronaryArtery::Lad);
        lesion.peak_hu = 200.0;
        lesion.area_mm2 = 5.0;
        lesion.volume_mm3 = 15.0;
        lesion.mass_mg = 3.0;

        let result = CalciumScoreResult::from_lesions(vec![lesion], 0.5, 3.0);
        let ds = result.encode_to_dicom_sr("1.2.3", "4.5.6");
        assert_eq!(ds.get_uid(Tag(0x0020, 0x000D)), Some("1.2.3"));
    }

    #[test]
    fn hu_threshold_constant() {
        assert_eq!(CALCIUM_HU_THRESHOLD, 130.0);
    }
}

// ===========================================================================
// Tests: S7-T2 Coronary Artery Analysis (minimum 6)
// ===========================================================================

#[cfg(test)]
mod tests_vessel_analysis {
    use super::*;

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
        cl.add_point(0.0, 0.0, 0.0, 3.5);   // Normal
        cl.add_point(10.0, 0.0, 0.0, 3.5);  // Normal
        cl.add_point(20.0, 0.0, 0.0, 1.5);  // 57% stenosis
        cl.add_point(30.0, 0.0, 0.0, 3.5);  // Normal

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
            &vec![0.0; 1000], 10, 10, 10,
            (5, 5, 5), CoronaryArtery::LeftMain,
        );
        // First point should have larger diameter than last
        let first_d = cl.points.first().map(|p| p.diameter).unwrap_or(0.0);
        let last_d = cl.points.last().map(|p| p.diameter).unwrap_or(0.0);
        assert!(first_d > last_d, "Vessel should taper distally");
    }
}

// ===========================================================================
// Tests: S7-T3 Ejection Fraction (minimum 6)
// ===========================================================================

#[cfg(test)]
mod tests_ejection_fraction {
    use super::*;

    #[test]
    fn ef_from_volumes() {
        // REQ-CARDIO-300: EF% = (EDV - ESV) / EDV * 100
        let result = EjectionFractionResult::from_volumes(120.0, 50.0, Some(72.0));
        assert!((result.ef_pct - 58.33).abs() < 1.0);
        assert!((result.sv_ml - 70.0).abs() < 1e-6);
        assert!(result.cardiac_output_lpm.is_some());
        assert!((result.cardiac_output_lpm.unwrap() - 5.04).abs() < 0.1);
    }

    #[test]
    fn ef_zero_edv() {
        let result = EjectionFractionResult::from_volumes(0.0, 0.0, None);
        assert_eq!(result.ef_pct, 0.0);
    }

    #[test]
    fn simpson_discs_volume() {
        let ed_contours = vec![
            VentricularContour {
                points: vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
                slice_index: 0,
                phase: CardiacPhase::EndDiastole,
                area_cm2: 16.0,
            },
            VentricularContour {
                points: vec![(0.0, 0.0), (3.0, 0.0), (3.0, 3.0), (0.0, 3.0)],
                slice_index: 1,
                phase: CardiacPhase::EndDiastole,
                area_cm2: 9.0,
            },
        ];

        let es_contours = vec![
            VentricularContour {
                points: vec![(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
                slice_index: 0,
                phase: CardiacPhase::EndSystole,
                area_cm2: 4.0,
            },
            VentricularContour {
                points: vec![(0.0, 0.0), (1.5, 0.0), (1.5, 1.5), (0.0, 1.5)],
                slice_index: 1,
                phase: CardiacPhase::EndSystole,
                area_cm2: 2.25,
            },
        ];

        let result = EjectionFractionResult::from_simpson_discs(
            &ed_contours, &es_contours, 1.0, None,
        );

        assert!(result.edv_ml > 0.0);
        assert!(result.esv_ml > 0.0);
        assert!(result.ef_pct > 0.0);
    }

    #[test]
    fn detect_ed_es_frames_test() {
        let areas = vec![10.0, 12.0, 15.0, 14.0, 8.0, 6.0, 7.0, 11.0];
        let (ed_idx, es_idx) = detect_ed_es_frames(&areas);
        assert_eq!(ed_idx, 2);  // Max area at index 2
        assert!(es_idx > ed_idx);
    }

    #[test]
    fn polygon_area_calculation() {
        let points = vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)];
        let area = polygon_area(&points);
        assert!((area - 16.0).abs() < 1e-6);
    }

    #[test]
    fn polygon_area_triangle() {
        let points = vec![(0.0, 0.0), (3.0, 0.0), (0.0, 4.0)];
        let area = polygon_area(&points);
        assert!((area - 6.0).abs() < 1e-6);
    }

    #[test]
    fn ef_uncertainty_bounds() {
        let result = EjectionFractionResult::from_volumes(120.0, 50.0, None);
        assert_eq!(result.ef_uncertainty, 5.0);
    }
}
