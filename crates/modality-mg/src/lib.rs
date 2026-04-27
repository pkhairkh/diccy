#![deny(missing_docs)]

//! MG modality pack: mammography SOP support, tomosynthesis, CADe, and MQSA.
//!
//! Provides:
//! - Original: Mammography SOP class gating and measurement gating.
//! - **S7-T4**: Tomosynthesis (3D mammography) slice navigation, CADe integration
//!   hooks for breast lesion detection, dual-monitor hanging protocol (CC/MLO arrangement),
//!   and MQSA compliance display controls.

use dicom_core::{Error, ErrorKind, Result, Tag};
use serde::{Deserialize, Serialize};

/// Mammography X-Ray Image Storage - For Presentation.
pub const SOP_CLASS_MG_PRESENTATION: &str = "1.2.840.10008.5.1.4.1.1.1.2";
/// Mammography X-Ray Image Storage - For Processing.
pub const SOP_CLASS_MG_PROCESSING: &str = "1.2.840.10008.5.1.4.1.1.1.2.1";
/// Breast Tomosynthesis Image Storage.
pub const SOP_CLASS_BREAST_TOMOSYNTHESIS: &str = "1.2.840.10008.5.1.4.1.1.13.1.3";

/// Mammography SOP Class manifest (static list).
pub const MG_SOP_CLASS_UIDS: &[&str] = &[
    SOP_CLASS_MG_PRESENTATION,
    SOP_CLASS_MG_PROCESSING,
    SOP_CLASS_BREAST_TOMOSYNTHESIS,
];

// ===========================================================================
// S7-T4: Tomosynthesis (3D Mammography) Slice Navigation
// ===========================================================================

/// Cine playback state.
///
/// Replaces the boolean trap of `cine_active: bool` + `cine_direction: i32`
/// with a single enum that makes all valid states explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CineState {
    /// Cine playback is stopped.
    Stopped,
    /// Cine is playing forward.
    PlayingForward,
    /// Cine is playing backward.
    PlayingBackward,
}

impl Default for CineState {
    fn default() -> Self {
        CineState::Stopped
    }
}

impl CineState {
    /// Return true when cine is active (playing forward or backward).
    pub fn is_active(&self) -> bool {
        !matches!(self, CineState::Stopped)
    }

    /// Return true when playing forward.
    pub fn is_forward(&self) -> bool {
        matches!(self, CineState::PlayingForward)
    }

    /// Return true when playing backward.
    pub fn is_backward(&self) -> bool {
        matches!(self, CineState::PlayingBackward)
    }
}

/// Tomosynthesis slice navigation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TomoNavigation {
    /// Current slice index in the tomosynthesis stack.
    pub current_slice: u32,
    /// Total number of slices in the tomosynthesis stack.
    pub total_slices: u32,
    /// Slice thickness in millimeters.
    pub slice_thickness_mm: f64,
    /// Slice spacing in millimeters.
    pub slice_spacing_mm: f64,
    /// Playback frame rate for cine mode (frames per second).
    pub cine_fps: f64,
    /// Cine playback state.
    pub cine: CineState,
}

impl Default for TomoNavigation {
    fn default() -> Self {
        Self {
            current_slice: 0,
            total_slices: 0,
            slice_thickness_mm: 1.0,
            slice_spacing_mm: 1.0,
            cine_fps: 15.0,
            cine: CineState::default(),
        }
    }
}

impl TomoNavigation {
    /// Create a new tomo navigation for a given number of slices.
    pub fn new(total_slices: u32, slice_thickness_mm: f64, slice_spacing_mm: f64) -> Self {
        Self {
            current_slice: 0,
            total_slices,
            slice_thickness_mm,
            slice_spacing_mm,
            ..Self::default()
        }
    }

    /// Navigate to a specific slice index.
    pub fn go_to_slice(&mut self, index: u32) {
        self.current_slice = index.min(self.total_slices.saturating_sub(1));
    }

    /// Navigate to the next slice.
    pub fn next_slice(&mut self) -> u32 {
        if self.current_slice + 1 < self.total_slices {
            self.current_slice += 1;
        }
        self.current_slice
    }

    /// Navigate to the previous slice.
    pub fn prev_slice(&mut self) -> u32 {
        if self.current_slice > 0 {
            self.current_slice -= 1;
        }
        self.current_slice
    }

    /// Start cine playback forward.
    pub fn start_cine(&mut self) {
        self.cine = CineState::PlayingForward;
    }

    /// Stop cine playback.
    pub fn stop_cine(&mut self) {
        self.cine = CineState::Stopped;
    }

    /// Advance one cine frame. Returns the new slice index.
    pub fn advance_cine_frame(&mut self) -> u32 {
        if !self.cine.is_active() {
            return self.current_slice;
        }

        if self.cine.is_forward() {
            if self.current_slice + 1 < self.total_slices {
                self.current_slice += 1;
            } else {
                // Bounce back
                self.cine = CineState::PlayingBackward;
                if self.current_slice > 0 {
                    self.current_slice -= 1;
                }
            }
        } else {
            if self.current_slice > 0 {
                self.current_slice -= 1;
            } else {
                // Bounce forward
                self.cine = CineState::PlayingForward;
                if self.current_slice + 1 < self.total_slices {
                    self.current_slice += 1;
                }
            }
        }

        self.current_slice
    }

    /// Get the Z position of the current slice in millimeters.
    pub fn current_z_mm(&self) -> f64 {
        self.current_slice as f64 * self.slice_spacing_mm
    }
}

// ===========================================================================
// S7-T4: CADe Integration Hooks for Breast Lesion Detection
// ===========================================================================

/// Breast laterality.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BreastLaterality {
    /// Left breast.
    Left,
    /// Right breast.
    Right,
}

impl BreastLaterality {
    /// Return the DICOM code value.
    pub fn dicom_code(&self) -> &str {
        match self {
            BreastLaterality::Left => "428851000",
            BreastLaterality::Right => "24028007",
        }
    }
}

/// Mammography view type.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MammographyView {
    /// Craniocaudal view.
    CC,
    /// Mediolateral oblique view.
    MLO,
    /// Mediolateral view.
    ML,
    /// Lateromedial view.
    LM,
    /// Exaggerated craniocaudal view.
    XCCL,
}

impl MammographyView {
    /// Return the DICOM code value for this view.
    pub fn dicom_code(&self) -> &str {
        match self {
            MammographyView::CC => "R-10226",
            MammographyView::MLO => "R-10227",
            MammographyView::ML => "R-10224",
            MammographyView::LM => "R-10225",
            MammographyView::XCCL => "R-102A2",
        }
    }

    /// Return the display name.
    pub fn display_name(&self) -> &str {
        match self {
            MammographyView::CC => "CC",
            MammographyView::MLO => "MLO",
            MammographyView::ML => "ML",
            MammographyView::LM => "LM",
            MammographyView::XCCL => "XCCL",
        }
    }
}

/// A CADe finding on a mammography image.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MammographyCadeFinding {
    /// Finding identifier.
    pub finding_id: String,
    /// Laterality (left or right breast).
    pub laterality: BreastLaterality,
    /// View on which the finding was detected.
    pub view: MammographyView,
    /// Bounding box (x_min, y_min, x_max, y_max) in image coordinates.
    pub bounding_box: (f64, f64, f64, f64),
    /// Detection confidence (0.0-1.0).
    pub confidence: f64,
    /// Finding type (e.g., "mass", "calcification", "distortion", "asymmetry").
    pub finding_type: String,
    /// BI-RADS assessment category (0-6).
    pub birads_category: u32,
}

/// CADe integration hook for breast lesion detection.
pub struct MammographyCadeHook {
    /// Registered CADe findings.
    findings: Vec<MammographyCadeFinding>,
    /// Minimum confidence threshold for display.
    confidence_threshold: f64,
    /// Whether CADe overlay is visible.
    overlay_visible: bool,
}

impl Default for MammographyCadeHook {
    fn default() -> Self {
        Self::new()
    }
}

impl MammographyCadeHook {
    /// Create a new CADe hook.
    pub fn new() -> Self {
        Self {
            findings: Vec::new(),
            confidence_threshold: 0.5,
            overlay_visible: true,
        }
    }

    /// Add a CADe finding.
    pub fn add_finding(&mut self, finding: MammographyCadeFinding) {
        self.findings.push(finding);
    }

    /// Get all findings above the confidence threshold.
    pub fn visible_findings(&self) -> Vec<&MammographyCadeFinding> {
        if !self.overlay_visible {
            return Vec::new();
        }
        self.findings
            .iter()
            .filter(|f| f.confidence >= self.confidence_threshold)
            .collect()
    }

    /// Set the confidence threshold.
    pub fn set_confidence_threshold(&mut self, threshold: f64) {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
    }

    /// Toggle CADe overlay visibility.
    pub fn toggle_overlay(&mut self) {
        self.overlay_visible = !self.overlay_visible;
    }

    /// Return the number of registered findings.
    pub fn finding_count(&self) -> usize {
        self.findings.len()
    }

    /// Return overlay visibility state.
    pub fn is_overlay_visible(&self) -> bool {
        self.overlay_visible
    }
}

// ===========================================================================
// S7-T4: Dual-Monitor Hanging Protocol (CC/MLO Arrangement)
// ===========================================================================

/// Prior study display mode.
///
/// Replaces the boolean trap of `show_priors: bool` with a semantically
/// meaningful enum that can be extended to cover additional display modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriorDisplay {
    /// Prior studies are hidden.
    Hidden,
    /// Prior studies are visible alongside current.
    Visible,
}

impl Default for PriorDisplay {
    fn default() -> Self {
        PriorDisplay::Hidden
    }
}

impl PriorDisplay {
    /// Return true when priors are visible.
    pub fn is_visible(&self) -> bool {
        matches!(self, PriorDisplay::Visible)
    }
}

/// Display slot in a dual-monitor hanging protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplaySlot {
    /// Monitor index (0 = left, 1 = right).
    pub monitor: usize,
    /// Laterality for this slot.
    pub laterality: BreastLaterality,
    /// View type for this slot.
    pub view: MammographyView,
    /// Whether this is the current (most recent) study.
    pub is_current: bool,
}

/// Dual-monitor mammography hanging protocol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DualMonitorHangingProtocol {
    /// Display slots for the current study.
    pub current_slots: Vec<DisplaySlot>,
    /// Display slots for the prior study (if available).
    pub prior_slots: Vec<DisplaySlot>,
    /// Prior display mode.
    pub prior_display: PriorDisplay,
}

impl Default for DualMonitorHangingProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl DualMonitorHangingProtocol {
    /// Create the standard CC/MLO dual-monitor hanging protocol.
    pub fn new() -> Self {
        Self {
            current_slots: vec![
                DisplaySlot {
                    monitor: 0,
                    laterality: BreastLaterality::Left,
                    view: MammographyView::CC,
                    is_current: true,
                },
                DisplaySlot {
                    monitor: 0,
                    laterality: BreastLaterality::Left,
                    view: MammographyView::MLO,
                    is_current: true,
                },
                DisplaySlot {
                    monitor: 1,
                    laterality: BreastLaterality::Right,
                    view: MammographyView::CC,
                    is_current: true,
                },
                DisplaySlot {
                    monitor: 1,
                    laterality: BreastLaterality::Right,
                    view: MammographyView::MLO,
                    is_current: true,
                },
            ],
            prior_slots: Vec::new(),
            prior_display: PriorDisplay::default(),
        }
    }

    /// Create a hanging protocol with prior studies.
    pub fn with_priors() -> Self {
        let mut protocol = Self::new();
        protocol.prior_display = PriorDisplay::Visible;
        protocol.prior_slots = vec![
            DisplaySlot {
                monitor: 0,
                laterality: BreastLaterality::Left,
                view: MammographyView::CC,
                is_current: false,
            },
            DisplaySlot {
                monitor: 0,
                laterality: BreastLaterality::Left,
                view: MammographyView::MLO,
                is_current: false,
            },
            DisplaySlot {
                monitor: 1,
                laterality: BreastLaterality::Right,
                view: MammographyView::CC,
                is_current: false,
            },
            DisplaySlot {
                monitor: 1,
                laterality: BreastLaterality::Right,
                view: MammographyView::MLO,
                is_current: false,
            },
        ];
        protocol
    }

    /// Get all slots for a specific monitor.
    pub fn slots_for_monitor(&self, monitor: usize) -> Vec<&DisplaySlot> {
        let current = self.current_slots.iter().filter(|s| s.monitor == monitor);
        let prior = self.prior_slots.iter().filter(|s| s.monitor == monitor);
        if self.prior_display.is_visible() {
            current.chain(prior).collect()
        } else {
            current.collect()
        }
    }

    /// Toggle prior display.
    pub fn toggle_priors(&mut self) {
        self.prior_display = match self.prior_display {
            PriorDisplay::Hidden => PriorDisplay::Visible,
            PriorDisplay::Visible => PriorDisplay::Hidden,
        };
    }
}

// ===========================================================================
// S7-T4: MQSA Compliance Display Controls
// ===========================================================================

/// Display calibration state.
///
/// Replaces the boolean trap of `gsdf_calibrated: bool` with a semantically
/// meaningful enum that can distinguish between uncalibrated, calibrated, and
/// expired calibration states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalibrationState {
    /// Display has not been calibrated.
    Uncalibrated,
    /// Display has been calibrated per DICOM GSDF.
    Calibrated,
}

impl Default for CalibrationState {
    fn default() -> Self {
        CalibrationState::Uncalibrated
    }
}

impl CalibrationState {
    /// Return true when calibrated.
    pub fn is_calibrated(&self) -> bool {
        matches!(self, CalibrationState::Calibrated)
    }
}

/// MQSA display calibration settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqsaDisplayControls {
    /// Maximum luminance in cd/m2.
    pub max_luminance: f64,
    /// Minimum luminance in cd/m2.
    pub min_luminance: f64,
    /// Display bit depth (8, 10, or 12).
    pub bit_depth: u32,
    /// Display calibration state.
    pub calibration: CalibrationState,
    /// Ambient light level in lux.
    pub ambient_light_lux: f64,
    /// Whether the display meets MQSA requirements.
    pub mqsa_compliant: bool,
    /// Last calibration date (ISO 8601).
    pub last_calibration_date: String,
}

impl Default for MqsaDisplayControls {
    fn default() -> Self {
        Self {
            max_luminance: 450.0,
            min_luminance: 1.0,
            bit_depth: 10,
            calibration: CalibrationState::default(),
            ambient_light_lux: 20.0,
            mqsa_compliant: false,
            last_calibration_date: String::new(),
        }
    }
}

impl MqsaDisplayControls {
    /// Create MQSA display controls with specific parameters.
    pub fn new(max_luminance: f64, min_luminance: f64, bit_depth: u32) -> Self {
        Self {
            max_luminance,
            min_luminance,
            bit_depth,
            ..Self::default()
        }
    }

    /// Check if the display meets MQSA requirements for mammography.
    ///
    /// MQSA requires:
    /// - Minimum 5 MP (megapixel) display resolution
    /// - Minimum 450 cd/m2 maximum luminance
    /// - Minimum 8-bit gray scale display
    /// - GSDF calibration
    /// - Annual calibration verification
    pub fn check_mqsa_compliance(&mut self) -> bool {
        self.mqsa_compliant = self.max_luminance >= 450.0
            && self.min_luminance <= 1.5
            && self.bit_depth >= 8
            && self.calibration.is_calibrated();
        self.mqsa_compliant
    }

    /// Apply GSDF calibration.
    pub fn apply_gsdf_calibration(&mut self) {
        self.calibration = CalibrationState::Calibrated;
        self.check_mqsa_compliance();
    }

    /// Get the luminance ratio (max / min).
    pub fn luminance_ratio(&self) -> f64 {
        if self.min_luminance > 0.0 {
            self.max_luminance / self.min_luminance
        } else {
            0.0
        }
    }
}

// ===========================================================================
// Original: MG Pack Marker and SOP Gating
// ===========================================================================

/// Marker type for MG modality features.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MgPack;

impl MgPack {
    /// Return true when MG pack functionality is enabled for this crate.
    pub const fn enabled() -> bool {
        cfg!(feature = "modality-mg")
    }

    /// Require the MG pack to be enabled for mammography SOP classes.
    pub fn ensure_mg_supported(sop_class_uid: &str) -> Result<()> {
        if !Self::enabled() {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "mammography requires modality-mg feature",
            )));
        }
        if !MG_SOP_CLASS_UIDS.contains(&sop_class_uid) {
            return Err(Box::new(Error::from_kind(
                ErrorKind::UnsupportedSopClass {
                    sop_class_uid: sop_class_uid.to_string(),
                },
                "unsupported mammography SOP class",
            )));
        }
        Ok(())
    }

    /// Require the MG pack to be enabled for physical-unit measurements.
    pub fn ensure_physical_measurements_enabled() -> Result<()> {
        if Self::enabled() {
            Ok(())
        } else {
            Err(Box::new(Error::from_kind(
                ErrorKind::InvalidPixelTransform {
                    stage: "measurement_mode".to_string(),
                    detail: "physical-unit measurements require modality-mg feature".to_string(),
                },
                "physical-unit measurements require modality-mg feature",
            )))
        }
    }
}

/// DICOM tag for SOP Class UID.
pub const TAG_SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
