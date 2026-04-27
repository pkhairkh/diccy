//! Deterministic multi-planar reconstruction (MPR) primitives.

use crate::volume::VolumeGrid;
use std::fmt;

/// Resampling kernel for MPR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResampleKernel {
    /// Nearest-neighbor deterministic sampling.
    Nearest,
    /// Deterministic linear interpolation.
    Linear,
}

/// Deterministic slab composition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlabMode {
    /// Arithmetic mean across slab samples.
    Average,
    /// Maximum intensity projection across slab samples.
    Max,
}

/// MPR slicing plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MprPlane {
    /// Axial plane (`z` fixed).
    Axial,
    /// Coronal plane (`y` fixed).
    Coronal,
    /// Sagittal plane (`x` fixed).
    Sagittal,
    /// Arbitrary plane.
    Arbitrary {
        /// Plane origin in voxel space.
        origin: [f64; 3],
        /// Plane x-axis direction in voxel units.
        axis_u: [f64; 3],
        /// Plane y-axis direction in voxel units.
        axis_v: [f64; 3],
    },
}

/// Patient-space MPR request plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatientMprPlane {
    /// Axial plane offset in micrometers along volume `z` axis.
    Axial {
        /// Axial offset in micrometers.
        offset_um: i64,
    },
    /// Coronal plane offset in micrometers along volume `y` axis.
    Coronal {
        /// Coronal offset in micrometers.
        offset_um: i64,
    },
    /// Sagittal plane offset in micrometers along volume `x` axis.
    Sagittal {
        /// Sagittal offset in micrometers.
        offset_um: i64,
    },
    /// Arbitrary patient-space plane geometry in micrometers.
    Arbitrary {
        /// Plane origin in patient space.
        origin_um: [i64; 3],
        /// Plane x-axis direction in patient-space micrometers.
        axis_u_um: [i64; 3],
        /// Plane y-axis direction in patient-space micrometers.
        axis_v_um: [i64; 3],
    },
}

/// A deterministic patient-space MPR request.
#[derive(Debug, Clone, PartialEq)]
pub struct PatientMprRequest {
    /// Plane selection in patient-space coordinates.
    pub plane: PatientMprPlane,
    /// Output width.
    pub output_width: u32,
    /// Output height.
    pub output_height: u32,
    /// Sampling kernel.
    pub kernel: ResampleKernel,
    /// Out-of-bounds value.
    pub background: i32,
    /// Optional VOI window center/width.
    pub voi_window: Option<(f64, f64)>,
    /// Deterministic slab thickness in samples.
    pub slab_thickness: u32,
    /// Deterministic slab composition mode.
    pub slab_mode: SlabMode,
}

impl Default for PatientMprRequest {
    fn default() -> Self {
        Self {
            plane: PatientMprPlane::Axial { offset_um: 0 },
            output_width: 0,
            output_height: 0,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 1,
            slab_mode: SlabMode::Average,
        }
    }
}

/// MPR resource and output limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MprLimits {
    /// Maximum voxels in a source volume.
    pub max_source_voxels: u64,
    /// Maximum pixels in one output frame.
    pub max_output_pixels: u64,
    /// Maximum bytes in one output frame.
    pub max_output_bytes: u64,
}

impl Default for MprLimits {
    fn default() -> Self {
        Self {
            max_source_voxels: 512 * 512 * 2048,
            max_output_pixels: 4096 * 4096,
            max_output_bytes: 4096 * 4096 * 4,
        }
    }
}

/// A deterministic MPR request.
#[derive(Debug, Clone, PartialEq)]
pub struct MprRequest {
    /// Plane selection.
    pub plane: MprPlane,
    /// Plane index (used for axial/coronal/sagittal).
    pub index: i32,
    /// Output width.
    pub output_width: u32,
    /// Output height.
    pub output_height: u32,
    /// Sampling kernel.
    pub kernel: ResampleKernel,
    /// Out-of-bounds value.
    pub background: i32,
    /// Optional VOI window center/width.
    pub voi_window: Option<(f64, f64)>,
    /// Deterministic slab thickness in samples.
    pub slab_thickness: u32,
    /// Deterministic slab composition mode.
    pub slab_mode: SlabMode,
}

impl Default for MprRequest {
    fn default() -> Self {
        Self {
            plane: MprPlane::Axial,
            index: 0,
            output_width: 0,
            output_height: 0,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: 1,
            slab_mode: SlabMode::Average,
        }
    }
}

/// A deterministic MPR output frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MprFrame {
    /// Output width.
    pub width: u32,
    /// Output height.
    pub height: u32,
    /// Quantized 8-bit grayscale output.
    pub pixels: Vec<u8>,
    /// Deterministic cache key fragment for this request.
    pub cache_key: String,
}

/// MPR failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MprError {
    /// Request shape was invalid.
    InvalidRequest(&'static str),
    /// Source volume exceeds configured limits.
    SourceLimitExceeded {
        /// Observed value.
        observed: u64,
        /// Allowed value.
        allowed: u64,
    },
    /// Output exceeds configured limits.
    OutputLimitExceeded {
        /// Observed value.
        observed: u64,
        /// Allowed value.
        allowed: u64,
    },
}

impl MprError {
    /// Stable error code for MPR API failures.
    pub fn code(&self) -> &'static str {
        match self {
            MprError::InvalidRequest(_) => "DVF.MPR.INVALID_REQUEST",
            MprError::SourceLimitExceeded { .. } => "DVF.MPR.SOURCE_LIMIT_EXCEEDED",
            MprError::OutputLimitExceeded { .. } => "DVF.MPR.OUTPUT_LIMIT_EXCEEDED",
        }
    }
}

impl fmt::Display for MprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MprError::InvalidRequest(detail) => write!(f, "invalid mpr request: {detail}"),
            MprError::SourceLimitExceeded { observed, allowed } => {
                write!(f, "source voxel count {observed} exceeds limit {allowed}")
            }
            MprError::OutputLimitExceeded { observed, allowed } => {
                write!(f, "output size {observed} exceeds limit {allowed}")
            }
        }
    }
}

impl std::error::Error for MprError {}

/// Quantize a plane parameter for stable cache keys.
pub fn quantize_plane_parameter(value: f64) -> i64 {
    (value * 1_000_000.0).round() as i64
}

/// Convert a patient-space request into a voxel-space request.
pub fn patient_request_to_voxel_request(
    volume: &VolumeGrid,
    request: &PatientMprRequest,
) -> Option<MprRequest> {
    let spacing = volume.spacing_um();
    let to_index = |offset_um: i64, spacing_um: u64| -> Option<i32> {
        if spacing_um == 0 {
            return None;
        }
        Some((offset_um as f64 / spacing_um as f64).round() as i32)
    };

    let plane = match request.plane {
        PatientMprPlane::Axial { offset_um: _ } => MprPlane::Axial,
        PatientMprPlane::Coronal { offset_um: _ } => MprPlane::Coronal,
        PatientMprPlane::Sagittal { offset_um: _ } => MprPlane::Sagittal,
        PatientMprPlane::Arbitrary {
            origin_um,
            axis_u_um,
            axis_v_um,
        } => {
            let origin = volume.patient_to_voxel_f64([
                origin_um[0] as f64,
                origin_um[1] as f64,
                origin_um[2] as f64,
            ])?;
            let origin_plus_u = volume.patient_to_voxel_f64([
                origin_um[0].checked_add(axis_u_um[0])? as f64,
                origin_um[1].checked_add(axis_u_um[1])? as f64,
                origin_um[2].checked_add(axis_u_um[2])? as f64,
            ])?;
            let origin_plus_v = volume.patient_to_voxel_f64([
                origin_um[0].checked_add(axis_v_um[0])? as f64,
                origin_um[1].checked_add(axis_v_um[1])? as f64,
                origin_um[2].checked_add(axis_v_um[2])? as f64,
            ])?;
            MprPlane::Arbitrary {
                origin,
                axis_u: [
                    origin_plus_u[0] - origin[0],
                    origin_plus_u[1] - origin[1],
                    origin_plus_u[2] - origin[2],
                ],
                axis_v: [
                    origin_plus_v[0] - origin[0],
                    origin_plus_v[1] - origin[1],
                    origin_plus_v[2] - origin[2],
                ],
            }
        }
    };

    let index = match request.plane {
        PatientMprPlane::Axial { offset_um } => to_index(offset_um, spacing[2])?,
        PatientMprPlane::Coronal { offset_um } => to_index(offset_um, spacing[1])?,
        PatientMprPlane::Sagittal { offset_um } => to_index(offset_um, spacing[0])?,
        PatientMprPlane::Arbitrary { .. } => 0,
    };

    Some(MprRequest {
        plane,
        index,
        output_width: request.output_width,
        output_height: request.output_height,
        kernel: request.kernel,
        background: request.background,
        voi_window: request.voi_window,
        slab_thickness: request.slab_thickness,
        slab_mode: request.slab_mode,
    })
}

/// Generate an MPR frame from a patient-space request.
pub fn reslice_volume_patient(
    volume: &VolumeGrid,
    request: &PatientMprRequest,
    limits: MprLimits,
) -> Result<MprFrame, MprError> {
    let Some(voxel_request) = patient_request_to_voxel_request(volume, request) else {
        return Err(MprError::InvalidRequest(
            "patient geometry is unavailable or invalid for this request",
        ));
    };
    reslice_volume(volume, &voxel_request, limits)
}

/// Generate an MPR frame from a volume.
pub fn reslice_volume(
    volume: &VolumeGrid,
    request: &MprRequest,
    limits: MprLimits,
) -> Result<MprFrame, MprError> {
    if request.output_width == 0 || request.output_height == 0 {
        return Err(MprError::InvalidRequest(
            "output dimensions must be non-zero",
        ));
    }
    if request.slab_thickness == 0 {
        return Err(MprError::InvalidRequest("slab_thickness must be >= 1"));
    }
    let dims = volume.dimensions();
    let source_voxels = dims[0] as u64 * dims[1] as u64 * dims[2] as u64;
    if source_voxels > limits.max_source_voxels {
        return Err(MprError::SourceLimitExceeded {
            observed: source_voxels,
            allowed: limits.max_source_voxels,
        });
    }

    let output_pixels = request.output_width as u64 * request.output_height as u64;
    if output_pixels > limits.max_output_pixels {
        return Err(MprError::OutputLimitExceeded {
            observed: output_pixels,
            allowed: limits.max_output_pixels,
        });
    }
    if output_pixels.saturating_mul(4) > limits.max_output_bytes {
        return Err(MprError::OutputLimitExceeded {
            observed: output_pixels.saturating_mul(4),
            allowed: limits.max_output_bytes,
        });
    }

    let mut out = vec![0u8; output_pixels as usize];
    for y in 0..request.output_height {
        for x in 0..request.output_width {
            let sample = match request.plane {
                MprPlane::Axial => sample_axial(volume, request, x, y),
                MprPlane::Coronal => sample_coronal(volume, request, x, y),
                MprPlane::Sagittal => sample_sagittal(volume, request, x, y),
                MprPlane::Arbitrary {
                    origin,
                    axis_u,
                    axis_v,
                } => sample_arbitrary(volume, request, x, y, origin, axis_u, axis_v),
            };
            let mapped = apply_voi(sample, request.voi_window);
            out[(y as usize * request.output_width as usize) + x as usize] = mapped;
        }
    }
    Ok(MprFrame {
        width: request.output_width,
        height: request.output_height,
        pixels: out,
        cache_key: request_cache_key(request),
    })
}

fn request_cache_key(request: &MprRequest) -> String {
    match request.plane {
        MprPlane::Axial => format!(
            "axial:{}:{}x{}:{:?}:slab{}:{:?}",
            request.index,
            request.output_width,
            request.output_height,
            request.kernel,
            request.slab_thickness,
            request.slab_mode,
        ),
        MprPlane::Coronal => format!(
            "coronal:{}:{}x{}:{:?}:slab{}:{:?}",
            request.index,
            request.output_width,
            request.output_height,
            request.kernel,
            request.slab_thickness,
            request.slab_mode,
        ),
        MprPlane::Sagittal => format!(
            "sagittal:{}:{}x{}:{:?}:slab{}:{:?}",
            request.index,
            request.output_width,
            request.output_height,
            request.kernel,
            request.slab_thickness,
            request.slab_mode,
        ),
        MprPlane::Arbitrary {
            origin,
            axis_u,
            axis_v,
        } => format!(
            "arb:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}x{}:{:?}:slab{}:{:?}",
            quantize_plane_parameter(origin[0]),
            quantize_plane_parameter(origin[1]),
            quantize_plane_parameter(origin[2]),
            quantize_plane_parameter(axis_u[0]),
            quantize_plane_parameter(axis_u[1]),
            quantize_plane_parameter(axis_u[2]),
            quantize_plane_parameter(axis_v[0]),
            quantize_plane_parameter(axis_v[1]),
            quantize_plane_parameter(axis_v[2]),
            request.output_width,
            request.output_height,
            request.kernel,
            request.slab_thickness,
            request.slab_mode,
        ),
    }
}

fn slab_indices(center: i32, thickness: u32, max_extent: usize) -> Vec<usize> {
    let mut out = Vec::with_capacity(thickness.max(1) as usize);
    let half = (thickness as i32 - 1) / 2;
    let start = center - half;
    for step in 0..thickness as i32 {
        let idx = (start + step).clamp(0, max_extent.saturating_sub(1) as i32) as usize;
        out.push(idx);
    }
    out
}

fn aggregate_slab(samples: &[i32], mode: SlabMode) -> i32 {
    if samples.is_empty() {
        return 0;
    }
    match mode {
        SlabMode::Average => {
            let sum: i64 = samples.iter().map(|value| *value as i64).sum();
            (sum as f64 / samples.len() as f64).round() as i32
        }
        SlabMode::Max => *samples.iter().max().unwrap_or(&samples[0]),
    }
}

fn sample_axial(volume: &VolumeGrid, request: &MprRequest, x: u32, y: u32) -> i32 {
    let dims = volume.dimensions();
    let slab = slab_indices(request.index, request.slab_thickness, dims[2]);
    let mut samples = Vec::with_capacity(slab.len());
    for z in slab {
        samples.push(sample_mapped_xy(
            volume,
            request,
            x,
            y,
            dims[0],
            dims[1],
            |ix, iy| volume.voxel(ix, iy, z),
        ));
    }
    aggregate_slab(&samples, request.slab_mode)
}

fn sample_coronal(volume: &VolumeGrid, request: &MprRequest, x: u32, y: u32) -> i32 {
    let dims = volume.dimensions();
    let slab = slab_indices(request.index, request.slab_thickness, dims[1]);
    let mut samples = Vec::with_capacity(slab.len());
    for fixed_y in slab {
        samples.push(sample_mapped_xy(
            volume,
            request,
            x,
            y,
            dims[0],
            dims[2],
            |ix, iz| volume.voxel(ix, fixed_y, iz),
        ));
    }
    aggregate_slab(&samples, request.slab_mode)
}

fn sample_sagittal(volume: &VolumeGrid, request: &MprRequest, x: u32, y: u32) -> i32 {
    let dims = volume.dimensions();
    let slab = slab_indices(request.index, request.slab_thickness, dims[0]);
    let mut samples = Vec::with_capacity(slab.len());
    for fixed_x in slab {
        samples.push(sample_mapped_xy(
            volume,
            request,
            x,
            y,
            dims[1],
            dims[2],
            |iy, iz| volume.voxel(fixed_x, iy, iz),
        ));
    }
    aggregate_slab(&samples, request.slab_mode)
}

fn sample_arbitrary(
    volume: &VolumeGrid,
    request: &MprRequest,
    x: u32,
    y: u32,
    origin: [f64; 3],
    axis_u: [f64; 3],
    axis_v: [f64; 3],
) -> i32 {
    let u = (x as f64 + 0.5) / request.output_width as f64;
    let v = (y as f64 + 0.5) / request.output_height as f64;
    let base = [
        origin[0] + axis_u[0] * u + axis_v[0] * v,
        origin[1] + axis_u[1] * u + axis_v[1] * v,
        origin[2] + axis_u[2] * u + axis_v[2] * v,
    ];

    let mut normal = [
        axis_u[1] * axis_v[2] - axis_u[2] * axis_v[1],
        axis_u[2] * axis_v[0] - axis_u[0] * axis_v[2],
        axis_u[0] * axis_v[1] - axis_u[1] * axis_v[0],
    ];
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length.is_finite() && length > f64::EPSILON {
        normal = [normal[0] / length, normal[1] / length, normal[2] / length];
    } else {
        normal = [0.0, 0.0, 1.0];
    }

    let mut samples = Vec::with_capacity(request.slab_thickness as usize);
    let half = (request.slab_thickness as i32 - 1) / 2;
    for step in 0..request.slab_thickness as i32 {
        let offset = (step - half) as f64;
        samples.push(sample_by_kernel(
            volume,
            base[0] + normal[0] * offset,
            base[1] + normal[1] * offset,
            base[2] + normal[2] * offset,
            request.kernel,
            request.background,
        ));
    }
    aggregate_slab(&samples, request.slab_mode)
}

fn sample_mapped_xy<F>(
    _volume: &VolumeGrid,
    request: &MprRequest,
    x: u32,
    y: u32,
    source_w: usize,
    source_h: usize,
    f: F,
) -> i32
where
    F: Fn(usize, usize) -> Option<i32>,
{
    let sx = map_axis(x, request.output_width, source_w);
    let sy = map_axis(y, request.output_height, source_h);
    match request.kernel {
        ResampleKernel::Nearest => {
            f(sx.round() as usize, sy.round() as usize).unwrap_or(request.background)
        }
        ResampleKernel::Linear => {
            let x0 = sx.floor();
            let y0 = sy.floor();
            let x1 = x0 + 1.0;
            let y1 = y0 + 1.0;
            let tx = sx - x0;
            let ty = sy - y0;
            let p00 = f(x0 as usize, y0 as usize).unwrap_or(request.background) as f64;
            let p10 = f(x1 as usize, y0 as usize).unwrap_or(request.background) as f64;
            let p01 = f(x0 as usize, y1 as usize).unwrap_or(request.background) as f64;
            let p11 = f(x1 as usize, y1 as usize).unwrap_or(request.background) as f64;
            let top = p00 * (1.0 - tx) + p10 * tx;
            let bottom = p01 * (1.0 - tx) + p11 * tx;
            (top * (1.0 - ty) + bottom * ty).round() as i32
        }
    }
}

fn map_axis(coord: u32, out_extent: u32, source_extent: usize) -> f64 {
    if source_extent <= 1 {
        return 0.0;
    }
    ((coord as f64 + 0.5) / out_extent as f64) * (source_extent as f64 - 1.0)
}

fn sample_by_kernel(
    volume: &VolumeGrid,
    x: f64,
    y: f64,
    z: f64,
    kernel: ResampleKernel,
    background: i32,
) -> i32 {
    match kernel {
        ResampleKernel::Nearest => volume
            .voxel(x.round() as usize, y.round() as usize, z.round() as usize)
            .unwrap_or(background),
        ResampleKernel::Linear => {
            let x0 = x.floor();
            let y0 = y.floor();
            let z0 = z.floor();
            let x1 = x0 + 1.0;
            let y1 = y0 + 1.0;
            let z1 = z0 + 1.0;
            let tx = x - x0;
            let ty = y - y0;
            let tz = z - z0;
            let v000 = volume
                .voxel(x0 as usize, y0 as usize, z0 as usize)
                .unwrap_or(background) as f64;
            let v100 = volume
                .voxel(x1 as usize, y0 as usize, z0 as usize)
                .unwrap_or(background) as f64;
            let v010 = volume
                .voxel(x0 as usize, y1 as usize, z0 as usize)
                .unwrap_or(background) as f64;
            let v110 = volume
                .voxel(x1 as usize, y1 as usize, z0 as usize)
                .unwrap_or(background) as f64;
            let v001 = volume
                .voxel(x0 as usize, y0 as usize, z1 as usize)
                .unwrap_or(background) as f64;
            let v101 = volume
                .voxel(x1 as usize, y0 as usize, z1 as usize)
                .unwrap_or(background) as f64;
            let v011 = volume
                .voxel(x0 as usize, y1 as usize, z1 as usize)
                .unwrap_or(background) as f64;
            let v111 = volume
                .voxel(x1 as usize, y1 as usize, z1 as usize)
                .unwrap_or(background) as f64;
            let c00 = v000 * (1.0 - tx) + v100 * tx;
            let c10 = v010 * (1.0 - tx) + v110 * tx;
            let c01 = v001 * (1.0 - tx) + v101 * tx;
            let c11 = v011 * (1.0 - tx) + v111 * tx;
            let c0 = c00 * (1.0 - ty) + c10 * ty;
            let c1 = c01 * (1.0 - ty) + c11 * ty;
            (c0 * (1.0 - tz) + c1 * tz).round() as i32
        }
    }
}

fn apply_voi(value: i32, voi_window: Option<(f64, f64)>) -> u8 {
    match voi_window {
        Some((center, width)) if width.is_finite() && width > 0.0 && center.is_finite() => {
            let min = center - width / 2.0;
            let max = center + width / 2.0;
            let clamped = (value as f64).clamp(min, max);
            let normalized = if (max - min).abs() < f64::EPSILON {
                0.0
            } else {
                (clamped - min) / (max - min)
            };
            (normalized * 255.0).round() as u8
        }
        _ => value.clamp(0, 255) as u8,
    }
}
