use dicom_core::Limits;
use dicom_pixel::PixelFormat;

const SMALL_UPLOAD_CHUNK_BYTES: usize = 256 * 1024;
const MEDIUM_UPLOAD_CHUNK_BYTES: usize = 1024 * 1024;
const LARGE_UPLOAD_CHUNK_BYTES: usize = 2 * 1024 * 1024;
const XL_UPLOAD_CHUNK_BYTES: usize = 4 * 1024 * 1024;
const DEFAULT_FRAME_INTERVAL_MS: u32 = 16;
const FALLBACK_LATENCY_BUDGET_FACTOR: u32 = 2;
const FALLBACK_LATENCY_BUDGET_CEILING_MS: u32 = 250;
const WEBGPU_SHADER_WGSL: &str = r#"
@group(0) @binding(0)
var frame_sampler: sampler;
@group(0) @binding(1)
var frame_texture: texture_2d<f32>;

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
  return textureSample(frame_texture, frame_sampler, in.uv);
}
"#;

/// GLSL vertex shader for WebGL2 MPR 2D texture quad rendering.
/// Renders a full-screen quad with a 2D texture sample.
const WEBGL2_MPR_VERT_GLSL: &str = r#"#version 300 es
precision highp float;
layout(location = 0) in vec2 a_position;
layout(location = 1) in vec2 a_texcoord;
out vec2 v_uv;
void main() {
    gl_Position = vec4(a_position, 0.0, 1.0);
    v_uv = a_texcoord;
}
"#;

/// GLSL fragment shader for WebGL2 MPR 2D texture quad rendering.
/// Samples a 2D texture and applies window/level adjustment.
const WEBGL2_MPR_FRAG_GLSL: &str = r#"#version 300 es
precision highp float;
precision highp sampler2D;
in vec2 v_uv;
out vec4 frag_color;
uniform sampler2D u_slice_texture;
uniform float u_window_center;
uniform float u_window_width;
uniform float u_slope;
uniform float u_intercept;
void main() {
    float raw = texture(u_slice_texture, v_uv).r;
    float hu = raw * u_slope + u_intercept;
    float lo = u_window_center - u_window_width * 0.5;
    float hi = u_window_center + u_window_width * 0.5;
    float val = clamp((hu - lo) / max(u_window_width, 1.0), 0.0, 1.0);
    frag_color = vec4(val, val, val, 1.0);
}
"#;

/// GLSL vertex shader for WebGL2 MIP/MinIP ray-marching.
/// Full-screen triangle approach for maximum GPU utilization.
const WEBGL2_MIP_VERT_GLSL: &str = r#"#version 300 es
precision highp float;
out vec2 v_uv;
void main() {
    float x = float((gl_VertexID & 1) << 2);
    float y = float((gl_VertexID & 2) << 1);
    v_uv = vec2(x, y) * 0.5;
    gl_Position = vec4(x - 1.0, y - 1.0, 0.0, 1.0);
}
"#;

/// GLSL fragment shader for WebGL2 MIP/MinIP ray-marching via 2D texture arrays.
/// WebGL2 lacks native 3D textures, so we use a 2D texture array (sampler2DArray)
/// where each layer represents one slice of the volume. The ray-march steps through
/// the volume, sampling the appropriate layer at each step.
const WEBGL2_MIP_FRAG_GLSL: &str = r#"#version 300 es
precision highp float;
precision highp sampler2DArray;
in vec2 v_uv;
out vec4 frag_color;
uniform sampler2DArray u_volume_slices;
uniform vec3 u_volume_dims;       // (width, height, depth)
uniform float u_slice_count;      // depth as float
uniform mat4 u_inv_view_proj;
uniform vec3 u_camera_pos;
uniform float u_step_size;
uniform int u_max_steps;
uniform int u_mip_mode;           // 0 = MIP, 1 = MinIP
uniform float u_window_center;
uniform float u_window_width;
uniform float u_slope;
uniform float u_intercept;
uniform float u_slab_thickness;
uniform float u_slab_center;

// Sample the volume at a given 3D position using 2D texture array.
// Each slice of the volume occupies one layer in the array.
float sample_volume(vec3 pos) {
    float z = clamp(pos.z, 0.0, 1.0) * (u_slice_count - 1.0);
    int layer0 = int(floor(z));
    int layer1 = min(layer0 + 1, int(u_slice_count) - 1);
    float frac_z = fract(z);
    float s0 = texture(u_volume_slices, vec3(pos.xy, float(layer0))).r;
    float s1 = texture(u_volume_slices, vec3(pos.xy, float(layer1))).r;
    return mix(s0, s1, frac_z);
}

// Compute gradient via central differences using 2D texture array.
vec3 compute_gradient(vec3 pos) {
    float dx = 1.0 / max(u_volume_dims.x, 1.0);
    float dy = 1.0 / max(u_volume_dims.y, 1.0);
    float dz = 1.0 / max(u_volume_dims.z, 1.0);
    float gx = sample_volume(pos + vec3(dx, 0.0, 0.0))
             - sample_volume(pos - vec3(dx, 0.0, 0.0));
    float gy = sample_volume(pos + vec3(0.0, dy, 0.0))
             - sample_volume(pos - vec3(0.0, dy, 0.0));
    float gz = sample_volume(pos + vec3(0.0, 0.0, dz))
             - sample_volume(pos - vec3(0.0, 0.0, dz));
    return vec3(gx, gy, gz);
}

void main() {
    // Ray setup: compute ray from camera through fragment
    vec4 ndc = vec4(v_uv * 2.0 - 1.0, -1.0, 1.0);
    vec4 world_near = u_inv_view_proj * ndc;
    world_near /= world_near.w;
    vec3 ray_dir = normalize(world_near.xyz - u_camera_pos);
    vec3 ray_origin = u_camera_pos;

    // Ray-box intersection with unit cube [0,1]^3
    vec3 inv_dir = 1.0 / max(abs(ray_dir), 1e-8) * sign(ray_dir);
    vec3 t_bottom = (0.0 - ray_origin) * inv_dir;
    vec3 t_top = (1.0 - ray_origin) * inv_dir;
    vec3 t_min_vec = min(t_top, t_bottom);
    vec3 t_max_vec = max(t_top, t_bottom);
    float t_near = max(max(t_min_vec.x, t_min_vec.y), t_min_vec.z);
    float t_far = min(min(t_max_vec.x, t_max_vec.y), t_max_vec.z);
    if (t_near > t_far || t_far < 0.0) {
        frag_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    t_near = max(t_near, 0.0);

    // Apply slab bounds
    float slab_lo = u_slab_center - u_slab_thickness * 0.5;
    float slab_hi = u_slab_center + u_slab_thickness * 0.5;

    float result = (u_mip_mode == 1) ? 1.0 : -1.0;
    bool hit = false;

    for (int i = 0; i < u_max_steps; i++) {
        float t = t_near + float(i) * u_step_size;
        if (t > t_far) break;
        vec3 pos = ray_origin + ray_dir * t;
        if (any(lessThan(pos, vec3(0.0))) || any(greaterThan(pos, vec3(1.0)))) continue;

        // Slab culling
        float depth_pos = pos.z;
        if (depth_pos < slab_lo || depth_pos > slab_hi) continue;

        float val = sample_volume(pos);
        hit = true;
        if (u_mip_mode == 0) {
            result = max(result, val);
        } else {
            result = min(result, val);
        }
    }

    if (!hit) {
        frag_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    // Apply windowing
    float hu = result * u_slope + u_intercept;
    float lo = u_window_center - u_window_width * 0.5;
    float hi = u_window_center + u_window_width * 0.5;
    float display_val = clamp((hu - lo) / max(u_window_width, 1.0), 0.0, 1.0);
    frag_color = vec4(display_val, display_val, display_val, 1.0);
}
"#;

/// GLSL vertex shader for WebGL2 volume rendering (transfer function + gradient shading).
const WEBGL2_VR_VERT_GLSL: &str = r#"#version 300 es
precision highp float;
out vec2 v_uv;
void main() {
    float x = float((gl_VertexID & 1) << 2);
    float y = float((gl_VertexID & 2) << 1);
    v_uv = vec2(x, y) * 0.5;
    gl_Position = vec4(x - 1.0, y - 1.0, 0.0, 1.0);
}
"#;

/// GLSL fragment shader for WebGL2 volume rendering with transfer function and gradient shading.
/// Uses 2D texture arrays (sampler2DArray) to represent the 3D volume since WebGL2
/// lacks native 3D textures. Implements front-to-back compositing with a 1D transfer
/// function texture and Phong-style gradient shading.
const WEBGL2_VR_FRAG_GLSL: &str = r#"#version 300 es
precision highp float;
precision highp sampler2DArray;
precision highp sampler2D;
in vec2 v_uv;
out vec4 frag_color;
uniform sampler2DArray u_volume_slices;
uniform sampler2D u_transfer_func;  // 1D transfer function (256x1)
uniform vec3 u_volume_dims;
uniform float u_slice_count;
uniform mat4 u_inv_view_proj;
uniform vec3 u_camera_pos;
uniform float u_step_size;
uniform int u_max_steps;
uniform float u_slope;
uniform float u_intercept;
uniform vec3 u_light_dir;

// Sample the volume at a given 3D position using 2D texture array.
float sample_volume(vec3 pos) {
    float z = clamp(pos.z, 0.0, 1.0) * (u_slice_count - 1.0);
    int layer0 = int(floor(z));
    int layer1 = min(layer0 + 1, int(u_slice_count) - 1);
    float frac_z = fract(z);
    float s0 = texture(u_volume_slices, vec3(pos.xy, float(layer0))).r;
    float s1 = texture(u_volume_slices, vec3(pos.xy, float(layer1))).r;
    return mix(s0, s1, frac_z);
}

// Compute gradient via central differences using 2D texture array.
vec3 compute_gradient(vec3 pos) {
    float dx = 1.0 / max(u_volume_dims.x, 1.0);
    float dy = 1.0 / max(u_volume_dims.y, 1.0);
    float dz = 1.0 / max(u_volume_dims.z, 1.0);
    float gx = sample_volume(pos + vec3(dx, 0.0, 0.0))
             - sample_volume(pos - vec3(dx, 0.0, 0.0));
    float gy = sample_volume(pos + vec3(0.0, dy, 0.0))
             - sample_volume(pos - vec3(0.0, dy, 0.0));
    float gz = sample_volume(pos + vec3(0.0, 0.0, dz))
             - sample_volume(pos - vec3(0.0, 0.0, dz));
    return vec3(gx, gy, gz);
}

void main() {
    // Ray setup
    vec4 ndc = vec4(v_uv * 2.0 - 1.0, -1.0, 1.0);
    vec4 world_near = u_inv_view_proj * ndc;
    world_near /= world_near.w;
    vec3 ray_dir = normalize(world_near.xyz - u_camera_pos);
    vec3 ray_origin = u_camera_pos;

    // Ray-box intersection with unit cube
    vec3 inv_dir = 1.0 / max(abs(ray_dir), 1e-8) * sign(ray_dir);
    vec3 t_bottom = (0.0 - ray_origin) * inv_dir;
    vec3 t_top = (1.0 - ray_origin) * inv_dir;
    vec3 t_min_vec = min(t_top, t_bottom);
    vec3 t_max_vec = max(t_top, t_bottom);
    float t_near = max(max(t_min_vec.x, t_min_vec.y), t_min_vec.z);
    float t_far = min(min(t_max_vec.x, t_max_vec.y), t_max_vec.z);
    if (t_near > t_far || t_far < 0.0) {
        frag_color = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }
    t_near = max(t_near, 0.0);

    // Front-to-back compositing
    vec4 accumulated = vec4(0.0);
    for (int i = 0; i < u_max_steps; i++) {
        float t = t_near + float(i) * u_step_size;
        if (t > t_far) break;

        vec3 pos = ray_origin + ray_dir * t;
        if (any(lessThan(pos, vec3(0.0))) || any(greaterThan(pos, vec3(1.0)))) continue;

        float scalar = sample_volume(pos);

        // Apply slope/intercept to get HU value, normalize to [0,1] for TF lookup
        float hu = scalar * u_slope + u_intercept;
        float tf_coord = clamp(hu, 0.0, 1.0);
        vec4 tf_rgba = texture(u_transfer_func, vec2(tf_coord, 0.5));

        // Gradient-based shading (Phong model)
        vec3 gradient = compute_gradient(pos);
        float grad_mag = length(gradient);
        float ambient = 0.3;
        float diffuse = 0.0;
        float specular = 0.0;
        if (grad_mag > 1e-6) {
            vec3 normal = normalize(gradient);
            diffuse = max(dot(normal, u_light_dir), 0.0) * 0.6;
            vec3 half_vec = normalize(u_light_dir - ray_dir);
            specular = pow(max(dot(normal, half_vec), 0.0), 32.0) * 0.3;
        }
        float shading = ambient + diffuse + specular;

        // Apply shading to TF color
        vec3 shaded_color = tf_rgba.rgb * shading;

        // Front-to-back compositing
        float alpha = tf_rgba.a * u_step_size * 50.0; // Scale by step size
        alpha = clamp(alpha, 0.0, 1.0);
        accumulated.rgb += (1.0 - accumulated.a) * alpha * shaded_color;
        accumulated.a += (1.0 - accumulated.a) * alpha;

        // Early ray termination
        if (accumulated.a > 0.98) break;
    }

    frag_color = vec4(accumulated.rgb, 1.0);
}
"#;

/// Stable backend names exposed to host code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    /// CPU/canvas presentation path.
    Cpu,
    /// WebGPU presentation path.
    WebGpu,
    /// WebGL2 presentation path (fallback for browsers without WebGPU).
    WebGL2,
}

impl RendererBackend {
    /// Return the backend name as a static string.
    pub fn as_str(self) -> &'static str {
        match self {
            RendererBackend::Cpu => "CPU",
            RendererBackend::WebGpu => "WebGPU",
            RendererBackend::WebGL2 => "WebGL2",
        }
    }
}

/// WebGPU lifecycle states for deterministic host reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebGpuLifecycle {
    /// Backend not initialized.
    Uninitialized,
    /// Backend ready to submit.
    Ready,
    /// Backend encountered device loss.
    DeviceLost,
    /// Backend failed initialization or submit.
    Failed,
}

/// WebGL2 lifecycle states for deterministic host reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebGL2Lifecycle {
    /// Backend not initialized.
    Uninitialized,
    /// Backend ready to submit.
    Ready,
    /// Backend encountered context loss.
    ContextLost,
    /// Backend failed initialization or submit.
    Failed,
}

/// Typed backend error codes for host/runtime diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendErrorCode {
    /// GPU init failed.
    InitFailed,
    /// GPU device-lost state.
    DeviceLost,
    /// GPU submit failed.
    SubmitFailed,
    /// Production WebGPU runtime flag disabled.
    FlagDisabled,
    /// WebGL2 init failed.
    WebGL2InitFailed,
    /// WebGL2 context lost.
    WebGL2ContextLost,
}

impl BackendErrorCode {
    /// Return the error code as a static string identifier.
    pub fn as_code(self) -> &'static str {
        match self {
            BackendErrorCode::InitFailed => "DVF.WASM.GPU.INIT_FAILED",
            BackendErrorCode::DeviceLost => "DVF.WASM.GPU.DEVICE_LOST",
            BackendErrorCode::SubmitFailed => "DVF.WASM.GPU.SUBMIT_FAILED",
            BackendErrorCode::FlagDisabled => "DVF.WASM.GPU.FLAG_DISABLED",
            BackendErrorCode::WebGL2InitFailed => "DVF.WASM.WEBGL2.INIT_FAILED",
            BackendErrorCode::WebGL2ContextLost => "DVF.WASM.WEBGL2.CONTEXT_LOST",
        }
    }
}

/// Capability snapshot provided by host probing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendCapabilityProbe {
    /// Whether the WebGPU API is available.
    pub webgpu_api: bool,
    /// Whether a WebGPU adapter is available.
    pub adapter_available: bool,
    /// Whether the WebGL2 API is available.
    pub webgl2_api: bool,
    /// Maximum 2D texture dimension supported.
    pub max_texture_dimension_2d: u32,
}

/// Non-PHI backend selection and upload metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSelectionMetrics {
    /// Number of backend probe attempts.
    pub probe_attempts: u64,
    /// Number of WebGPU initialization attempts.
    pub webgpu_init_attempts: u64,
    /// Number of WebGPU initialization failures.
    pub webgpu_init_failures: u64,
    /// Number of WebGL2 initialization attempts.
    pub webgl2_init_attempts: u64,
    /// Number of WebGL2 initialization failures.
    pub webgl2_init_failures: u64,
    /// Number of fallbacks to CPU rendering.
    pub fallback_to_cpu_count: u64,
    /// Total uploaded bytes.
    pub uploaded_bytes: u64,
    /// Total upload chunks.
    pub upload_chunks: u64,
    /// Total rendered frames.
    pub rendered_frames: u64,
    /// CPU presentation count.
    pub cpu_present_count: u64,
    /// WebGPU presentation count.
    pub webgpu_present_count: u64,
    /// WebGL2 presentation count.
    pub webgl2_present_count: u64,
    /// Target frame interval in milliseconds.
    pub target_frame_interval_ms: u32,
    /// Number of auto-tune adjustments.
    pub auto_tune_adjustments: u64,
    /// Last render duration in milliseconds.
    pub last_render_duration_ms: u32,
    /// Total render duration in milliseconds.
    pub total_render_duration_ms: u64,
    /// Fallback latency budget in milliseconds.
    pub fallback_latency_budget_ms: u32,
    /// Last fallback latency in milliseconds.
    pub last_fallback_latency_ms: u32,
    /// Number of fallback budget violations.
    pub fallback_budget_violations: u64,
    /// Number of backend transitions.
    pub backend_transition_count: u64,
    /// Last backend transition reason.
    pub last_transition_reason: Option<&'static str>,
    /// Last fallback reason.
    pub last_fallback_reason: Option<&'static str>,
    /// Last backend error code.
    pub last_error_code: Option<BackendErrorCode>,
    /// Last texture format used.
    pub last_texture_format: Option<&'static str>,
    /// Last color space used.
    pub last_color_space: Option<&'static str>,
    /// Last upload chunk size in bytes.
    pub last_upload_chunk_bytes: u64,
    /// Whether the production WebGPU flag is enabled.
    pub webgpu_production_flag_enabled: bool,
    /// Whether the WebGL2 backend feature flag is enabled.
    pub webgl2_backend_flag_enabled: bool,
}

impl Default for BackendSelectionMetrics {
    fn default() -> Self {
        Self {
            probe_attempts: 0,
            webgpu_init_attempts: 0,
            webgpu_init_failures: 0,
            webgl2_init_attempts: 0,
            webgl2_init_failures: 0,
            fallback_to_cpu_count: 0,
            uploaded_bytes: 0,
            upload_chunks: 0,
            rendered_frames: 0,
            cpu_present_count: 0,
            webgpu_present_count: 0,
            webgl2_present_count: 0,
            target_frame_interval_ms: DEFAULT_FRAME_INTERVAL_MS,
            auto_tune_adjustments: 0,
            last_render_duration_ms: 0,
            total_render_duration_ms: 0,
            fallback_latency_budget_ms: fallback_latency_budget_ms(DEFAULT_FRAME_INTERVAL_MS),
            last_fallback_latency_ms: 0,
            fallback_budget_violations: 0,
            backend_transition_count: 0,
            last_transition_reason: Some("startup_cpu"),
            last_fallback_reason: None,
            last_error_code: None,
            last_texture_format: None,
            last_color_space: None,
            last_upload_chunk_bytes: 0,
            webgpu_production_flag_enabled: false,
            webgl2_backend_flag_enabled: false,
        }
    }
}

/// Renderer backend trait for the WASM host runtime.
pub trait WasmRenderBackend {
    /// Initialize the backend with capability probe data.
    fn initialize(&mut self, probe: &BackendCapabilityProbe) -> Result<(), BackendErrorCode>;
    /// Submit a CPU-rendered frame.
    fn submit_cpu_frame(
        &mut self,
        source_format: PixelFormat,
        frame_bytes: usize,
        target_frame_interval_ms: u32,
        limits: &Limits,
    ) -> Result<UploadStats, BackendErrorCode>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Upload statistics for a single frame submission.
pub struct UploadStats {
    /// Total bytes uploaded.
    pub bytes: u64,
    /// Number of chunks uploaded.
    pub chunks: u64,
    /// Chunk size in bytes.
    pub chunk_bytes: u64,
}

/// Calculate the fallback latency budget in milliseconds based on the target frame interval.
pub fn fallback_latency_budget_ms(target_frame_interval_ms: u32) -> u32 {
    let scaled = target_frame_interval_ms.saturating_mul(FALLBACK_LATENCY_BUDGET_FACTOR);
    scaled.clamp(target_frame_interval_ms, FALLBACK_LATENCY_BUDGET_CEILING_MS)
}

/// Select upload chunk size in bytes based on frame size and target frame interval.
pub fn select_upload_chunk_bytes(
    frame_bytes: usize,
    target_frame_interval_ms: u32,
) -> usize {
    if frame_bytes <= 512 * 1024 {
        return SMALL_UPLOAD_CHUNK_BYTES;
    }
    if frame_bytes <= 4 * 1024 * 1024 {
        return MEDIUM_UPLOAD_CHUNK_BYTES;
    }
    if frame_bytes <= 16 * 1024 * 1024 {
        if target_frame_interval_ms <= 24 {
            return LARGE_UPLOAD_CHUNK_BYTES;
        }
        return MEDIUM_UPLOAD_CHUNK_BYTES;
    }
    if target_frame_interval_ms <= 24 {
        LARGE_UPLOAD_CHUNK_BYTES
    } else {
        XL_UPLOAD_CHUNK_BYTES
    }
}

/// Auto-tune the frame interval in milliseconds based on last render duration.
pub fn auto_tune_frame_interval_ms(last_render_duration_ms: u32) -> u32 {
    match last_render_duration_ms {
        0..=12 => 16,
        13..=20 => 24,
        21..=33 => 33,
        34..=50 => 50,
        _ => 66,
    }
}

fn texture_mapping(source_format: PixelFormat) -> Option<(&'static str, &'static str)> {
    match source_format {
        PixelFormat::Luma8 => Some(("r8unorm", "srgb")),
        PixelFormat::Rgba8 => Some(("rgba8unorm-srgb", "srgb")),
        PixelFormat::Luma16 => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CpuBackend;

impl WasmRenderBackend for CpuBackend {
    fn initialize(&mut self, _probe: &BackendCapabilityProbe) -> Result<(), BackendErrorCode> {
        Ok(())
    }

    fn submit_cpu_frame(
        &mut self,
        _source_format: PixelFormat,
        frame_bytes: usize,
        _target_frame_interval_ms: u32,
        _limits: &Limits,
    ) -> Result<UploadStats, BackendErrorCode> {
        Ok(UploadStats {
            bytes: frame_bytes as u64,
            chunks: 1,
            chunk_bytes: frame_bytes as u64,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WebGpuBackend {
    lifecycle: WebGpuLifecycle,
}

impl Default for WebGpuBackend {
    fn default() -> Self {
        Self {
            lifecycle: WebGpuLifecycle::Uninitialized,
        }
    }
}

impl WasmRenderBackend for WebGpuBackend {
    fn initialize(&mut self, probe: &BackendCapabilityProbe) -> Result<(), BackendErrorCode> {
        if !probe.webgpu_api || !probe.adapter_available {
            self.lifecycle = WebGpuLifecycle::Failed;
            return Err(BackendErrorCode::InitFailed);
        }
        self.lifecycle = WebGpuLifecycle::Ready;
        Ok(())
    }

    fn submit_cpu_frame(
        &mut self,
        source_format: PixelFormat,
        frame_bytes: usize,
        target_frame_interval_ms: u32,
        limits: &Limits,
    ) -> Result<UploadStats, BackendErrorCode> {
        if self.lifecycle == WebGpuLifecycle::DeviceLost {
            return Err(BackendErrorCode::DeviceLost);
        }
        if self.lifecycle != WebGpuLifecycle::Ready {
            return Err(BackendErrorCode::InitFailed);
        }
        match source_format {
            PixelFormat::Luma8 | PixelFormat::Rgba8 => {}
            PixelFormat::Luma16 => return Err(BackendErrorCode::SubmitFailed),
        }
        if frame_bytes as u64 > limits.max_gpu_texture_bytes() {
            return Err(BackendErrorCode::SubmitFailed);
        }
        let chunk_bytes = select_upload_chunk_bytes(frame_bytes, target_frame_interval_ms);
        let chunks = frame_bytes.div_ceil(chunk_bytes) as u64;
        Ok(UploadStats {
            bytes: frame_bytes as u64,
            chunks,
            chunk_bytes: chunk_bytes as u64,
        })
    }
}

/// WebGL2 backend state for the WASM host runtime.
///
/// Implements rendering via 2D texture arrays since WebGL2 does not support
/// native 3D textures. The backend tracks lifecycle state for fail-closed
/// fallback to CPU when WebGL2 context is lost or initialization fails.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WebGL2Backend {
    lifecycle: WebGL2Lifecycle,
}

impl Default for WebGL2Backend {
    fn default() -> Self {
        Self {
            lifecycle: WebGL2Lifecycle::Uninitialized,
        }
    }
}

impl WasmRenderBackend for WebGL2Backend {
    fn initialize(&mut self, probe: &BackendCapabilityProbe) -> Result<(), BackendErrorCode> {
        if !probe.webgl2_api {
            self.lifecycle = WebGL2Lifecycle::Failed;
            return Err(BackendErrorCode::WebGL2InitFailed);
        }
        self.lifecycle = WebGL2Lifecycle::Ready;
        Ok(())
    }

    fn submit_cpu_frame(
        &mut self,
        source_format: PixelFormat,
        frame_bytes: usize,
        target_frame_interval_ms: u32,
        limits: &Limits,
    ) -> Result<UploadStats, BackendErrorCode> {
        if self.lifecycle == WebGL2Lifecycle::ContextLost {
            return Err(BackendErrorCode::WebGL2ContextLost);
        }
        if self.lifecycle != WebGL2Lifecycle::Ready {
            return Err(BackendErrorCode::WebGL2InitFailed);
        }
        match source_format {
            PixelFormat::Luma8 | PixelFormat::Rgba8 => {}
            PixelFormat::Luma16 => return Err(BackendErrorCode::SubmitFailed),
        }
        if frame_bytes as u64 > limits.max_gpu_texture_bytes() {
            return Err(BackendErrorCode::SubmitFailed);
        }
        let chunk_bytes = select_upload_chunk_bytes(frame_bytes, target_frame_interval_ms);
        let chunks = frame_bytes.div_ceil(chunk_bytes) as u64;
        Ok(UploadStats {
            bytes: frame_bytes as u64,
            chunks,
            chunk_bytes: chunk_bytes as u64,
        })
    }
}

/// Return the WebGL2 MPR GLSL shader sources as a JSON string.
///
/// The returned JSON has keys `"vert"` and `"frag"` containing the
/// vertex and fragment GLSL shader source strings respectively.
pub fn webgl2_mpr_shader_sources_json() -> String {
    format!(
        "{{\"vert\":\"{}\",\"frag\":\"{}\"}}",
        escape_json_string(WEBGL2_MPR_VERT_GLSL),
        escape_json_string(WEBGL2_MPR_FRAG_GLSL)
    )
}

/// Return the WebGL2 MIP/MinIP GLSL shader sources as a JSON string.
///
/// The returned JSON has keys `"vert"` and `"frag"` containing the
/// vertex and fragment GLSL shader source strings respectively.
pub fn webgl2_mip_shader_sources_json() -> String {
    format!(
        "{{\"vert\":\"{}\",\"frag\":\"{}\"}}",
        escape_json_string(WEBGL2_MIP_VERT_GLSL),
        escape_json_string(WEBGL2_MIP_FRAG_GLSL)
    )
}

/// Return the WebGL2 volume rendering GLSL shader sources as a JSON string.
///
/// The returned JSON has keys `"vert"` and `"frag"` containing the
/// vertex and fragment GLSL shader source strings respectively.
pub fn webgl2_vr_shader_sources_json() -> String {
    format!(
        "{{\"vert\":\"{}\",\"frag\":\"{}\"}}",
        escape_json_string(WEBGL2_VR_VERT_GLSL),
        escape_json_string(WEBGL2_VR_FRAG_GLSL)
    )
}

fn escape_json_string(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureDirective {
    UseCpu {
        reason: &'static str,
        code: Option<BackendErrorCode>,
        webgpu_lifecycle: WebGpuLifecycle,
        webgl2_lifecycle: WebGL2Lifecycle,
    },
    AttemptWebGpuInit,
    AttemptWebGL2Init,
    AttemptWebGpuThenWebGL2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoverDirective {
    KeepCurrent,
    UseCpu {
        reason: &'static str,
        code: Option<BackendErrorCode>,
        webgpu_lifecycle: WebGpuLifecycle,
        webgl2_lifecycle: WebGL2Lifecycle,
    },
    AttemptWebGpuInit,
    AttemptWebGL2Init,
}

struct BackendPolicy;

impl BackendPolicy {
    fn configure_directive(
        force_cpu: bool,
        preferred: &str,
        production_webgpu_enabled: bool,
        webgl2_backend_enabled: bool,
    ) -> ConfigureDirective {
        if force_cpu || preferred.eq_ignore_ascii_case("cpu") {
            return ConfigureDirective::UseCpu {
                reason: "force_cpu_or_cpu_preferred",
                code: None,
                webgpu_lifecycle: WebGpuLifecycle::Uninitialized,
                webgl2_lifecycle: WebGL2Lifecycle::Uninitialized,
            };
        }

        // Explicit WebGL2 preference
        if preferred.eq_ignore_ascii_case("webgl2") {
            if webgl2_backend_enabled {
                return ConfigureDirective::AttemptWebGL2Init;
            }
            return ConfigureDirective::UseCpu {
                reason: "webgl2_backend_flag_disabled",
                code: Some(BackendErrorCode::WebGL2InitFailed),
                webgpu_lifecycle: WebGpuLifecycle::Uninitialized,
                webgl2_lifecycle: WebGL2Lifecycle::Failed,
            };
        }

        // WebGPU preferred: try WebGPU first, fall back to WebGL2 if available
        if production_webgpu_enabled && webgl2_backend_enabled {
            return ConfigureDirective::AttemptWebGpuThenWebGL2;
        }

        if production_webgpu_enabled {
            return ConfigureDirective::AttemptWebGpuInit;
        }

        // WebGPU production flag disabled, try WebGL2
        if webgl2_backend_enabled {
            return ConfigureDirective::AttemptWebGL2Init;
        }

        ConfigureDirective::UseCpu {
            reason: "production_webgpu_flag_disabled",
            code: Some(BackendErrorCode::FlagDisabled),
            webgpu_lifecycle: WebGpuLifecycle::Failed,
            webgl2_lifecycle: WebGL2Lifecycle::Uninitialized,
        }
    }

    fn recover_directive(
        force_cpu: bool,
        webgpu_lifecycle: WebGpuLifecycle,
        webgl2_lifecycle: WebGL2Lifecycle,
        production_webgpu_enabled: bool,
        webgl2_backend_enabled: bool,
    ) -> RecoverDirective {
        if force_cpu {
            return RecoverDirective::UseCpu {
                reason: "recover_force_cpu_enabled",
                code: None,
                webgpu_lifecycle: WebGpuLifecycle::Uninitialized,
                webgl2_lifecycle: WebGL2Lifecycle::Uninitialized,
            };
        }
        if webgpu_lifecycle == WebGpuLifecycle::DeviceLost && production_webgpu_enabled {
            return RecoverDirective::AttemptWebGpuInit;
        }
        if webgl2_lifecycle == WebGL2Lifecycle::ContextLost && webgl2_backend_enabled {
            return RecoverDirective::AttemptWebGL2Init;
        }
        if webgpu_lifecycle != WebGpuLifecycle::DeviceLost
            && webgl2_lifecycle != WebGL2Lifecycle::ContextLost
        {
            return RecoverDirective::KeepCurrent;
        }
        RecoverDirective::UseCpu {
            reason: "recover_no_available_backend",
            code: None,
            webgpu_lifecycle,
            webgl2_lifecycle,
        }
    }

    fn submit_failure_mapping(code: BackendErrorCode) -> (&'static str, WebGpuLifecycle, WebGL2Lifecycle) {
        match code {
            BackendErrorCode::DeviceLost => ("submit_device_lost", WebGpuLifecycle::DeviceLost, WebGL2Lifecycle::Uninitialized),
            BackendErrorCode::SubmitFailed => ("submit_failed", WebGpuLifecycle::Failed, WebGL2Lifecycle::Uninitialized),
            BackendErrorCode::InitFailed => {
                ("submit_init_missing_or_failed", WebGpuLifecycle::Failed, WebGL2Lifecycle::Uninitialized)
            }
            BackendErrorCode::FlagDisabled => ("submit_flag_disabled", WebGpuLifecycle::Failed, WebGL2Lifecycle::Uninitialized),
            BackendErrorCode::WebGL2InitFailed => ("submit_webgl2_init_failed", WebGpuLifecycle::Uninitialized, WebGL2Lifecycle::Failed),
            BackendErrorCode::WebGL2ContextLost => ("submit_webgl2_context_lost", WebGpuLifecycle::Uninitialized, WebGL2Lifecycle::ContextLost),
        }
    }
}

/// Runtime state coordinating backend selection and fail-closed fallback.
///
/// Implements the WebGPU → WebGL2 → CPU cascade: on configure, the runtime
/// attempts WebGPU first. If WebGPU is unavailable or fails, it falls back
/// to WebGL2 (when the `webgl2-backend` feature is enabled and the probe
/// reports WebGL2 availability). If WebGL2 also fails, it falls back to CPU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendRuntimeState {
    active: RendererBackend,
    force_cpu: bool,
    production_webgpu_enabled: bool,
    webgl2_backend_enabled: bool,
    probe: BackendCapabilityProbe,
    metrics: BackendSelectionMetrics,
    webgpu_lifecycle: WebGpuLifecycle,
    webgl2_lifecycle: WebGL2Lifecycle,
    cpu: CpuBackend,
    webgpu: WebGpuBackend,
    webgl2: WebGL2Backend,
}

impl Default for BackendRuntimeState {
    fn default() -> Self {
        Self {
            active: RendererBackend::Cpu,
            force_cpu: false,
            production_webgpu_enabled: false,
            webgl2_backend_enabled: cfg!(feature = "webgl2-backend"),
            probe: BackendCapabilityProbe::default(),
            metrics: BackendSelectionMetrics::default(),
            webgpu_lifecycle: WebGpuLifecycle::Uninitialized,
            webgl2_lifecycle: WebGL2Lifecycle::Uninitialized,
            cpu: CpuBackend,
            webgpu: WebGpuBackend::default(),
            webgl2: WebGL2Backend::default(),
        }
    }
}

impl BackendRuntimeState {
    fn transition_backend(&mut self, next: RendererBackend, reason: &'static str) {
        if self.active != next {
            self.metrics.backend_transition_count =
                self.metrics.backend_transition_count.saturating_add(1);
        }
        self.active = next;
        self.metrics.last_transition_reason = Some(reason);
    }

    fn record_fallback_to_cpu(
        &mut self,
        reason: &'static str,
        code: BackendErrorCode,
        webgpu_lifecycle: WebGpuLifecycle,
        webgl2_lifecycle: WebGL2Lifecycle,
    ) {
        self.metrics.last_error_code = Some(code);
        self.metrics.last_fallback_reason = Some(reason);
        self.metrics.fallback_to_cpu_count = self.metrics.fallback_to_cpu_count.saturating_add(1);
        self.webgpu_lifecycle = webgpu_lifecycle;
        self.webgl2_lifecycle = webgl2_lifecycle;
        let budget = fallback_latency_budget_ms(self.metrics.target_frame_interval_ms);
        self.metrics.fallback_latency_budget_ms = budget;
        self.metrics.last_fallback_latency_ms = self.metrics.target_frame_interval_ms;
        if self.metrics.last_fallback_latency_ms > budget {
            self.metrics.fallback_budget_violations =
                self.metrics.fallback_budget_violations.saturating_add(1);
        }
        self.transition_backend(RendererBackend::Cpu, reason);
    }

    fn record_fallback(
        &mut self,
        reason: &'static str,
        code: BackendErrorCode,
        lifecycle: WebGpuLifecycle,
    ) {
        self.record_fallback_to_cpu(reason, code, lifecycle, self.webgl2_lifecycle);
    }

    fn apply_auto_tune_policy(&mut self, render_duration_ms: u32) {
        let tuned = auto_tune_frame_interval_ms(render_duration_ms);
        if tuned != self.metrics.target_frame_interval_ms {
            self.metrics.target_frame_interval_ms = tuned;
            self.metrics.auto_tune_adjustments =
                self.metrics.auto_tune_adjustments.saturating_add(1);
        }
        self.metrics.fallback_latency_budget_ms = fallback_latency_budget_ms(tuned);
    }

    /// Configure the backend with the given preference and capability probe.
    ///
    /// Implements the cascade: WebGPU → WebGL2 → CPU.
    /// When `preferred` is `"webgpu"`, the runtime first attempts WebGPU init.
    /// If that fails and WebGL2 is available, it attempts WebGL2 init.
    /// If both fail, it falls back to CPU.
    pub fn configure_backend(
        &mut self,
        preferred: &str,
        probe: BackendCapabilityProbe,
        limits: &Limits,
    ) -> RendererBackend {
        self.metrics.probe_attempts = self.metrics.probe_attempts.saturating_add(1);
        self.probe = probe;
        self.metrics.webgpu_production_flag_enabled = self.production_webgpu_enabled;
        self.metrics.webgl2_backend_flag_enabled = self.webgl2_backend_enabled;

        let directive = BackendPolicy::configure_directive(
            self.force_cpu,
            preferred,
            self.production_webgpu_enabled,
            self.webgl2_backend_enabled,
        );

        match directive {
            ConfigureDirective::UseCpu {
                reason,
                code,
                webgpu_lifecycle,
                webgl2_lifecycle,
            } => {
                if let Some(code) = code {
                    self.record_fallback_to_cpu(reason, code, webgpu_lifecycle, webgl2_lifecycle);
                } else {
                    self.webgpu_lifecycle = webgpu_lifecycle;
                    self.webgl2_lifecycle = webgl2_lifecycle;
                    self.transition_backend(RendererBackend::Cpu, reason);
                }
                let _ = self.cpu.initialize(&self.probe);
                return self.active;
            }
            ConfigureDirective::AttemptWebGpuInit => {
                self.metrics.webgpu_init_attempts = self.metrics.webgpu_init_attempts.saturating_add(1);
                match self.webgpu.initialize(&self.probe) {
                    Ok(()) => {
                        self.webgpu_lifecycle = WebGpuLifecycle::Ready;
                        self.transition_backend(RendererBackend::WebGpu, "configure_webgpu_success");
                        self.metrics.last_error_code = None;
                    }
                    Err(code) => {
                        self.metrics.webgpu_init_failures =
                            self.metrics.webgpu_init_failures.saturating_add(1);
                        self.record_fallback(
                            "configure_webgpu_init_failed",
                            code,
                            WebGpuLifecycle::Failed,
                        );
                        let _ = self.cpu.initialize(&self.probe);
                    }
                }
            }
            ConfigureDirective::AttemptWebGL2Init => {
                self.metrics.webgl2_init_attempts = self.metrics.webgl2_init_attempts.saturating_add(1);
                match self.webgl2.initialize(&self.probe) {
                    Ok(()) => {
                        self.webgl2_lifecycle = WebGL2Lifecycle::Ready;
                        self.transition_backend(RendererBackend::WebGL2, "configure_webgl2_success");
                        self.metrics.last_error_code = None;
                    }
                    Err(code) => {
                        self.metrics.webgl2_init_failures =
                            self.metrics.webgl2_init_failures.saturating_add(1);
                        self.record_fallback_to_cpu(
                            "configure_webgl2_init_failed",
                            code,
                            self.webgpu_lifecycle,
                            WebGL2Lifecycle::Failed,
                        );
                        let _ = self.cpu.initialize(&self.probe);
                    }
                }
            }
            ConfigureDirective::AttemptWebGpuThenWebGL2 => {
                // Try WebGPU first
                self.metrics.webgpu_init_attempts = self.metrics.webgpu_init_attempts.saturating_add(1);
                match self.webgpu.initialize(&self.probe) {
                    Ok(()) => {
                        self.webgpu_lifecycle = WebGpuLifecycle::Ready;
                        self.transition_backend(RendererBackend::WebGpu, "configure_webgpu_success");
                        self.metrics.last_error_code = None;
                    }
                    Err(gpu_code) => {
                        self.metrics.webgpu_init_failures =
                            self.metrics.webgpu_init_failures.saturating_add(1);
                        self.webgpu_lifecycle = WebGpuLifecycle::Failed;

                        // WebGPU failed, try WebGL2
                        self.metrics.webgl2_init_attempts = self.metrics.webgl2_init_attempts.saturating_add(1);
                        match self.webgl2.initialize(&self.probe) {
                            Ok(()) => {
                                self.webgl2_lifecycle = WebGL2Lifecycle::Ready;
                                self.transition_backend(
                                    RendererBackend::WebGL2,
                                    "configure_webgpu_failed_webgl2_success",
                                );
                                self.metrics.last_error_code = Some(gpu_code);
                            }
                            Err(gl2_code) => {
                                self.metrics.webgl2_init_failures =
                                    self.metrics.webgl2_init_failures.saturating_add(1);
                                self.record_fallback_to_cpu(
                                    "configure_webgpu_and_webgl2_init_failed",
                                    gl2_code,
                                    WebGpuLifecycle::Failed,
                                    WebGL2Lifecycle::Failed,
                                );
                                let _ = self.cpu.initialize(&self.probe);
                            }
                        }
                    }
                }
            }
        }

        if limits.max_gpu_texture_bytes() == 0 && self.active != RendererBackend::Cpu {
            self.record_fallback_to_cpu(
                "configure_limits_max_gpu_texture_zero",
                BackendErrorCode::SubmitFailed,
                WebGpuLifecycle::Failed,
                WebGL2Lifecycle::Failed,
            );
        }

        self.active
    }

    pub fn record_present_with_timing(
        &mut self,
        source_format: PixelFormat,
        frame_bytes: usize,
        limits: &Limits,
        render_duration_ms: u32,
    ) {
        let result = match self.active {
            RendererBackend::Cpu => self.cpu.submit_cpu_frame(
                source_format,
                frame_bytes,
                self.metrics.target_frame_interval_ms,
                limits,
            ),
            RendererBackend::WebGpu => self.webgpu.submit_cpu_frame(
                source_format,
                frame_bytes,
                self.metrics.target_frame_interval_ms,
                limits,
            ),
            RendererBackend::WebGL2 => self.webgl2.submit_cpu_frame(
                source_format,
                frame_bytes,
                self.metrics.target_frame_interval_ms,
                limits,
            ),
        };

        match result {
            Ok(stats) => {
                self.metrics.uploaded_bytes =
                    self.metrics.uploaded_bytes.saturating_add(stats.bytes);
                self.metrics.upload_chunks =
                    self.metrics.upload_chunks.saturating_add(stats.chunks);
                self.metrics.last_upload_chunk_bytes = stats.chunk_bytes;
                self.metrics.rendered_frames = self.metrics.rendered_frames.saturating_add(1);
                self.metrics.last_render_duration_ms = render_duration_ms;
                self.metrics.total_render_duration_ms = self
                    .metrics
                    .total_render_duration_ms
                    .saturating_add(render_duration_ms as u64);
                match self.active {
                    RendererBackend::Cpu => {
                        self.metrics.cpu_present_count =
                            self.metrics.cpu_present_count.saturating_add(1)
                    }
                    RendererBackend::WebGpu => {
                        self.metrics.webgpu_present_count =
                            self.metrics.webgpu_present_count.saturating_add(1)
                    }
                    RendererBackend::WebGL2 => {
                        self.metrics.webgl2_present_count =
                            self.metrics.webgl2_present_count.saturating_add(1)
                    }
                }
                if let Some((texture_format, color_space)) = texture_mapping(source_format) {
                    self.metrics.last_texture_format = Some(texture_format);
                    self.metrics.last_color_space = Some(color_space);
                }
                self.apply_auto_tune_policy(render_duration_ms);
            }
            Err(code) => {
                let (reason, gpu_lifecycle, gl2_lifecycle) = BackendPolicy::submit_failure_mapping(code);
                self.record_fallback_to_cpu(reason, code, gpu_lifecycle, gl2_lifecycle);
            }
        }
    }

    pub fn active_backend(&self) -> RendererBackend {
        self.active
    }

    pub fn force_cpu(&mut self, enabled: bool) {
        self.force_cpu = enabled;
        if enabled {
            self.transition_backend(RendererBackend::Cpu, "force_cpu_enabled");
        }
    }

    pub fn set_production_webgpu_enabled(&mut self, enabled: bool) {
        self.production_webgpu_enabled = enabled;
        self.metrics.webgpu_production_flag_enabled = enabled;
    }

    pub fn production_webgpu_enabled(&self) -> bool {
        self.production_webgpu_enabled
    }

    /// Enable or disable the WebGL2 backend feature flag.
    pub fn set_webgl2_backend_enabled(&mut self, enabled: bool) {
        self.webgl2_backend_enabled = enabled;
        self.metrics.webgl2_backend_flag_enabled = enabled;
    }

    /// Return whether the WebGL2 backend feature flag is enabled.
    pub fn webgl2_backend_enabled(&self) -> bool {
        self.webgl2_backend_enabled
    }

    pub fn set_target_frame_interval_ms(&mut self, interval_ms: u32) -> bool {
        if interval_ms == 0 || interval_ms > 1000 {
            return false;
        }
        self.metrics.target_frame_interval_ms = interval_ms;
        self.metrics.fallback_latency_budget_ms = fallback_latency_budget_ms(interval_ms);
        true
    }

    pub fn target_frame_interval_ms(&self) -> u32 {
        self.metrics.target_frame_interval_ms
    }

    pub fn simulate_device_lost(&mut self) -> bool {
        if self.active == RendererBackend::WebGpu {
            self.record_fallback_to_cpu(
                "simulate_device_lost",
                BackendErrorCode::DeviceLost,
                WebGpuLifecycle::DeviceLost,
                self.webgl2_lifecycle,
            );
            return true;
        }
        if self.active == RendererBackend::WebGL2 {
            self.record_fallback_to_cpu(
                "simulate_webgl2_context_lost",
                BackendErrorCode::WebGL2ContextLost,
                self.webgpu_lifecycle,
                WebGL2Lifecycle::ContextLost,
            );
            return true;
        }
        false
    }

    pub fn recover_device_lost(&mut self, limits: &Limits) -> RendererBackend {
        let directive = BackendPolicy::recover_directive(
            self.force_cpu,
            self.webgpu_lifecycle,
            self.webgl2_lifecycle,
            self.production_webgpu_enabled,
            self.webgl2_backend_enabled,
        );

        match directive {
            RecoverDirective::KeepCurrent => return self.active,
            RecoverDirective::UseCpu {
                reason,
                code,
                webgpu_lifecycle,
                webgl2_lifecycle,
            } => {
                if let Some(code) = code {
                    self.record_fallback_to_cpu(reason, code, webgpu_lifecycle, webgl2_lifecycle);
                } else {
                    self.webgpu_lifecycle = webgpu_lifecycle;
                    self.webgl2_lifecycle = webgl2_lifecycle;
                    self.transition_backend(RendererBackend::Cpu, reason);
                }
                return self.active;
            }
            RecoverDirective::AttemptWebGpuInit => {
                self.metrics.webgpu_init_attempts = self.metrics.webgpu_init_attempts.saturating_add(1);
                match self.webgpu.initialize(&self.probe) {
                    Ok(()) if limits.max_gpu_texture_bytes() > 0 => {
                        self.webgpu_lifecycle = WebGpuLifecycle::Ready;
                        self.metrics.last_error_code = None;
                        self.transition_backend(RendererBackend::WebGpu, "recover_device_lost_success");
                    }
                    Ok(()) | Err(_) => {
                        self.metrics.webgpu_init_failures =
                            self.metrics.webgpu_init_failures.saturating_add(1);
                        // WebGPU recovery failed, try WebGL2 if available
                        if self.webgl2_backend_enabled {
                            self.metrics.webgl2_init_attempts =
                                self.metrics.webgl2_init_attempts.saturating_add(1);
                            match self.webgl2.initialize(&self.probe) {
                                Ok(()) if limits.max_gpu_texture_bytes() > 0 => {
                                    self.webgl2_lifecycle = WebGL2Lifecycle::Ready;
                                    self.metrics.last_error_code = None;
                                    self.transition_backend(
                                        RendererBackend::WebGL2,
                                        "recover_webgpu_failed_webgl2_success",
                                    );
                                }
                                Ok(()) | Err(_) => {
                                    self.metrics.webgl2_init_failures =
                                        self.metrics.webgl2_init_failures.saturating_add(1);
                                    self.record_fallback_to_cpu(
                                        "recover_webgpu_and_webgl2_failed",
                                        BackendErrorCode::InitFailed,
                                        WebGpuLifecycle::Failed,
                                        WebGL2Lifecycle::Failed,
                                    );
                                }
                            }
                        } else {
                            self.record_fallback_to_cpu(
                                "recover_device_lost_init_failed",
                                BackendErrorCode::InitFailed,
                                WebGpuLifecycle::Failed,
                                self.webgl2_lifecycle,
                            );
                        }
                    }
                }
            }
            RecoverDirective::AttemptWebGL2Init => {
                self.metrics.webgl2_init_attempts = self.metrics.webgl2_init_attempts.saturating_add(1);
                match self.webgl2.initialize(&self.probe) {
                    Ok(()) if limits.max_gpu_texture_bytes() > 0 => {
                        self.webgl2_lifecycle = WebGL2Lifecycle::Ready;
                        self.metrics.last_error_code = None;
                        self.transition_backend(
                            RendererBackend::WebGL2,
                            "recover_webgl2_context_lost_success",
                        );
                    }
                    Ok(()) | Err(_) => {
                        self.metrics.webgl2_init_failures =
                            self.metrics.webgl2_init_failures.saturating_add(1);
                        self.record_fallback_to_cpu(
                            "recover_webgl2_context_lost_init_failed",
                            BackendErrorCode::WebGL2InitFailed,
                            self.webgpu_lifecycle,
                            WebGL2Lifecycle::Failed,
                        );
                    }
                }
            }
        }
        self.active
    }

    /// Simulate WebGL2 context loss for deterministic recovery testing.
    pub fn simulate_webgl2_context_lost(&mut self) -> bool {
        if self.active != RendererBackend::WebGL2 {
            return false;
        }
        self.record_fallback_to_cpu(
            "simulate_webgl2_context_lost",
            BackendErrorCode::WebGL2ContextLost,
            self.webgpu_lifecycle,
            WebGL2Lifecycle::ContextLost,
        );
        true
    }

    pub fn csp_safe_shader_source(&self) -> &'static str {
        WEBGPU_SHADER_WGSL
    }

    /// Return whether GPU volume rendering is available.
    ///
    /// Volume rendering requires an active WebGPU or WebGL2 backend.
    /// Returns `true` when the active backend is WebGPU (lifecycle Ready)
    /// or WebGL2 (lifecycle Ready).
    pub fn volume_rendering_available(&self) -> bool {
        (self.active == RendererBackend::WebGpu && self.webgpu_lifecycle == WebGpuLifecycle::Ready)
            || (self.active == RendererBackend::WebGL2
                && self.webgl2_lifecycle == WebGL2Lifecycle::Ready)
    }

    /// Return WebGL2 shader sources for all render modes as a JSON string.
    ///
    /// Contains `"mpr"`, `"mip"`, and `"vr"` keys, each with `"vert"` and `"frag"`
    /// sub-keys containing the GLSL ES 3.00 shader source strings.
    pub fn webgl2_shader_sources_json(&self) -> String {
        format!(
            "{{\"mpr\":{},\"mip\":{},\"vr\":{}}}",
            webgl2_mpr_shader_sources_json(),
            webgl2_mip_shader_sources_json(),
            webgl2_vr_shader_sources_json()
        )
    }

    pub fn probe_json(&self) -> String {
        format!(
            "{{\"webgpu_api\":{},\"adapter_available\":{},\"webgl2_api\":{},\"max_texture_dimension_2d\":{}}}",
            self.probe.webgpu_api,
            self.probe.adapter_available,
            self.probe.webgl2_api,
            self.probe.max_texture_dimension_2d
        )
    }

    pub fn metrics_json(&self) -> String {
        let last_error = self
            .metrics
            .last_error_code
            .map(|code| format!("\"{}\"", code.as_code()))
            .unwrap_or_else(|| "null".to_string());
        let last_texture_format = self
            .metrics
            .last_texture_format
            .map(|value| format!("\"{}\"", value))
            .unwrap_or_else(|| "null".to_string());
        let last_color_space = self
            .metrics
            .last_color_space
            .map(|value| format!("\"{}\"", value))
            .unwrap_or_else(|| "null".to_string());
        let last_transition_reason = self
            .metrics
            .last_transition_reason
            .map(|value| format!("\"{}\"", value))
            .unwrap_or_else(|| "null".to_string());
        let last_fallback_reason = self
            .metrics
            .last_fallback_reason
            .map(|value| format!("\"{}\"", value))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"active_backend\":\"{}\",\"probe_attempts\":{},\"webgpu_init_attempts\":{},\"webgpu_init_failures\":{},\"webgl2_init_attempts\":{},\"webgl2_init_failures\":{},\"fallback_to_cpu_count\":{},\"uploaded_bytes\":{},\"upload_chunks\":{},\"rendered_frames\":{},\"cpu_present_count\":{},\"webgpu_present_count\":{},\"webgl2_present_count\":{},\"target_frame_interval_ms\":{},\"auto_tune_adjustments\":{},\"fallback_latency_budget_ms\":{},\"last_fallback_latency_ms\":{},\"fallback_budget_violations\":{},\"backend_transition_count\":{},\"last_transition_reason\":{},\"last_fallback_reason\":{},\"last_render_duration_ms\":{},\"total_render_duration_ms\":{},\"webgpu_lifecycle\":\"{:?}\",\"webgl2_lifecycle\":\"{:?}\",\"last_error_code\":{},\"last_texture_format\":{},\"last_color_space\":{},\"last_upload_chunk_bytes\":{},\"webgpu_production_flag_enabled\":{},\"webgl2_backend_flag_enabled\":{}}}",
            self.active.as_str(),
            self.metrics.probe_attempts,
            self.metrics.webgpu_init_attempts,
            self.metrics.webgpu_init_failures,
            self.metrics.webgl2_init_attempts,
            self.metrics.webgl2_init_failures,
            self.metrics.fallback_to_cpu_count,
            self.metrics.uploaded_bytes,
            self.metrics.upload_chunks,
            self.metrics.rendered_frames,
            self.metrics.cpu_present_count,
            self.metrics.webgpu_present_count,
            self.metrics.webgl2_present_count,
            self.metrics.target_frame_interval_ms,
            self.metrics.auto_tune_adjustments,
            self.metrics.fallback_latency_budget_ms,
            self.metrics.last_fallback_latency_ms,
            self.metrics.fallback_budget_violations,
            self.metrics.backend_transition_count,
            last_transition_reason,
            last_fallback_reason,
            self.metrics.last_render_duration_ms,
            self.metrics.total_render_duration_ms,
            self.webgpu_lifecycle,
            self.webgl2_lifecycle,
            last_error,
            last_texture_format,
            last_color_space,
            self.metrics.last_upload_chunk_bytes,
            self.metrics.webgpu_production_flag_enabled,
            self.metrics.webgl2_backend_flag_enabled,
        )
    }
}
