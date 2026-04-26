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
    /// Whether cine mode is currently active.
    pub cine_active: bool,
    /// Playback direction (positive = forward, negative = backward).
    pub cine_direction: i32,
}

impl Default for TomoNavigation {
    fn default() -> Self {
        Self {
            current_slice: 0,
            total_slices: 0,
            slice_thickness_mm: 1.0,
            slice_spacing_mm: 1.0,
            cine_fps: 15.0,
            cine_active: false,
            cine_direction: 1,
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

    /// Start cine playback.
    pub fn start_cine(&mut self) {
        self.cine_active = true;
    }

    /// Stop cine playback.
    pub fn stop_cine(&mut self) {
        self.cine_active = false;
    }

    /// Advance one cine frame. Returns the new slice index.
    pub fn advance_cine_frame(&mut self) -> u32 {
        if !self.cine_active {
            return self.current_slice;
        }

        if self.cine_direction > 0 {
            if self.current_slice + 1 < self.total_slices {
                self.current_slice += 1;
            } else {
                // Bounce back
                self.cine_direction = -1;
                if self.current_slice > 0 {
                    self.current_slice -= 1;
                }
            }
        } else {
            if self.current_slice > 0 {
                self.current_slice -= 1;
            } else {
                // Bounce forward
                self.cine_direction = 1;
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
    /// Whether to show priors.
    pub show_priors: bool,
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
            show_priors: false,
        }
    }

    /// Create a hanging protocol with prior studies.
    pub fn with_priors() -> Self {
        let mut protocol = Self::new();
        protocol.show_priors = true;
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
        if self.show_priors {
            current.chain(prior).collect()
        } else {
            current.collect()
        }
    }

    /// Toggle prior display.
    pub fn toggle_priors(&mut self) {
        self.show_priors = !self.show_priors;
    }
}

// ===========================================================================
// S7-T4: MQSA Compliance Display Controls
// ===========================================================================

/// MQSA display calibration settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MqsaDisplayControls {
    /// Maximum luminance in cd/m2.
    pub max_luminance: f64,
    /// Minimum luminance in cd/m2.
    pub min_luminance: f64,
    /// Display bit depth (8, 10, or 12).
    pub bit_depth: u32,
    /// Whether DICOM GSDF calibration is active.
    pub gsdf_calibrated: bool,
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
            gsdf_calibrated: false,
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
            && self.gsdf_calibrated;
        self.mqsa_compliant
    }

    /// Apply GSDF calibration.
    pub fn apply_gsdf_calibration(&mut self) {
        self.gsdf_calibrated = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    const MG_MANIFEST: &str = include_str!("../manifest.toml");

    fn parse_manifest_uids() -> Vec<String> {
        MG_MANIFEST
            .split('"')
            .enumerate()
            .filter_map(|(idx, part)| {
                if idx % 2 == 1 {
                    Some(part.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    fn invalid_tag_value(tag: Tag, detail: impl Into<String>) -> Box<Error> {
        Error::from_kind(
            ErrorKind::InvalidTagValue {
                tag,
                detail: detail.into(),
            },
            "invalid tag value",
        )
        .into()
    }

    #[test]
    #[cfg(not(feature = "modality-mg"))]
    fn mg_pack_disabled_rejects() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(!MgPack::enabled());
        let err = MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).unwrap_err();
        assert_eq!(err.code(), "DVF.DICOM.UNSUPPORTED_SOP");
    }

    #[test]
    #[cfg(feature = "modality-mg")]
    fn mg_pack_enabled_allows() {
        // REQ-FEAT-302, REQ-SOP-301: MG pack requires explicit Cargo feature.
        assert!(MgPack::enabled());
        MgPack::ensure_mg_supported(SOP_CLASS_MG_PRESENTATION).expect("mg pack enabled");
    }

    #[test]
    fn mg_manifest_matches_constants() {
        // REQ-CONF-012: MG pack includes a SOP class manifest.
        let parsed = parse_manifest_uids();
        assert!(!parsed.is_empty());
        for uid in MG_SOP_CLASS_UIDS {
            assert!(parsed.contains(&uid.to_string()));
        }
    }

    #[test]
    fn mg_manifest_uids_look_like_uids() {
        // REQ-CONF-012: manifest entries must be valid UID-like strings.
        for uid in MG_SOP_CLASS_UIDS {
            if uid.is_empty() || uid.starts_with('.') || uid.ends_with('.') {
                let err = invalid_tag_value(TAG_SOP_CLASS_UID, "invalid UID format");
                assert_eq!(err.code(), "DVF.DICOM.INVALID_TAG_VALUE");
            }
            assert!(uid.chars().all(|ch| ch.is_ascii_digit() || ch == '.'));
        }
    }

    #[test]
    fn mg_measurements_gate() {
        // REQ-MEAS-010
        if MgPack::enabled() {
            MgPack::ensure_physical_measurements_enabled().expect("mg pack enabled");
        } else {
            let err = MgPack::ensure_physical_measurements_enabled().unwrap_err();
            assert_eq!(err.code(), "DVF.PIXEL.INVALID_TRANSFORM");
        }
    }

    // -----------------------------------------------------------------------
    // S7-T4: Tomosynthesis Tests
    // -----------------------------------------------------------------------

    #[test]
    fn tomo_navigation_slice_navigation() {
        let mut nav = TomoNavigation::new(50, 1.0, 1.0);
        assert_eq!(nav.current_slice, 0);

        nav.next_slice();
        assert_eq!(nav.current_slice, 1);

        nav.go_to_slice(25);
        assert_eq!(nav.current_slice, 25);

        nav.prev_slice();
        assert_eq!(nav.current_slice, 24);
    }

    #[test]
    fn tomo_navigation_bounds_check() {
        let mut nav = TomoNavigation::new(10, 1.0, 1.0);
        nav.go_to_slice(100); // Beyond bounds
        assert_eq!(nav.current_slice, 9); // Clamped to last slice
    }

    #[test]
    fn tomo_cine_bounce() {
        let mut nav = TomoNavigation::new(5, 1.0, 1.0);
        nav.start_cine();
        nav.go_to_slice(4); // Last slice

        nav.advance_cine_frame(); // Should bounce back
        assert_eq!(nav.current_slice, 3);
        assert_eq!(nav.cine_direction, -1);
    }

    #[test]
    fn tomo_z_position() {
        let nav = TomoNavigation::new(100, 1.0, 2.0);
        let mut nav2 = nav.clone();
        nav2.go_to_slice(10);
        assert!((nav2.current_z_mm() - 20.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // S7-T4: CADe Tests
    // -----------------------------------------------------------------------

    #[test]
    fn cade_add_and_retrieve_findings() {
        let mut hook = MammographyCadeHook::new();
        hook.add_finding(MammographyCadeFinding {
            finding_id: "f1".to_string(),
            laterality: BreastLaterality::Left,
            view: MammographyView::CC,
            bounding_box: (100.0, 100.0, 200.0, 200.0),
            confidence: 0.8,
            finding_type: "mass".to_string(),
            birads_category: 4,
        });
        assert_eq!(hook.finding_count(), 1);

        let visible = hook.visible_findings();
        assert_eq!(visible.len(), 1);
    }

    #[test]
    fn cade_confidence_filtering() {
        let mut hook = MammographyCadeHook::new();
        hook.add_finding(MammographyCadeFinding {
            finding_id: "f1".to_string(),
            laterality: BreastLaterality::Right,
            view: MammographyView::MLO,
            bounding_box: (50.0, 50.0, 150.0, 150.0),
            confidence: 0.3,
            finding_type: "calcification".to_string(),
            birads_category: 3,
        });

        hook.set_confidence_threshold(0.5);
        let visible = hook.visible_findings();
        assert!(visible.is_empty());
    }

    #[test]
    fn cade_overlay_toggle() {
        let mut hook = MammographyCadeHook::new();
        assert!(hook.is_overlay_visible());
        hook.toggle_overlay();
        assert!(!hook.is_overlay_visible());
    }

    // -----------------------------------------------------------------------
    // S7-T4: Dual-Monitor Hanging Protocol Tests
    // -----------------------------------------------------------------------

    #[test]
    fn dual_monitor_protocol_default() {
        let protocol = DualMonitorHangingProtocol::new();
        assert_eq!(protocol.current_slots.len(), 4); // LCC, LMLO, RCC, RMLO
        assert!(!protocol.show_priors);
    }

    #[test]
    fn dual_monitor_protocol_with_priors() {
        let protocol = DualMonitorHangingProtocol::with_priors();
        assert!(protocol.show_priors);
        assert_eq!(protocol.prior_slots.len(), 4);
    }

    #[test]
    fn dual_monitor_slots_for_monitor() {
        let protocol = DualMonitorHangingProtocol::new();
        let left_monitor = protocol.slots_for_monitor(0);
        let right_monitor = protocol.slots_for_monitor(1);
        assert_eq!(left_monitor.len(), 2);
        assert_eq!(right_monitor.len(), 2);
    }

    // -----------------------------------------------------------------------
    // S7-T4: MQSA Display Controls Tests
    // -----------------------------------------------------------------------

    #[test]
    fn mqsa_compliance_check() {
        let mut controls = MqsaDisplayControls::new(500.0, 0.5, 10);
        controls.apply_gsdf_calibration();
        assert!(controls.mqsa_compliant);
    }

    #[test]
    fn mqsa_non_compliant_no_gsdf() {
        let mut controls = MqsaDisplayControls::new(500.0, 0.5, 10);
        assert!(!controls.check_mqsa_compliance());
    }

    #[test]
    fn mqsa_luminance_ratio() {
        let controls = MqsaDisplayControls::new(450.0, 1.0, 10);
        assert!((controls.luminance_ratio() - 450.0).abs() < 1e-6);
    }
}
