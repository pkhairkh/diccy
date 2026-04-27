#![deny(missing_docs)]

//! GPU-agnostic viewer core types.

// Re-export shared value types from the dicom-types crate so that downstream
// consumers can import them directly from viewer-core.
pub use dicom_types::{PatientPosition, WindowLevel};

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ===========================================================================
// S12-T6: Monotonic Tick & TickProvider
// ===========================================================================

/// A monotonically increasing tick value that can only be advanced, never
/// decremented or set to an arbitrary value.
///
/// The internal `u64` is private; the only way to obtain a new value is via
/// [`MonotonicTick::zero`] (the origin) or [`MonotonicTick::next`] (increment).
/// This guarantees, at the type level, that ticks never go backwards within a
/// single provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonotonicTick(u64);

impl MonotonicTick {
    /// The origin tick (value 0).
    pub const fn zero() -> Self {
        MonotonicTick(0)
    }

    /// Advance to the next tick. Saturates at `u64::MAX`.
    pub fn next(self) -> Self {
        MonotonicTick(self.0.saturating_add(1))
    }

    /// Return the raw `u64` value of this tick.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl Default for MonotonicTick {
    fn default() -> Self {
        Self::zero()
    }
}

impl fmt::Display for MonotonicTick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Trait for types that provide monotonically increasing ticks.
///
/// Implementors guarantee that every call to [`TickProvider::next_tick`] returns
/// a value strictly greater than the previous one (until `u64::MAX` saturation).
pub trait TickProvider {
    /// Return the current tick without advancing it.
    fn current_tick(&self) -> MonotonicTick;

    /// Advance the tick and return the new value.
    fn next_tick(&mut self) -> MonotonicTick;
}

/// A global tick provider backed by an `AtomicU64`, suitable for coordinating
/// ticks across multiple stores (`MeasurementStore`, `SegmentationStore`,
/// `Annotation3dStore`, `ViewerModel`, etc.) so that no two subsystems ever
/// produce the same tick value.
///
/// Thread-safe: the interior mutability is handled via atomic operations.
pub struct GlobalTickProvider {
    counter: AtomicU64,
}

impl Default for GlobalTickProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalTickProvider {
    /// Create a new provider starting at tick 0.
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    /// Create a provider starting at a given tick value.
    ///
    /// Intended for restoring from persisted state; the tick will only
    /// advance from here via `next_tick()`.
    pub fn starting_at(initial: u64) -> Self {
        Self {
            counter: AtomicU64::new(initial),
        }
    }
}

impl TickProvider for GlobalTickProvider {
    fn current_tick(&self) -> MonotonicTick {
        MonotonicTick(self.counter.load(Ordering::SeqCst))
    }

    fn next_tick(&mut self) -> MonotonicTick {
        // fetch_add returns the *previous* value; add 1 for the new one.
        let prev = self.counter.fetch_add(1, Ordering::SeqCst);
        MonotonicTick(prev.saturating_add(1))
    }
}

impl fmt::Debug for GlobalTickProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlobalTickProvider")
            .field("current", &self.current_tick())
            .finish()
    }
}

pub mod cache;
pub mod clinical;
pub mod gsdf;
pub mod hanging_protocol;
pub mod mpr;
pub mod prefetch;
pub mod volume;

pub use cache::{CacheMetrics, DeterministicCache};
pub use clinical::{
    compute_roi_statistics, interpolate_slices_linear, interpolate_slices_morphological,
    region_grow, threshold_segment_volume, Annotation3d, Annotation3dStore, BrushConfig, BrushMode,
    BrushStroke, ClinicalAuditEvent, ClinicalError, ClippingMode, ExportBundle, FusionOverlayState,
    FusionRegistrationState, InterpolationMethod, LabelMap3D, MeasurementRecord, MeasurementStore,
    MipProjectionMode, RegionGrowSeed, RoiShape, RoiStatistics, RtDoseOverlayState,
    RtssOverlayState, SegmentationDiff, SegmentationRecord, SegmentationSource, SegmentationStore,
    SegmentationStyle, ThresholdConfig, VolumeWorkflowCapabilities, VolumeWorkflowState,
    VolumeWorkflowStatus,
};
// S12-T6: Monotonic tick types (defined above, already in scope)
pub use gsdf::{
    apply_calibration, compute_conformance, generate_calibration_table, generate_gsdf_lut,
    jnd_to_luminance, luminance_to_jnd, DisplayCalibrationConfig, GsdfError, GsdfLut,
    GSDF_P_VALUE_COUNT,
};
pub use hanging_protocol::{
    ct_chest_abdomen_protocol, default_fallback_protocol, mammography_protocol,
    DisplaySetAssignment, HangingProtocol, HangingProtocolEngine, HangingProtocolError,
    HangingProtocolMatch, ImageSetDefinition, MatchCriterion, StudyMatchContext, TimePerspective,
};
pub use mpr::{
    patient_request_to_voxel_request, quantize_plane_parameter, reslice_volume,
    reslice_volume_patient, MprError, MprFrame, MprLimits, MprPlane, MprRequest, PatientMprPlane,
    PatientMprRequest, ResampleKernel, SlabMode,
};
pub use prefetch::{
    default_prefetch_rules, PrefetchEngine, PrefetchPriority, PrefetchRequest, PrefetchRule,
    PrefetchStats, PrefetchStatus, WorklistTrigger,
};
pub use volume::{
    LegacyVolumeGrid, PatientGeometry, SlicePlane, VolumeAssemblyConfig, VolumeError, VolumeGrid,
};

/// A 2D point in image space (continuous coordinates).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ImagePoint {
    /// X coordinate in image space.
    pub x: f64,
    /// Y coordinate in image space.
    pub y: f64,
}

/// A 2D point in screen space (pixel coordinates).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ScreenPoint {
    /// X coordinate in screen space.
    pub x: f64,
    /// Y coordinate in screen space.
    pub y: f64,
}

/// A 2D point in normalized device coordinates (NDC).
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct NdcPoint {
    /// X coordinate in NDC.
    pub x: f64,
    /// Y coordinate in NDC.
    pub y: f64,
}

/// Errors emitted by viewport operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewportError {
    /// Rotation degrees are not a multiple of 90.
    InvalidRotationDegrees {
        /// Provided degrees value.
        degrees: i32,
    },
}

impl fmt::Display for ViewportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewportError::InvalidRotationDegrees { degrees } => {
                write!(f, "rotation degrees must be a multiple of 90: {degrees}")
            }
        }
    }
}

impl std::error::Error for ViewportError {}

/// Unified error type for the viewer-core crate.
///
/// Aggregates all sub-domain error types into a single enum so callers can
/// handle viewer errors uniformly while still matching on the specific domain
/// when needed. Integrates with [`dicom_core::Error`] via
/// `From<ViewerError> for Box<dicom_core::Error>` for backward compatibility
/// with the top-level error model.
#[derive(Debug, Clone, PartialEq)]
pub enum ViewerError {
    /// Clinical workflow error (measurements, segmentation, fusion, RT overlays).
    Clinical(ClinicalError),
    /// Multi-planar reconstruction error.
    Mpr(MprError),
    /// Volume assembly error.
    Volume(VolumeError),
    /// Grayscale Standard Display Function error.
    Gsdf(GsdfError),
    /// Hanging protocol matching error.
    HangingProtocol(HangingProtocolError),
    /// Viewport transform error.
    Viewport(ViewportError),
}

impl fmt::Display for ViewerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewerError::Clinical(err) => write!(f, "clinical error: {err}"),
            ViewerError::Mpr(err) => write!(f, "mpr error: {err}"),
            ViewerError::Volume(err) => write!(f, "volume error: {err}"),
            ViewerError::Gsdf(err) => write!(f, "gsdf error: {err}"),
            ViewerError::HangingProtocol(err) => write!(f, "hanging protocol error: {err}"),
            ViewerError::Viewport(err) => write!(f, "viewport error: {err}"),
        }
    }
}

impl std::error::Error for ViewerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ViewerError::Clinical(err) => Some(err),
            ViewerError::Mpr(err) => Some(err),
            ViewerError::Volume(err) => Some(err),
            ViewerError::Gsdf(err) => Some(err),
            ViewerError::HangingProtocol(err) => Some(err),
            ViewerError::Viewport(err) => Some(err),
        }
    }
}

impl From<ClinicalError> for ViewerError {
    fn from(err: ClinicalError) -> Self {
        ViewerError::Clinical(err)
    }
}

impl From<MprError> for ViewerError {
    fn from(err: MprError) -> Self {
        ViewerError::Mpr(err)
    }
}

impl From<VolumeError> for ViewerError {
    fn from(err: VolumeError) -> Self {
        ViewerError::Volume(err)
    }
}

impl From<GsdfError> for ViewerError {
    fn from(err: GsdfError) -> Self {
        ViewerError::Gsdf(err)
    }
}

impl From<HangingProtocolError> for ViewerError {
    fn from(err: HangingProtocolError) -> Self {
        ViewerError::HangingProtocol(err)
    }
}

impl From<ViewportError> for ViewerError {
    fn from(err: ViewportError) -> Self {
        ViewerError::Viewport(err)
    }
}

impl From<ViewerError> for Box<dicom_core::Error> {
    fn from(err: ViewerError) -> Self {
        let (kind, message) = match &err {
            ViewerError::Clinical(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("clinical: {e}"),
                },
                format!("viewer clinical error: {e}"),
            ),
            ViewerError::Mpr(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("mpr: {e}"),
                },
                format!("viewer mpr error: {e}"),
            ),
            ViewerError::Volume(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("volume: {e}"),
                },
                format!("viewer volume error: {e}"),
            ),
            ViewerError::Gsdf(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("gsdf: {e}"),
                },
                format!("viewer gsdf error: {e}"),
            ),
            ViewerError::HangingProtocol(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("hanging-protocol: {e}"),
                },
                format!("viewer hanging protocol error: {e}"),
            ),
            ViewerError::Viewport(e) => (
                dicom_core::ErrorKind::InternalError {
                    detail: format!("viewport: {e}"),
                },
                format!("viewer viewport error: {e}"),
            ),
        };
        Box::new(dicom_core::Error::from_kind(kind, message))
    }
}

/// Window/level selection for display.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum WindowLevelState {
    /// Auto windowing.
    Auto,
    /// Explicit window center/width.
    Explicit {
        /// Window center.
        center: f64,
        /// Window width.
        width: f64,
    },
}

/// A 2D viewport description.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewport2D {
    /// Screen width in pixels.
    pub width: u32,
    /// Screen height in pixels.
    pub height: u32,
    /// Image-space point mapped to the center of the screen.
    pub center_img: ImagePoint,
    /// Zoom factor (screen pixels per image pixel).
    pub zoom: f64,
    /// Rotation in multiples of 90 degrees (0..=3).
    pub rotation_quadrants: i32,
    /// Horizontal flip.
    pub flip_x: bool,
    /// Vertical flip.
    pub flip_y: bool,
    /// Window/level selection.
    pub window_level: WindowLevelState,
}

impl Viewport2D {
    /// Create a new viewport with default transform state.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            center_img: ImagePoint { x: 0.0, y: 0.0 },
            zoom: 1.0,
            rotation_quadrants: 0,
            flip_x: false,
            flip_y: false,
            window_level: WindowLevelState::Auto,
        }
    }

    /// Return the screen-space center point.
    pub fn screen_center(&self) -> ScreenPoint {
        ScreenPoint {
            x: self.width as f64 * 0.5,
            y: self.height as f64 * 0.5,
        }
    }

    /// Return the canonical pixel-center position for a pixel index.
    pub fn pixel_center(i: u32, j: u32) -> ImagePoint {
        ImagePoint {
            x: i as f64 + 0.5,
            y: j as f64 + 0.5,
        }
    }

    /// Set rotation in degrees. Only multiples of 90 are accepted.
    pub fn set_rotation_degrees(&mut self, degrees: i32) -> Result<(), ViewportError> {
        if degrees % 90 != 0 {
            return Err(ViewportError::InvalidRotationDegrees { degrees });
        }
        let mut quadrants = (degrees / 90) % 4;
        if quadrants < 0 {
            quadrants += 4;
        }
        self.rotation_quadrants = quadrants;
        Ok(())
    }

    /// Convert an image-space point into screen space.
    pub fn image_to_screen(&self, point: ImagePoint) -> ScreenPoint {
        let mut dx = point.x - self.center_img.x;
        let mut dy = point.y - self.center_img.y;
        if self.flip_x {
            dx = -dx;
        }
        if self.flip_y {
            dy = -dy;
        }
        let (rdx, rdy) = match self.rotation_quadrants.rem_euclid(4) {
            0 => (dx, dy),
            1 => (dy, -dx),
            2 => (-dx, -dy),
            _ => (-dy, dx),
        };
        let zoomed_x = rdx * self.zoom;
        let zoomed_y = rdy * self.zoom;
        let center = self.screen_center();
        ScreenPoint {
            x: center.x + zoomed_x,
            y: center.y + zoomed_y,
        }
    }

    /// Convert a screen-space point into image space.
    pub fn screen_to_image(&self, point: ScreenPoint) -> ImagePoint {
        let center = self.screen_center();
        let mut dx = point.x - center.x;
        let mut dy = point.y - center.y;
        if self.zoom != 0.0 {
            dx /= self.zoom;
            dy /= self.zoom;
        }
        let (rdx, rdy) = match self.rotation_quadrants.rem_euclid(4) {
            0 => (dx, dy),
            1 => (-dy, dx),
            2 => (-dx, -dy),
            _ => (dy, -dx),
        };
        let mut ix = rdx;
        let mut iy = rdy;
        if self.flip_x {
            ix = -ix;
        }
        if self.flip_y {
            iy = -iy;
        }
        ImagePoint {
            x: ix + self.center_img.x,
            y: iy + self.center_img.y,
        }
    }
}

/// Supported tool modes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ToolMode {
    /// No active tool.
    None,
    /// Distance measurement tool.
    Distance,
    /// Angle measurement tool.
    Angle,
    /// Probe tool.
    Probe,
}

/// Tool state container.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ToolState {
    /// Selected tool mode.
    pub mode: ToolMode,
}

impl Default for ToolState {
    fn default() -> Self {
        Self {
            mode: ToolMode::None,
        }
    }
}

/// Viewer interaction state machine.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum InteractionState {
    /// No active interaction.
    #[default]
    Idle,
    /// Panning in progress.
    Panning {
        /// Last known screen-space position.
        last_screen: ScreenPoint,
    },
    /// Window/level adjustment in progress.
    Windowing {
        /// Last known screen-space position.
        last_screen: ScreenPoint,
    },
    /// Zoom gesture in progress.
    Zooming,
    /// Distance measurement in progress.
    MeasuringDistance {
        /// Start point in image space.
        start_img: ImagePoint,
        /// Current point in image space.
        current_img: ImagePoint,
    },
    /// Angle measurement in progress.
    MeasuringAngle {
        /// Vertex point in image space.
        vertex_img: ImagePoint,
        /// First arm endpoint in image space once captured.
        first_arm_img: Option<ImagePoint>,
        /// Current point in image space.
        current_img: ImagePoint,
    },
    /// Probe in progress.
    Probing {
        /// Current image-space probe position.
        position_img: ImagePoint,
    },
}

/// Pointer buttons used by inputs.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PointerButton {
    /// Primary pointer button.
    Primary,
    /// Secondary pointer button.
    Secondary,
}

/// Keyboard modifier state.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// Shift modifier.
    pub shift: bool,
    /// Control modifier.
    pub ctrl: bool,
    /// Alt modifier.
    pub alt: bool,
}

/// Input events processed by the interaction state machine.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum InputEvent {
    /// Pointer pressed.
    PointerDown {
        /// Button pressed.
        button: PointerButton,
        /// Screen-space position.
        position: ScreenPoint,
        /// Keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Pointer moved.
    PointerMove {
        /// Screen-space position.
        position: ScreenPoint,
        /// Keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Pointer released.
    PointerUp {
        /// Screen-space position.
        position: ScreenPoint,
        /// Keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Wheel/scroll event.
    Wheel {
        /// Scroll delta.
        delta: f32,
        /// Screen-space position.
        position: ScreenPoint,
        /// Keyboard modifiers.
        modifiers: Modifiers,
    },
    /// Explicit completion signal for gestures like zoom.
    EventComplete,
}

/// Deterministic frame navigation state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FrameNavigationState {
    /// Active frame index (zero-based).
    pub frame_index: u32,
    /// Total number of available frames.
    pub total_frames: u32,
}

impl Default for FrameNavigationState {
    fn default() -> Self {
        Self {
            frame_index: 0,
            total_frames: 1,
        }
    }
}

/// Tri-planar view selection axes for MPR controls.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TriPlanarPlane {
    /// Axial view (`z` index).
    Axial,
    /// Coronal view (`y` index).
    Coronal,
    /// Sagittal view (`x` index).
    Sagittal,
}

/// Tri-planar synchronized crosshair and index state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TriPlanarState {
    /// Source volume dimensions as `[x, y, z]`.
    pub volume_dimensions: [u32; 3],
    /// Crosshair voxel coordinate as `[x, y, z]`.
    pub crosshair_voxel: [u32; 3],
    /// Active axial index (`z`).
    pub axial_index: u32,
    /// Active coronal index (`y`).
    pub coronal_index: u32,
    /// Active sagittal index (`x`).
    pub sagittal_index: u32,
}

impl Default for TriPlanarState {
    fn default() -> Self {
        Self {
            volume_dimensions: [1, 1, 1],
            crosshair_voxel: [0, 0, 0],
            axial_index: 0,
            coronal_index: 0,
            sagittal_index: 0,
        }
    }
}

/// Measurement mode selection.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum MeasurementMode {
    /// Pixel-domain measurements only.
    #[default]
    PixelDomain,
    /// Physical-unit measurements (mm).
    PhysicalUnits,
}

/// Measurement units.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementUnit {
    /// Pixel units.
    Pixel,
    /// Millimeter units.
    Millimeter,
    /// Degree units.
    Degree,
}

/// Measurement type identifier.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementKind {
    /// 2D distance measurement.
    Distance2D,
    /// 2D angle measurement.
    Angle2D,
    /// Probe measurement.
    Probe,
}

/// Provenance metadata for calibrated measurements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementProvenance {
    /// Tag identifiers used to calibrate the measurement.
    pub tags: Vec<String>,
}

/// Calibration metadata used for physical-unit measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementCalibration {
    /// Pixel spacing `(row_mm, col_mm)` from DICOM tags.
    pub pixel_spacing: (f64, f64),
    /// Provenance tags backing this calibration.
    pub provenance: MeasurementProvenance,
}

/// Integrator-supplied uncertainty descriptor for quantitative display.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementUncertainty {
    /// Optional absolute uncertainty in millimeters.
    pub absolute_mm: Option<f64>,
    /// Optional relative uncertainty (0..1).
    pub relative_fraction: Option<f64>,
    /// Free-form uncertainty model identifier.
    pub model_id: Option<String>,
}

/// Revision metadata for measurement lifecycle changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementEditRecord {
    /// Monotonic measurement revision number.
    pub revision: u32,
    /// Edit action label.
    pub action: &'static str,
}

/// A measurement record.
#[derive(Debug, Clone, PartialEq)]
pub struct Measurement {
    /// Measurement identifier.
    pub id: String,
    /// Measurement kind.
    pub kind: MeasurementKind,
    /// Value in units when available.
    pub value: Option<f64>,
    /// Measurement units.
    pub unit: MeasurementUnit,
    /// Calibration flag.
    pub calibrated: bool,
    /// Provenance metadata if calibrated.
    pub provenance: Option<MeasurementProvenance>,
    /// Integrator-provided uncertainty metadata.
    pub uncertainty: Option<MeasurementUncertainty>,
    /// Measurement edit history metadata.
    pub edit_history: Vec<MeasurementEditRecord>,
    /// Points used to compute the measurement.
    pub points: Vec<ImagePoint>,
}

/// Window/level values displayed in operator controls.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WindowLevelDisplay {
    /// Current center value.
    pub center: f64,
    /// Current width value.
    pub width: f64,
    /// Preset label.
    pub preset: &'static str,
}

/// Quantitative context banner for operator-facing measurements.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantitativeContextBanner {
    /// Measurement mode currently selected.
    pub mode: MeasurementMode,
    /// Operator-visible disclaimer text.
    pub disclaimer: String,
    /// Calibrated availability state.
    pub calibrated: bool,
    /// Optional uncertainty metadata.
    pub uncertainty: Option<MeasurementUncertainty>,
}

/// Interpretation layout indicators for safety-critical overlays.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct InterpretationLayoutIndicators {
    /// Orientation marker visibility.
    pub orientation_markers_visible: bool,
    /// Scale ruler visibility.
    pub scale_rulers_visible: bool,
}

impl Default for InterpretationLayoutIndicators {
    fn default() -> Self {
        Self {
            orientation_markers_visible: true,
            scale_rulers_visible: true,
        }
    }
}

/// Probe readout payload with coordinate and modality context.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReadout {
    /// Probe coordinate in image space.
    pub coordinate: ImagePoint,
    /// Pixel/sample value when available.
    pub value: Option<f64>,
    /// Value-unit context.
    pub units: Option<String>,
    /// Modality context label.
    pub modality: String,
}

/// Measurement action state used by operator-facing controls.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MeasurementUiState {
    /// Pixel-domain measurements are active.
    PixelDomainOnly,
    /// Physical-unit measurements are available and calibrated.
    PhysicalUnitsReady,
    /// Physical-unit measurements are selected but blocked by invalid/missing calibration.
    PhysicalUnitsBlocked,
}

/// Overlay layer precedence for deterministic composition.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OverlayLayer {
    /// CPU pixel pipeline output including DICOM overlays.
    PixelData,
    /// GSPS shutters.
    GspsShutter,
    /// GSPS graphics.
    GspsGraphics,
    /// Segmentation overlays.
    Segmentation,
    /// User annotations.
    Annotation,
}

/// Overlay render item with deterministic tie-break fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayRenderItem {
    /// Overlay layer.
    pub layer: OverlayLayer,
    /// Secondary deterministic ordering key for same-layer objects.
    pub tie_break: u32,
    /// Stable identifier used as final deterministic tie-break.
    pub stable_id: String,
}

/// Sort overlay items in deterministic render order.
pub fn sort_overlay_render_items(items: &mut [OverlayRenderItem]) {
    items.sort_by(|left, right| {
        overlay_layer_rank(left.layer)
            .cmp(&overlay_layer_rank(right.layer))
            .then(left.tie_break.cmp(&right.tie_break))
            .then(left.stable_id.cmp(&right.stable_id))
    });
}

/// Deterministic warnings emitted by the viewer core.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ViewerWarning {
    /// Physical units requested but calibration is unavailable.
    UncalibratedPhysicalUnits,
    /// Measurement input was invalid.
    InvalidMeasurementInput,
}

/// Severity class for operator-visible issues.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ViewerIssueSeverity {
    /// Informational condition.
    Info,
    /// Non-blocking warning.
    Warning,
    /// Blocking condition.
    Blocking,
    /// Critical condition requiring escalation.
    Critical,
}

/// Stable operator-visible issue representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerIssue {
    /// Stable issue code.
    pub code: &'static str,
    /// Severity classification.
    pub severity: ViewerIssueSeverity,
    /// Plain-language summary.
    pub message: &'static str,
    /// Deterministic next-step guidance.
    pub guidance: &'static str,
}

/// Blocking-action gate status derived from unresolved blocking issues.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BlockingActionGate {
    /// Whether final signoff is allowed.
    pub can_finalize_signoff: bool,
    /// Whether controlled export is allowed.
    pub can_controlled_export: bool,
}

/// UI control availability state with structured rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlAvailability {
    /// Control enabled state.
    pub enabled: bool,
    /// Structured rationale when disabled.
    pub rationale: Option<String>,
}

/// Error-to-audit reference view model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorAuditReference {
    /// Stable issue code.
    pub issue_code: String,
    /// Linked audit event identifier.
    pub audit_event_id: String,
}

/// Escalation pathway metadata for critical conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscalationPathway {
    /// Workflow target for escalation.
    pub route: String,
    /// Escalation requirement flag.
    pub required: bool,
}

/// Persistent runtime security banner model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSecurityBanner {
    /// Banner visibility.
    pub visible: bool,
    /// Persistent display requirement.
    pub persistent: bool,
    /// Banner message text.
    pub message: String,
}

/// Side-by-side comparison synchronization state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ComparisonSyncState {
    /// Pan/zoom lock state.
    pub pan_zoom_locked: bool,
    /// Frame lock state.
    pub frame_locked: bool,
    /// Synchronization outcome.
    pub synchronized: bool,
}

/// Hanging protocol layout template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HangingProtocolTemplate {
    /// Template identifier.
    pub template_id: String,
    /// Row count.
    pub rows: u8,
    /// Column count.
    pub columns: u8,
}

/// Result of applying a hanging-protocol template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HangingProtocolApplication {
    /// Applied template identifier.
    pub applied_template_id: String,
    /// Indicates fallback selection.
    pub used_fallback: bool,
}

/// Study/series context pair used for prior-study alignment checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySeriesContext {
    /// Study Instance UID.
    pub study_uid: String,
    /// Series Instance UID.
    pub series_uid: String,
}

const TAG_PIXEL_SPACING_ID: &str = "0028,0030";

/// Viewer model state.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewerModel {
    /// Current viewport state.
    pub viewport: Viewport2D,
    /// Deterministic frame navigation state.
    pub frame_navigation: FrameNavigationState,
    /// Tri-planar synchronized crosshair/index state.
    pub tri_planar: TriPlanarState,
    /// Interaction state.
    pub interaction: InteractionState,
    /// Tool state.
    pub tools: ToolState,
    /// Measurement mode selection.
    pub measurement_mode: MeasurementMode,
    /// Active calibration context for physical-unit measurements.
    pub measurement_calibration: Option<MeasurementCalibration>,
    /// Integrator-provided uncertainty descriptor.
    pub measurement_uncertainty: Option<MeasurementUncertainty>,
    /// Completed measurements.
    pub measurements: Vec<Measurement>,
    /// Stable warnings emitted during processing.
    pub warnings: Vec<ViewerWarning>,
    /// Deterministic measurement id counter.
    pub measurement_counter: u64,
    /// Safety-critical interpretation layout visibility state.
    pub interpretation_layout: InterpretationLayoutIndicators,
}

impl ViewerModel {
    /// Create a new viewer model.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            viewport: Viewport2D::new(width, height),
            frame_navigation: FrameNavigationState::default(),
            tri_planar: TriPlanarState::default(),
            interaction: InteractionState::Idle,
            tools: ToolState::default(),
            measurement_mode: MeasurementMode::default(),
            measurement_calibration: None,
            measurement_uncertainty: None,
            measurements: Vec::new(),
            warnings: Vec::new(),
            measurement_counter: 0,
            interpretation_layout: InterpretationLayoutIndicators::default(),
        }
    }

    /// Set the active tool mode explicitly.
    pub fn set_tool_mode(&mut self, mode: ToolMode) {
        self.tools.mode = mode;
    }

    /// Set measurement mode explicitly.
    pub fn set_measurement_mode(&mut self, mode: MeasurementMode) {
        self.measurement_mode = mode;
    }

    /// Reset transform state to canonical orientation in one step.
    pub fn reset_to_canonical_orientation(&mut self) {
        self.viewport.rotation_quadrants = 0;
        self.viewport.flip_x = false;
        self.viewport.flip_y = false;
    }

    /// Set total frames for frame navigation.
    ///
    /// Returns `false` when `total_frames` is zero.
    pub fn set_total_frames(&mut self, total_frames: u32) -> bool {
        if total_frames == 0 {
            return false;
        }
        self.frame_navigation.total_frames = total_frames;
        if self.frame_navigation.frame_index >= total_frames {
            self.frame_navigation.frame_index = total_frames - 1;
        }
        true
    }

    /// Set active frame index.
    ///
    /// Returns `false` when `frame_index` is out of range.
    pub fn set_frame_index(&mut self, frame_index: u32) -> bool {
        if frame_index >= self.frame_navigation.total_frames {
            return false;
        }
        self.frame_navigation.frame_index = frame_index;
        true
    }

    /// Set source volume dimensions for tri-planar controls.
    ///
    /// Returns `false` when any dimension is zero.
    pub fn set_mpr_volume_dimensions(&mut self, dimensions: [u32; 3]) -> bool {
        if dimensions.contains(&0) {
            return false;
        }
        self.tri_planar.volume_dimensions = dimensions;
        self.clamp_crosshair_to_volume();
        self.sync_tri_planar_indices_from_crosshair();
        true
    }

    /// Set one tri-planar plane index and synchronize crosshair state.
    ///
    /// Returns `false` when index exceeds source dimensions.
    pub fn set_mpr_plane_index(&mut self, plane: TriPlanarPlane, index: u32) -> bool {
        let max_index = match plane {
            TriPlanarPlane::Sagittal => self.tri_planar.volume_dimensions[0].saturating_sub(1),
            TriPlanarPlane::Coronal => self.tri_planar.volume_dimensions[1].saturating_sub(1),
            TriPlanarPlane::Axial => self.tri_planar.volume_dimensions[2].saturating_sub(1),
        };
        if index > max_index {
            return false;
        }
        match plane {
            TriPlanarPlane::Sagittal => self.tri_planar.crosshair_voxel[0] = index,
            TriPlanarPlane::Coronal => self.tri_planar.crosshair_voxel[1] = index,
            TriPlanarPlane::Axial => self.tri_planar.crosshair_voxel[2] = index,
        }
        self.sync_tri_planar_indices_from_crosshair();
        true
    }

    /// Step one plane index by signed delta with deterministic clamping.
    ///
    /// Returns `true` when the index changed.
    pub fn step_mpr_plane(&mut self, plane: TriPlanarPlane, delta: i32) -> bool {
        let current = match plane {
            TriPlanarPlane::Sagittal => self.tri_planar.sagittal_index,
            TriPlanarPlane::Coronal => self.tri_planar.coronal_index,
            TriPlanarPlane::Axial => self.tri_planar.axial_index,
        } as i64;
        let max_index = match plane {
            TriPlanarPlane::Sagittal => self.tri_planar.volume_dimensions[0].saturating_sub(1),
            TriPlanarPlane::Coronal => self.tri_planar.volume_dimensions[1].saturating_sub(1),
            TriPlanarPlane::Axial => self.tri_planar.volume_dimensions[2].saturating_sub(1),
        } as i64;
        let next = (current + i64::from(delta)).clamp(0, max_index) as u32;
        let changed = next != current as u32;
        let _ = self.set_mpr_plane_index(plane, next);
        changed
    }

    /// Set tri-planar crosshair voxel and synchronize all plane indices.
    ///
    /// Returns `false` when voxel is out of source bounds.
    pub fn set_mpr_crosshair_voxel(&mut self, voxel: [u32; 3]) -> bool {
        let [dx, dy, dz] = self.tri_planar.volume_dimensions;
        if voxel[0] >= dx || voxel[1] >= dy || voxel[2] >= dz {
            return false;
        }
        self.tri_planar.crosshair_voxel = voxel;
        self.sync_tri_planar_indices_from_crosshair();
        true
    }

    /// Return tri-planar synchronization state.
    pub fn tri_planar_state(&self) -> TriPlanarState {
        self.tri_planar
    }

    /// Step frame index by signed delta with deterministic clamping.
    ///
    /// Returns `true` when the frame index changed.
    pub fn step_frame(&mut self, delta: i32) -> bool {
        let max_index = (self.frame_navigation.total_frames.saturating_sub(1)) as i64;
        let next = (self.frame_navigation.frame_index as i64 + i64::from(delta)).clamp(0, max_index)
            as u32;
        let changed = next != self.frame_navigation.frame_index;
        self.frame_navigation.frame_index = next;
        changed
    }

    fn clamp_crosshair_to_volume(&mut self) {
        let [dx, dy, dz] = self.tri_planar.volume_dimensions;
        self.tri_planar.crosshair_voxel[0] = self.tri_planar.crosshair_voxel[0].min(dx - 1);
        self.tri_planar.crosshair_voxel[1] = self.tri_planar.crosshair_voxel[1].min(dy - 1);
        self.tri_planar.crosshair_voxel[2] = self.tri_planar.crosshair_voxel[2].min(dz - 1);
    }

    fn sync_tri_planar_indices_from_crosshair(&mut self) {
        self.tri_planar.sagittal_index = self.tri_planar.crosshair_voxel[0];
        self.tri_planar.coronal_index = self.tri_planar.crosshair_voxel[1];
        self.tri_planar.axial_index = self.tri_planar.crosshair_voxel[2];
    }

    /// Return measurement UI availability state.
    pub fn measurement_ui_state(&self) -> MeasurementUiState {
        match self.measurement_mode {
            MeasurementMode::PixelDomain => MeasurementUiState::PixelDomainOnly,
            MeasurementMode::PhysicalUnits => {
                let calibrated = self
                    .measurement_calibration
                    .as_ref()
                    .map(|value| is_valid_spacing(value.pixel_spacing))
                    .unwrap_or(false);
                if calibrated {
                    MeasurementUiState::PhysicalUnitsReady
                } else {
                    MeasurementUiState::PhysicalUnitsBlocked
                }
            }
        }
    }

    /// Return explicit center/width values shown in windowing controls.
    pub fn window_level_display(&self) -> WindowLevelDisplay {
        match self.viewport.window_level {
            WindowLevelState::Auto => WindowLevelDisplay {
                center: 0.0,
                width: 1.0,
                preset: "auto",
            },
            WindowLevelState::Explicit { center, width } => WindowLevelDisplay {
                center,
                width,
                preset: "manual",
            },
        }
    }

    /// Return quantitative context banner for the active measurement mode.
    pub fn quantitative_context_banner(&self) -> QuantitativeContextBanner {
        let calibrated = self
            .measurement_calibration
            .as_ref()
            .map(|value| is_valid_spacing(value.pixel_spacing))
            .unwrap_or(false);
        let disclaimer = match self.measurement_mode {
            MeasurementMode::PixelDomain => {
                "Pixel-domain measurements are non-physical and should be interpreted without physical-unit claims."
                    .to_string()
            }
            MeasurementMode::PhysicalUnits => {
                "Physical-unit output is advisory and must include uncertainty context before clinical interpretation."
                    .to_string()
            }
        };
        QuantitativeContextBanner {
            mode: self.measurement_mode,
            disclaimer,
            calibrated,
            uncertainty: self.measurement_uncertainty.clone(),
        }
    }

    /// Return interpretation layout visibility indicators.
    pub fn interpretation_layout_indicators(&self) -> InterpretationLayoutIndicators {
        self.interpretation_layout
    }

    /// Build a probe readout with coordinate, value units, and modality context.
    pub fn probe_readout(
        &self,
        coordinate: ImagePoint,
        value: Option<f64>,
        units: Option<&str>,
        modality: &str,
    ) -> ProbeReadout {
        ProbeReadout {
            coordinate,
            value,
            units: units.map(str::to_string),
            modality: modality.to_string(),
        }
    }

    /// Render stable operator-visible issues for current warnings.
    pub fn rendered_issues(&self) -> Vec<ViewerIssue> {
        let mut rendered = self
            .warnings
            .iter()
            .copied()
            .map(ViewerWarning::to_issue)
            .collect::<Vec<_>>();
        rendered.sort_by_key(|issue| issue.code);
        rendered
    }

    /// Evaluate whether blocking errors allow signoff/export workflows.
    pub fn blocking_action_gate(&self) -> BlockingActionGate {
        let unresolved = self
            .rendered_issues()
            .iter()
            .any(|issue| issue.severity == ViewerIssueSeverity::Blocking);
        BlockingActionGate {
            can_finalize_signoff: !unresolved,
            can_controlled_export: !unresolved,
        }
    }

    /// Set physical calibration using Pixel Spacing (0028,0030).
    ///
    /// Returns `true` when the provided calibration is valid and accepted.
    pub fn set_pixel_spacing_calibration(&mut self, pixel_spacing: (f64, f64)) -> bool {
        if is_valid_spacing(pixel_spacing) {
            self.measurement_calibration = Some(MeasurementCalibration {
                pixel_spacing,
                provenance: MeasurementProvenance {
                    tags: vec![TAG_PIXEL_SPACING_ID.to_string()],
                },
            });
            true
        } else {
            self.measurement_calibration = None;
            false
        }
    }

    /// Clear any previously configured measurement calibration.
    pub fn clear_measurement_calibration(&mut self) {
        self.measurement_calibration = None;
    }

    /// Configure an uncertainty descriptor for future measurements.
    pub fn set_measurement_uncertainty(&mut self, uncertainty: Option<MeasurementUncertainty>) {
        self.measurement_uncertainty = uncertainty;
    }

    /// Apply a single input event to the interaction state machine.
    pub fn apply_event(&mut self, event: InputEvent) {
        match (self.interaction.clone(), event) {
            (
                InteractionState::Idle,
                InputEvent::PointerDown {
                    button, position, ..
                },
            ) => match (self.tools.mode, button) {
                (ToolMode::None, PointerButton::Primary) => {
                    self.interaction = InteractionState::Panning {
                        last_screen: position,
                    };
                }
                (ToolMode::None, PointerButton::Secondary) => {
                    self.interaction = InteractionState::Windowing {
                        last_screen: position,
                    };
                }
                (ToolMode::Distance, _) => {
                    let start_img = self.viewport.screen_to_image(position);
                    self.interaction = InteractionState::MeasuringDistance {
                        start_img,
                        current_img: start_img,
                    };
                }
                (ToolMode::Angle, _) => {
                    let vertex_img = self.viewport.screen_to_image(position);
                    self.interaction = InteractionState::MeasuringAngle {
                        vertex_img,
                        first_arm_img: None,
                        current_img: vertex_img,
                    };
                }
                (ToolMode::Probe, _) => {
                    let position_img = self.viewport.screen_to_image(position);
                    self.interaction = InteractionState::Probing { position_img };
                }
            },
            (
                InteractionState::Panning { last_screen },
                InputEvent::PointerMove { position, .. },
            ) => {
                let dx = position.x - last_screen.x;
                let dy = position.y - last_screen.y;
                let delta_img = self.screen_delta_to_image_delta(dx, dy);
                self.viewport.center_img.x -= delta_img.x;
                self.viewport.center_img.y -= delta_img.y;
                self.interaction = InteractionState::Panning {
                    last_screen: position,
                };
            }
            (InteractionState::Panning { .. }, InputEvent::PointerUp { .. }) => {
                self.interaction = InteractionState::Idle;
            }
            (
                InteractionState::Windowing { last_screen },
                InputEvent::PointerMove { position, .. },
            ) => {
                let dx = position.x - last_screen.x;
                let dy = position.y - last_screen.y;
                let (center, width) = match self.viewport.window_level {
                    WindowLevelState::Auto => (0.0, 1.0),
                    WindowLevelState::Explicit { center, width } => (center, width),
                };
                let next_center = center + dx * 0.1;
                let next_width = (width + dy * 0.1).max(0.1);
                self.viewport.window_level = WindowLevelState::Explicit {
                    center: next_center,
                    width: next_width,
                };
                self.interaction = InteractionState::Windowing {
                    last_screen: position,
                };
            }
            (InteractionState::Windowing { .. }, InputEvent::PointerUp { .. }) => {
                self.interaction = InteractionState::Idle;
            }
            (InteractionState::Idle, InputEvent::Wheel { delta, .. }) => {
                self.viewport.zoom = (self.viewport.zoom * (1.0 + delta as f64 * 0.001)).max(0.01);
                self.interaction = InteractionState::Zooming;
            }
            (InteractionState::Zooming, InputEvent::Wheel { delta, .. }) => {
                self.viewport.zoom = (self.viewport.zoom * (1.0 + delta as f64 * 0.001)).max(0.01);
            }
            (InteractionState::Zooming, InputEvent::EventComplete) => {
                self.interaction = InteractionState::Idle;
            }
            (
                InteractionState::MeasuringDistance { start_img, .. },
                InputEvent::PointerMove { position, .. },
            ) => {
                let current_img = self.viewport.screen_to_image(position);
                self.interaction = InteractionState::MeasuringDistance {
                    start_img,
                    current_img,
                };
            }
            (
                InteractionState::MeasuringDistance {
                    start_img,
                    current_img,
                },
                InputEvent::PointerUp { .. },
            ) => {
                let measurement = self.build_distance_measurement(start_img, current_img);
                self.record_measurement(measurement);
                self.interaction = InteractionState::Idle;
            }
            (
                InteractionState::MeasuringAngle {
                    vertex_img,
                    first_arm_img,
                    ..
                },
                InputEvent::PointerMove { position, .. },
            ) => {
                let current_img = self.viewport.screen_to_image(position);
                self.interaction = InteractionState::MeasuringAngle {
                    vertex_img,
                    first_arm_img,
                    current_img,
                };
            }
            (
                InteractionState::MeasuringAngle {
                    vertex_img,
                    first_arm_img,
                    current_img,
                },
                InputEvent::PointerUp { .. },
            ) => match first_arm_img {
                None => {
                    if points_are_equal(vertex_img, current_img) {
                        self.record_warning(ViewerWarning::InvalidMeasurementInput);
                        let id = self.next_measurement_id();
                        self.record_measurement(Measurement {
                            id,
                            kind: MeasurementKind::Angle2D,
                            value: None,
                            unit: MeasurementUnit::Degree,
                            calibrated: false,
                            provenance: None,
                            uncertainty: self.measurement_uncertainty.clone(),
                            edit_history: vec![MeasurementEditRecord {
                                revision: 0,
                                action: "created",
                            }],
                            points: vec![vertex_img, current_img, current_img],
                        });
                        self.interaction = InteractionState::Idle;
                    } else {
                        self.interaction = InteractionState::MeasuringAngle {
                            vertex_img,
                            first_arm_img: Some(current_img),
                            current_img,
                        };
                    }
                }
                Some(first_arm) => {
                    let measurement =
                        self.build_angle_measurement(vertex_img, first_arm, current_img);
                    self.record_measurement(measurement);
                    self.interaction = InteractionState::Idle;
                }
            },
            (InteractionState::Probing { .. }, InputEvent::PointerMove { position, .. }) => {
                let position_img = self.viewport.screen_to_image(position);
                self.interaction = InteractionState::Probing { position_img };
            }
            (InteractionState::Probing { position_img }, InputEvent::PointerUp { .. }) => {
                let id = self.next_measurement_id();
                self.record_measurement(Measurement {
                    id,
                    kind: MeasurementKind::Probe,
                    value: None,
                    unit: MeasurementUnit::Pixel,
                    calibrated: false,
                    provenance: None,
                    uncertainty: self.measurement_uncertainty.clone(),
                    edit_history: vec![MeasurementEditRecord {
                        revision: 0,
                        action: "created",
                    }],
                    points: vec![position_img],
                });
                self.interaction = InteractionState::Idle;
            }
            _ => {}
        }
    }

    fn next_measurement_id(&mut self) -> String {
        let id = self.measurement_counter;
        self.measurement_counter += 1;
        format!("m{id}")
    }

    fn record_measurement(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    fn record_warning(&mut self, warning: ViewerWarning) {
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    fn build_distance_measurement(
        &mut self,
        start_img: ImagePoint,
        current_img: ImagePoint,
    ) -> Measurement {
        let pixel_distance = distance_pixels(start_img, current_img);
        let mut value = pixel_distance;
        let mut unit = MeasurementUnit::Pixel;
        let mut calibrated = false;
        let mut provenance = None;

        if self.measurement_mode == MeasurementMode::PhysicalUnits {
            match (pixel_distance, self.measurement_calibration.as_ref()) {
                (Some(_), Some(calibration)) if is_valid_spacing(calibration.pixel_spacing) => {
                    let (sx, sy) = calibration.pixel_spacing;
                    let dx_mm = (current_img.x - start_img.x) * sx;
                    let dy_mm = (current_img.y - start_img.y) * sy;
                    let mm = (dx_mm * dx_mm + dy_mm * dy_mm).sqrt();
                    if mm.is_finite() {
                        value = Some(mm);
                        unit = MeasurementUnit::Millimeter;
                        calibrated = true;
                        provenance = Some(calibration.provenance.clone());
                    } else {
                        self.record_warning(ViewerWarning::InvalidMeasurementInput);
                    }
                }
                _ => self.record_warning(ViewerWarning::UncalibratedPhysicalUnits),
            }
        }

        if value.is_none() {
            self.record_warning(ViewerWarning::InvalidMeasurementInput);
        }

        Measurement {
            id: self.next_measurement_id(),
            kind: MeasurementKind::Distance2D,
            value,
            unit,
            calibrated,
            provenance,
            uncertainty: self.measurement_uncertainty.clone(),
            edit_history: vec![MeasurementEditRecord {
                revision: 0,
                action: "created",
            }],
            points: vec![start_img, current_img],
        }
    }

    fn build_angle_measurement(
        &mut self,
        vertex_img: ImagePoint,
        first_arm: ImagePoint,
        current_img: ImagePoint,
    ) -> Measurement {
        let value = angle_degrees(vertex_img, first_arm, current_img);
        if value.is_none() {
            self.record_warning(ViewerWarning::InvalidMeasurementInput);
        }
        Measurement {
            id: self.next_measurement_id(),
            kind: MeasurementKind::Angle2D,
            value,
            unit: MeasurementUnit::Degree,
            calibrated: false,
            provenance: None,
            uncertainty: self.measurement_uncertainty.clone(),
            edit_history: vec![MeasurementEditRecord {
                revision: 0,
                action: "created",
            }],
            points: vec![vertex_img, first_arm, current_img],
        }
    }

    fn screen_delta_to_image_delta(&self, dx: f64, dy: f64) -> ImagePoint {
        if self.viewport.zoom == 0.0 {
            return ImagePoint { x: 0.0, y: 0.0 };
        }
        let mut ix = dx / self.viewport.zoom;
        let mut iy = dy / self.viewport.zoom;
        let (rdx, rdy) = match self.viewport.rotation_quadrants.rem_euclid(4) {
            0 => (ix, iy),
            1 => (-iy, ix),
            2 => (-ix, -iy),
            _ => (iy, -ix),
        };
        ix = rdx;
        iy = rdy;
        if self.viewport.flip_x {
            ix = -ix;
        }
        if self.viewport.flip_y {
            iy = -iy;
        }
        ImagePoint { x: ix, y: iy }
    }
}

fn is_valid_spacing(spacing: (f64, f64)) -> bool {
    spacing.0.is_finite() && spacing.1.is_finite() && spacing.0 > 0.0 && spacing.1 > 0.0
}

fn overlay_layer_rank(layer: OverlayLayer) -> u8 {
    match layer {
        OverlayLayer::PixelData => 0,
        OverlayLayer::GspsShutter => 1,
        OverlayLayer::GspsGraphics => 2,
        OverlayLayer::Segmentation => 3,
        OverlayLayer::Annotation => 4,
    }
}

/// Validate clinically relevant numeric input without auto-correction.
pub fn validate_clinical_numeric_input(value: f64) -> Option<f64> {
    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}

/// Build control availability state with rationale when disabled.
pub fn control_availability(enabled: bool, rationale: Option<&str>) -> ControlAvailability {
    ControlAvailability {
        enabled,
        rationale: if enabled {
            None
        } else {
            rationale.map(str::to_string)
        },
    }
}

/// Build explicit error-to-audit references.
pub fn link_error_to_audit(issue_code: &str, audit_event_id: &str) -> Option<ErrorAuditReference> {
    if issue_code.trim().is_empty() || audit_event_id.trim().is_empty() {
        return None;
    }
    Some(ErrorAuditReference {
        issue_code: issue_code.to_string(),
        audit_event_id: audit_event_id.to_string(),
    })
}

/// Redact support-bundle text for UID/path/host-like tokens.
pub fn redact_support_bundle_text(text: &str) -> String {
    text.split_whitespace()
        .map(|token| {
            if looks_like_uid(token) || looks_like_path(token) || looks_like_host(token) {
                return "[redacted]".to_string();
            }
            if let Some((key, value)) = token.split_once('=') {
                if looks_like_uid(value) || looks_like_path(value) || looks_like_host(value) {
                    return format!("{key}=[redacted]");
                }
            }
            token.to_string()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build deterministic incident correlation identifier.
pub fn incident_correlation_id(issue_code: &str, epoch_secs: u64) -> String {
    format!("INC-{}-{epoch_secs}", issue_code.replace('.', "_"))
}

/// Determine whether progression is blocked until critical warnings are acknowledged.
pub fn progression_allowed_for_critical_warning(
    critical_warning_active: bool,
    acknowledged: bool,
) -> bool {
    !critical_warning_active || acknowledged
}

/// Build escalation pathway metadata.
pub fn escalation_pathway(route: &str, required: bool) -> Option<EscalationPathway> {
    if route.trim().is_empty() {
        return None;
    }
    Some(EscalationPathway {
        route: route.to_string(),
        required,
    })
}

/// Build persistent runtime security banner state.
pub fn runtime_security_banner(insecure_runtime_detected: bool) -> RuntimeSecurityBanner {
    RuntimeSecurityBanner {
        visible: insecure_runtime_detected,
        persistent: insecure_runtime_detected,
        message: if insecure_runtime_detected {
            "Insecure runtime state detected. Secure transport or policy controls are degraded."
                .to_string()
        } else {
            "Runtime security posture is compliant.".to_string()
        },
    }
}

/// Determine whether a destructive action is permitted.
pub fn destructive_action_allowed(confirmed: bool, scope: &str) -> bool {
    confirmed && !scope.trim().is_empty()
}

/// Evaluate synchronization state for side-by-side comparison workflows.
pub fn evaluate_side_by_side_sync(
    primary: &Viewport2D,
    secondary: &Viewport2D,
    frame_primary: u32,
    frame_secondary: u32,
    lock_enabled: bool,
) -> ComparisonSyncState {
    if !lock_enabled {
        return ComparisonSyncState {
            pan_zoom_locked: false,
            frame_locked: false,
            synchronized: false,
        };
    }
    let pan_zoom_locked =
        primary.center_img == secondary.center_img && primary.zoom == secondary.zoom;
    let frame_locked = frame_primary == frame_secondary;
    ComparisonSyncState {
        pan_zoom_locked,
        frame_locked,
        synchronized: pan_zoom_locked && frame_locked,
    }
}

/// Deterministically apply a hanging-protocol template with explicit fallback.
pub fn apply_hanging_protocol_template(
    templates: &[HangingProtocolTemplate],
    requested_template_id: &str,
) -> Option<HangingProtocolApplication> {
    if templates.is_empty() {
        return None;
    }
    if let Some(template) = templates
        .iter()
        .find(|item| item.template_id == requested_template_id)
    {
        return Some(HangingProtocolApplication {
            applied_template_id: template.template_id.clone(),
            used_fallback: false,
        });
    }
    let fallback = templates
        .iter()
        .min_by(|left, right| left.template_id.cmp(&right.template_id))?;
    Some(HangingProtocolApplication {
        applied_template_id: fallback.template_id.clone(),
        used_fallback: true,
    })
}

/// Validate prior-study comparison alignment before synchronized display.
pub fn validate_prior_study_alignment(
    active: &StudySeriesContext,
    prior: &StudySeriesContext,
) -> bool {
    if active.study_uid.trim().is_empty()
        || active.series_uid.trim().is_empty()
        || prior.study_uid.trim().is_empty()
        || prior.series_uid.trim().is_empty()
    {
        return false;
    }
    active.study_uid != prior.study_uid && active.series_uid == prior.series_uid
}

impl ViewerWarning {
    fn to_issue(self) -> ViewerIssue {
        match self {
            ViewerWarning::UncalibratedPhysicalUnits => ViewerIssue {
                code: "DVF.HI.WARN.UNCALIBRATED_PHYSICAL_UNITS",
                severity: ViewerIssueSeverity::Warning,
                message: "Physical-unit measurements are unavailable because calibration is missing or invalid.",
                guidance: "Use pixel-domain output or provide valid Pixel Spacing (0028,0030).",
            },
            ViewerWarning::InvalidMeasurementInput => ViewerIssue {
                code: "DVF.HI.WARN.INVALID_MEASUREMENT_INPUT",
                severity: ViewerIssueSeverity::Blocking,
                message: "Measurement input is invalid and could not be computed.",
                guidance: "Retry with finite points and non-zero geometry.",
            },
        }
    }
}

fn points_are_equal(a: ImagePoint, b: ImagePoint) -> bool {
    a.x == b.x && a.y == b.y
}

fn looks_like_uid(token: &str) -> bool {
    token.matches('.').count() >= 2 && token.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
}

fn looks_like_path(token: &str) -> bool {
    token.contains('/') || token.contains('\\')
}

fn looks_like_host(token: &str) -> bool {
    token.contains(':') && token.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn distance_pixels(a: ImagePoint, b: ImagePoint) -> Option<f64> {
    if !a.x.is_finite() || !a.y.is_finite() || !b.x.is_finite() || !b.y.is_finite() {
        return None;
    }
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let distance = (dx * dx + dy * dy).sqrt();
    if distance.is_finite() {
        Some(distance)
    } else {
        None
    }
}

fn angle_degrees(vertex: ImagePoint, b: ImagePoint, c: ImagePoint) -> Option<f64> {
    if !vertex.x.is_finite()
        || !vertex.y.is_finite()
        || !b.x.is_finite()
        || !b.y.is_finite()
        || !c.x.is_finite()
        || !c.y.is_finite()
    {
        return None;
    }

    let vb = (b.x - vertex.x, b.y - vertex.y);
    let vc = (c.x - vertex.x, c.y - vertex.y);
    let vb_len = (vb.0 * vb.0 + vb.1 * vb.1).sqrt();
    let vc_len = (vc.0 * vc.0 + vc.1 * vc.1).sqrt();
    if vb_len == 0.0 || vc_len == 0.0 {
        return None;
    }

    let mut cos_theta = (vb.0 * vc.0 + vb.1 * vc.1) / (vb_len * vc_len);
    if !cos_theta.is_finite() {
        return None;
    }
    cos_theta = cos_theta.clamp(-1.0, 1.0);
    let angle = cos_theta.acos().to_degrees();
    if angle.is_finite() {
        Some(angle)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_center_is_half_offset() {
        // REQ-UI-002
        let center = Viewport2D::pixel_center(0, 0);
        assert_eq!(center, ImagePoint { x: 0.5, y: 0.5 });
    }

    #[test]
    fn measurement_mode_defaults_to_pixel_domain() {
        // REQ-MEAS-001
        assert_eq!(MeasurementMode::default(), MeasurementMode::PixelDomain);
    }

    #[test]
    fn deterministic_event_processing_produces_same_state() {
        // REQ-UI-010, REQ-UI-012
        let mut model_a = ViewerModel::new(800, 600);
        let mut model_b = ViewerModel::new(800, 600);
        let events = [
            InputEvent::PointerDown {
                button: PointerButton::Primary,
                position: ScreenPoint { x: 10.0, y: 10.0 },
                modifiers: Modifiers::default(),
            },
            InputEvent::PointerMove {
                position: ScreenPoint { x: 20.0, y: 20.0 },
                modifiers: Modifiers::default(),
            },
            InputEvent::PointerUp {
                position: ScreenPoint { x: 20.0, y: 20.0 },
                modifiers: Modifiers::default(),
            },
        ];
        for event in events {
            model_a.apply_event(event);
            model_b.apply_event(event);
        }
        assert_eq!(model_a.interaction, model_b.interaction);
        assert_eq!(model_a.viewport, model_b.viewport);
    }

    #[test]
    fn tool_mode_is_explicit() {
        // REQ-UI-011
        let mut model = ViewerModel::new(640, 480);
        model.set_tool_mode(ToolMode::Distance);
        model.apply_event(InputEvent::PointerDown {
            button: PointerButton::Primary,
            position: ScreenPoint { x: 5.0, y: 5.0 },
            modifiers: Modifiers::default(),
        });
        assert!(matches!(
            model.interaction,
            InteractionState::MeasuringDistance { .. }
        ));
    }

    #[test]
    fn rotation_rejects_non_quadrants() {
        // REQ-UI-004
        let mut viewport = Viewport2D::new(100, 100);
        let err = viewport
            .set_rotation_degrees(45)
            .expect_err("reject 45 degrees");
        assert!(format!("{err}").contains("multiple of 90"));
    }

    #[test]
    fn viewport_transform_round_trip() {
        // REQ-UI-003
        let mut viewport = Viewport2D::new(800, 600);
        viewport.center_img = ImagePoint { x: 10.0, y: 20.0 };
        viewport.zoom = 2.0;
        viewport.rotation_quadrants = 0;
        let point = ImagePoint { x: 12.0, y: 23.0 };
        let screen = viewport.image_to_screen(point);
        let round_trip = viewport.screen_to_image(screen);
        let dx = (round_trip.x - point.x).abs();
        let dy = (round_trip.y - point.y).abs();
        assert!(dx < 1e-9 && dy < 1e-9);
    }

    #[test]
    fn angle_degrees_rejects_zero_length_segments() {
        // REQ-MEAS-060
        let vertex = ImagePoint { x: 0.0, y: 0.0 };
        let err = angle_degrees(vertex, vertex, ImagePoint { x: 1.0, y: 0.0 });
        assert!(err.is_none());
    }

    #[test]
    fn angle_degrees_clamps_rounding_overshoot() {
        // REQ-MEAS-061
        let vertex = ImagePoint { x: 0.0, y: 0.0 };
        let b = ImagePoint { x: 1.0, y: 0.0 };
        let c = ImagePoint { x: 1.0, y: 1e-14 };
        let angle = angle_degrees(vertex, b, c).expect("angle");
        assert!(angle.is_finite());
    }

    #[test]
    fn invalid_calibration_is_rejected() {
        // REQ-MEAS-020
        let mut model = ViewerModel::new(32, 32);
        assert!(!model.set_pixel_spacing_calibration((0.0, 1.0)));
        assert!(model.measurement_calibration.is_none());
    }

    #[test]
    fn tri_planar_plane_index_navigation_updates_crosshair() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([64, 32, 16]));
        assert!(model.set_mpr_plane_index(TriPlanarPlane::Axial, 7));
        let state = model.tri_planar_state();
        assert_eq!(state.axial_index, 7);
        assert_eq!(state.crosshair_voxel, [0, 0, 7]);
    }

    #[test]
    fn tri_planar_crosshair_synchronizes_all_views() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([64, 32, 16]));
        assert!(model.set_mpr_crosshair_voxel([11, 12, 13]));
        let state = model.tri_planar_state();
        assert_eq!(state.sagittal_index, 11);
        assert_eq!(state.coronal_index, 12);
        assert_eq!(state.axial_index, 13);
    }

    #[test]
    fn tri_planar_step_clamps_to_bounds() {
        let mut model = ViewerModel::new(256, 256);
        assert!(model.set_mpr_volume_dimensions([4, 3, 2]));
        assert!(model.step_mpr_plane(TriPlanarPlane::Sagittal, 10));
        assert!(!model.step_mpr_plane(TriPlanarPlane::Sagittal, 1));
        let state = model.tri_planar_state();
        assert_eq!(state.sagittal_index, 3);
        assert_eq!(state.crosshair_voxel[0], 3);
    }
}
