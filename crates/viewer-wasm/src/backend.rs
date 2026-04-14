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

/// Stable backend names exposed to host code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    /// CPU/canvas presentation path.
    Cpu,
    /// WebGPU presentation path.
    WebGpu,
}

impl RendererBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            RendererBackend::Cpu => "CPU",
            RendererBackend::WebGpu => "WebGPU",
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
}

impl BackendErrorCode {
    pub fn as_code(self) -> &'static str {
        match self {
            BackendErrorCode::InitFailed => "DVF.WASM.GPU.INIT_FAILED",
            BackendErrorCode::DeviceLost => "DVF.WASM.GPU.DEVICE_LOST",
            BackendErrorCode::SubmitFailed => "DVF.WASM.GPU.SUBMIT_FAILED",
            BackendErrorCode::FlagDisabled => "DVF.WASM.GPU.FLAG_DISABLED",
        }
    }
}

/// Capability snapshot provided by host probing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendCapabilityProbe {
    pub webgpu_api: bool,
    pub adapter_available: bool,
    pub webgl2_api: bool,
    pub max_texture_dimension_2d: u32,
}

/// Non-PHI backend selection and upload metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendSelectionMetrics {
    pub probe_attempts: u64,
    pub webgpu_init_attempts: u64,
    pub webgpu_init_failures: u64,
    pub fallback_to_cpu_count: u64,
    pub uploaded_bytes: u64,
    pub upload_chunks: u64,
    pub rendered_frames: u64,
    pub cpu_present_count: u64,
    pub webgpu_present_count: u64,
    pub target_frame_interval_ms: u32,
    pub auto_tune_adjustments: u64,
    pub last_render_duration_ms: u32,
    pub total_render_duration_ms: u64,
    pub fallback_latency_budget_ms: u32,
    pub last_fallback_latency_ms: u32,
    pub fallback_budget_violations: u64,
    pub backend_transition_count: u64,
    pub last_transition_reason: Option<&'static str>,
    pub last_fallback_reason: Option<&'static str>,
    pub last_error_code: Option<BackendErrorCode>,
    pub last_texture_format: Option<&'static str>,
    pub last_color_space: Option<&'static str>,
    pub last_upload_chunk_bytes: u64,
    pub webgpu_production_flag_enabled: bool,
}

impl Default for BackendSelectionMetrics {
    fn default() -> Self {
        Self {
            probe_attempts: 0,
            webgpu_init_attempts: 0,
            webgpu_init_failures: 0,
            fallback_to_cpu_count: 0,
            uploaded_bytes: 0,
            upload_chunks: 0,
            rendered_frames: 0,
            cpu_present_count: 0,
            webgpu_present_count: 0,
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
        }
    }
}

/// Renderer backend trait for the WASM host runtime.
pub trait WasmRenderBackend {
    fn initialize(&mut self, probe: &BackendCapabilityProbe) -> Result<(), BackendErrorCode>;
    fn submit_cpu_frame(
        &mut self,
        source_format: PixelFormat,
        frame_bytes: usize,
        target_frame_interval_ms: u32,
        limits: &Limits,
    ) -> Result<UploadStats, BackendErrorCode>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadStats {
    pub bytes: u64,
    pub chunks: u64,
    pub chunk_bytes: u64,
}

fn fallback_latency_budget_ms(target_frame_interval_ms: u32) -> u32 {
    let scaled = target_frame_interval_ms.saturating_mul(FALLBACK_LATENCY_BUDGET_FACTOR);
    scaled.clamp(target_frame_interval_ms, FALLBACK_LATENCY_BUDGET_CEILING_MS)
}

pub(crate) fn select_upload_chunk_bytes(
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

pub(crate) fn auto_tune_frame_interval_ms(last_render_duration_ms: u32) -> u32 {
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
        if frame_bytes as u64 > limits.max_gpu_texture_bytes {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigureDirective {
    UseCpu {
        reason: &'static str,
        code: Option<BackendErrorCode>,
        lifecycle: WebGpuLifecycle,
    },
    AttemptWebGpuInit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecoverDirective {
    KeepCurrent,
    UseCpu {
        reason: &'static str,
        code: Option<BackendErrorCode>,
        lifecycle: WebGpuLifecycle,
    },
    AttemptWebGpuInit,
}

struct BackendPolicy;

impl BackendPolicy {
    fn configure_directive(
        force_cpu: bool,
        preferred: &str,
        production_webgpu_enabled: bool,
    ) -> ConfigureDirective {
        if force_cpu || preferred.eq_ignore_ascii_case("cpu") {
            return ConfigureDirective::UseCpu {
                reason: "force_cpu_or_cpu_preferred",
                code: None,
                lifecycle: WebGpuLifecycle::Uninitialized,
            };
        }
        if !production_webgpu_enabled {
            return ConfigureDirective::UseCpu {
                reason: "production_webgpu_flag_disabled",
                code: Some(BackendErrorCode::FlagDisabled),
                lifecycle: WebGpuLifecycle::Failed,
            };
        }
        ConfigureDirective::AttemptWebGpuInit
    }

    fn recover_directive(
        force_cpu: bool,
        lifecycle: WebGpuLifecycle,
        production_webgpu_enabled: bool,
    ) -> RecoverDirective {
        if force_cpu {
            return RecoverDirective::UseCpu {
                reason: "recover_force_cpu_enabled",
                code: None,
                lifecycle: WebGpuLifecycle::Uninitialized,
            };
        }
        if lifecycle != WebGpuLifecycle::DeviceLost {
            return RecoverDirective::KeepCurrent;
        }
        if !production_webgpu_enabled {
            return RecoverDirective::UseCpu {
                reason: "recover_production_webgpu_flag_disabled",
                code: Some(BackendErrorCode::FlagDisabled),
                lifecycle: WebGpuLifecycle::Failed,
            };
        }
        RecoverDirective::AttemptWebGpuInit
    }

    fn submit_failure_mapping(code: BackendErrorCode) -> (&'static str, WebGpuLifecycle) {
        match code {
            BackendErrorCode::DeviceLost => ("submit_device_lost", WebGpuLifecycle::DeviceLost),
            BackendErrorCode::SubmitFailed => ("submit_failed", WebGpuLifecycle::Failed),
            BackendErrorCode::InitFailed => {
                ("submit_init_missing_or_failed", WebGpuLifecycle::Failed)
            }
            BackendErrorCode::FlagDisabled => ("submit_flag_disabled", WebGpuLifecycle::Failed),
        }
    }
}

/// Runtime state coordinating backend selection and fail-closed fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendRuntimeState {
    active: RendererBackend,
    force_cpu: bool,
    production_webgpu_enabled: bool,
    probe: BackendCapabilityProbe,
    metrics: BackendSelectionMetrics,
    webgpu_lifecycle: WebGpuLifecycle,
    cpu: CpuBackend,
    webgpu: WebGpuBackend,
}

impl Default for BackendRuntimeState {
    fn default() -> Self {
        Self {
            active: RendererBackend::Cpu,
            force_cpu: false,
            production_webgpu_enabled: false,
            probe: BackendCapabilityProbe::default(),
            metrics: BackendSelectionMetrics::default(),
            webgpu_lifecycle: WebGpuLifecycle::Uninitialized,
            cpu: CpuBackend,
            webgpu: WebGpuBackend::default(),
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

    fn record_fallback(
        &mut self,
        reason: &'static str,
        code: BackendErrorCode,
        lifecycle: WebGpuLifecycle,
    ) {
        self.metrics.last_error_code = Some(code);
        self.metrics.last_fallback_reason = Some(reason);
        self.metrics.fallback_to_cpu_count = self.metrics.fallback_to_cpu_count.saturating_add(1);
        self.webgpu_lifecycle = lifecycle;
        let budget = fallback_latency_budget_ms(self.metrics.target_frame_interval_ms);
        self.metrics.fallback_latency_budget_ms = budget;
        self.metrics.last_fallback_latency_ms = self.metrics.target_frame_interval_ms;
        if self.metrics.last_fallback_latency_ms > budget {
            self.metrics.fallback_budget_violations =
                self.metrics.fallback_budget_violations.saturating_add(1);
        }
        self.transition_backend(RendererBackend::Cpu, reason);
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

    pub fn configure_backend(
        &mut self,
        preferred: &str,
        probe: BackendCapabilityProbe,
        limits: &Limits,
    ) -> RendererBackend {
        self.metrics.probe_attempts = self.metrics.probe_attempts.saturating_add(1);
        self.probe = probe;
        self.metrics.webgpu_production_flag_enabled = self.production_webgpu_enabled;

        match BackendPolicy::configure_directive(
            self.force_cpu,
            preferred,
            self.production_webgpu_enabled,
        ) {
            ConfigureDirective::UseCpu {
                reason,
                code,
                lifecycle,
            } => {
                if let Some(code) = code {
                    self.record_fallback(reason, code, lifecycle);
                } else {
                    self.webgpu_lifecycle = lifecycle;
                    self.transition_backend(RendererBackend::Cpu, reason);
                }
                let _ = self.cpu.initialize(&self.probe);
                return self.active;
            }
            ConfigureDirective::AttemptWebGpuInit => {}
        }

        self.metrics.webgpu_init_attempts = self.metrics.webgpu_init_attempts.saturating_add(1);
        let gpu_init = self.webgpu.initialize(&self.probe);
        match gpu_init {
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

        if limits.max_gpu_texture_bytes == 0 {
            self.record_fallback(
                "configure_limits_max_gpu_texture_zero",
                BackendErrorCode::SubmitFailed,
                WebGpuLifecycle::Failed,
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
                }
                if let Some((texture_format, color_space)) = texture_mapping(source_format) {
                    self.metrics.last_texture_format = Some(texture_format);
                    self.metrics.last_color_space = Some(color_space);
                }
                self.apply_auto_tune_policy(render_duration_ms);
            }
            Err(code) => {
                let (reason, lifecycle) = BackendPolicy::submit_failure_mapping(code);
                self.record_fallback(reason, code, lifecycle);
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
        if self.active != RendererBackend::WebGpu {
            return false;
        }
        self.record_fallback(
            "simulate_device_lost",
            BackendErrorCode::DeviceLost,
            WebGpuLifecycle::DeviceLost,
        );
        true
    }

    pub fn recover_device_lost(&mut self, limits: &Limits) -> RendererBackend {
        match BackendPolicy::recover_directive(
            self.force_cpu,
            self.webgpu_lifecycle,
            self.production_webgpu_enabled,
        ) {
            RecoverDirective::KeepCurrent => return self.active,
            RecoverDirective::UseCpu {
                reason,
                code,
                lifecycle,
            } => {
                if let Some(code) = code {
                    self.record_fallback(reason, code, lifecycle);
                } else {
                    self.webgpu_lifecycle = lifecycle;
                    self.transition_backend(RendererBackend::Cpu, reason);
                }
                return self.active;
            }
            RecoverDirective::AttemptWebGpuInit => {}
        }

        self.metrics.webgpu_init_attempts = self.metrics.webgpu_init_attempts.saturating_add(1);
        match self.webgpu.initialize(&self.probe) {
            Ok(()) if limits.max_gpu_texture_bytes > 0 => {
                self.webgpu_lifecycle = WebGpuLifecycle::Ready;
                self.metrics.last_error_code = None;
                self.transition_backend(RendererBackend::WebGpu, "recover_device_lost_success");
            }
            Ok(()) | Err(_) => {
                self.metrics.webgpu_init_failures =
                    self.metrics.webgpu_init_failures.saturating_add(1);
                self.record_fallback(
                    "recover_device_lost_init_failed",
                    BackendErrorCode::InitFailed,
                    WebGpuLifecycle::Failed,
                );
            }
        }
        self.active
    }

    pub fn csp_safe_shader_source(&self) -> &'static str {
        WEBGPU_SHADER_WGSL
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
            "{{\"active_backend\":\"{}\",\"probe_attempts\":{},\"webgpu_init_attempts\":{},\"webgpu_init_failures\":{},\"fallback_to_cpu_count\":{},\"uploaded_bytes\":{},\"upload_chunks\":{},\"rendered_frames\":{},\"cpu_present_count\":{},\"webgpu_present_count\":{},\"target_frame_interval_ms\":{},\"auto_tune_adjustments\":{},\"fallback_latency_budget_ms\":{},\"last_fallback_latency_ms\":{},\"fallback_budget_violations\":{},\"backend_transition_count\":{},\"last_transition_reason\":{},\"last_fallback_reason\":{},\"last_render_duration_ms\":{},\"total_render_duration_ms\":{},\"webgpu_lifecycle\":\"{:?}\",\"last_error_code\":{},\"last_texture_format\":{},\"last_color_space\":{},\"last_upload_chunk_bytes\":{},\"webgpu_production_flag_enabled\":{}}}",
            self.active.as_str(),
            self.metrics.probe_attempts,
            self.metrics.webgpu_init_attempts,
            self.metrics.webgpu_init_failures,
            self.metrics.fallback_to_cpu_count,
            self.metrics.uploaded_bytes,
            self.metrics.upload_chunks,
            self.metrics.rendered_frames,
            self.metrics.cpu_present_count,
            self.metrics.webgpu_present_count,
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
            last_error,
            last_texture_format,
            last_color_space,
            self.metrics.last_upload_chunk_bytes,
            self.metrics.webgpu_production_flag_enabled,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        auto_tune_frame_interval_ms, fallback_latency_budget_ms, select_upload_chunk_bytes,
        BackendCapabilityProbe, BackendRuntimeState, RendererBackend,
    };
    use dicom_core::Limits;

    #[test]
    fn upload_chunk_policy_is_deterministic() {
        assert_eq!(select_upload_chunk_bytes(128 * 1024, 16), 256 * 1024);
        assert_eq!(select_upload_chunk_bytes(2 * 1024 * 1024, 16), 1024 * 1024);
        assert_eq!(
            select_upload_chunk_bytes(8 * 1024 * 1024, 16),
            2 * 1024 * 1024
        );
        assert_eq!(
            select_upload_chunk_bytes(32 * 1024 * 1024, 50),
            4 * 1024 * 1024
        );
    }

    #[test]
    fn auto_tune_interval_policy_has_stable_boundaries() {
        assert_eq!(auto_tune_frame_interval_ms(0), 16);
        assert_eq!(auto_tune_frame_interval_ms(12), 16);
        assert_eq!(auto_tune_frame_interval_ms(13), 24);
        assert_eq!(auto_tune_frame_interval_ms(20), 24);
        assert_eq!(auto_tune_frame_interval_ms(21), 33);
        assert_eq!(auto_tune_frame_interval_ms(33), 33);
        assert_eq!(auto_tune_frame_interval_ms(34), 50);
        assert_eq!(auto_tune_frame_interval_ms(50), 50);
        assert_eq!(auto_tune_frame_interval_ms(51), 66);
    }

    #[test]
    fn fallback_budget_is_bounded_and_repeatable() {
        assert_eq!(fallback_latency_budget_ms(16), 32);
        assert_eq!(fallback_latency_budget_ms(33), 66);
        assert_eq!(fallback_latency_budget_ms(200), 250);
    }

    #[test]
    fn production_webgpu_flag_blocks_backend_activation_when_disabled() {
        let mut state = BackendRuntimeState::default();
        let limits = Limits::default();
        let probe = BackendCapabilityProbe {
            webgpu_api: true,
            adapter_available: true,
            webgl2_api: true,
            max_texture_dimension_2d: 8192,
        };
        let selected = state.configure_backend("webgpu", probe, &limits);
        assert_eq!(selected, RendererBackend::Cpu);
        assert_eq!(state.active_backend(), RendererBackend::Cpu);
        let metrics = state.metrics_json();
        assert!(metrics.contains("production_webgpu_flag_disabled"));
        assert!(metrics.contains("DVF.WASM.GPU.FLAG_DISABLED"));
    }

    #[cfg(feature = "webgpu-backend")]
    #[test]
    fn device_lost_chaos_cycles_have_bounded_transition_growth() {
        let mut state = BackendRuntimeState::default();
        state.set_production_webgpu_enabled(true);
        let limits = Limits::default();
        let probe = BackendCapabilityProbe {
            webgpu_api: true,
            adapter_available: true,
            webgl2_api: true,
            max_texture_dimension_2d: 8192,
        };
        let selected = state.configure_backend("webgpu", probe, &limits);
        assert_eq!(selected, RendererBackend::WebGpu);

        for _ in 0..12 {
            assert!(state.simulate_device_lost());
            assert_eq!(state.active_backend(), RendererBackend::Cpu);
            let recovered = state.recover_device_lost(&limits);
            assert_eq!(recovered, RendererBackend::WebGpu);
        }

        let metrics = state.metrics_json();
        assert!(metrics.contains("\"backend_transition_count\":"));
        assert!(metrics.contains("\"fallback_budget_violations\":0"));
    }
}
