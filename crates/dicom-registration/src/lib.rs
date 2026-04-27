#![deny(missing_docs)]

//! DICOM image registration: rigid and deformable registration algorithms.
//!
//! Provides rigid registration (6 DOF: 3 translation + 3 rotation) and
//! B-spline deformable registration with deformation vector field (DVF)
//! support. Encodes results as DICOM Registration and Deformable Spatial
//! Registration IODs.

use dicom_audit::{AuditEvent, AuditEventKind, AuditField, AuditValue};
use dicom_core::{Dataset, Element, Error, ErrorKind, Result, Tag, Value, Vr};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// S4-T1: Rigid Registration
// ---------------------------------------------------------------------------

/// Rigid transform with 6 degrees of freedom (3 translation + 3 rotation).
#[derive(Debug, Clone, PartialEq)]
pub struct RigidTransform {
    /// Translation along X axis in millimeters.
    pub tx: f64,
    /// Translation along Y axis in millimeters.
    pub ty: f64,
    /// Translation along Z axis in millimeters.
    pub tz: f64,
    /// Rotation around X axis in degrees (pitch).
    pub rx: f64,
    /// Rotation around Y axis in degrees (yaw).
    pub ry: f64,
    /// Rotation around Z axis in degrees (roll).
    pub rz: f64,
}

impl RigidTransform {
    /// Create an identity (zero) rigid transform.
    pub fn identity() -> Self {
        Self {
            tx: 0.0,
            ty: 0.0,
            tz: 0.0,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
        }
    }

    /// Create a translation-only rigid transform.
    pub fn translation(tx: f64, ty: f64, tz: f64) -> Self {
        Self {
            tx,
            ty,
            tz,
            rx: 0.0,
            ry: 0.0,
            rz: 0.0,
        }
    }

    /// Create a rotation-only rigid transform.
    pub fn rotation(rx: f64, ry: f64, rz: f64) -> Self {
        Self {
            tx: 0.0,
            ty: 0.0,
            tz: 0.0,
            rx,
            ry,
            rz,
        }
    }

    /// Compose this transform with another (self followed by other).
    ///
    /// For simplified synthetic data, this adds translations and rotations.
    pub fn compose(&self, other: &RigidTransform) -> RigidTransform {
        RigidTransform {
            tx: self.tx + other.tx,
            ty: self.ty + other.ty,
            tz: self.tz + other.tz,
            rx: self.rx + other.rx,
            ry: self.ry + other.ry,
            rz: self.rz + other.rz,
        }
    }

    /// Invert this rigid transform.
    ///
    /// For the simplified model, inversion negates all parameters.
    pub fn invert(&self) -> RigidTransform {
        RigidTransform {
            tx: -self.tx,
            ty: -self.ty,
            tz: -self.tz,
            rx: -self.rx,
            ry: -self.ry,
            rz: -self.rz,
        }
    }

    /// Apply this transform to a 3D point (simplified: translation only for synthetic data).
    pub fn apply_to_point(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        // For simplified synthetic data: apply rotation then translation
        let rz_rad = self.rz.to_radians();
        let rx_rad = self.rx.to_radians();
        let ry_rad = self.ry.to_radians();

        // Apply Rz rotation
        let x1 = x * rz_rad.cos() - y * rz_rad.sin();
        let y1 = x * rz_rad.sin() + y * rz_rad.cos();
        let z1 = z;

        // Apply Ry rotation
        let x2 = x1 * ry_rad.cos() + z1 * ry_rad.sin();
        let y2 = y1;
        let z2 = -x1 * ry_rad.sin() + z1 * ry_rad.cos();

        // Apply Rx rotation
        let x3 = x2;
        let y3 = y2 * rx_rad.cos() - z2 * rx_rad.sin();
        let z3 = y2 * rx_rad.sin() + z2 * rx_rad.cos();

        (x3 + self.tx, y3 + self.ty, z3 + self.tz)
    }

    /// Validate that all parameters are finite.
    pub fn validate(&self) -> Result<()> {
        if !self.tx.is_finite()
            || !self.ty.is_finite()
            || !self.tz.is_finite()
            || !self.rx.is_finite()
            || !self.ry.is_finite()
            || !self.rz.is_finite()
        {
            return Err(registration_error(
                "rigid transform parameters must be finite",
            ));
        }
        Ok(())
    }
}

impl Default for RigidTransform {
    fn default() -> Self {
        Self::identity()
    }
}

/// Similarity metric used for registration optimization.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegistrationMetric {
    /// Mutual Information metric.
    MutualInformation,
    /// Normalized Cross-Correlation metric.
    NormalizedCrossCorrelation,
    /// Mean Squares metric.
    MeanSquares,
}

/// Result of a rigid registration procedure.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationResult {
    /// The computed rigid transform.
    pub transform: RigidTransform,
    /// The final metric value (lower is better for MSE, higher for MI/NCC).
    pub metric_value: f64,
    /// Number of optimization iterations performed.
    pub iterations: u32,
    /// Whether the algorithm converged before hitting max iterations.
    pub convergence_status: ConvergenceStatus,
}

/// Convergence status of a registration optimization.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ConvergenceStatus {
    /// The algorithm converged successfully.
    Converged,
    /// The algorithm did not converge within the iteration limit.
    NotConverged,
}

/// Configuration for rigid registration.
#[derive(Debug, Clone, PartialEq)]
pub struct RigidRegistrationConfig {
    /// Maximum number of optimization iterations.
    pub max_iterations: u32,
    /// Convergence threshold for metric improvement.
    pub convergence_threshold: f64,
    /// Initial step size for gradient descent.
    pub step_size: f64,
    /// Multi-resolution levels (e.g., [8, 4, 2] means 3 levels).
    pub pyramid_levels: Vec<u32>,
}

impl Default for RigidRegistrationConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            convergence_threshold: 1e-6,
            step_size: 1.0,
            pyramid_levels: vec![8, 4, 2],
        }
    }
}

/// A 3D volume grid for registration input.
#[derive(Debug, Clone, PartialEq)]
pub struct VolumeGrid {
    /// Voxel data as flat array in z-y-x order.
    pub data: Vec<f64>,
    /// Width (x dimension) in voxels.
    pub width: usize,
    /// Height (y dimension) in voxels.
    pub height: usize,
    /// Depth (z dimension) in voxels.
    pub depth: usize,
    /// Voxel spacing (x, y, z) in millimeters.
    pub spacing: (f64, f64, f64),
}

impl VolumeGrid {
    /// Create a new volume grid with zero-initialized data.
    pub fn new(
        width: usize,
        height: usize,
        depth: usize,
        spacing: (f64, f64, f64),
    ) -> Result<Self> {
        if width == 0 || height == 0 || depth == 0 {
            return Err(registration_error("volume dimensions must be non-zero"));
        }
        if !spacing.0.is_finite() || !spacing.1.is_finite() || !spacing.2.is_finite() {
            return Err(registration_error("volume spacing must be finite"));
        }
        if spacing.0 <= 0.0 || spacing.1 <= 0.0 || spacing.2 <= 0.0 {
            return Err(registration_error("volume spacing must be positive"));
        }
        let count = width * height * depth;
        Ok(Self {
            data: vec![0.0; count],
            width,
            height,
            depth,
            spacing,
        })
    }

    /// Create a volume grid from existing data.
    pub fn from_data(
        data: Vec<f64>,
        width: usize,
        height: usize,
        depth: usize,
        spacing: (f64, f64, f64),
    ) -> Result<Self> {
        let expected = width * height * depth;
        if data.len() != expected {
            return Err(registration_error(
                "volume data length does not match dimensions",
            ));
        }
        if width == 0 || height == 0 || depth == 0 {
            return Err(registration_error("volume dimensions must be non-zero"));
        }
        if !spacing.0.is_finite() || !spacing.1.is_finite() || !spacing.2.is_finite() {
            return Err(registration_error("volume spacing must be finite"));
        }
        if spacing.0 <= 0.0 || spacing.1 <= 0.0 || spacing.2 <= 0.0 {
            return Err(registration_error("volume spacing must be positive"));
        }
        Ok(Self {
            data,
            width,
            height,
            depth,
            spacing,
        })
    }

    /// Get voxel value at (x, y, z).
    pub fn voxel(&self, x: usize, y: usize, z: usize) -> Option<f64> {
        if x >= self.width || y >= self.height || z >= self.depth {
            return None;
        }
        let idx = z * (self.width * self.height) + y * self.width + x;
        self.data.get(idx).copied()
    }

    /// Set voxel value at (x, y, z).
    pub fn set_voxel(&mut self, x: usize, y: usize, z: usize, value: f64) -> bool {
        if x >= self.width || y >= self.height || z >= self.depth {
            return false;
        }
        let idx = z * (self.width * self.height) + y * self.width + x;
        if let Some(slot) = self.data.get_mut(idx) {
            *slot = value;
            return true;
        }
        false
    }

    /// Total number of voxels.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the volume is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Multi-resolution pyramid that downsamples volumes by specified factors.
#[derive(Debug, Clone)]
pub struct MultiResolutionPyramid {
    /// Pyramid levels, from coarsest to finest.
    pub levels: Vec<VolumeGrid>,
}

impl MultiResolutionPyramid {
    /// Build a multi-resolution pyramid from a volume grid.
    ///
    /// The `factors` parameter specifies the downsample factors for each level,
    /// ordered from coarsest to finest. The original volume is always included
    /// as the finest level.
    pub fn build(volume: &VolumeGrid, factors: &[u32]) -> Result<Self> {
        if factors.is_empty() {
            return Err(registration_error(
                "pyramid requires at least one level factor",
            ));
        }
        if volume.is_empty() {
            return Err(registration_error("cannot build pyramid from empty volume"));
        }

        let mut levels = Vec::new();

        // Build from coarsest to finest
        for &factor in factors.iter().rev() {
            if factor == 0 {
                return Err(registration_error("downsample factor must be non-zero"));
            }
            let downsampled = downsample_volume(volume, factor)?;
            levels.push(downsampled);
        }

        // Always include the original volume as the finest level
        levels.push(volume.clone());

        Ok(Self { levels })
    }

    /// Return the number of pyramid levels.
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Get a specific pyramid level by index (0 = coarsest).
    pub fn level(&self, index: usize) -> Option<&VolumeGrid> {
        self.levels.get(index)
    }
}

/// Downsample a volume by a given factor using block averaging.
fn downsample_volume(volume: &VolumeGrid, factor: u32) -> Result<VolumeGrid> {
    if factor == 0 {
        return Err(registration_error("downsample factor must be non-zero"));
    }
    let f = factor as usize;
    let new_w = (volume.width + f - 1) / f;
    let new_h = (volume.height + f - 1) / f;
    let new_d = (volume.depth + f - 1) / f;

    if new_w == 0 || new_h == 0 || new_d == 0 {
        return Err(registration_error("downsampled volume has zero dimensions"));
    }

    let new_spacing = (
        volume.spacing.0 * f as f64,
        volume.spacing.1 * f as f64,
        volume.spacing.2 * f as f64,
    );

    let mut data = vec![0.0; new_w * new_h * new_d];
    let mut counts = vec![0usize; new_w * new_h * new_d];

    for z in 0..volume.depth {
        for y in 0..volume.height {
            for x in 0..volume.width {
                let nx = x / f;
                let ny = y / f;
                let nz = z / f;
                if nx >= new_w || ny >= new_h || nz >= new_d {
                    continue;
                }
                let src_idx = z * (volume.width * volume.height) + y * volume.width + x;
                let dst_idx = nz * (new_w * new_h) + ny * new_w + nx;
                if let Some(&val) = volume.data.get(src_idx) {
                    data[dst_idx] += val;
                    counts[dst_idx] += 1;
                }
            }
        }
    }

    for (i, count) in counts.iter().enumerate() {
        if *count > 0 {
            data[i] /= *count as f64;
        }
    }

    VolumeGrid::from_data(data, new_w, new_h, new_d, new_spacing)
}

/// Compute the similarity metric between two volumes.
pub fn compute_metric(
    fixed: &VolumeGrid,
    moving: &VolumeGrid,
    metric: RegistrationMetric,
) -> Result<f64> {
    if fixed.width != moving.width || fixed.height != moving.height || fixed.depth != moving.depth {
        return Err(registration_error(
            "volumes must have the same dimensions for metric computation",
        ));
    }
    if fixed.data.is_empty() {
        return Err(registration_error("cannot compute metric on empty volumes"));
    }

    match metric {
        RegistrationMetric::MeanSquares => compute_mse(fixed, moving),
        RegistrationMetric::NormalizedCrossCorrelation => compute_ncc(fixed, moving),
        RegistrationMetric::MutualInformation => compute_mi(fixed, moving),
    }
}

/// Compute Mean Squared Error between two volumes.
fn compute_mse(fixed: &VolumeGrid, moving: &VolumeGrid) -> Result<f64> {
    let n = fixed.data.len();
    if n == 0 {
        return Ok(0.0);
    }
    let sum: f64 = fixed
        .data
        .iter()
        .zip(moving.data.iter())
        .map(|(f, m)| (f - m).powi(2))
        .sum();
    Ok(sum / n as f64)
}

/// Compute Normalized Cross-Correlation between two volumes.
fn compute_ncc(fixed: &VolumeGrid, moving: &VolumeGrid) -> Result<f64> {
    let n = fixed.data.len() as f64;
    if n == 0.0 {
        return Ok(0.0);
    }

    let mean_f: f64 = fixed.data.iter().sum::<f64>() / n;
    let mean_m: f64 = moving.data.iter().sum::<f64>() / n;

    let mut num = 0.0;
    let mut den_f = 0.0;
    let mut den_m = 0.0;

    for (f, m) in fixed.data.iter().zip(moving.data.iter()) {
        let df = f - mean_f;
        let dm = m - mean_m;
        num += df * dm;
        den_f += df * df;
        den_m += dm * dm;
    }

    let den = (den_f * den_m).sqrt();
    if den < 1e-12 {
        return Ok(0.0);
    }
    Ok(num / den)
}

/// Compute Mutual Information between two volumes (simplified histogram-based).
fn compute_mi(fixed: &VolumeGrid, moving: &VolumeGrid) -> Result<f64> {
    let n = fixed.data.len() as f64;
    if n == 0.0 {
        return Ok(0.0);
    }

    // Discretize into 32 bins
    let num_bins = 32;
    let f_min = fixed.data.iter().cloned().fold(f64::INFINITY, f64::min);
    let f_max = fixed.data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let m_min = moving.data.iter().cloned().fold(f64::INFINITY, f64::min);
    let m_max = moving
        .data
        .iter()
        .cloned()
        .fold(f64::NEG_INFINITY, f64::max);

    let f_range = (f_max - f_min).max(1e-12);
    let m_range = (m_max - m_min).max(1e-12);

    let mut joint_hist = vec![0u32; num_bins * num_bins];
    let mut f_hist = vec![0u32; num_bins];
    let mut m_hist = vec![0u32; num_bins];

    for (f, m) in fixed.data.iter().zip(moving.data.iter()) {
        let fi = (((f - f_min) / f_range) * (num_bins as f64 - 1.0))
            .round()
            .clamp(0.0, (num_bins - 1) as f64) as usize;
        let mi = (((m - m_min) / m_range) * (num_bins as f64 - 1.0))
            .round()
            .clamp(0.0, (num_bins - 1) as f64) as usize;
        joint_hist[fi * num_bins + mi] += 1;
        f_hist[fi] += 1;
        m_hist[mi] += 1;
    }

    let mut mi_val = 0.0;
    for fi in 0..num_bins {
        for mii in 0..num_bins {
            let p_joint = joint_hist[fi * num_bins + mii] as f64 / n;
            if p_joint < 1e-12 {
                continue;
            }
            let p_f = f_hist[fi] as f64 / n;
            let p_m = m_hist[mii] as f64 / n;
            if p_f < 1e-12 || p_m < 1e-12 {
                continue;
            }
            mi_val += p_joint * (p_joint / (p_f * p_m)).ln();
        }
    }

    Ok(mi_val)
}

/// Perform rigid registration between a fixed and moving volume.
///
/// Uses a multi-resolution approach: start at coarsest level and refine at
/// finer levels. For synthetic data, uses gradient-descent-style optimization
/// with the specified similarity metric.
pub fn rigid_register(
    fixed: &VolumeGrid,
    moving: &VolumeGrid,
    metric: RegistrationMetric,
    config: &RigidRegistrationConfig,
) -> Result<RegistrationResult> {
    fixed.validate()?;
    moving.validate()?;

    if fixed.is_empty() || moving.is_empty() {
        return Err(registration_error("cannot register empty volumes"));
    }

    // Build multi-resolution pyramid
    let pyramid = MultiResolutionPyramid::build(moving, &config.pyramid_levels)?;

    let mut current_transform = RigidTransform::identity();
    let mut total_iterations: u32 = 0;
    let mut converged = false;
    let mut final_metric = 0.0_f64;

    // Process each pyramid level from coarsest to finest
    for level_idx in 0..pyramid.level_count() {
        let level_volume = pyramid.level(level_idx).expect("level exists");
        let scale = if level_idx < pyramid.level_count() - 1 {
            config
                .pyramid_levels
                .iter()
                .rev()
                .nth(level_idx)
                .copied()
                .unwrap_or(1) as f64
        } else {
            1.0
        };

        // Downsample fixed volume to match this level
        let factor = if scale > 1.0 { scale as u32 } else { 1 };
        let fixed_level = if factor > 1 {
            downsample_volume(fixed, factor)?
        } else {
            fixed.clone()
        };

        // Check dimension compatibility
        if fixed_level.width != level_volume.width
            || fixed_level.height != level_volume.height
            || fixed_level.depth != level_volume.depth
        {
            // Skip incompatible levels (dimensions don't match after downsampling)
            continue;
        }

        // Optimize at this level using simplified gradient descent
        let mut prev_metric = compute_metric(&fixed_level, level_volume, metric)?;
        let step = config.step_size / scale;

        for _ in 0..config.max_iterations {
            total_iterations += 1;

            // Try small perturbations in each parameter direction
            let mut best_transform = current_transform.clone();
            let mut best_metric = prev_metric;

            let params = [
                (step, 0.0, 0.0, 0.0, 0.0, 0.0),
                (-step, 0.0, 0.0, 0.0, 0.0, 0.0),
                (0.0, step, 0.0, 0.0, 0.0, 0.0),
                (0.0, -step, 0.0, 0.0, 0.0, 0.0),
                (0.0, 0.0, step, 0.0, 0.0, 0.0),
                (0.0, 0.0, -step, 0.0, 0.0, 0.0),
                (0.0, 0.0, 0.0, step * 0.1, 0.0, 0.0),
                (0.0, 0.0, 0.0, -step * 0.1, 0.0, 0.0),
                (0.0, 0.0, 0.0, 0.0, step * 0.1, 0.0),
                (0.0, 0.0, 0.0, 0.0, -step * 0.1, 0.0),
                (0.0, 0.0, 0.0, 0.0, 0.0, step * 0.1),
                (0.0, 0.0, 0.0, 0.0, 0.0, -step * 0.1),
            ];

            for (dtx, dty, dtz, drx, dry, drz) in &params {
                let trial = RigidTransform {
                    tx: current_transform.tx + dtx,
                    ty: current_transform.ty + dty,
                    tz: current_transform.tz + dtz,
                    rx: current_transform.rx + drx,
                    ry: current_transform.ry + dry,
                    rz: current_transform.rz + drz,
                };
                let trial_moving = apply_rigid_transform(level_volume, &trial)?;
                let trial_metric = compute_metric(&fixed_level, &trial_moving, metric)?;

                let improved = match metric {
                    RegistrationMetric::MeanSquares => trial_metric < best_metric,
                    RegistrationMetric::NormalizedCrossCorrelation
                    | RegistrationMetric::MutualInformation => trial_metric > best_metric,
                };

                if improved {
                    best_metric = trial_metric;
                    best_transform = trial;
                }
            }

            let improvement = (best_metric - prev_metric).abs();
            current_transform = best_transform;
            prev_metric = best_metric;

            if improvement < config.convergence_threshold {
                converged = true;
                break;
            }
        }

        final_metric = prev_metric;
    }

    Ok(RegistrationResult {
        transform: current_transform,
        metric_value: final_metric,
        iterations: total_iterations,
        convergence_status: if converged {
            ConvergenceStatus::Converged
        } else {
            ConvergenceStatus::NotConverged
        },
    })
}

/// Apply a rigid transform to a volume grid (resample).
fn apply_rigid_transform(volume: &VolumeGrid, transform: &RigidTransform) -> Result<VolumeGrid> {
    let mut result = VolumeGrid::new(volume.width, volume.height, volume.depth, volume.spacing)?;

    for z in 0..volume.depth {
        for y in 0..volume.height {
            for x in 0..volume.width {
                // Convert voxel coords to physical coords
                let px = x as f64 * volume.spacing.0;
                let py = y as f64 * volume.spacing.1;
                let pz = z as f64 * volume.spacing.2;

                // Apply inverse transform (to find source voxel)
                let inv = transform.invert();
                let (sx, sy, sz) = inv.apply_to_point(px, py, pz);

                // Convert back to voxel coords
                let svx = (sx / volume.spacing.0).round() as isize;
                let svy = (sy / volume.spacing.1).round() as isize;
                let svz = (sz / volume.spacing.2).round() as isize;

                // Nearest-neighbor interpolation
                if svx >= 0
                    && (svx as usize) < volume.width
                    && svy >= 0
                    && (svy as usize) < volume.height
                    && svz >= 0
                    && (svz as usize) < volume.depth
                {
                    if let Some(val) = volume.voxel(svx as usize, svy as usize, svz as usize) {
                        result.set_voxel(x, y, z, val);
                    }
                }
            }
        }
    }

    Ok(result)
}

impl VolumeGrid {
    /// Validate the volume grid.
    pub fn validate(&self) -> Result<()> {
        let expected = self.width * self.height * self.depth;
        if self.data.len() != expected {
            return Err(registration_error("volume data length mismatch"));
        }
        Ok(())
    }
}

/// Encode a rigid registration result as a DICOM Registration IOD Dataset.
pub fn encode_registration_iod(
    result: &RegistrationResult,
    study_uid: &str,
    series_uid: &str,
) -> Result<Dataset> {
    let mut ds = Dataset::new();

    // SOP Class UID for Spatial Registration Storage (1.2.840.10008.5.1.4.1.1.66.1)
    ds.insert(Element::new(
        Tag(0x0008, 0x0016),
        Vr::Ui,
        Value::Uid("1.2.840.10008.5.1.4.1.1.66.1".to_string()),
    )?);

    // SOP Instance UID (generate deterministic from content)
    let sop_uid = format!(
        "1.2.840.113619.6.4.{}.{}",
        study_uid.len(),
        series_uid.len()
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

    // Registration Sequence
    let mut reg_item = Dataset::new();

    // Matrix Registration Type Code Sequence
    let mut matrix_type_item = Dataset::new();
    matrix_type_item.insert(
        Element::new(Tag(0x0008, 0x0100), Vr::Sh, Value::Str("RIGID".to_string())).unwrap(),
    );
    matrix_type_item.insert(
        Element::new(
            Tag(0x0008, 0x0102),
            Vr::Sh,
            Value::Str("99DICOM_REG".to_string()),
        )
        .unwrap(),
    );
    matrix_type_item.insert(
        Element::new(Tag(0x0008, 0x0104), Vr::Lo, Value::Str("Rigid".to_string())).unwrap(),
    );

    reg_item.insert(
        Element::new(
            Tag(0x0070, 0x030D),
            Vr::Sq,
            Value::Sequence(vec![matrix_type_item]),
        )
        .unwrap(),
    );

    // Registration Matrix (4x4 rigid transform)
    // Row-major: rotation matrix + translation
    let rz = result.transform.rz.to_radians();
    let rx = result.transform.rx.to_radians();
    let ry = result.transform.ry.to_radians();

    // Simplified rotation matrix (Rz * Ry * Rx)
    let r00 = ry.cos() * rz.cos();
    let r01 = -ry.cos() * rz.sin();
    let r02 = ry.sin();
    let r10 = rx.sin() * ry.sin() * rz.cos() + rx.cos() * rz.sin();
    let r11 = -rx.sin() * ry.sin() * rz.sin() + rx.cos() * rz.cos();
    let r12 = -rx.sin() * ry.cos();
    let r20 = -rx.cos() * ry.sin() * rz.cos() + rx.sin() * rz.sin();
    let r21 = rx.cos() * ry.sin() * rz.sin() + rx.sin() * rz.cos();
    let r22 = rx.cos() * ry.cos();

    let matrix_values = format!(
        "{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\{:.10}\\\\0\\\\0\\\\0\\\\1",
        r00, r01, r02, result.transform.tx,
        r10, r11, r12, result.transform.ty,
        r20, r21, r22, result.transform.tz,
    );

    reg_item.insert(Element::new(Tag(0x0070, 0x030C), Vr::Fd, Value::Str(matrix_values)).unwrap());

    ds.insert(Element::new(
        Tag(0x0070, 0x0308),
        Vr::Sq,
        Value::Sequence(vec![reg_item]),
    )?);

    Ok(ds)
}

// ---------------------------------------------------------------------------
// S4-T2: Deformable Registration
// ---------------------------------------------------------------------------

/// B-spline control point grid for deformable registration.
#[derive(Debug, Clone, PartialEq)]
pub struct BSplineTransform {
    /// Grid spacing in millimeters (x, y, z).
    pub grid_spacing: (f64, f64, f64),
    /// Control points: 4D array [nz][ny][nx][3] where the 3 values are (dx, dy, dz) displacements.
    pub control_points: Vec<Vec<Vec<[f64; 3]>>>,
    /// Grid dimensions (nx, ny, nz) in control points.
    pub grid_dimensions: (usize, usize, usize),
}

impl BSplineTransform {
    /// Create a new B-spline transform with zero-initialized control points.
    pub fn new(
        grid_dimensions: (usize, usize, usize),
        grid_spacing: (f64, f64, f64),
    ) -> Result<Self> {
        if grid_dimensions.0 == 0 || grid_dimensions.1 == 0 || grid_dimensions.2 == 0 {
            return Err(registration_error(
                "B-spline grid dimensions must be non-zero",
            ));
        }
        if !grid_spacing.0.is_finite() || !grid_spacing.1.is_finite() || !grid_spacing.2.is_finite()
        {
            return Err(registration_error("B-spline grid spacing must be finite"));
        }
        if grid_spacing.0 <= 0.0 || grid_spacing.1 <= 0.0 || grid_spacing.2 <= 0.0 {
            return Err(registration_error("B-spline grid spacing must be positive"));
        }

        let (nx, ny, nz) = grid_dimensions;
        let control_points = vec![vec![vec![[0.0f64; 3]; nx]; ny]; nz];

        Ok(Self {
            grid_spacing,
            control_points,
            grid_dimensions,
        })
    }

    /// Get control point displacement at grid position (ix, iy, iz).
    pub fn control_point(&self, ix: usize, iy: usize, iz: usize) -> Option<[f64; 3]> {
        self.control_points
            .get(iz)
            .and_then(|z_slice| z_slice.get(iy))
            .and_then(|y_row| y_row.get(ix))
            .copied()
    }

    /// Set control point displacement at grid position (ix, iy, iz).
    pub fn set_control_point(
        &mut self,
        ix: usize,
        iy: usize,
        iz: usize,
        displacement: [f64; 3],
    ) -> bool {
        if let Some(z_slice) = self.control_points.get_mut(iz) {
            if let Some(y_row) = z_slice.get_mut(iy) {
                if let Some(slot) = y_row.get_mut(ix) {
                    *slot = displacement;
                    return true;
                }
            }
        }
        false
    }

    /// Interpolate displacement at a continuous position using B-spline basis (simplified: linear).
    pub fn interpolate_displacement(&self, x: f64, y: f64, z: f64) -> [f64; 3] {
        let (nx, ny, nz) = self.grid_dimensions;
        let sx = x / self.grid_spacing.0;
        let sy = y / self.grid_spacing.1;
        let sz = z / self.grid_spacing.2;

        // Trilinear interpolation between control points
        let ix0 = (sx.floor() as usize).min(nx - 1);
        let iy0 = (sy.floor() as usize).min(ny - 1);
        let iz0 = (sz.floor() as usize).min(nz - 1);
        let ix1 = (ix0 + 1).min(nx - 1);
        let iy1 = (iy0 + 1).min(ny - 1);
        let iz1 = (iz0 + 1).min(nz - 1);

        let fx = (sx - ix0 as f64).clamp(0.0, 1.0);
        let fy = (sy - iy0 as f64).clamp(0.0, 1.0);
        let fz = (sz - iz0 as f64).clamp(0.0, 1.0);

        let c000 = self.control_point(ix0, iy0, iz0).unwrap_or([0.0; 3]);
        let c100 = self.control_point(ix1, iy0, iz0).unwrap_or([0.0; 3]);
        let c010 = self.control_point(ix0, iy1, iz0).unwrap_or([0.0; 3]);
        let c110 = self.control_point(ix1, iy1, iz0).unwrap_or([0.0; 3]);
        let c001 = self.control_point(ix0, iy0, iz1).unwrap_or([0.0; 3]);
        let c101 = self.control_point(ix1, iy0, iz1).unwrap_or([0.0; 3]);
        let c011 = self.control_point(ix0, iy1, iz1).unwrap_or([0.0; 3]);
        let c111 = self.control_point(ix1, iy1, iz1).unwrap_or([0.0; 3]);

        let mut result = [0.0; 3];
        for i in 0..3 {
            let v000 = c000[i];
            let v100 = c100[i];
            let v010 = c010[i];
            let v110 = c110[i];
            let v001 = c001[i];
            let v101 = c101[i];
            let v011 = c011[i];
            let v111 = c111[i];

            result[i] = v000 * (1.0 - fx) * (1.0 - fy) * (1.0 - fz)
                + v100 * fx * (1.0 - fy) * (1.0 - fz)
                + v010 * (1.0 - fx) * fy * (1.0 - fz)
                + v110 * fx * fy * (1.0 - fz)
                + v001 * (1.0 - fx) * (1.0 - fy) * fz
                + v101 * fx * (1.0 - fy) * fz
                + v011 * (1.0 - fx) * fy * fz
                + v111 * fx * fy * fz;
        }
        result
    }
}

/// Configuration for deformable B-spline registration.
#[derive(Debug, Clone, PartialEq)]
pub struct DeformableRegistrationConfig {
    /// Control point grid spacing in millimeters.
    pub grid_spacing: (f64, f64, f64),
    /// Maximum number of optimization iterations.
    pub max_iterations: u32,
    /// Similarity metric convergence threshold.
    pub similarity_threshold: f64,
}

impl Default for DeformableRegistrationConfig {
    fn default() -> Self {
        Self {
            grid_spacing: (50.0, 50.0, 50.0),
            max_iterations: 50,
            similarity_threshold: 1e-5,
        }
    }
}

/// Deformation Vector Field (DVF) — one (dx, dy, dz) displacement per voxel.
#[derive(Debug, Clone, PartialEq)]
pub struct DeformationVectorField {
    /// Volume width (x dimension).
    pub width: usize,
    /// Volume height (y dimension).
    pub height: usize,
    /// Volume depth (z dimension).
    pub depth: usize,
    /// Displacement vectors as flat array in z-y-x order: [dx, dy, dz] per voxel.
    pub displacements: Vec<[f64; 3]>,
}

impl DeformationVectorField {
    /// Create a zero-initialized (identity) DVF.
    pub fn identity(width: usize, height: usize, depth: usize) -> Result<Self> {
        if width == 0 || height == 0 || depth == 0 {
            return Err(registration_error("DVF dimensions must be non-zero"));
        }
        let count = width * height * depth;
        Ok(Self {
            width,
            height,
            depth,
            displacements: vec![[0.0; 3]; count],
        })
    }

    /// Create a DVF with a known uniform translation.
    pub fn uniform_translation(
        width: usize,
        height: usize,
        depth: usize,
        dx: f64,
        dy: f64,
        dz: f64,
    ) -> Result<Self> {
        let mut dvf = Self::identity(width, height, depth)?;
        for d in &mut dvf.displacements {
            *d = [dx, dy, dz];
        }
        Ok(dvf)
    }

    /// Get displacement at voxel (x, y, z).
    pub fn displacement(&self, x: usize, y: usize, z: usize) -> Option<[f64; 3]> {
        if x >= self.width || y >= self.height || z >= self.depth {
            return None;
        }
        let idx = z * (self.width * self.height) + y * self.width + x;
        self.displacements.get(idx).copied()
    }

    /// Set displacement at voxel (x, y, z).
    pub fn set_displacement(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        displacement: [f64; 3],
    ) -> bool {
        if x >= self.width || y >= self.height || z >= self.depth {
            return false;
        }
        let idx = z * (self.width * self.height) + y * self.width + x;
        if let Some(slot) = self.displacements.get_mut(idx) {
            *slot = displacement;
            return true;
        }
        false
    }

    /// Apply this DVF to resample a moving image onto the fixed image grid.
    pub fn apply_to_volume(&self, volume: &VolumeGrid) -> Result<VolumeGrid> {
        if volume.width != self.width || volume.height != self.height || volume.depth != self.depth
        {
            return Err(registration_error(
                "DVF and volume dimensions must match for resampling",
            ));
        }

        let mut result =
            VolumeGrid::new(volume.width, volume.height, volume.depth, volume.spacing)?;

        for z in 0..volume.depth {
            for y in 0..volume.height {
                for x in 0..volume.width {
                    let idx = z * (volume.width * volume.height) + y * volume.width + x;
                    let [dx, dy, dz] = self.displacements[idx];

                    // Apply displacement to find source voxel
                    let sx = x as f64 + dx / volume.spacing.0;
                    let sy = y as f64 + dy / volume.spacing.1;
                    let sz = z as f64 + dz / volume.spacing.2;

                    let svx = sx.round() as isize;
                    let svy = sy.round() as isize;
                    let svz = sz.round() as isize;

                    // Nearest-neighbor interpolation with bounds check
                    if svx >= 0
                        && (svx as usize) < volume.width
                        && svy >= 0
                        && (svy as usize) < volume.height
                        && svz >= 0
                        && (svz as usize) < volume.depth
                    {
                        if let Some(val) = volume.voxel(svx as usize, svy as usize, svz as usize) {
                            result.set_voxel(x, y, z, val);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// Compose this DVF with another DVF (apply self first, then other).
    ///
    /// The composed DVF maps from the original source space through self to
    /// the intermediate space, then through other to the target space.
    pub fn compose(&self, other: &DeformationVectorField) -> Result<DeformationVectorField> {
        if self.width != other.width || self.height != other.height || self.depth != other.depth {
            return Err(registration_error(
                "DVF dimensions must match for composition",
            ));
        }

        let mut result = DeformationVectorField::identity(self.width, self.height, self.depth)?;

        for z in 0..self.depth {
            for y in 0..self.height {
                for x in 0..self.width {
                    let [dx1, dy1, dz1] = self.displacement(x, y, z).unwrap_or([0.0; 3]);

                    // Find where self maps this voxel to
                    let mid_x = x as f64 + dx1;
                    let mid_y = y as f64 + dy1;
                    let mid_z = z as f64 + dz1;

                    // Look up other's displacement at the intermediate position
                    let oix = mid_x.round() as isize;
                    let oiy = mid_y.round() as isize;
                    let oiz = mid_z.round() as isize;

                    let [dx2, dy2, dz2] = if oix >= 0
                        && (oix as usize) < other.width
                        && oiy >= 0
                        && (oiy as usize) < other.height
                        && oiz >= 0
                        && (oiz as usize) < other.depth
                    {
                        other
                            .displacement(oix as usize, oiy as usize, oiz as usize)
                            .unwrap_or([0.0; 3])
                    } else {
                        [0.0; 3]
                    };

                    result.set_displacement(x, y, z, [dx1 + dx2, dy1 + dy2, dz1 + dz2]);
                }
            }
        }

        Ok(result)
    }

    /// Convert a B-spline transform to a DVF by interpolating control points.
    pub fn from_bspline(bspline: &BSplineTransform, volume: &VolumeGrid) -> Result<Self> {
        let mut dvf = DeformationVectorField::identity(volume.width, volume.height, volume.depth)?;

        for z in 0..volume.depth {
            for y in 0..volume.height {
                for x in 0..volume.width {
                    let px = x as f64 * volume.spacing.0;
                    let py = y as f64 * volume.spacing.1;
                    let pz = z as f64 * volume.spacing.2;

                    let displacement = bspline.interpolate_displacement(px, py, pz);
                    dvf.set_displacement(x, y, z, displacement);
                }
            }
        }

        Ok(dvf)
    }
}

/// Perform deformable B-spline registration between a fixed and moving volume.
///
/// Uses B-spline control point optimization with multi-resolution approach.
/// For synthetic data, uses simplified gradient-descent optimization.
pub fn deformable_register(
    fixed: &VolumeGrid,
    moving: &VolumeGrid,
    config: &DeformableRegistrationConfig,
) -> Result<(BSplineTransform, DeformationVectorField, f64)> {
    fixed.validate()?;
    moving.validate()?;

    if fixed.is_empty() || moving.is_empty() {
        return Err(registration_error(
            "cannot register empty volumes for deformable registration",
        ));
    }

    if fixed.width != moving.width || fixed.height != moving.height || fixed.depth != moving.depth {
        return Err(registration_error(
            "fixed and moving volumes must have the same dimensions for deformable registration",
        ));
    }

    // Determine control point grid dimensions based on volume and spacing
    let nx =
        ((fixed.width as f64 * fixed.spacing.0 / config.grid_spacing.0).ceil() as usize).max(4);
    let ny =
        ((fixed.height as f64 * fixed.spacing.1 / config.grid_spacing.1).ceil() as usize).max(4);
    let nz =
        ((fixed.depth as f64 * fixed.spacing.2 / config.grid_spacing.2).ceil() as usize).max(4);

    let mut bspline = BSplineTransform::new((nx, ny, nz), config.grid_spacing)?;

    // Simplified optimization: perturb each control point to minimize MSE
    let mut best_metric = compute_metric(fixed, moving, RegistrationMetric::MeanSquares)?;
    let step = 1.0; // mm step for control point displacement

    for _iteration in 0..config.max_iterations {
        let mut improved = false;

        for iz in 0..nz {
            for iy in 0..ny {
                for ix in 0..nx {
                    let current = bspline.control_point(ix, iy, iz).unwrap_or([0.0; 3]);

                    // Try perturbations in each displacement direction
                    for dim in 0..3 {
                        for &delta in &[step, -step] {
                            let mut trial = current;
                            trial[dim] += delta;
                            bspline.set_control_point(ix, iy, iz, trial);

                            let dvf = DeformationVectorField::from_bspline(&bspline, moving)?;
                            let resampled = dvf.apply_to_volume(moving)?;
                            let trial_metric =
                                compute_metric(fixed, &resampled, RegistrationMetric::MeanSquares)?;

                            if trial_metric < best_metric {
                                best_metric = trial_metric;
                                improved = true;
                            } else {
                                // Revert
                                bspline.set_control_point(ix, iy, iz, current);
                            }
                        }
                    }
                }
            }
        }

        if !improved || best_metric < config.similarity_threshold {
            break;
        }
    }

    let dvf = DeformationVectorField::from_bspline(&bspline, moving)?;

    Ok((bspline, dvf, best_metric))
}

/// Encode a deformable registration result as a DICOM Deformable Spatial Registration IOD Dataset.
pub fn encode_deformable_registration_iod(
    dvf: &DeformationVectorField,
    bspline: &BSplineTransform,
    study_uid: &str,
    series_uid: &str,
) -> Result<Dataset> {
    let mut ds = Dataset::new();

    // SOP Class UID for Deformable Spatial Registration Storage (1.2.840.10008.5.1.4.1.1.66.3)
    ds.insert(Element::new(
        Tag(0x0008, 0x0016),
        Vr::Ui,
        Value::Uid("1.2.840.10008.5.1.4.1.1.66.3".to_string()),
    )?);

    // SOP Instance UID
    let sop_uid = format!(
        "1.2.840.113619.6.4.{}.{}.{}",
        study_uid.len(),
        series_uid.len(),
        dvf.width
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

    // Deformable Registration Sequence
    let mut reg_item = Dataset::new();

    // Deformable Registration Type Code Sequence
    let mut type_item = Dataset::new();
    type_item.insert(
        Element::new(
            Tag(0x0008, 0x0100),
            Vr::Sh,
            Value::Str("DEFORMABLE".to_string()),
        )
        .unwrap(),
    );
    type_item.insert(
        Element::new(
            Tag(0x0008, 0x0102),
            Vr::Sh,
            Value::Str("99DICOM_REG".to_string()),
        )
        .unwrap(),
    );
    type_item.insert(
        Element::new(
            Tag(0x0008, 0x0104),
            Vr::Lo,
            Value::Str("Deformable".to_string()),
        )
        .unwrap(),
    );

    reg_item.insert(
        Element::new(
            Tag(0x0070, 0x030D),
            Vr::Sq,
            Value::Sequence(vec![type_item]),
        )
        .unwrap(),
    );

    // Grid dimensions
    let (nx, ny, nz) = bspline.grid_dimensions;
    reg_item.insert(Element::new(Tag(0x0070, 0x0305), Vr::Us, Value::I32(nx as i32)).unwrap());
    reg_item.insert(Element::new(Tag(0x0070, 0x0306), Vr::Us, Value::I32(ny as i32)).unwrap());
    reg_item.insert(Element::new(Tag(0x0070, 0x0307), Vr::Us, Value::I32(nz as i32)).unwrap());

    // Grid spacing
    reg_item.insert(
        Element::new(
            Tag(0x0070, 0x0308),
            Vr::Ds,
            Value::Str(format!(
                "{:.6}\\\\{:.6}\\\\{:.6}",
                bspline.grid_spacing.0, bspline.grid_spacing.1, bspline.grid_spacing.2
            )),
        )
        .unwrap(),
    );

    // Vector Grid Data (DVF displacements as OD)
    let mut dvf_bytes = Vec::new();
    for d in &dvf.displacements {
        let dx_bytes = d[0].to_le_bytes();
        let dy_bytes = d[1].to_le_bytes();
        let dz_bytes = d[2].to_le_bytes();
        dvf_bytes.extend_from_slice(&dx_bytes);
        dvf_bytes.extend_from_slice(&dy_bytes);
        dvf_bytes.extend_from_slice(&dz_bytes);
    }

    reg_item.insert(Element::new(Tag(0x0070, 0x0309), Vr::Of, Value::Bytes(dvf_bytes)).unwrap());

    ds.insert(Element::new(
        Tag(0x0070, 0x0308),
        Vr::Sq,
        Value::Sequence(vec![reg_item]),
    )?);

    Ok(ds)
}

// ---------------------------------------------------------------------------
// Shared: Audit and Error helpers
// ---------------------------------------------------------------------------

/// Audit callback for registration operations.
pub type AuditCallback = Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>;

fn registration_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::InvalidGeometry {
            detail: detail.into(),
        },
        "registration error",
    )
    .into()
}

fn record_audit(
    audit: &Option<AuditCallback>,
    operation: &'static str,
    subject_id: Option<&str>,
) -> Result<()> {
    let Some(callback) = audit else {
        return Ok(());
    };
    let mut fields = vec![AuditField {
        key: "operation",
        value: AuditValue::Plain(operation.to_string()),
    }];
    if let Some(id) = subject_id {
        fields.push(AuditField {
            key: "subject_id",
            value: AuditValue::Sensitive(id.to_string()),
        });
    }
    callback(AuditEvent {
        kind: AuditEventKind::ServiceEvent,
        fields,
    })
}

// ---------------------------------------------------------------------------
// Tests: S4-T1 Rigid Registration (minimum 15)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Tests: S4-T2 Deformable Registration (minimum 12)
// ---------------------------------------------------------------------------
