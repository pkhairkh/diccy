#![deny(missing_docs)]

//! GPU renderer implementation (wgpu backend).

pub mod volume_renderer;

pub use volume_renderer::{
    ArcballCamera, ClipPlane, GpuMipRequest, GpuMprRequest, GpuVrRequest, TransferFunctionPreset,
    VolumeRenderer, invert_mat4, look_at_rh, mul_mat4, perspective_rh,
};

use std::borrow::Cow;

use dicom_core::{Error, ErrorKind, Limits, Result};
use dicom_pixel::{DisplayFrame, PixelFormat};
use viewer_core::{CacheMetrics, DeterministicCache, Viewport2D};

const FULLSCREEN_SHADER_WGSL: &str = r#"
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

/// Renderer capability summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuCapabilities {
    /// Maximum 2D texture dimension.
    pub max_texture_dimension_2d: u32,
}

impl GpuCapabilities {
    /// Build capabilities from a `wgpu::Limits` snapshot.
    pub fn from_limits(limits: &wgpu::Limits) -> Self {
        Self {
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
        }
    }
}

/// GPU texture budget tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuBudget {
    /// Maximum allowed bytes.
    pub max_texture_bytes: u64,
    /// Current reserved bytes.
    pub used_texture_bytes: u64,
}

impl GpuBudget {
    /// Create a new GPU budget tracker.
    pub fn new(max_texture_bytes: u64) -> Self {
        Self {
            max_texture_bytes,
            used_texture_bytes: 0,
        }
    }

    /// Reserve texture bytes, failing if the budget would be exceeded.
    pub fn reserve(&mut self, bytes: u64) -> Result<()> {
        let next = self.used_texture_bytes.saturating_add(bytes);
        if next > self.max_texture_bytes {
            return Err(Error::from_kind(
                ErrorKind::LimitExceeded {
                    limit_name: "max_gpu_texture_bytes",
                    observed: next,
                    allowed: self.max_texture_bytes,
                },
                "GPU texture budget exceeded",
            )
            .into());
        }
        self.used_texture_bytes = next;
        Ok(())
    }

    /// Release texture bytes back to the budget.
    pub fn release(&mut self, bytes: u64) {
        self.used_texture_bytes = self.used_texture_bytes.saturating_sub(bytes);
    }
}

/// Deterministic surface configuration tracked by the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceState {
    /// Configured surface width in pixels.
    pub width: u32,
    /// Configured surface height in pixels.
    pub height: u32,
    /// Selected swapchain texture format.
    pub format: wgpu::TextureFormat,
    /// Monotonic generation incremented on reconfiguration.
    pub generation: u64,
}

/// Snapshot of renderer lifecycle counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderLifecycleState {
    /// True when a device-backed renderer was initialized.
    pub device_ready: bool,
    /// Current configured surface state.
    pub surface: Option<SurfaceState>,
    /// Number of rendered frames submitted.
    pub rendered_frames: u64,
    /// Number of draw passes submitted.
    pub submitted_passes: u64,
    /// Number of acquired frame targets.
    pub acquired_frames: u64,
    /// Number of presented frame targets.
    pub presented_frames: u64,
    /// Number of draw passes encoded into a command buffer.
    pub encoded_passes: u64,
}

/// Renderer trait for GPU backends.
pub trait Renderer {
    /// Render a frame into the viewport.
    fn render(&mut self, frame: &DisplayFrame, viewport: &Viewport2D) -> Result<()>;
}

#[derive(Debug)]
struct SurfaceFrame {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

#[derive(Debug)]
struct LiveRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sampler: wgpu::Sampler,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    pipeline_format: wgpu::TextureFormat,
    acquired: Option<SurfaceFrame>,
}

impl LiveRenderer {
    fn new(device: wgpu::Device, queue: wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diccy.fullscreen_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("diccy.fullscreen_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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
        let pipeline = create_pipeline(&device, &bind_group_layout, target_format);
        Self {
            device,
            queue,
            sampler,
            bind_group_layout,
            pipeline,
            pipeline_format: target_format,
            acquired: None,
        }
    }

    fn acquire_frame(&mut self, surface: SurfaceState) {
        if self.pipeline_format != surface.format {
            self.pipeline = create_pipeline(&self.device, &self.bind_group_layout, surface.format);
            self.pipeline_format = surface.format;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diccy.surface_target"),
            size: wgpu::Extent3d {
                width: surface.width,
                height: surface.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.acquired = Some(SurfaceFrame { texture, view });
    }

    fn encode_draw(&mut self, frame: &DisplayFrame) -> Result<()> {
        let (source_texture, bind_group) = self.prepare_source(frame)?;
        let acquired = self.acquired.as_ref().ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu_lifecycle".to_string(),
                    detail: "surface frame must be acquired before draw".to_string(),
                },
                "GPU renderer surface frame was not acquired",
            )
        })?;

        // Keep source texture alive until submission finishes.
        let _source_texture_guard = source_texture;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("diccy.render_encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("diccy.fullscreen_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &acquired.view,
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn present(&mut self) {
        if let Some(frame) = self.acquired.take() {
            let _ = &frame.texture;
            let _ = &frame.view;
        }
    }

    fn prepare_source(&self, frame: &DisplayFrame) -> Result<(wgpu::Texture, wgpu::BindGroup)> {
        let rgba = frame_to_rgba(frame)?;
        let source = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("diccy.source_texture"),
            size: wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
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
                texture: &source,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        let source_view = source.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diccy.fullscreen_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&source_view),
                },
            ],
        });
        Ok((source, bind_group))
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    target_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("diccy.fullscreen_shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(FULLSCREEN_SHADER_WGSL)),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("diccy.fullscreen_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("diccy.fullscreen_pipeline"),
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

fn frame_to_rgba(frame: &DisplayFrame) -> Result<Vec<u8>> {
    match frame.format {
        PixelFormat::Rgba8 => Ok(frame.bytes.clone()),
        PixelFormat::Luma8 => {
            let mut out = Vec::with_capacity(frame.bytes.len().saturating_mul(4));
            for mono in &frame.bytes {
                out.extend_from_slice(&[*mono, *mono, *mono, 255]);
            }
            Ok(out)
        }
        PixelFormat::Luma16 => Err(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "gpu".to_string(),
                detail: "renderer accepts only CPU-oracle quantized Luma8/Rgba8".to_string(),
            },
            "GPU renderer rejected unsupported frame format",
        )
        .into()),
    }
}

fn upload_size_bytes(frame: &DisplayFrame) -> Result<u64> {
    let pixels = (frame.width as u64)
        .checked_mul(frame.height as u64)
        .ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu".to_string(),
                    detail: "frame dimensions overflow pixel count".to_string(),
                },
                "GPU frame validation failed",
            )
        })?;
    match frame.format {
        PixelFormat::Luma8 | PixelFormat::Rgba8 => pixels.checked_mul(4).ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu".to_string(),
                    detail: "frame dimensions overflow byte count".to_string(),
                },
                "GPU frame validation failed",
            )
            .into()
        }),
        PixelFormat::Luma16 => Err(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "gpu".to_string(),
                detail: "renderer accepts only CPU-oracle quantized Luma8/Rgba8".to_string(),
            },
            "GPU renderer rejected unsupported frame format",
        )
        .into()),
    }
}

/// WGPU-backed renderer implementation.
#[derive(Debug)]
pub struct WgpuRenderer {
    /// Probed GPU capabilities.
    pub capabilities: GpuCapabilities,
    /// GPU texture budget.
    pub budget: GpuBudget,
    surface: Option<SurfaceState>,
    rendered_frames: u64,
    submitted_passes: u64,
    acquired_frames: u64,
    presented_frames: u64,
    encoded_passes: u64,
    texture_cache: DeterministicCache<u64>,
    live: Option<LiveRenderer>,
}

impl WgpuRenderer {
    /// Create a renderer from an already-probed capability snapshot.
    pub fn from_capabilities(capabilities: GpuCapabilities, limits: &Limits) -> Self {
        let budget = GpuBudget::new(limits.max_gpu_texture_bytes());
        Self {
            capabilities,
            budget,
            surface: None,
            rendered_frames: 0,
            submitted_passes: 0,
            acquired_frames: 0,
            presented_frames: 0,
            encoded_passes: 0,
            texture_cache: DeterministicCache::new(limits.max_gpu_texture_bytes()),
            live: None,
        }
    }

    /// Query device limits and build capability summary.
    pub fn probe_capabilities(device: &wgpu::Device) -> GpuCapabilities {
        let limits = device.limits();
        GpuCapabilities::from_limits(&limits)
    }

    /// Create a renderer from a device and configured limits.
    pub fn new(device: &wgpu::Device, limits: &Limits) -> Self {
        let capabilities = Self::probe_capabilities(device);
        Self::from_capabilities(capabilities, limits)
    }

    /// Attach a live WGPU device/queue runtime for real command encoding.
    pub fn attach_runtime(&mut self, device: wgpu::Device, queue: wgpu::Queue) -> Result<()> {
        let surface = self.surface.ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu_lifecycle".to_string(),
                    detail: "configure_surface must run before attach_runtime".to_string(),
                },
                "GPU renderer surface is not configured",
            )
        })?;
        self.live = Some(LiveRenderer::new(device, queue, surface.format));
        Ok(())
    }

    /// Return true when a live WGPU runtime is attached.
    pub fn has_live_runtime(&self) -> bool {
        self.live.is_some()
    }

    /// Reserve texture bytes for a new GPU resource.
    pub fn reserve_texture_bytes(&mut self, bytes: u64) -> Result<()> {
        self.budget.reserve(bytes)
    }

    /// Release texture bytes when a resource is evicted.
    pub fn release_texture_bytes(&mut self, bytes: u64) {
        self.budget.release(bytes);
    }

    /// Configure the render surface before draw submission.
    pub fn configure_surface(
        &mut self,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Result<()> {
        validate_surface_extent(width, height, self.capabilities.max_texture_dimension_2d)?;
        let generation = match self.surface {
            Some(previous)
                if previous.width == width
                    && previous.height == height
                    && previous.format == format =>
            {
                previous.generation
            }
            Some(previous) => previous.generation.saturating_add(1),
            None => 1,
        };
        self.surface = Some(SurfaceState {
            width,
            height,
            format,
            generation,
        });
        Ok(())
    }

    /// Resize the surface while preserving its configured format.
    pub fn resize_surface(&mut self, width: u32, height: u32) -> Result<()> {
        let Some(surface) = self.surface else {
            return Err(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu_lifecycle".to_string(),
                    detail: "surface must be configured before resize".to_string(),
                },
                "GPU renderer surface is not configured",
            )
            .into());
        };
        self.configure_surface(width, height, surface.format)
    }

    /// Return an immutable snapshot of lifecycle counters and surface state.
    pub fn lifecycle_state(&self) -> RenderLifecycleState {
        RenderLifecycleState {
            device_ready: self.live.is_some(),
            surface: self.surface,
            rendered_frames: self.rendered_frames,
            submitted_passes: self.submitted_passes,
            acquired_frames: self.acquired_frames,
            presented_frames: self.presented_frames,
            encoded_passes: self.encoded_passes,
        }
    }

    /// Return deterministic texture-cache metrics for renderer diagnostics.
    pub fn texture_cache_metrics(&self) -> CacheMetrics {
        self.texture_cache.metrics()
    }
}

fn validate_cpu_oracle_frame(frame: &DisplayFrame) -> Result<()> {
    let pixels = (frame.width as u64)
        .checked_mul(frame.height as u64)
        .ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu".to_string(),
                    detail: "frame dimensions overflow pixel count".to_string(),
                },
                "GPU frame validation failed",
            )
        })?;
    let expected_bytes = match frame.format {
        PixelFormat::Luma8 => pixels,
        PixelFormat::Rgba8 => pixels.checked_mul(4).ok_or_else(|| {
            Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu".to_string(),
                    detail: "RGBA frame dimensions overflow byte count".to_string(),
                },
                "GPU frame validation failed",
            )
        })?,
        PixelFormat::Luma16 => {
            return Err(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu".to_string(),
                    detail: "renderer accepts only CPU-oracle quantized Luma8/Rgba8".to_string(),
                },
                "GPU renderer rejected unsupported frame format",
            )
            .into())
        }
    };
    if frame.bytes.len() as u64 != expected_bytes {
        return Err(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "gpu".to_string(),
                detail: "frame byte length does not match dimensions/format".to_string(),
            },
            "GPU renderer rejected invalid frame byte length",
        )
        .into());
    }
    Ok(())
}

fn validate_surface_extent(width: u32, height: u32, max_dimension: u32) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(Error::from_kind(
            ErrorKind::InvalidPixelTransform {
                stage: "gpu_lifecycle".to_string(),
                detail: "surface width/height must be > 0".to_string(),
            },
            "GPU renderer rejected invalid surface configuration",
        )
        .into());
    }
    if width > max_dimension || height > max_dimension {
        return Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "max_texture_dimension_2d",
                observed: width.max(height) as u64,
                allowed: max_dimension as u64,
            },
            "GPU renderer surface exceeds device limits",
        )
        .into());
    }
    Ok(())
}

impl Renderer for WgpuRenderer {
    fn render(&mut self, frame: &DisplayFrame, _viewport: &Viewport2D) -> Result<()> {
        // REQ-GPU-210 / REQ-PIX-201: GPU path is presentation-only and accepts CPU-quantized
        // boundary outputs (`Luma8` / `Rgba8`) only.
        let Some(surface) = self.surface else {
            return Err(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "gpu_lifecycle".to_string(),
                    detail: "configure_surface must run before render".to_string(),
                },
                "GPU renderer surface is not configured",
            )
            .into());
        };
        validate_cpu_oracle_frame(frame)?;

        let cache_key = format!(
            "{}:{}:{:?}:{}",
            frame.width, frame.height, frame.format, self.rendered_frames
        );
        self.texture_cache
            .insert(cache_key, self.rendered_frames, frame.bytes.len() as u64);

        let upload_bytes = upload_size_bytes(frame)?;
        self.reserve_texture_bytes(upload_bytes)?;

        if let Some(live) = self.live.as_mut() {
            live.acquire_frame(surface);
            self.acquired_frames = self.acquired_frames.saturating_add(1);
            live.encode_draw(frame)?;
            self.encoded_passes = self.encoded_passes.saturating_add(1);
            self.submitted_passes = self.submitted_passes.saturating_add(1);
            live.present();
            self.presented_frames = self.presented_frames.saturating_add(1);
        } else {
            self.submitted_passes = self.submitted_passes.saturating_add(1);
        }

        self.release_texture_bytes(upload_bytes);
        self.rendered_frames = self.rendered_frames.saturating_add(1);
        Ok(())
    }
}
