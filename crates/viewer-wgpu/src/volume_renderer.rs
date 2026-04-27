//! GPU volume rendering pipeline for 3D medical image visualization.
//!
//! Implements ray-marching volume rendering with three primary modes:
//! - **MPR**: Multi-planar reconstruction via 3D texture slice sampling
//! - **MIP/MinIP**: Maximum/Minimum Intensity Projection via ray marching
//! - **VR**: Volume rendering with transfer function, gradient shading, and clip planes
//!
//! All rendering modes use WGSL shaders and the `wgpu` 0.19 GPU runtime.
//! Volume data is uploaded as a 3D texture (`R32Sfloat`) from `VolumeGrid`.
//!
//! # Determinism
//!
//! All pixel outputs are deterministic for identical inputs per REQ-DET-001.
//! Error codes follow the `DVF.VOLUME.*` namespace.

use std::borrow::Cow;

use bytemuck::{Pod, Zeroable};
use dicom_core::{Error, ErrorKind, Result};
use viewer_core::{MipProjectionMode, VolumeGrid};
use wgpu::util::DeviceExt;

use crate::GpuBudget;

// ---------------------------------------------------------------------------
// WGSL Shader Sources
// ---------------------------------------------------------------------------

/// MPR slice rendering shader: samples a single slice from the 3D volume texture
/// with VOI windowing and optional slab composition.
const VOLUME_MPR_WGSL: &str = r#"
struct MprUniforms {
    // Slice plane parameters
    plane_normal: vec3<f32>,    // Normal of the slice plane
    plane_origin: vec3<f32>,   // Origin of the slice plane in texel coords [0,dim]
    axis_u: vec3<f32>,         // U axis of the slice plane
    axis_v: vec3<f32>,         // V axis of the slice plane
    // Volume dimensions (as float for arithmetic)
    volume_dims: vec3<u32>,
    // VOI windowing
    window_center: f32,
    window_width: f32,
    // Slab parameters
    slab_thickness: f32,       // In voxels
    slab_mode: u32,            // 0 = average, 1 = max
    // Padding
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<uniform> uniforms: MprUniforms;
@group(0) @binding(1) var volume_sampler: sampler;
@group(0) @binding(2) var volume_texture: texture_3d<f32>;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    let pos = positions[vertex_index];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

fn apply_voi(value: f32, center: f32, width: f32) -> f32 {
    let min_val = center - width * 0.5;
    let max_val = center + width * 0.5;
    let clamped = clamp(value, min_val, max_val);
    let range = max_val - min_val;
    if (abs(range) < 0.0001) {
        return 0.0;
    }
    return (clamped - min_val) / range;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec3<f32>(uniforms.volume_dims);
    // Map UV to texture coordinates
    let u_coord = in.uv.x;
    let v_coord = in.uv.y;

    let half_slab = (uniforms.slab_thickness - 1.0) * 0.5;
    let samples = max(1u, u32(uniforms.slab_thickness));

    var accumulated = 0.0;
    var max_val = -1e30;
    var min_val = 1e30;
    var count = 0u;

    for (var i = 0u; i < samples; i = i + 1u) {
        let offset = f32(i) - half_slab;
        let sample_pos = uniforms.plane_origin
            + uniforms.axis_u * u_coord * dims.x
            + uniforms.axis_v * v_coord * dims.y
            + uniforms.plane_normal * offset;

        // Normalize to [0, 1] for texture sampling
        let tex_coord = (sample_pos + vec3<f32>(0.5, 0.5, 0.5)) / dims;
        let value = textureSample(volume_texture, volume_sampler, tex_coord).r;

        accumulated = accumulated + value;
        if (value > max_val) { max_val = value; }
        if (value < min_val) { min_val = value; }
        count = count + 1u;
    }

    var result: f32;
    if (uniforms.slab_mode == 1u) {
        result = max_val; // MIP
    } else {
        result = accumulated / f32(count); // Average
    }

    // Apply VOI windowing
    let windowed = apply_voi(result, uniforms.window_center, uniforms.window_width);
    return vec4<f32>(windowed, windowed, windowed, 1.0);
}
"#;

/// MIP/MinIP ray-marching shader: casts rays through the volume and accumulates
/// the maximum or minimum intensity along each ray.
const VOLUME_MIP_WGSL: &str = r#"
struct MipUniforms {
    // Inverse view-projection matrix (column-major)
    inv_view_proj: mat4x4<f32>,
    // Camera position in volume space
    camera_pos: vec3<f32>,
    // Volume dimensions
    volume_dims: vec3<u32>,
    // VOI windowing
    window_center: f32,
    window_width: f32,
    // Ray marching parameters
    step_size: f32,           // Ray step size in texel units
    max_steps: u32,           // Maximum number of steps
    // MIP mode: 0 = max, 1 = min
    mip_mode: u32,
    // Slab parameters
    slab_thickness: f32,      // 0.0 = full volume, >0 = limited slab
    slab_center: f32,         // Center of slab along view direction in [0,1]
    // Padding
    _pad0: u32,
};

@group(0) @binding(0) var<uniform> uniforms: MipUniforms;
@group(0) @binding(1) var volume_sampler: sampler;
@group(0) @binding(2) var volume_texture: texture_3d<f32>;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    let pos = positions[vertex_index];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

fn apply_voi(value: f32, center: f32, width: f32) -> f32 {
    let min_val = center - width * 0.5;
    let max_val = center + width * 0.5;
    let clamped = clamp(value, min_val, max_val);
    let range = max_val - min_val;
    if (abs(range) < 0.0001) {
        return 0.0;
    }
    return (clamped - min_val) / range;
}

fn intersect_box(ray_origin: vec3<f32>, ray_dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> vec2<f32> {
    let inv_dir = 1.0 / ray_dir;
    let t_min_tmp = (box_min - ray_origin) * inv_dir;
    let t_max_tmp = (box_max - ray_origin) * inv_dir;
    let t_min = min(t_min_tmp, t_max_tmp);
    let t_max = max(t_min_tmp, t_max_tmp);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(t_near, t_far);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec3<f32>(uniforms.volume_dims);

    // Reconstruct ray direction from clip-space
    let clip_pos = vec4<f32>(in.uv * 2.0 - 1.0, 1.0, 1.0);
    let world_pos = uniforms.inv_view_proj * clip_pos;
    let ray_dir = normalize(world_pos.xyz / world_pos.w - uniforms.camera_pos);

    // Compute volume-space ray (volume occupies [0, dims])
    let vol_origin = uniforms.camera_pos;

    // Intersect with unit box [0, 1] in normalized volume coords
    let hit = intersect_box(vol_origin / dims, ray_dir, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    var t_near = hit.x;
    var t_far = hit.y;

    if (t_far < t_near || t_far < 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    t_near = max(t_near, 0.0);

    // Apply slab clipping
    if (uniforms.slab_thickness > 0.0) {
        let slab_half = uniforms.slab_thickness * 0.5;
        let slab_center_t = uniforms.slab_center;
        let slab_near = slab_center_t - slab_half;
        let slab_far = slab_center_t + slab_half;
        t_near = max(t_near, slab_near);
        t_far = min(t_far, slab_far);
        if (t_far < t_near) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }

    // Ray march
    var result: f32;
    if (uniforms.mip_mode == 0u) {
        result = -1e30; // Max IP
    } else {
        result = 1e30;  // Min IP
    }

    let step = uniforms.step_size / dims.x; // Normalize step to [0,1] volume coords
    var t = t_near;
    for (var i = 0u; i < uniforms.max_steps; i = i + 1u) {
        if (t > t_far) { break; }
        let sample_pos = (vol_origin / dims) + ray_dir * t;
        if (sample_pos.x >= 0.0 && sample_pos.x <= 1.0 &&
            sample_pos.y >= 0.0 && sample_pos.y <= 1.0 &&
            sample_pos.z >= 0.0 && sample_pos.z <= 1.0) {
            let value = textureSample(volume_texture, volume_sampler, sample_pos).r;
            if (uniforms.mip_mode == 0u) {
                if (value > result) { result = value; }
            } else {
                if (value < result) { result = value; }
            }
        }
        t = t + step;
    }

    let windowed = apply_voi(result, uniforms.window_center, uniforms.window_width);
    return vec4<f32>(windowed, windowed, windowed, 1.0);
}
"#;

/// Volume Rendering shader with transfer function, gradient-based Phong shading,
/// and interactive clip planes.
const VOLUME_VR_WGSL: &str = r#"
struct VrUniforms {
    // Inverse view-projection matrix (column-major)
    inv_view_proj: mat4x4<f32>,
    // Camera position in volume space
    camera_pos: vec3<f32>,
    // Volume dimensions
    volume_dims: vec3<u32>,
    // Ray marching parameters
    step_size: f32,
    max_steps: u32,
    // Lighting parameters (Phong model)
    light_dir: vec3<f32>,     // Directional light direction (normalized)
    ambient: f32,
    diffuse: f32,
    specular: f32,
    shininess: f32,
    // Clip planes (6 planes: +x, -x, +y, -y, +z, -z)
    // Each plane is (normal.x, normal.y, normal.z, distance)
    clip_plane_0: vec4<f32>;
    clip_plane_1: vec4<f32>;
    clip_plane_2: vec4<f32>;
    clip_plane_3: vec4<f32>;
    clip_plane_4: vec4<f32>;
    clip_plane_5: vec4<f32>;
    // Clip planes enabled count
    clip_planes_enabled: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> uniforms: VrUniforms;
@group(0) @binding(1) var volume_sampler: sampler;
@group(0) @binding(2) var volume_texture: texture_3d<f32>;
@group(0) @binding(3) var tf_sampler: sampler;
@group(0) @binding(4) var tf_texture: texture_2d<f32>;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    var out: VsOut;
    let pos = positions[vertex_index];
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = (pos + vec2<f32>(1.0, 1.0)) * 0.5;
    return out;
}

fn intersect_box(ray_origin: vec3<f32>, ray_dir: vec3<f32>, box_min: vec3<f32>, box_max: vec3<f32>) -> vec2<f32> {
    let inv_dir = 1.0 / ray_dir;
    let t_min_tmp = (box_min - ray_origin) * inv_dir;
    let t_max_tmp = (box_max - ray_origin) * inv_dir;
    let t_min = min(t_min_tmp, t_max_tmp);
    let t_max = max(t_min_tmp, t_max_tmp);
    let t_near = max(max(t_min.x, t_min.y), t_min.z);
    let t_far = min(min(t_max.x, t_max.y), t_max.z);
    return vec2<f32>(t_near, t_far);
}

fn is_clipped(pos: vec3<f32>) -> bool {
    if (uniforms.clip_planes_enabled == 0u) { return false; }
    if (uniforms.clip_planes_enabled >= 1u && dot(uniforms.clip_plane_0.xyz, pos) > uniforms.clip_plane_0.w) { return true; }
    if (uniforms.clip_planes_enabled >= 2u && dot(uniforms.clip_plane_1.xyz, pos) > uniforms.clip_plane_1.w) { return true; }
    if (uniforms.clip_planes_enabled >= 3u && dot(uniforms.clip_plane_2.xyz, pos) > uniforms.clip_plane_2.w) { return true; }
    if (uniforms.clip_planes_enabled >= 4u && dot(uniforms.clip_plane_3.xyz, pos) > uniforms.clip_plane_3.w) { return true; }
    if (uniforms.clip_planes_enabled >= 5u && dot(uniforms.clip_plane_4.xyz, pos) > uniforms.clip_plane_4.w) { return true; }
    if (uniforms.clip_planes_enabled >= 6u && dot(uniforms.clip_plane_5.xyz, pos) > uniforms.clip_plane_5.w) { return true; }
    return false;
}

fn compute_gradient(pos: vec3<f32>) -> vec3<f32> {
    let dims = vec3<f32>(uniforms.volume_dims);
    let step = 1.0 / dims;
    // Central differences
    let px = textureSample(volume_texture, volume_sampler, pos + vec3<f32>(step.x, 0.0, 0.0)).r;
    let mx = textureSample(volume_texture, volume_sampler, pos - vec3<f32>(step.x, 0.0, 0.0)).r;
    let py = textureSample(volume_texture, volume_sampler, pos + vec3<f32>(0.0, step.y, 0.0)).r;
    let my = textureSample(volume_texture, volume_sampler, pos - vec3<f32>(0.0, step.y, 0.0)).r;
    let pz = textureSample(volume_texture, volume_sampler, pos + vec3<f32>(0.0, 0.0, step.z)).r;
    let mz = textureSample(volume_texture, volume_sampler, pos - vec3<f32>(0.0, 0.0, step.z)).r;
    return vec3<f32>(px - mx, py - my, pz - mz);
}

fn phong_shading(normal: vec3<f32>, view_dir: vec3<f32>) -> f32 {
    let n = normalize(normal);
    let l = normalize(uniforms.light_dir);
    let r = reflect(-l, n);
    let diff = max(dot(n, l), 0.0) * uniforms.diffuse;
    let spec = pow(max(dot(r, view_dir), 0.0), uniforms.shininess) * uniforms.specular;
    return uniforms.ambient + diff + spec;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dims = vec3<f32>(uniforms.volume_dims);

    // Reconstruct ray direction from clip-space
    let clip_pos = vec4<f32>(in.uv * 2.0 - 1.0, 1.0, 1.0);
    let world_pos = uniforms.inv_view_proj * clip_pos;
    let ray_dir = normalize(world_pos.xyz / world_pos.w - uniforms.camera_pos);

    // Intersect with unit box [0, 1] in normalized volume coords
    let vol_origin_norm = uniforms.camera_pos / dims;
    let hit = intersect_box(vol_origin_norm, ray_dir, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    var t_near = hit.x;
    var t_far = hit.y;

    if (t_far < t_near || t_far < 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    t_near = max(t_near, 0.0);

    // Front-to-back compositing
    var accumulated_color = vec3<f32>(0.0, 0.0, 0.0);
    var accumulated_alpha = 0.0;

    let step = uniforms.step_size / max(max(dims.x, dims.y), dims.z);
    var t = t_near;

    for (var i = 0u; i < uniforms.max_steps; i = i + 1u) {
        if (t > t_far || accumulated_alpha >= 0.99) { break; }

        let sample_pos = vol_origin_norm + ray_dir * t;

        // Check bounds
        if (sample_pos.x < 0.0 || sample_pos.x > 1.0 ||
            sample_pos.y < 0.0 || sample_pos.y > 1.0 ||
            sample_pos.z < 0.0 || sample_pos.z > 1.0) {
            t = t + step;
            continue;
        }

        // Check clip planes
        if (is_clipped(sample_pos)) {
            t = t + step;
            continue;
        }

        let value = textureSample(volume_texture, volume_sampler, sample_pos).r;

        // Transfer function lookup: row 0 = color, row 1 = opacity
        let tf_color = textureSample(tf_texture, tf_sampler, vec2<f32>(value, 0.0)).rgb;
        let tf_opacity = textureSample(tf_texture, tf_sampler, vec2<f32>(value, 1.0)).r;

        if (tf_opacity > 0.001) {
            // Compute gradient for shading
            let gradient = compute_gradient(sample_pos);
            let grad_len = length(gradient);
            var shaded_color = tf_color;
            if (grad_len > 0.0001) {
                let view_dir = -ray_dir;
                let shading = phong_shading(gradient, view_dir);
                shaded_color = tf_color * shading;
            }

            // Front-to-back compositing
            let alpha_contrib = tf_opacity * (1.0 - accumulated_alpha);
            accumulated_color = accumulated_color + shaded_color * alpha_contrib;
            accumulated_alpha = accumulated_alpha + alpha_contrib;
        }

        t = t + step;
    }

    return vec4<f32>(accumulated_color, 1.0);
}
"#;

// ---------------------------------------------------------------------------
// Uniform Buffer Types
// ---------------------------------------------------------------------------

/// Uniform data for MPR slice rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct MprUniforms {
    /// Slice plane normal direction.
    plane_normal: [f32; 3],
    /// Slice plane origin in texel coordinates.
    plane_origin: [f32; 3],
    /// U-axis of the slice plane.
    axis_u: [f32; 3],
    /// V-axis of the slice plane.
    axis_v: [f32; 3],
    /// Volume dimensions `[x, y, z]`.
    volume_dims: [u32; 3],
    /// VOI window center.
    window_center: f32,
    /// VOI window width.
    window_width: f32,
    /// Slab thickness in voxels.
    slab_thickness: f32,
    /// Slab composition mode: 0 = average, 1 = max.
    slab_mode: u32,
    /// Padding.
    _pad0: u32,
    /// Padding.
    _pad1: u32,
}

/// Uniform data for MIP/MinIP ray-marching rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct MipUniforms {
    /// Inverse view-projection matrix (column-major).
    inv_view_proj: [[f32; 4]; 4],
    /// Camera position in volume texel space.
    camera_pos: [f32; 3],
    /// Volume dimensions `[x, y, z]`.
    volume_dims: [u32; 3],
    /// VOI window center.
    window_center: f32,
    /// VOI window width.
    window_width: f32,
    /// Ray step size in texel units.
    step_size: f32,
    /// Maximum number of ray steps.
    max_steps: u32,
    /// MIP mode: 0 = MaxIntensity, 1 = MinIntensity.
    mip_mode: u32,
    /// Slab thickness (0.0 = full volume).
    slab_thickness: f32,
    /// Slab center along view direction `[0, 1]`.
    slab_center: f32,
    /// Padding.
    _pad0: u32,
}

/// Uniform data for 3D Volume Rendering.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct VrUniforms {
    /// Inverse view-projection matrix (column-major).
    inv_view_proj: [[f32; 4]; 4],
    /// Camera position in volume texel space.
    camera_pos: [f32; 3],
    /// Volume dimensions `[x, y, z]`.
    volume_dims: [u32; 3],
    /// Ray step size in texel units.
    step_size: f32,
    /// Maximum number of ray steps.
    max_steps: u32,
    /// Directional light direction (normalized).
    light_dir: [f32; 3],
    /// Ambient lighting coefficient.
    ambient: f32,
    /// Diffuse lighting coefficient.
    diffuse: f32,
    /// Specular lighting coefficient.
    specular: f32,
    /// Specular shininess exponent.
    shininess: f32,
    /// Clip plane 0 `(normal.xyz, distance)`.
    clip_plane_0: [f32; 4],
    /// Clip plane 1.
    clip_plane_1: [f32; 4],
    /// Clip plane 2.
    clip_plane_2: [f32; 4],
    /// Clip plane 3.
    clip_plane_3: [f32; 4],
    /// Clip plane 4.
    clip_plane_4: [f32; 4],
    /// Clip plane 5.
    clip_plane_5: [f32; 4],
    /// Number of active clip planes (0..=6).
    clip_planes_enabled: u32,
    /// Padding.
    _pad0: u32,
    /// Padding.
    _pad1: u32,
    /// Padding.
    _pad2: u32,
}

// ---------------------------------------------------------------------------
// Transfer Function Presets
// ---------------------------------------------------------------------------

/// Preset transfer function identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferFunctionPreset {
    /// Bone visualization: high opacity for high-density values.
    Bone,
    /// Soft tissue visualization: mid-range opacity.
    SoftTissue,
    /// Lung visualization: low-density emphasis.
    Lung,
    /// Default linear ramp from transparent to opaque.
    Linear,
}

impl TransferFunctionPreset {
    /// Generate a 256×2 RGBA transfer function texture for this preset.
    ///
    /// Row 0 contains RGB color; row 1 contains opacity.
    /// The input `value_range` is `(min_value, max_value)` for normalizing scalar values.
    pub fn generate_rgba(&self, value_range: (f32, f32)) -> Vec<[u8; 4]> {
        let (min_v, max_v) = value_range;
        let range = max_v - min_v;
        let mut data = vec![[0u8; 4]; 512]; // 256 columns × 2 rows

        for i in 0..256 {
            let t = i as f32 / 255.0; // Normalized scalar value [0, 1]
            let value = min_v + t * range;

            let (r, g, b, a) = match self {
                TransferFunctionPreset::Bone => {
                    // Bone: high density (typically > 400 HU) gets high opacity
                    let bone_t = ((value - 400.0) / 1500.0).clamp(0.0, 1.0);
                    let opacity = bone_t.powf(0.5);
                    let r = (200.0 + 55.0 * bone_t) as u8;
                    let g = (180.0 + 55.0 * bone_t) as u8;
                    let b = (140.0 + 60.0 * bone_t) as u8;
                    (r, g, b, (opacity * 255.0) as u8)
                }
                TransferFunctionPreset::SoftTissue => {
                    // Soft tissue: mid-range (typically -100 to 200 HU)
                    let tissue_t = ((value - (-100.0)) / 300.0).clamp(0.0, 1.0);
                    let opacity = tissue_t * (1.0 - tissue_t) * 4.0 * 0.7;
                    let r = (180.0 + 40.0 * tissue_t) as u8;
                    let g = (100.0 + 50.0 * tissue_t) as u8;
                    let b = (100.0 + 50.0 * tissue_t) as u8;
                    (r, g, b, (opacity * 255.0) as u8)
                }
                TransferFunctionPreset::Lung => {
                    // Lung: low density emphasis (-1000 to -200 HU)
                    let lung_t = ((value - (-1000.0)) / 800.0).clamp(0.0, 1.0);
                    let opacity = (1.0 - lung_t).powf(2.0) * 0.5 + lung_t * 0.05;
                    let r = (100.0 + 100.0 * lung_t) as u8;
                    let g = (140.0 + 80.0 * lung_t) as u8;
                    let b = (180.0 + 60.0 * lung_t) as u8;
                    (r, g, b, (opacity * 255.0) as u8)
                }
                TransferFunctionPreset::Linear => {
                    // Linear ramp: full range
                    let opacity = t;
                    let v = (t * 255.0) as u8;
                    (v, v, v, (opacity * 255.0) as u8)
                }
            };

            // Row 0: color
            data[i] = [r, g, b, 255];
            // Row 1: opacity
            data[256 + i] = [a, a, a, 255];
        }

        data
    }
}

// ---------------------------------------------------------------------------
// Camera and Clip Plane Types
// ---------------------------------------------------------------------------

/// Arcball camera controller for 3D viewport rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct ArcballCamera {
    /// Camera position in volume texel space.
    pub eye: [f32; 3],
    /// Look-at target (volume center).
    pub target: [f32; 3],
    /// Up vector.
    pub up: [f32; 3],
    /// Field of view in radians.
    pub fov_y: f32,
    /// Aspect ratio (width / height).
    pub aspect: f32,
    /// Near plane distance.
    pub near: f32,
    /// Far plane distance.
    pub far: f32,
}

impl ArcballCamera {
    /// Create a default arcball camera looking at the center of a volume.
    pub fn new(volume_dims: [u32; 3], aspect: f32) -> Self {
        let max_dim = volume_dims[0].max(volume_dims[1]).max(volume_dims[2]) as f32;
        let center = [
            volume_dims[0] as f32 / 2.0,
            volume_dims[1] as f32 / 2.0,
            volume_dims[2] as f32 / 2.0,
        ];
        Self {
            eye: [center[0], center[1], center[2] + max_dim * 1.5],
            target: center,
            up: [0.0, 1.0, 0.0],
            fov_y: std::f32::consts::FRAC_PI_4,
            aspect,
            near: 0.1,
            far: max_dim * 10.0,
        }
    }

    /// Compute the inverse view-projection matrix (column-major).
    pub fn inverse_view_projection(&self) -> [[f32; 4]; 4] {
        let view = look_at_rh(self.eye, self.target, self.up);
        let proj = perspective_rh(self.fov_y, self.aspect, self.near, self.far);
        let vp = mul_mat4(proj, view);
        invert_mat4(vp)
    }
}

/// A clip plane defined by `(normal, distance)`.
///
/// Points `p` where `dot(normal, p) > distance` are clipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipPlane {
    /// Plane normal (does not need to be unit length).
    pub normal: [f32; 3],
    /// Plane distance from origin.
    pub distance: f32,
}

impl ClipPlane {
    /// Create a new clip plane.
    pub fn new(normal: [f32; 3], distance: f32) -> Self {
        Self { normal, distance }
    }
}

// ---------------------------------------------------------------------------
// MPR Slice Request
// ---------------------------------------------------------------------------

/// MPR rendering request parameters for the GPU volume renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuMprRequest {
    /// Plane normal in texel coordinates.
    pub plane_normal: [f32; 3],
    /// Plane origin in texel coordinates.
    pub plane_origin: [f32; 3],
    /// U-axis direction in texel coordinates.
    pub axis_u: [f32; 3],
    /// V-axis direction in texel coordinates.
    pub axis_v: [f32; 3],
    /// VOI window center.
    pub window_center: f32,
    /// VOI window width.
    pub window_width: f32,
    /// Slab thickness in voxels (1 = single slice).
    pub slab_thickness: f32,
    /// Slab mode: 0 = average, 1 = max.
    pub slab_mode: u32,
}

impl GpuMprRequest {
    /// Create an axial MPR request at the given z-index.
    pub fn axial(z_index: f32, _dims: [u32; 3], window_center: f32, window_width: f32) -> Self {
        Self {
            plane_normal: [0.0, 0.0, 1.0],
            plane_origin: [0.0, 0.0, z_index],
            axis_u: [1.0, 0.0, 0.0],
            axis_v: [0.0, 1.0, 0.0],
            window_center,
            window_width,
            slab_thickness: 1.0,
            slab_mode: 0,
        }
    }

    /// Create a coronal MPR request at the given y-index.
    pub fn coronal(y_index: f32, _dims: [u32; 3], window_center: f32, window_width: f32) -> Self {
        Self {
            plane_normal: [0.0, 1.0, 0.0],
            plane_origin: [0.0, y_index, 0.0],
            axis_u: [1.0, 0.0, 0.0],
            axis_v: [0.0, 0.0, 1.0],
            window_center,
            window_width,
            slab_thickness: 1.0,
            slab_mode: 0,
        }
    }

    /// Create a sagittal MPR request at the given x-index.
    pub fn sagittal(x_index: f32, _dims: [u32; 3], window_center: f32, window_width: f32) -> Self {
        Self {
            plane_normal: [1.0, 0.0, 0.0],
            plane_origin: [x_index, 0.0, 0.0],
            axis_u: [0.0, 1.0, 0.0],
            axis_v: [0.0, 0.0, 1.0],
            window_center,
            window_width,
            slab_thickness: 1.0,
            slab_mode: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// MIP Render Request
// ---------------------------------------------------------------------------

/// MIP/MinIP rendering request parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuMipRequest {
    /// Camera for view projection.
    pub camera: ArcballCamera,
    /// MIP projection mode.
    pub mode: MipProjectionMode,
    /// VOI window center.
    pub window_center: f32,
    /// VOI window width.
    pub window_width: f32,
    /// Ray step size in texel units.
    pub step_size: f32,
    /// Maximum ray steps.
    pub max_steps: u32,
    /// Slab thickness (0.0 = full volume).
    pub slab_thickness: f32,
    /// Slab center along view direction [0, 1].
    pub slab_center: f32,
}

// ---------------------------------------------------------------------------
// VR Render Request
// ---------------------------------------------------------------------------

/// Volume Rendering request parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuVrRequest {
    /// Camera for view projection.
    pub camera: ArcballCamera,
    /// Transfer function preset.
    pub tf_preset: TransferFunctionPreset,
    /// Data value range for transfer function normalization `(min, max)`.
    pub value_range: (f32, f32),
    /// Ray step size in texel units.
    pub step_size: f32,
    /// Maximum ray steps.
    pub max_steps: u32,
    /// Interactive clip planes (up to 6).
    pub clip_planes: Vec<ClipPlane>,
    /// Ambient lighting coefficient.
    pub ambient: f32,
    /// Diffuse lighting coefficient.
    pub diffuse: f32,
    /// Specular lighting coefficient.
    pub specular: f32,
    /// Specular shininess exponent.
    pub shininess: f32,
}

// ---------------------------------------------------------------------------
// VolumeRenderer
// ---------------------------------------------------------------------------

/// GPU volume renderer implementing MPR, MIP/MinIP, and 3D VR pipelines.
///
/// This renderer owns the WGPU device and queue, and manages volume texture
/// uploads, pipeline state, and rendering for all volumetric modes.
///
/// # Determinism
///
/// All rendering operations produce deterministic pixel output for identical
/// input parameters, per the project's determinism-first principle.
///
/// # Fail-Closed
///
/// Errors use `DVF.VOLUME.*` error codes and the shared `dicom_core::Error` model.
pub struct VolumeRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    budget: GpuBudget,

    // Volume texture
    volume_texture: Option<wgpu::Texture>,
    volume_view: Option<wgpu::TextureView>,
    volume_sampler: wgpu::Sampler,
    volume_dims: [u32; 3],
    volume_data_range: (f32, f32), // (min, max) for normalization

    // Transfer function texture (256 × 2, RGBA8UnormSrgb)
    tf_texture: Option<wgpu::Texture>,
    tf_view: Option<wgpu::TextureView>,
    tf_sampler: wgpu::Sampler,

    // MPR pipeline
    mpr_pipeline: wgpu::RenderPipeline,
    mpr_bind_group_layout: wgpu::BindGroupLayout,

    // MIP pipeline
    mip_pipeline: wgpu::RenderPipeline,
    mip_bind_group_layout: wgpu::BindGroupLayout,

    // VR pipeline
    vr_pipeline: wgpu::RenderPipeline,
    vr_bind_group_layout: wgpu::BindGroupLayout,

    // Target format (reserved for dynamic pipeline recreation)
    #[allow(dead_code)]
    target_format: wgpu::TextureFormat,
}

impl std::fmt::Debug for VolumeRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolumeRenderer")
            .field("volume_dims", &self.volume_dims)
            .field("volume_data_range", &self.volume_data_range)
            .field("budget", &self.budget)
            .finish()
    }
}

impl VolumeRenderer {
    /// Create a new volume renderer with the given WGPU device, queue, and GPU budget.
    ///
    /// The `target_format` specifies the output texture format for rendered frames.
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        budget: GpuBudget,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let volume_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diccy.volume_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let tf_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diccy.tf_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        // MPR bind group layout: uniform + sampler + 3D texture
        let mpr_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("diccy.volume_mpr_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // MIP bind group layout: same as MPR (uniform + sampler + 3D texture)
        let mip_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("diccy.volume_mip_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // VR bind group layout: uniform + sampler + 3D texture + tf_sampler + tf_texture
        let vr_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("diccy.volume_vr_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // Create pipelines
        let mpr_pipeline = create_volume_pipeline(
            &device,
            &mpr_bind_group_layout,
            VOLUME_MPR_WGSL,
            "diccy.volume_mpr_pipeline",
            target_format,
        );
        let mip_pipeline = create_volume_pipeline(
            &device,
            &mip_bind_group_layout,
            VOLUME_MIP_WGSL,
            "diccy.volume_mip_pipeline",
            target_format,
        );
        let vr_pipeline = create_volume_pipeline(
            &device,
            &vr_bind_group_layout,
            VOLUME_VR_WGSL,
            "diccy.volume_vr_pipeline",
            target_format,
        );

        Self {
            device,
            queue,
            budget,
            volume_texture: None,
            volume_view: None,
            volume_sampler,
            volume_dims: [0, 0, 0],
            volume_data_range: (0.0, 1.0),
            tf_texture: None,
            tf_view: None,
            tf_sampler,
            mpr_pipeline,
            mpr_bind_group_layout,
            mip_pipeline,
            mip_bind_group_layout,
            vr_pipeline,
            vr_bind_group_layout,
            target_format,
        }
    }

    /// Upload volume data from a `VolumeGrid` to a 3D GPU texture.
    ///
    /// Converts `i32` voxel values to `f32` and uploads as a `R32Sfloat` 3D texture.
    /// The data range is tracked for normalization in shaders.
    ///
    /// # Errors
    ///
    /// Returns `DVF.VOLUME.EMPTY_INPUT` if the volume has zero dimensions.
    /// Returns `DVF.VOLUME.SIZE_OVERFLOW` if the volume exceeds GPU budget.
    pub fn upload_volume(&mut self, volume: &VolumeGrid) -> Result<()> {
        let dims = volume.dimensions();
        if dims[0] == 0 || dims[1] == 0 || dims[2] == 0 {
            return Err(Error::new(
                "DVF.VOLUME.EMPTY_INPUT",
                ErrorKind::InvalidPixelTransform {
                    stage: "volume_upload".to_string(),
                    detail: "volume dimensions must be non-zero".to_string(),
                },
                "Volume upload rejected: zero dimensions",
            )
            .into());
        }

        let width = dims[0] as u32;
        let height = dims[1] as u32;
        let depth = dims[2] as u32;

        // Release previous texture budget
        if let Some(prev_texture) = &self.volume_texture {
            let prev_bytes = texture_size_bytes(&prev_texture);
            self.budget.release(prev_bytes);
        }

        // Calculate required texture bytes
        let texture_bytes = (width as u64) * (height as u64) * (depth as u64) * 4; // R32Sfloat = 4 bytes
        self.budget.reserve(texture_bytes).map_err(|_| {
            Error::new(
                "DVF.VOLUME.SIZE_OVERFLOW",
                ErrorKind::LimitExceeded {
                    limit_name: "max_gpu_texture_bytes",
                    observed: texture_bytes,
                    allowed: self.budget.max_texture_bytes,
                },
                "Volume upload rejected: GPU texture budget exceeded",
            )
        })?;

        // Convert i32 voxels to f32
        let voxels = volume.voxels();
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        let mut float_data = Vec::with_capacity(voxels.len());
        for &v in voxels {
            let f = v as f32;
            float_data.push(f);
            if f < min_val {
                min_val = f;
            }
            if f > max_val {
                max_val = f;
            }
        }
        if min_val > max_val {
            min_val = 0.0;
            max_val = 1.0;
        }
        self.volume_data_range = (min_val, max_val);

        // Normalize to [0, 1] for the texture (shaders denormalize using data range)
        let range = max_val - min_val;
        if range.abs() > f32::EPSILON {
            for f in &mut float_data {
                *f = (*f - min_val) / range;
            }
        } else {
            for f in &mut float_data {
                *f = 0.5;
            }
        }

        // Convert f32 to bytes
        let byte_data: Vec<u8> = float_data.iter().flat_map(|f| f.to_le_bytes()).collect();

        // Create 3D texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diccy.volume_3d_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload data
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &byte_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: depth,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("diccy.volume_3d_view"),
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        });

        self.volume_texture = Some(texture);
        self.volume_view = Some(view);
        self.volume_dims = [width, height, depth];

        Ok(())
    }

    /// Upload a transfer function preset as a 256×2 RGBA texture.
    ///
    /// Row 0 contains color (RGB); row 1 contains opacity (A).
    pub fn upload_transfer_function(&mut self, preset: TransferFunctionPreset) -> Result<()> {
        let data = preset.generate_rgba(self.volume_data_range);

        // Convert [u8; 4] to raw bytes
        let byte_data: Vec<u8> = data.iter().flat_map(|rgba| rgba.iter().copied()).collect();

        // Release previous TF texture budget
        if let Some(prev_texture) = &self.tf_texture {
            let prev_bytes = texture_size_bytes(&prev_texture);
            self.budget.release(prev_bytes);
        }

        let tf_bytes = 256u64 * 2 * 4; // 256 x 2 x RGBA8
        self.budget.reserve(tf_bytes).map_err(|_| {
            Error::new(
                "DVF.VOLUME.SIZE_OVERFLOW",
                ErrorKind::LimitExceeded {
                    limit_name: "max_gpu_texture_bytes",
                    observed: tf_bytes,
                    allowed: self.budget.max_texture_bytes,
                },
                "Transfer function upload rejected: GPU texture budget exceeded",
            )
        })?;

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diccy.tf_texture"),
            size: wgpu::Extent3d {
                width: 256,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &byte_data,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 256,
                height: 2,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("diccy.tf_view"),
            dimension: Some(wgpu::TextureViewDimension::D2),
            ..Default::default()
        });

        self.tf_texture = Some(texture);
        self.tf_view = Some(view);

        Ok(())
    }

    /// Return the current volume dimensions.
    pub fn volume_dims(&self) -> [u32; 3] {
        self.volume_dims
    }

    /// Return the tracked volume data range `(min, max)`.
    pub fn volume_data_range(&self) -> (f32, f32) {
        self.volume_data_range
    }

    /// Return a reference to the GPU budget.
    pub fn budget(&self) -> &GpuBudget {
        &self.budget
    }

    /// Render an MPR slice into the given target texture view.
    ///
    /// # Errors
    ///
    /// Returns `DVF.VOLUME.EMPTY_INPUT` if no volume has been uploaded.
    pub fn render_mpr(&self, target: &wgpu::TextureView, request: &GpuMprRequest) -> Result<()> {
        let volume_view = self.volume_view.as_ref().ok_or_else(|| {
            Error::new(
                "DVF.VOLUME.EMPTY_INPUT",
                ErrorKind::InvalidPixelTransform {
                    stage: "volume_mpr".to_string(),
                    detail: "no volume texture uploaded".to_string(),
                },
                "MPR render rejected: no volume uploaded",
            )
        })?;

        let uniforms = MprUniforms {
            plane_normal: request.plane_normal,
            plane_origin: request.plane_origin,
            axis_u: request.axis_u,
            axis_v: request.axis_v,
            volume_dims: self.volume_dims,
            window_center: request.window_center,
            window_width: request.window_width,
            slab_thickness: request.slab_thickness,
            slab_mode: request.slab_mode,
            _pad0: 0,
            _pad1: 0,
        };

        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("diccy.mpr_uniform_buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diccy.volume_mpr_bind_group"),
            layout: &self.mpr_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.volume_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(volume_view),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("diccy.mpr_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("diccy.mpr_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.mpr_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render a MIP/MinIP projection into the given target texture view.
    ///
    /// # Errors
    ///
    /// Returns `DVF.VOLUME.EMPTY_INPUT` if no volume has been uploaded.
    pub fn render_mip(&self, target: &wgpu::TextureView, request: &GpuMipRequest) -> Result<()> {
        let volume_view = self.volume_view.as_ref().ok_or_else(|| {
            Error::new(
                "DVF.VOLUME.EMPTY_INPUT",
                ErrorKind::InvalidPixelTransform {
                    stage: "volume_mip".to_string(),
                    detail: "no volume texture uploaded".to_string(),
                },
                "MIP render rejected: no volume uploaded",
            )
        })?;

        let mip_mode = match request.mode {
            MipProjectionMode::MaxIntensity => 0u32,
            MipProjectionMode::MinIntensity => 1u32,
        };

        let uniforms = MipUniforms {
            inv_view_proj: request.camera.inverse_view_projection(),
            camera_pos: request.camera.eye,
            volume_dims: self.volume_dims,
            window_center: request.window_center,
            window_width: request.window_width,
            step_size: request.step_size,
            max_steps: request.max_steps,
            mip_mode,
            slab_thickness: request.slab_thickness,
            slab_center: request.slab_center,
            _pad0: 0,
        };

        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("diccy.mip_uniform_buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diccy.volume_mip_bind_group"),
            layout: &self.mip_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.volume_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(volume_view),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("diccy.mip_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("diccy.mip_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.mip_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render a 3D Volume Rendering into the given target texture view.
    ///
    /// Requires that both a volume and a transfer function have been uploaded.
    ///
    /// # Errors
    ///
    /// Returns `DVF.VOLUME.EMPTY_INPUT` if no volume has been uploaded.
    /// Returns `DVF.VOLUME.INVALID_REQUEST` if no transfer function has been uploaded.
    pub fn render_vr(&self, target: &wgpu::TextureView, request: &GpuVrRequest) -> Result<()> {
        let volume_view = self.volume_view.as_ref().ok_or_else(|| {
            Error::new(
                "DVF.VOLUME.EMPTY_INPUT",
                ErrorKind::InvalidPixelTransform {
                    stage: "volume_vr".to_string(),
                    detail: "no volume texture uploaded".to_string(),
                },
                "VR render rejected: no volume uploaded",
            )
        })?;

        let tf_view = self.tf_view.as_ref().ok_or_else(|| {
            Error::new(
                "DVF.VOLUME.INVALID_REQUEST",
                ErrorKind::InvalidPixelTransform {
                    stage: "volume_vr".to_string(),
                    detail: "no transfer function uploaded".to_string(),
                },
                "VR render rejected: no transfer function uploaded",
            )
        })?;

        // Build clip planes (up to 6)
        let mut clip_planes = [[0.0f32; 4]; 6];
        let enabled = request.clip_planes.len().min(6);
        for (i, plane) in request.clip_planes.iter().enumerate().take(enabled) {
            clip_planes[i] = [
                plane.normal[0],
                plane.normal[1],
                plane.normal[2],
                plane.distance,
            ];
        }

        let uniforms = VrUniforms {
            inv_view_proj: request.camera.inverse_view_projection(),
            camera_pos: request.camera.eye,
            volume_dims: self.volume_dims,
            step_size: request.step_size,
            max_steps: request.max_steps,
            light_dir: [0.577, 0.577, 0.577], // Default directional light
            ambient: request.ambient,
            diffuse: request.diffuse,
            specular: request.specular,
            shininess: request.shininess,
            clip_plane_0: clip_planes[0],
            clip_plane_1: clip_planes[1],
            clip_plane_2: clip_planes[2],
            clip_plane_3: clip_planes[3],
            clip_plane_4: clip_planes[4],
            clip_plane_5: clip_planes[5],
            clip_planes_enabled: enabled as u32,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };

        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("diccy.vr_uniform_buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diccy.volume_vr_bind_group"),
            layout: &self.vr_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.volume_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(volume_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.tf_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(tf_view),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("diccy.vr_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("diccy.vr_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.vr_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pipeline Creation Helper
// ---------------------------------------------------------------------------

fn create_volume_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    wgsl_source: &str,
    label: &str,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(wgsl_source)),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn texture_size_bytes(texture: &wgpu::Texture) -> u64 {
    let size = texture.size();
    let bytes_per_pixel = 4u64; // R32Sfloat or Rgba8UnormSrgb
    (size.width as u64)
        * (size.height as u64)
        * (size.depth_or_array_layers as u64)
        * bytes_per_pixel
}

// ---------------------------------------------------------------------------
// Simple Math Helpers
// ---------------------------------------------------------------------------

/// Build a right-handed look-at view matrix.
pub fn look_at_rh(eye: [f32; 3], target: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize3(sub3(target, eye));
    let s = normalize3(cross3(f, up));
    let u = cross3(s, f);

    [
        [s[0], u[0], -f[0], 0.0],
        [s[1], u[1], -f[1], 0.0],
        [s[2], u[2], -f[2], 0.0],
        [-dot3(s, eye), -dot3(u, eye), dot3(f, eye), 1.0],
    ]
}

/// Build a right-handed perspective projection matrix.
pub fn perspective_rh(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (fov_y * 0.5).tan();
    let range = far - near;
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / range, 1.0],
        [0.0, 0.0, -near * far / range, 0.0],
    ]
}

/// Multiply two 4x4 matrices.
pub fn mul_mat4(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

/// Invert a 4x4 matrix.
pub fn invert_mat4(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Cofactor-based 4x4 matrix inversion
    let mut inv = [[0.0f32; 4]; 4];

    inv[0][0] =
        m[1][1] * m[2][2] * m[3][3] - m[1][1] * m[2][3] * m[3][2] - m[2][1] * m[1][2] * m[3][3]
            + m[2][1] * m[1][3] * m[3][2]
            + m[3][1] * m[1][2] * m[2][3]
            - m[3][1] * m[1][3] * m[2][2];

    inv[1][0] =
        -m[1][0] * m[2][2] * m[3][3] + m[1][0] * m[2][3] * m[3][2] + m[2][0] * m[1][2] * m[3][3]
            - m[2][0] * m[1][3] * m[3][2]
            - m[3][0] * m[1][2] * m[2][3]
            + m[3][0] * m[1][3] * m[2][2];

    inv[2][0] =
        m[1][0] * m[2][1] * m[3][3] - m[1][0] * m[2][3] * m[3][1] - m[2][0] * m[1][1] * m[3][3]
            + m[2][0] * m[1][3] * m[3][1]
            + m[3][0] * m[1][1] * m[2][3]
            - m[3][0] * m[1][3] * m[2][1];

    inv[3][0] =
        -m[1][0] * m[2][1] * m[3][2] + m[1][0] * m[2][2] * m[3][1] + m[2][0] * m[1][1] * m[3][2]
            - m[2][0] * m[1][2] * m[3][1]
            - m[3][0] * m[1][1] * m[2][2]
            + m[3][0] * m[1][2] * m[2][1];

    let det = m[0][0] * inv[0][0] + m[0][1] * inv[1][0] + m[0][2] * inv[2][0] + m[0][3] * inv[3][0];

    if det.abs() < f32::EPSILON {
        return [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
    }

    let inv_det = 1.0 / det;

    inv[0][1] =
        -m[0][1] * m[2][2] * m[3][3] + m[0][1] * m[2][3] * m[3][2] + m[2][1] * m[0][2] * m[3][3]
            - m[2][1] * m[0][3] * m[3][2]
            - m[3][1] * m[0][2] * m[2][3]
            + m[3][1] * m[0][3] * m[2][2];

    inv[1][1] =
        m[0][0] * m[2][2] * m[3][3] - m[0][0] * m[2][3] * m[3][2] - m[2][0] * m[0][2] * m[3][3]
            + m[2][0] * m[0][3] * m[3][2]
            + m[3][0] * m[0][2] * m[2][3]
            - m[3][0] * m[0][3] * m[2][2];

    inv[2][1] =
        -m[0][0] * m[2][1] * m[3][3] + m[0][0] * m[2][3] * m[3][1] + m[2][0] * m[0][1] * m[3][3]
            - m[2][0] * m[0][3] * m[3][1]
            - m[3][0] * m[0][1] * m[2][3]
            + m[3][0] * m[0][3] * m[2][1];

    inv[3][1] =
        m[0][0] * m[2][1] * m[3][2] - m[0][0] * m[2][2] * m[3][1] - m[2][0] * m[0][1] * m[3][2]
            + m[2][0] * m[0][2] * m[3][1]
            + m[3][0] * m[0][1] * m[2][2]
            - m[3][0] * m[0][2] * m[2][1];

    inv[0][2] =
        m[0][1] * m[1][2] * m[3][3] - m[0][1] * m[1][3] * m[3][2] - m[1][1] * m[0][2] * m[3][3]
            + m[1][1] * m[0][3] * m[3][2]
            + m[3][1] * m[0][2] * m[1][3]
            - m[3][1] * m[0][3] * m[1][2];

    inv[1][2] =
        -m[0][0] * m[1][2] * m[3][3] + m[0][0] * m[1][3] * m[3][2] + m[1][0] * m[0][2] * m[3][3]
            - m[1][0] * m[0][3] * m[3][2]
            - m[3][0] * m[0][2] * m[1][3]
            + m[3][0] * m[0][3] * m[1][2];

    inv[2][2] =
        m[0][0] * m[1][1] * m[3][3] - m[0][0] * m[1][3] * m[3][1] - m[1][0] * m[0][1] * m[3][3]
            + m[1][0] * m[0][3] * m[3][1]
            + m[3][0] * m[0][1] * m[1][3]
            - m[3][0] * m[0][3] * m[1][1];

    inv[3][2] =
        -m[0][0] * m[1][1] * m[3][2] + m[0][0] * m[1][2] * m[3][1] + m[1][0] * m[0][1] * m[3][2]
            - m[1][0] * m[0][2] * m[3][1]
            - m[3][0] * m[0][1] * m[1][2]
            + m[3][0] * m[0][2] * m[1][1];

    inv[0][3] =
        -m[0][1] * m[1][2] * m[2][3] + m[0][1] * m[1][3] * m[2][2] + m[1][1] * m[0][2] * m[2][3]
            - m[1][1] * m[0][3] * m[2][2]
            - m[2][1] * m[0][2] * m[1][3]
            + m[2][1] * m[0][3] * m[1][2];

    inv[1][3] =
        m[0][0] * m[1][2] * m[2][3] - m[0][0] * m[1][3] * m[2][2] - m[1][0] * m[0][2] * m[2][3]
            + m[1][0] * m[0][3] * m[2][2]
            + m[2][0] * m[0][2] * m[1][3]
            - m[2][0] * m[0][3] * m[1][2];

    inv[2][3] =
        -m[0][0] * m[1][1] * m[2][3] + m[0][0] * m[1][3] * m[2][1] + m[1][0] * m[0][1] * m[2][3]
            - m[1][0] * m[0][3] * m[2][1]
            - m[2][0] * m[0][1] * m[1][3]
            + m[2][0] * m[0][3] * m[1][1];

    inv[3][3] =
        m[0][0] * m[1][1] * m[2][2] - m[0][0] * m[1][2] * m[2][1] - m[1][0] * m[0][1] * m[2][2]
            + m[1][0] * m[0][2] * m[2][1]
            + m[2][0] * m[0][1] * m[1][2]
            - m[2][0] * m[0][2] * m[1][1];

    for i in 0..4 {
        for j in 0..4 {
            inv[i][j] *= inv_det;
        }
    }

    inv
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > f32::EPSILON {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
