#![deny(missing_docs)]

//! WASM integration glue (bindings and adapters) for viewer core.

mod backend;

use dicom_core::Limits;
use dicom_io::{BytesSource, P10Reader};
use dicom_pixel::{
    DisplayFrame, PixelDecodeInput, PixelFormat, PixelPipeline, PixelPipelineConfig,
};
use std::cell::RefCell;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::backend::{BackendCapabilityProbe, BackendRuntimeState};
use viewer_core::{
    reslice_volume, reslice_volume_patient, MprLimits, MprPlane, MprRequest, PatientMprPlane,
    PatientMprRequest, ResampleKernel, SlabMode, TriPlanarPlane, ViewerModel, VolumeGrid,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Escape metadata strings for safe HTML rendering.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "escapeMetadataHtml"))]
pub fn escape_metadata_html(input: &str) -> String {
    escape_html(input)
}

/// WASM viewer wrapper.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Debug, Clone, PartialEq)]
pub struct WasmViewer {
    model: ViewerModel,
    network_enabled: bool,
    limits: Limits,
    loaded_bytes: usize,
    preview_width: u32,
    preview_height: u32,
    preview_rgba: Vec<u8>,
    view_center_x: f64,
    view_center_y: f64,
    sampling_mode: SamplingMode,
    mpr_slab_thickness: u32,
    mpr_slab_mode: SlabMode,
    preview_source_format: PixelFormat,
    backend_runtime: RefCell<BackendRuntimeState>,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl WasmViewer {
    /// Create a new viewer model for a given viewport size.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            model: ViewerModel::new(width, height),
            network_enabled: false,
            limits: Limits::default(),
            loaded_bytes: 0,
            preview_width: 0,
            preview_height: 0,
            preview_rgba: Vec::new(),
            view_center_x: 0.0,
            view_center_y: 0.0,
            sampling_mode: SamplingMode::Linear,
            mpr_slab_thickness: 1,
            mpr_slab_mode: SlabMode::Average,
            preview_source_format: PixelFormat::Rgba8,
            backend_runtime: RefCell::new(BackendRuntimeState::default()),
        }
    }

    /// Update viewport dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.model.viewport.width = width;
        self.model.viewport.height = height;
    }

    /// Set a new zoom level. Returns false when input is non-finite or non-positive.
    pub fn set_zoom(&mut self, zoom: f64) -> bool {
        if !is_valid_zoom(zoom) {
            return false;
        }
        self.model.viewport.zoom = zoom;
        true
    }

    /// Apply a zoom delta. Returns false when input is non-finite or results in invalid zoom.
    pub fn zoom_by(&mut self, delta: f64) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let next = self.model.viewport.zoom + delta;
        if !is_valid_zoom(next) {
            return false;
        }
        self.model.viewport.zoom = next;
        true
    }

    /// Return the current viewport width.
    pub fn viewport_width(&self) -> u32 {
        self.model.viewport.width
    }

    /// Return the current viewport height.
    pub fn viewport_height(&self) -> u32 {
        self.model.viewport.height
    }

    /// Return the current zoom factor.
    pub fn viewport_zoom(&self) -> f64 {
        self.model.viewport.zoom
    }

    /// Return whether network access is enabled for this viewer instance.
    pub fn network_enabled(&self) -> bool {
        self.network_enabled
    }

    /// Request network access enablement. Returns true only when compiled with the `network` feature.
    pub fn set_network_enabled(&mut self, enabled: bool) -> bool {
        if !enabled {
            self.network_enabled = false;
            return true;
        }
        if cfg!(feature = "network") {
            self.network_enabled = true;
            true
        } else {
            false
        }
    }

    /// Escape metadata for HTML contexts (prefer text nodes).
    pub fn render_metadata_html(&self, raw: &str) -> String {
        escape_html(raw)
    }

    /// Ingest DICOM P10 bytes from the host boundary with fail-closed limit checks.
    pub fn ingest_bytes(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() as u64 > self.limits.max_input_bytes {
            return false;
        }
        let preview = match decode_preview_frame(bytes, &self.limits) {
            Some(preview) => preview,
            None => return false,
        };
        self.loaded_bytes = bytes.len();
        self.preview_width = preview.width;
        self.preview_height = preview.height;
        self.preview_source_format = preview.source_format;
        self.preview_rgba = preview.rgba;
        self.reset_view_center();
        true
    }

    /// Return true when a decoded preview frame is available.
    pub fn has_preview(&self) -> bool {
        self.preview_width > 0
            && self.preview_height > 0
            && !self.preview_rgba.is_empty()
            && (self.preview_rgba.len() / 4)
                == (self.preview_width as usize).saturating_mul(self.preview_height as usize)
    }

    /// Return width of the current preview frame, or 0 when none is loaded.
    pub fn preview_width(&self) -> u32 {
        self.preview_width
    }

    /// Return height of the current preview frame, or 0 when none is loaded.
    pub fn preview_height(&self) -> u32 {
        self.preview_height
    }

    /// Return RGBA8 bytes for the current preview frame.
    ///
    /// Returns an empty vector when no preview frame is loaded.
    pub fn preview_rgba_bytes(&self) -> Vec<u8> {
        self.preview_rgba.clone()
    }

    /// Set interpolation mode for viewport re-rendering from DICOM pixels.
    ///
    /// Supported values:
    /// - `nearest`
    /// - `linear`
    ///
    /// Returns false for unsupported values.
    pub fn set_interpolation_mode(&mut self, mode: &str) -> bool {
        match mode {
            "nearest" => {
                self.sampling_mode = SamplingMode::Nearest;
                true
            }
            "linear" => {
                self.sampling_mode = SamplingMode::Linear;
                true
            }
            _ => false,
        }
    }

    /// Return active interpolation mode string.
    pub fn interpolation_mode(&self) -> String {
        match self.sampling_mode {
            SamplingMode::Nearest => "nearest".to_string(),
            SamplingMode::Linear => "linear".to_string(),
        }
    }

    /// Reset viewport center to source-frame midpoint.
    pub fn reset_view_center(&mut self) {
        if self.preview_width > 0 {
            self.view_center_x = (self.preview_width as f64 - 1.0) * 0.5;
        } else {
            self.view_center_x = 0.0;
        }
        if self.preview_height > 0 {
            self.view_center_y = (self.preview_height as f64 - 1.0) * 0.5;
        } else {
            self.view_center_y = 0.0;
        }
    }

    /// Recenter the ROI using normalized viewport fractions `[0,1] x [0,1]`.
    ///
    /// The normalized coordinates are interpreted within the currently displayed
    /// source window, so clicking the preview can move the ROI center.
    pub fn recenter_from_viewport_fraction(&mut self, u: f64, v: f64) -> bool {
        if !self.has_preview()
            || !u.is_finite()
            || !v.is_finite()
            || !(0.0..=1.0).contains(&u)
            || !(0.0..=1.0).contains(&v)
        {
            return false;
        }
        let (start_x, start_y, src_w, src_h) = match self.source_window() {
            Some(window) => window,
            None => return false,
        };
        self.view_center_x = clamp_f64(
            start_x + u * src_w,
            0.0,
            (self.preview_width.saturating_sub(1)) as f64,
        );
        self.view_center_y = clamp_f64(
            start_y + v * src_h,
            0.0,
            (self.preview_height.saturating_sub(1)) as f64,
        );
        true
    }

    /// Render a viewport RGBA image using current zoom + ROI center.
    ///
    /// Output size matches the current viewport dimensions.
    pub fn render_viewport_rgba_bytes(&self) -> Vec<u8> {
        self.render_viewport_rgba_bytes_for_size(
            self.model.viewport.width,
            self.model.viewport.height,
        )
    }

    /// Render a viewport RGBA image using current zoom + ROI center at explicit output size.
    pub fn render_viewport_rgba_bytes_for_size(
        &self,
        output_width: u32,
        output_height: u32,
    ) -> Vec<u8> {
        self.render_viewport_rgba(output_width, output_height)
    }

    /// Load bytes into the viewer; alias for `ingest_bytes`.
    pub fn load_bytes(&mut self, bytes: &[u8]) -> bool {
        self.ingest_bytes(bytes)
    }

    /// Configure active renderer backend with host capability probe data.
    ///
    /// Supported `preferred` values are `cpu` and `webgpu`.
    pub fn configure_renderer_backend(
        &mut self,
        preferred: &str,
        webgpu_api: bool,
        adapter_available: bool,
        webgl2_api: bool,
        max_texture_dimension_2d: u32,
    ) -> String {
        let probe = BackendCapabilityProbe {
            webgpu_api,
            adapter_available,
            webgl2_api,
            max_texture_dimension_2d,
        };
        let backend =
            self.backend_runtime
                .borrow_mut()
                .configure_backend(preferred, probe, &self.limits);
        backend.as_str().to_string()
    }

    /// Return the active renderer backend label (`CPU` or `WebGPU`).
    pub fn active_renderer_backend(&self) -> String {
        self.backend_runtime
            .borrow()
            .active_backend()
            .as_str()
            .to_string()
    }

    /// Return backend capability probe snapshot as JSON.
    pub fn backend_capability_probe_json(&self) -> String {
        self.backend_runtime.borrow().probe_json()
    }

    /// Return backend selection metrics as JSON.
    pub fn backend_selection_metrics_json(&self) -> String {
        self.backend_runtime.borrow().metrics_json()
    }

    /// Force CPU renderer path for diagnostics.
    pub fn set_force_cpu_renderer(&mut self, enabled: bool) {
        self.backend_runtime.borrow_mut().force_cpu(enabled);
    }

    /// Enable or disable production WebGPU renderer path.
    pub fn set_production_webgpu_renderer(&mut self, enabled: bool) {
        self.backend_runtime
            .borrow_mut()
            .set_production_webgpu_enabled(enabled);
    }

    /// Return whether production WebGPU renderer path is enabled.
    pub fn production_webgpu_renderer_enabled(&self) -> bool {
        self.backend_runtime.borrow().production_webgpu_enabled()
    }

    /// Set target frame interval in milliseconds (render cadence hint).
    pub fn set_render_frame_interval_ms(&mut self, interval_ms: u32) -> bool {
        self.backend_runtime
            .borrow_mut()
            .set_target_frame_interval_ms(interval_ms)
    }

    /// Return the configured target frame interval in milliseconds.
    pub fn render_frame_interval_ms(&self) -> u32 {
        self.backend_runtime.borrow().target_frame_interval_ms()
    }

    /// Simulate WebGPU device loss for deterministic recovery testing.
    pub fn simulate_gpu_device_lost(&mut self) -> bool {
        self.backend_runtime.borrow_mut().simulate_device_lost()
    }

    /// Attempt deterministic recovery after device-loss.
    pub fn recover_gpu_device(&mut self) -> String {
        self.backend_runtime
            .borrow_mut()
            .recover_device_lost(&self.limits)
            .as_str()
            .to_string()
    }

    /// Return the inline WGSL shader source used by CSP-safe backend initialization.
    pub fn csp_safe_shader_source(&self) -> String {
        self.backend_runtime
            .borrow()
            .csp_safe_shader_source()
            .to_string()
    }

    /// Return byte length of the most recently ingested valid DICOM payload.
    pub fn loaded_input_bytes(&self) -> usize {
        self.loaded_bytes
    }

    /// Configure deterministic slab controls used by MPR render methods.
    pub fn set_mpr_slab(&mut self, thickness: u32, mode: &str) -> bool {
        if thickness == 0 || thickness > 64 {
            return false;
        }
        let Some(parsed_mode) = parse_slab_mode(mode) else {
            return false;
        };
        self.mpr_slab_thickness = thickness;
        self.mpr_slab_mode = parsed_mode;
        true
    }

    /// Return current slab configuration as JSON.
    pub fn mpr_slab_json(&self) -> String {
        let mode = match self.mpr_slab_mode {
            SlabMode::Average => "average",
            SlabMode::Max => "max",
        };
        format!(
            "{{\"thickness\":{},\"mode\":\"{}\"}}",
            self.mpr_slab_thickness, mode
        )
    }

    /// Run a baseline CPU MPR reslice over the current preview frame and return JSON output.
    pub fn mpr_reslice_json(
        &self,
        plane: &str,
        index: i32,
        output_width: u32,
        output_height: u32,
    ) -> String {
        let Some(volume) = self.preview_volume_grid() else {
            return "{\"ok\":false,\"error\":\"no-preview\"}".to_string();
        };
        let selected_plane = match plane.to_ascii_lowercase().as_str() {
            "axial" => MprPlane::Axial,
            "coronal" => MprPlane::Coronal,
            "sagittal" => MprPlane::Sagittal,
            _ => MprPlane::Axial,
        };
        let request = MprRequest {
            plane: selected_plane,
            index,
            output_width,
            output_height,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: self.mpr_slab_thickness,
            slab_mode: self.mpr_slab_mode,
        };
        match reslice_volume(&volume, &request, MprLimits::default()) {
            Ok(frame) => format!(
                "{{\"ok\":true,\"plane\":\"{}\",\"index\":{},\"width\":{},\"height\":{},\"cache_key\":\"{}\",\"pixel_count\":{}}}",
                plane,
                index,
                frame.width,
                frame.height,
                frame.cache_key,
                frame.pixels.len()
            ),
            Err(error) => format!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                escape_json_string(&error.to_string())
            ),
        }
    }

    /// Run patient-space MPR request (axis-aligned offsets in micrometers) and return JSON output.
    pub fn mpr_reslice_patient_json(
        &self,
        plane: &str,
        offset_um: i64,
        output_width: u32,
        output_height: u32,
    ) -> String {
        let Some(mut volume) = self.preview_volume_grid() else {
            return "{\"ok\":false,\"error\":\"no-preview\"}".to_string();
        };
        volume.migrate_patient_geometry_from_legacy(None);
        let selected_plane = match plane.to_ascii_lowercase().as_str() {
            "axial" => PatientMprPlane::Axial { offset_um },
            "coronal" => PatientMprPlane::Coronal { offset_um },
            "sagittal" => PatientMprPlane::Sagittal { offset_um },
            _ => PatientMprPlane::Axial { offset_um },
        };
        let request = PatientMprRequest {
            plane: selected_plane,
            output_width,
            output_height,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: self.mpr_slab_thickness,
            slab_mode: self.mpr_slab_mode,
        };
        match reslice_volume_patient(&volume, &request, MprLimits::default()) {
            Ok(frame) => format!(
                "{{\"ok\":true,\"plane\":\"{}\",\"offset_um\":{},\"width\":{},\"height\":{},\"cache_key\":\"{}\",\"pixel_count\":{}}}",
                plane,
                offset_um,
                frame.width,
                frame.height,
                frame.cache_key,
                frame.pixels.len()
            ),
            Err(error) => format!(
                "{{\"ok\":false,\"error\":\"{}\"}}",
                escape_json_string(&error.to_string())
            ),
        }
    }

    /// Set one tri-planar plane index and synchronize crosshair state.
    pub fn set_mpr_plane_index(&mut self, plane: &str, index: u32) -> bool {
        let Some(volume) = self.preview_volume_grid() else {
            return false;
        };
        let dims = volume.dimensions();
        if !self
            .model
            .set_mpr_volume_dimensions([dims[0] as u32, dims[1] as u32, dims[2] as u32])
        {
            return false;
        }
        let Some(axis) = parse_tri_planar_plane(plane) else {
            return false;
        };
        self.model.set_mpr_plane_index(axis, index)
    }

    /// Set the tri-planar crosshair voxel and synchronize all view indices.
    pub fn set_mpr_crosshair(&mut self, x: u32, y: u32, z: u32) -> bool {
        let Some(volume) = self.preview_volume_grid() else {
            return false;
        };
        let dims = volume.dimensions();
        if !self
            .model
            .set_mpr_volume_dimensions([dims[0] as u32, dims[1] as u32, dims[2] as u32])
        {
            return false;
        }
        self.model.set_mpr_crosshair_voxel([x, y, z])
    }

    /// Return tri-planar synchronization state as JSON.
    pub fn tri_planar_state_json(&mut self) -> String {
        if let Some(volume) = self.preview_volume_grid() {
            let dims = volume.dimensions();
            let _ = self.model.set_mpr_volume_dimensions([
                dims[0] as u32,
                dims[1] as u32,
                dims[2] as u32,
            ]);
        }
        let state = self.model.tri_planar_state();
        format!(
            "{{\"volume_dimensions\":[{},{},{}],\"crosshair_voxel\":[{},{},{}],\"axial_index\":{},\"coronal_index\":{},\"sagittal_index\":{}}}",
            state.volume_dimensions[0],
            state.volume_dimensions[1],
            state.volume_dimensions[2],
            state.crosshair_voxel[0],
            state.crosshair_voxel[1],
            state.crosshair_voxel[2],
            state.axial_index,
            state.coronal_index,
            state.sagittal_index
        )
    }

    /// Render one tri-planar MPR plane as RGBA bytes using synchronized indices.
    pub fn mpr_plane_rgba_bytes(
        &mut self,
        plane: &str,
        output_width: u32,
        output_height: u32,
    ) -> Vec<u8> {
        let Some(volume) = self.preview_volume_grid() else {
            return Vec::new();
        };
        let dims = volume.dimensions();
        if !self
            .model
            .set_mpr_volume_dimensions([dims[0] as u32, dims[1] as u32, dims[2] as u32])
        {
            return Vec::new();
        }
        let state = self.model.tri_planar_state();
        let (selected_plane, index) = match plane.to_ascii_lowercase().as_str() {
            "axial" => (MprPlane::Axial, state.axial_index as i32),
            "coronal" => (MprPlane::Coronal, state.coronal_index as i32),
            "sagittal" => (MprPlane::Sagittal, state.sagittal_index as i32),
            _ => return Vec::new(),
        };
        let request = MprRequest {
            plane: selected_plane,
            index,
            output_width,
            output_height,
            kernel: ResampleKernel::Nearest,
            background: 0,
            voi_window: None,
            slab_thickness: self.mpr_slab_thickness,
            slab_mode: self.mpr_slab_mode,
        };
        let Ok(frame) = reslice_volume(&volume, &request, MprLimits::default()) else {
            return Vec::new();
        };
        let mut rgba = Vec::with_capacity(frame.pixels.len().saturating_mul(4));
        for mono in frame.pixels {
            rgba.extend_from_slice(&[mono, mono, mono, 255]);
        }
        rgba
    }

    /// Borrow the inner viewer model.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn model(&self) -> &ViewerModel {
        &self.model
    }

    /// Mutably borrow the inner viewer model.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn model_mut(&mut self) -> &mut ViewerModel {
        &mut self.model
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewFrame {
    width: u32,
    height: u32,
    source_format: PixelFormat,
    rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplingMode {
    Nearest,
    Linear,
}

fn decode_preview_frame(bytes: &[u8], limits: &Limits) -> Option<PreviewFrame> {
    let mut meta_reader = P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
    let meta = meta_reader.read_meta().ok()?;

    let mut dataset_reader =
        P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
    let dataset = dataset_reader.read_dataset().ok()?;

    let pipeline = PixelPipeline::new(PixelPipelineConfig {
        limits: limits.clone(),
        ..PixelPipelineConfig::default()
    });

    let frame = pipeline
        .decode_input(PixelDecodeInput {
            dataset: &dataset,
            transfer_syntax_uid: &meta.transfer_syntax_uid,
            frame_index: 0,
        })
        .ok()?;

    display_frame_to_rgba(frame, limits)
}

fn display_frame_to_rgba(frame: DisplayFrame, limits: &Limits) -> Option<PreviewFrame> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }
    let pixel_count = (frame.width as usize).checked_mul(frame.height as usize)?;
    if pixel_count == 0 {
        return None;
    }
    let rgba_len = pixel_count.checked_mul(4)?;
    if (rgba_len as u64) > limits.max_decompressed_bytes {
        return None;
    }

    match frame.format {
        PixelFormat::Rgba8 => {
            if frame.bytes.len() != rgba_len {
                return None;
            }
            Some(PreviewFrame {
                width: frame.width,
                height: frame.height,
                source_format: PixelFormat::Rgba8,
                rgba: frame.bytes,
            })
        }
        PixelFormat::Luma8 => {
            if frame.bytes.len() != pixel_count {
                return None;
            }
            let mut rgba = Vec::with_capacity(rgba_len);
            for mono in frame.bytes {
                rgba.extend_from_slice(&[mono, mono, mono, 255]);
            }
            Some(PreviewFrame {
                width: frame.width,
                height: frame.height,
                source_format: PixelFormat::Luma8,
                rgba,
            })
        }
        PixelFormat::Luma16 => {
            let expected_bytes = pixel_count.checked_mul(2)?;
            if frame.bytes.len() != expected_bytes {
                return None;
            }
            let mut rgba = Vec::with_capacity(rgba_len);
            for chunk in frame.bytes.chunks_exact(2) {
                let mono16 = u16::from_le_bytes([chunk[0], chunk[1]]);
                let mono8 = (mono16 >> 8) as u8;
                rgba.extend_from_slice(&[mono8, mono8, mono8, 255]);
            }
            Some(PreviewFrame {
                width: frame.width,
                height: frame.height,
                source_format: PixelFormat::Luma8,
                rgba,
            })
        }
    }
}

impl WasmViewer {
    fn preview_volume_grid(&self) -> Option<VolumeGrid> {
        if !self.has_preview() || self.preview_width == 0 || self.preview_height == 0 {
            return None;
        }
        let width = self.preview_width as usize;
        let height = self.preview_height as usize;
        let mut volume = VolumeGrid::new([width, height, 1], [1_000, 1_000, 1_000]).ok()?;
        let mut voxels = Vec::with_capacity(width.checked_mul(height)?);
        for pixel in self.preview_rgba.chunks_exact(4) {
            voxels.push(pixel[0] as i32);
        }
        if voxels.len() != width.checked_mul(height)? {
            return None;
        }
        let mut cursor = 0usize;
        for y in 0..height {
            for x in 0..width {
                let value = voxels[cursor];
                cursor += 1;
                if !volume.set_voxel(x, y, 0, value) {
                    return None;
                }
            }
        }
        Some(volume)
    }

    fn render_viewport_rgba(&self, output_width: u32, output_height: u32) -> Vec<u8> {
        if !self.has_preview() || output_width == 0 || output_height == 0 {
            return Vec::new();
        }
        #[cfg(not(target_arch = "wasm32"))]
        let render_started = Instant::now();
        let pixel_count = match (output_width as usize).checked_mul(output_height as usize) {
            Some(value) => value,
            None => return Vec::new(),
        };
        let out_len = match pixel_count.checked_mul(4) {
            Some(value) => value,
            None => return Vec::new(),
        };
        if (out_len as u64) > self.limits.max_decompressed_bytes {
            return Vec::new();
        }
        let (start_x, start_y, src_w, src_h) = match self.source_window() {
            Some(window) => window,
            None => return Vec::new(),
        };

        let src_stride = self.preview_width as usize * 4;
        let mut out = vec![0u8; out_len];
        for y in 0..output_height {
            let sy = start_y + ((y as f64 + 0.5) / output_height as f64) * src_h;
            for x in 0..output_width {
                let sx = start_x + ((x as f64 + 0.5) / output_width as f64) * src_w;
                let rgba = match self.sampling_mode {
                    SamplingMode::Nearest => sample_nearest_rgba(
                        &self.preview_rgba,
                        self.preview_width,
                        self.preview_height,
                        src_stride,
                        sx,
                        sy,
                    ),
                    SamplingMode::Linear => sample_linear_rgba(
                        &self.preview_rgba,
                        self.preview_width,
                        self.preview_height,
                        src_stride,
                        sx,
                        sy,
                    ),
                };
                let offset = ((y as usize * output_width as usize) + x as usize) * 4;
                out[offset..offset + 4].copy_from_slice(&rgba);
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let render_duration_ms = render_started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        #[cfg(target_arch = "wasm32")]
        let render_duration_ms = 0u32;
        self.backend_runtime
            .borrow_mut()
            .record_present_with_timing(
                self.preview_source_format,
                out_len,
                &self.limits,
                render_duration_ms,
            );
        out
    }

    fn source_window(&self) -> Option<(f64, f64, f64, f64)> {
        if !self.has_preview() {
            return None;
        }
        let zoom = self.model.viewport.zoom;
        if !zoom.is_finite() || zoom <= 0.0 {
            return None;
        }
        let src_w_total = self.preview_width as f64;
        let src_h_total = self.preview_height as f64;
        let mut src_w = self.model.viewport.width as f64 / zoom;
        let mut src_h = self.model.viewport.height as f64 / zoom;
        src_w = clamp_f64(src_w, 1.0, src_w_total);
        src_h = clamp_f64(src_h, 1.0, src_h_total);

        let cx = clamp_f64(
            self.view_center_x,
            0.0,
            (self.preview_width.saturating_sub(1)) as f64,
        );
        let cy = clamp_f64(
            self.view_center_y,
            0.0,
            (self.preview_height.saturating_sub(1)) as f64,
        );

        let max_start_x = (src_w_total - src_w).max(0.0);
        let max_start_y = (src_h_total - src_h).max(0.0);
        let start_x = clamp_f64(cx - src_w * 0.5, 0.0, max_start_x);
        let start_y = clamp_f64(cy - src_h * 0.5, 0.0, max_start_y);
        Some((start_x, start_y, src_w, src_h))
    }
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

fn parse_tri_planar_plane(plane: &str) -> Option<TriPlanarPlane> {
    match plane.to_ascii_lowercase().as_str() {
        "axial" => Some(TriPlanarPlane::Axial),
        "coronal" => Some(TriPlanarPlane::Coronal),
        "sagittal" => Some(TriPlanarPlane::Sagittal),
        _ => None,
    }
}

fn parse_slab_mode(mode: &str) -> Option<SlabMode> {
    match mode.to_ascii_lowercase().as_str() {
        "average" => Some(SlabMode::Average),
        "max" | "mip" => Some(SlabMode::Max),
        _ => None,
    }
}

fn clamp_f64(value: f64, lo: f64, hi: f64) -> f64 {
    if value < lo {
        lo
    } else if value > hi {
        hi
    } else {
        value
    }
}

fn sample_nearest_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    sx: f64,
    sy: f64,
) -> [u8; 4] {
    let x = clamp_f64(sx, 0.0, (width.saturating_sub(1)) as f64).round() as usize;
    let y = clamp_f64(sy, 0.0, (height.saturating_sub(1)) as f64).round() as usize;
    let offset = y * stride + x * 4;
    [
        rgba[offset],
        rgba[offset + 1],
        rgba[offset + 2],
        rgba[offset + 3],
    ]
}

fn sample_linear_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    sx: f64,
    sy: f64,
) -> [u8; 4] {
    let x = clamp_f64(sx, 0.0, (width.saturating_sub(1)) as f64);
    let y = clamp_f64(sy, 0.0, (height.saturating_sub(1)) as f64);

    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(width.saturating_sub(1) as usize);
    let y1 = (y0 + 1).min(height.saturating_sub(1) as usize);
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;

    let p00 = y0 * stride + x0 * 4;
    let p10 = y0 * stride + x1 * 4;
    let p01 = y1 * stride + x0 * 4;
    let p11 = y1 * stride + x1 * 4;

    let mut out = [0u8; 4];
    for channel in 0..4 {
        let c00 = rgba[p00 + channel] as f64;
        let c10 = rgba[p10 + channel] as f64;
        let c01 = rgba[p01 + channel] as f64;
        let c11 = rgba[p11 + channel] as f64;
        let top = c00 + (c10 - c00) * tx;
        let bottom = c01 + (c11 - c01) * tx;
        let blended = top + (bottom - top) * ty;
        out[channel] = blended.round().clamp(0.0, 255.0) as u8;
    }
    out
}

fn is_valid_zoom(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use dicom_core::Tag;

    fn decode_failure_reason(bytes: &[u8]) -> String {
        let limits = Limits::default();
        let mut meta_reader =
            P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
        let meta = match meta_reader.read_meta() {
            Ok(meta) => meta,
            Err(err) => return format!("read_meta failed: {:?}", err.kind),
        };

        let mut dataset_reader =
            P10Reader::with_limits(BytesSource::new(bytes.to_vec()), limits.clone());
        let dataset = match dataset_reader.read_dataset() {
            Ok(dataset) => dataset,
            Err(err) => return format!("read_dataset failed: {:?}", err.kind),
        };

        let pipeline = PixelPipeline::new(PixelPipelineConfig {
            limits: limits.clone(),
            ..PixelPipelineConfig::default()
        });
        let frame = match pipeline.decode_input(PixelDecodeInput {
            dataset: &dataset,
            transfer_syntax_uid: &meta.transfer_syntax_uid,
            frame_index: 0,
        }) {
            Ok(frame) => frame,
            Err(err) => return format!("decode_input failed: {:?}", err.kind),
        };

        match display_frame_to_rgba(frame, &limits) {
            Some(_) => "ok".to_string(),
            None => "display_frame_to_rgba returned None".to_string(),
        }
    }

    fn meta_element_ui(tag: Tag, value: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(b"UI");
        let mut bytes = value.as_bytes().to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&bytes);
        buf
    }

    fn dataset_element_explicit(tag: Tag, vr: [u8; 2], value: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&tag.0.to_le_bytes());
        buf.extend_from_slice(&tag.1.to_le_bytes());
        buf.extend_from_slice(&vr);
        let mut bytes = value.to_vec();
        if bytes.len() % 2 == 1 {
            bytes.push(0);
        }
        match &vr {
            b"OB" | b"OW" | b"SQ" | b"UN" | b"UT" => {
                buf.extend_from_slice(&0u16.to_le_bytes());
                buf.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            }
            _ => {
                buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            }
        }
        buf.extend_from_slice(&bytes);
        buf
    }

    fn sample_p10() -> Vec<u8> {
        let mut dataset = Vec::new();
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0016),
            *b"UI",
            b"1.2.840.10008.5.1.4.1.1.7",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0008, 0x0018),
            *b"UI",
            b"1.2.3.4.5",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000D),
            *b"UI",
            b"1.2.3",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0020, 0x000E),
            *b"UI",
            b"1.2.3.4",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0002),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0004),
            *b"CS",
            b"MONOCHROME2",
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0010),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0011),
            *b"US",
            &1u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0100),
            *b"US",
            &16u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0101),
            *b"US",
            &12u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0102),
            *b"US",
            &11u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x0028, 0x0103),
            *b"US",
            &0u16.to_le_bytes(),
        ));
        dataset.extend_from_slice(&dataset_element_explicit(
            Tag(0x7FE0, 0x0010),
            *b"OB",
            &[0u8],
        ));

        let mut bytes = vec![0u8; 128];
        bytes.extend_from_slice(b"DICM");
        bytes.extend_from_slice(&meta_element_ui(Tag(0x0002, 0x0010), "1.2.840.10008.1.2.1"));
        bytes.extend_from_slice(&dataset);
        bytes
    }

    #[test]
    fn escape_metadata_html_escapes_entities() {
        // REQ-SEC-433: HTML-escape metadata in web contexts.
        let raw = r#"<script>alert("x")</script>&"#;
        let escaped = escape_metadata_html(raw);
        assert_eq!(
            escaped,
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;&amp;"
        );
    }

    #[test]
    fn network_disabled_by_default() {
        // REQ-SEC-433: no network by default in WASM builds.
        let mut viewer = WasmViewer::new(1, 1);
        assert!(!viewer.network_enabled());
        #[cfg(feature = "network")]
        {
            assert!(viewer.set_network_enabled(true));
            assert!(viewer.network_enabled());
        }
        #[cfg(not(feature = "network"))]
        {
            assert!(!viewer.set_network_enabled(true));
            assert!(!viewer.network_enabled());
        }
        assert!(viewer.set_network_enabled(false));
        assert!(!viewer.network_enabled());
    }

    #[test]
    fn zoom_rejects_non_finite_values() {
        // REQ-WASM-301: reject non-finite numeric inputs at the WASM boundary.
        let mut viewer = WasmViewer::new(10, 10);
        let original = viewer.viewport_zoom();
        assert!(!viewer.set_zoom(f64::NAN));
        assert!(!viewer.set_zoom(f64::INFINITY));
        assert_eq!(viewer.viewport_zoom(), original);
    }

    #[test]
    fn ingest_bytes_accepts_valid_p10_and_tracks_size() {
        // REQ-WASM-301: byte-ingestion validates DICOM payloads before accepting.
        let mut viewer = WasmViewer::new(1, 1);
        let bytes = sample_p10();
        assert!(
            viewer.ingest_bytes(&bytes),
            "{}",
            decode_failure_reason(&bytes)
        );
        assert_eq!(viewer.loaded_input_bytes(), bytes.len());
        assert!(viewer.has_preview());
        assert_eq!(viewer.preview_width(), 1);
        assert_eq!(viewer.preview_height(), 1);
        assert_eq!(viewer.preview_rgba_bytes().len(), 4);
    }

    #[test]
    fn load_bytes_rejects_non_dicom_input() {
        // REQ-WASM-301: malformed bytes must fail closed.
        let mut viewer = WasmViewer::new(1, 1);
        assert!(!viewer.load_bytes(b"not a dicom payload"));
        assert_eq!(viewer.loaded_input_bytes(), 0);
        assert!(!viewer.has_preview());
        assert!(viewer.preview_rgba_bytes().is_empty());
    }

    #[test]
    fn rejected_payload_does_not_mutate_existing_preview() {
        // REQ-WASM-301: failed ingest must not clobber previously accepted preview state.
        let mut viewer = WasmViewer::new(1, 1);
        let valid = sample_p10();
        assert!(
            viewer.load_bytes(&valid),
            "{}",
            decode_failure_reason(&valid)
        );

        let before_len = viewer.loaded_input_bytes();
        let before_width = viewer.preview_width();
        let before_height = viewer.preview_height();
        let before_rgba = viewer.preview_rgba_bytes();

        assert!(!viewer.load_bytes(b"invalid dicom"));
        assert_eq!(viewer.loaded_input_bytes(), before_len);
        assert_eq!(viewer.preview_width(), before_width);
        assert_eq!(viewer.preview_height(), before_height);
        assert_eq!(viewer.preview_rgba_bytes(), before_rgba);
    }

    #[test]
    fn interpolation_mode_accepts_known_values_only() {
        // REQ-HI-145/REQ-HI-361: interpolation mode is explicit and deterministic.
        let mut viewer = WasmViewer::new(1, 1);
        assert_eq!(viewer.interpolation_mode(), "linear");
        assert!(viewer.set_interpolation_mode("nearest"));
        assert_eq!(viewer.interpolation_mode(), "nearest");
        assert!(viewer.set_interpolation_mode("linear"));
        assert_eq!(viewer.interpolation_mode(), "linear");
        assert!(!viewer.set_interpolation_mode("cubic"));
        assert_eq!(viewer.interpolation_mode(), "linear");
    }

    #[test]
    fn viewport_rerender_returns_requested_size_bytes() {
        // REQ-HI-145: viewport rendering must be regenerated from source for target output size.
        let mut viewer = WasmViewer::new(16, 12);
        let valid = sample_p10();
        assert!(
            viewer.load_bytes(&valid),
            "{}",
            decode_failure_reason(&valid)
        );
        assert!(viewer.set_zoom(2.0));
        let bytes = viewer.render_viewport_rgba_bytes_for_size(20, 10);
        assert_eq!(bytes.len(), 20 * 10 * 4);
    }

    #[test]
    fn recenter_fraction_enforces_bounds() {
        // REQ-HI-145: ROI recentering rejects invalid normalized fractions.
        let mut viewer = WasmViewer::new(16, 12);
        let valid = sample_p10();
        assert!(
            viewer.load_bytes(&valid),
            "{}",
            decode_failure_reason(&valid)
        );
        assert!(!viewer.recenter_from_viewport_fraction(-0.1, 0.5));
        assert!(!viewer.recenter_from_viewport_fraction(0.5, 1.1));
        assert!(viewer.recenter_from_viewport_fraction(0.5, 0.5));
    }
}
