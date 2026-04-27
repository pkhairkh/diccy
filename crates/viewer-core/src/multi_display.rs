//! Multi-monitor diagnostic display layout engine.
//!
//! Provides the [`DiagnosticLayoutEngine`] for managing viewport assignments
//! across multiple monitors with preset layouts, synchronized scrolling, and
//! MQSA-compliant mammography configurations.
//!
//! # Layout Presets
//!
//! - **SingleUp** — 1 monitor, 1 viewport (default)
//! - **DualMonitor** — 2 monitors, 1 viewport each (side-by-side comparison)
//! - **QuadUp** — 4 viewports on 1 monitor
//! - **OnePlusTwo** — 1 primary + 2 priors
//! - **MammographyMqsa** — MQSA-compliant CC/MLO layout for dual-monitor
//!
//! # Synchronized Operations
//!
//! - **Scroll sync** — Scroll same series across multiple monitors
//! - **Window/level sync** — Synchronize window/level adjustments
//! - **Comparison lock** — Lock reference and comparison monitors together

use std::collections::HashMap;

use crate::Viewport2D;

// ---------------------------------------------------------------------------
// Monitor types
// ---------------------------------------------------------------------------

/// Monitor identifier (0-indexed).
pub type MonitorId = u8;

/// Information about a connected display monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorInfo {
    /// Monitor identifier.
    pub id: MonitorId,
    /// Display width in pixels.
    pub width: u32,
    /// Display height in pixels.
    pub height: u32,
    /// Whether this is the primary display.
    pub is_primary: bool,
    /// Optional display calibration data for diagnostic consistency.
    pub calibration: Option<DisplayCalibration>,
}

impl MonitorInfo {
    /// Create a new monitor info with the given ID and dimensions.
    pub fn new(id: MonitorId, width: u32, height: u32) -> Self {
        Self {
            id,
            width,
            height,
            is_primary: id == 0,
            calibration: None,
        }
    }
}

/// Display calibration data for diagnostic consistency.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayCalibration {
    /// Maximum luminance in cd/m².
    pub max_luminance: f64,
    /// Minimum luminance in cd/m².
    pub min_luminance: f64,
    /// GSDF conformance value (0.0-1.0, 1.0 = perfect).
    pub gsdf_conformance: f64,
    /// Calibration timestamp as epoch seconds.
    pub calibrated_at_epoch_secs: u64,
}

// ---------------------------------------------------------------------------
// Layout presets
// ---------------------------------------------------------------------------

/// Preset layout configurations for multi-monitor setups.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutPreset {
    /// 1 monitor, 1 viewport (default).
    SingleUp,
    /// 2 monitors, 1 viewport each (side-by-side comparison).
    DualMonitor,
    /// 4 viewports on 1 monitor.
    QuadUp,
    /// 1 primary + 2 priors.
    OnePlusTwo,
    /// MQSA-compliant CC/MLO layout for dual-monitor mammography.
    MammographyMqsa,
    /// Custom layout definition.
    Custom(CustomLayout),
}

/// Custom layout definition.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomLayout {
    /// Layout name.
    pub name: String,
    /// Viewport assignments per monitor.
    pub assignments: Vec<ViewportAssignment>,
}

// ---------------------------------------------------------------------------
// Viewport assignment
// ---------------------------------------------------------------------------

/// Assignment of a viewport to a specific monitor.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewportAssignment {
    /// Monitor this viewport is assigned to.
    pub monitor_id: MonitorId,
    /// Viewport state for this assignment.
    pub viewport: Viewport2D,
    /// Study Instance UID loaded in this viewport, if any.
    pub study_uid: Option<String>,
    /// Series Instance UID loaded in this viewport, if any.
    pub series_uid: Option<String>,
    /// Current image index within the series.
    pub image_index: usize,
}

impl ViewportAssignment {
    /// Create a new viewport assignment for a monitor.
    pub fn new(monitor_id: MonitorId, width: u32, height: u32) -> Self {
        Self {
            monitor_id,
            viewport: Viewport2D::new(width, height),
            study_uid: None,
            series_uid: None,
            image_index: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-monitor sync state
// ---------------------------------------------------------------------------

/// Cross-monitor synchronization state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossMonitorSyncState {
    /// Whether scroll is synchronized across monitors.
    pub scroll_sync_enabled: bool,
    /// Whether window/level is synchronized across monitors.
    pub wl_sync_enabled: bool,
    /// Whether zoom is synchronized across monitors.
    pub zoom_sync_enabled: bool,
    /// Whether pan is synchronized across monitors.
    pub pan_sync_enabled: bool,
    /// Optional comparison lock configuration.
    pub comparison_lock: Option<ComparisonLockConfig>,
}

impl Default for CrossMonitorSyncState {
    fn default() -> Self {
        Self {
            scroll_sync_enabled: false,
            wl_sync_enabled: false,
            zoom_sync_enabled: false,
            pan_sync_enabled: false,
            comparison_lock: None,
        }
    }
}

/// Comparison lock configuration for synchronized reference/comparison viewing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonLockConfig {
    /// The reference monitor that drives synchronization.
    pub reference_monitor: MonitorId,
    /// Monitors that are synchronized to the reference.
    pub synced_monitors: Vec<MonitorId>,
}

// ---------------------------------------------------------------------------
// Mammography MQSA layout
// ---------------------------------------------------------------------------

/// MQSA-compliant mammography dual-monitor layout.
///
/// Per MQSA requirements:
/// - CC views on the left monitor
/// - MLO views on the right monitor
/// - Prior studies displayed below current studies
/// - Both views must be displayed for each breast laterality
#[derive(Debug, Clone, PartialEq)]
pub struct MammographyMqsaLayout {
    /// Left monitor configuration (CC views).
    pub left_monitor: MammographyViewport,
    /// Right monitor configuration (MLO views).
    pub right_monitor: MammographyViewport,
    /// Whether prior studies are displayed below current.
    pub prior_below: bool,
}

/// A mammography viewport configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct MammographyViewport {
    /// Type of mammography view (CC, MLO, or magnification).
    pub view_type: MammographyView,
    /// Laterality (left, right, or bilateral).
    pub laterality: Laterality,
}

/// Mammography view type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MammographyView {
    /// Craniocaudal view.
    CC,
    /// Mediolateral oblique view.
    MLO,
    /// Magnification view.
    Magnification,
}

/// Breast laterality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Laterality {
    /// Left breast.
    Left,
    /// Right breast.
    Right,
    /// Bilateral (both).
    Bilateral,
}

// ---------------------------------------------------------------------------
// DiagnosticLayoutEngine
// ---------------------------------------------------------------------------

/// Multi-monitor diagnostic display layout engine.
///
/// Manages viewport assignments across multiple monitors with preset layouts,
/// synchronized scrolling, and MQSA-compliant mammography configurations.
#[derive(Debug, Clone)]
pub struct DiagnosticLayoutEngine {
    /// Connected monitors.
    monitors: Vec<MonitorInfo>,
    /// Current layout preset.
    current_layout: LayoutPreset,
    /// Viewport assignments per monitor.
    viewport_assignments: HashMap<MonitorId, Vec<ViewportAssignment>>,
    /// Cross-monitor synchronization state.
    sync_state: CrossMonitorSyncState,
}

impl DiagnosticLayoutEngine {
    /// Create a new layout engine with the given monitors.
    ///
    /// Defaults to `SingleUp` layout.
    pub fn new(monitors: Vec<MonitorInfo>) -> Self {
        let mut engine = Self {
            monitors,
            current_layout: LayoutPreset::SingleUp,
            viewport_assignments: HashMap::new(),
            sync_state: CrossMonitorSyncState::default(),
        };
        engine.apply_layout(LayoutPreset::SingleUp);
        engine
    }

    /// Apply a layout preset.
    ///
    /// This reconfigures all viewport assignments according to the selected preset.
    pub fn apply_layout(&mut self, preset: LayoutPreset) {
        self.current_layout = preset.clone();
        self.viewport_assignments.clear();

        match &preset {
            LayoutPreset::SingleUp => {
                if let Some(monitor) = self.monitors.first() {
                    self.viewport_assignments.insert(
                        monitor.id,
                        vec![ViewportAssignment::new(monitor.id, monitor.width, monitor.height)],
                    );
                }
            }
            LayoutPreset::DualMonitor => {
                for monitor in &self.monitors {
                    self.viewport_assignments.insert(
                        monitor.id,
                        vec![ViewportAssignment::new(monitor.id, monitor.width, monitor.height)],
                    );
                }
            }
            LayoutPreset::QuadUp => {
                if let Some(monitor) = self.monitors.first() {
                    let half_w = monitor.width / 2;
                    let half_h = monitor.height / 2;
                    self.viewport_assignments.insert(
                        monitor.id,
                        vec![
                            ViewportAssignment::new(monitor.id, half_w, half_h),
                            ViewportAssignment::new(monitor.id, half_w, half_h),
                            ViewportAssignment::new(monitor.id, half_w, half_h),
                            ViewportAssignment::new(monitor.id, half_w, half_h),
                        ],
                    );
                }
            }
            LayoutPreset::OnePlusTwo => {
                if let Some(primary) = self.monitors.first() {
                    self.viewport_assignments.insert(
                        primary.id,
                        vec![ViewportAssignment::new(primary.id, primary.width, primary.height)],
                    );
                }
                // Two prior viewports on the same or second monitor
                if self.monitors.len() > 1 {
                    let second = &self.monitors[1];
                    let half_h = second.height / 2;
                    self.viewport_assignments.insert(
                        second.id,
                        vec![
                            ViewportAssignment::new(second.id, second.width, half_h),
                            ViewportAssignment::new(second.id, second.width, half_h),
                        ],
                    );
                } else if let Some(primary) = self.monitors.first() {
                    // Single monitor: split primary into 3 sections
                    let existing = self.viewport_assignments.remove(&primary.id);
                    let _ = existing;
                    let third_h = primary.height / 3;
                    self.viewport_assignments.insert(
                        primary.id,
                        vec![
                            ViewportAssignment::new(primary.id, primary.width, primary.height - 2 * third_h),
                            ViewportAssignment::new(primary.id, primary.width, third_h),
                            ViewportAssignment::new(primary.id, primary.width, third_h),
                        ],
                    );
                }
            }
            LayoutPreset::MammographyMqsa => {
                self.apply_mammography_mqsa_layout();
            }
            LayoutPreset::Custom(custom) => {
                for assignment in &custom.assignments {
                    self.viewport_assignments
                        .entry(assignment.monitor_id)
                        .or_default()
                        .push(assignment.clone());
                }
            }
        }
    }

    /// Apply MQSA-compliant mammography dual-monitor layout.
    ///
    /// CC views on left monitor, MLO views on right monitor.
    fn apply_mammography_mqsa_layout(&mut self) {
        if self.monitors.len() >= 2 {
            let left = &self.monitors[0];
            let right = &self.monitors[1];

            // Left monitor: CC views (current on top, prior below if enabled)
            let left_assignments = vec![
                ViewportAssignment::new(left.id, left.width, left.height / 2),
                ViewportAssignment::new(left.id, left.width, left.height / 2),
            ];
            self.viewport_assignments.insert(left.id, left_assignments);

            // Right monitor: MLO views (current on top, prior below if enabled)
            let right_assignments = vec![
                ViewportAssignment::new(right.id, right.width, right.height / 2),
                ViewportAssignment::new(right.id, right.width, right.height / 2),
            ];
            self.viewport_assignments.insert(right.id, right_assignments);
        } else if let Some(monitor) = self.monitors.first() {
            // Single monitor: split into quadrants
            let half_w = monitor.width / 2;
            let half_h = monitor.height / 2;
            self.viewport_assignments.insert(
                monitor.id,
                vec![
                    ViewportAssignment::new(monitor.id, half_w, half_h), // CC current
                    ViewportAssignment::new(monitor.id, half_w, half_h), // MLO current
                    ViewportAssignment::new(monitor.id, half_w, half_h), // CC prior
                    ViewportAssignment::new(monitor.id, half_w, half_h), // MLO prior
                ],
            );
        }
    }

    /// Assign a study/series to a specific monitor's viewport.
    ///
    /// If the monitor has multiple viewports, assigns to the first unassigned one.
    pub fn assign_study(&mut self, monitor: MonitorId, study_uid: &str, series_uid: &str) {
        if let Some(assignments) = self.viewport_assignments.get_mut(&monitor) {
            for assignment in assignments.iter_mut() {
                if assignment.study_uid.is_none() {
                    assignment.study_uid = Some(study_uid.to_string());
                    assignment.series_uid = Some(series_uid.to_string());
                    assignment.image_index = 0;
                    return;
                }
            }
            // If all assigned, overwrite the first
            if let Some(assignment) = assignments.first_mut() {
                assignment.study_uid = Some(study_uid.to_string());
                assignment.series_uid = Some(series_uid.to_string());
                assignment.image_index = 0;
            }
        }
    }

    /// Synchronize scroll across monitors with scroll sync enabled.
    ///
    /// The delta is applied to all viewports on all monitors. Only applies
    /// when `scroll_sync_enabled` is true in the sync state.
    pub fn sync_scroll(&mut self, delta: i32) {
        if !self.sync_state.scroll_sync_enabled {
            return;
        }
        for assignments in self.viewport_assignments.values_mut() {
            for assignment in assignments.iter_mut() {
                if assignment.study_uid.is_some() {
                    let new_index = (assignment.image_index as i64 + delta as i64).max(0) as usize;
                    assignment.image_index = new_index;
                }
            }
        }
    }

    /// Synchronize window/level across monitors with WL sync enabled.
    ///
    /// Only applies when `wl_sync_enabled` is true in the sync state.
    pub fn sync_window_level(&mut self, center: f64, width: f64) {
        if !self.sync_state.wl_sync_enabled {
            return;
        }
        if !center.is_finite() || !width.is_finite() || width <= 0.0 {
            return;
        }
        for assignments in self.viewport_assignments.values_mut() {
            for assignment in assignments.iter_mut() {
                assignment.viewport.window_level = crate::WindowLevelState::Explicit { center, width };
            }
        }
    }

    /// Enable or disable scroll synchronization.
    pub fn set_scroll_sync(&mut self, enabled: bool) {
        self.sync_state.scroll_sync_enabled = enabled;
    }

    /// Enable or disable window/level synchronization.
    pub fn set_wl_sync(&mut self, enabled: bool) {
        self.sync_state.wl_sync_enabled = enabled;
    }

    /// Enable or disable zoom synchronization.
    pub fn set_zoom_sync(&mut self, enabled: bool) {
        self.sync_state.zoom_sync_enabled = enabled;
    }

    /// Enable or disable pan synchronization.
    pub fn set_pan_sync(&mut self, enabled: bool) {
        self.sync_state.pan_sync_enabled = enabled;
    }

    /// Set comparison lock configuration.
    pub fn set_comparison_lock(&mut self, config: Option<ComparisonLockConfig>) {
        self.sync_state.comparison_lock = config;
    }

    /// Return the current layout preset.
    pub fn current_layout(&self) -> &LayoutPreset {
        &self.current_layout
    }

    /// Return the monitor information.
    pub fn monitors(&self) -> &[MonitorInfo] {
        &self.monitors
    }

    /// Return the viewport assignments for a specific monitor.
    pub fn assignments_for(&self, monitor: MonitorId) -> Option<&[ViewportAssignment]> {
        self.viewport_assignments.get(&monitor).map(|v| v.as_slice())
    }

    /// Return the total number of viewports across all monitors.
    pub fn total_viewports(&self) -> usize {
        self.viewport_assignments.values().map(|v| v.len()).sum()
    }

    /// Return the cross-monitor synchronization state.
    pub fn sync_state(&self) -> &CrossMonitorSyncState {
        &self.sync_state
    }

    /// Return a mutable reference to the sync state.
    pub fn sync_state_mut(&mut self) -> &mut CrossMonitorSyncState {
        &mut self.sync_state
    }

    /// Create the MQSA-compliant mammography layout structure.
    ///
    /// Returns `None` if the current layout is not `MammographyMqsa`.
    pub fn mammography_layout(&self) -> Option<MammographyMqsaLayout> {
        if !matches!(self.current_layout, LayoutPreset::MammographyMqsa) {
            return None;
        }
        Some(MammographyMqsaLayout {
            left_monitor: MammographyViewport {
                view_type: MammographyView::CC,
                laterality: Laterality::Bilateral,
            },
            right_monitor: MammographyViewport {
                view_type: MammographyView::MLO,
                laterality: Laterality::Bilateral,
            },
            prior_below: true,
        })
    }

    /// Return the number of monitors.
    pub fn monitor_count(&self) -> usize {
        self.monitors.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dual_monitor_setup() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo::new(0, 1920, 1080),
            MonitorInfo::new(1, 1920, 1080),
        ]
    }

    fn single_monitor_setup() -> Vec<MonitorInfo> {
        vec![MonitorInfo::new(0, 1920, 1080)]
    }

    #[test]
    fn single_up_layout() {
        let engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        assert_eq!(engine.total_viewports(), 1);
        assert!(matches!(engine.current_layout(), LayoutPreset::SingleUp));
    }

    #[test]
    fn dual_monitor_layout() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        assert_eq!(engine.total_viewports(), 2);
        assert_eq!(engine.assignments_for(0).unwrap().len(), 1);
        assert_eq!(engine.assignments_for(1).unwrap().len(), 1);
    }

    #[test]
    fn quad_up_layout() {
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::QuadUp);
        assert_eq!(engine.total_viewports(), 4);
        assert_eq!(engine.assignments_for(0).unwrap().len(), 4);
    }

    #[test]
    fn one_plus_two_layout_dual_monitor() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::OnePlusTwo);
        // Primary: 1 viewport, Secondary: 2 viewports
        assert_eq!(engine.assignments_for(0).unwrap().len(), 1);
        assert_eq!(engine.assignments_for(1).unwrap().len(), 2);
        assert_eq!(engine.total_viewports(), 3);
    }

    #[test]
    fn one_plus_two_layout_single_monitor() {
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::OnePlusTwo);
        // Single monitor: 3 viewports
        assert_eq!(engine.assignments_for(0).unwrap().len(), 3);
    }

    #[test]
    fn mammography_mqsa_layout_dual_monitor() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::MammographyMqsa);
        // Each monitor: 2 viewports (current + prior)
        assert_eq!(engine.assignments_for(0).unwrap().len(), 2);
        assert_eq!(engine.assignments_for(1).unwrap().len(), 2);
        assert_eq!(engine.total_viewports(), 4);

        let layout = engine.mammography_layout().unwrap();
        assert_eq!(layout.left_monitor.view_type, MammographyView::CC);
        assert_eq!(layout.right_monitor.view_type, MammographyView::MLO);
        assert!(layout.prior_below);
    }

    #[test]
    fn mammography_mqsa_layout_single_monitor() {
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::MammographyMqsa);
        // Single monitor: 4 quadrants (CC current, MLO current, CC prior, MLO prior)
        assert_eq!(engine.assignments_for(0).unwrap().len(), 4);
    }

    #[test]
    fn mammography_layout_returns_none_for_non_mqsa() {
        let engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        assert!(engine.mammography_layout().is_none());
    }

    #[test]
    fn assign_study_to_viewport() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        engine.assign_study(0, "1.2.3.4", "5.6.7.8");
        let assignments = engine.assignments_for(0).unwrap();
        assert_eq!(assignments[0].study_uid.as_deref(), Some("1.2.3.4"));
        assert_eq!(assignments[0].series_uid.as_deref(), Some("5.6.7.8"));
    }

    #[test]
    fn assign_study_fills_unassigned_first() {
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::QuadUp);
        engine.assign_study(0, "study-1", "series-1");
        engine.assign_study(0, "study-2", "series-2");
        let assignments = engine.assignments_for(0).unwrap();
        assert_eq!(assignments[0].study_uid.as_deref(), Some("study-1"));
        assert_eq!(assignments[1].study_uid.as_deref(), Some("study-2"));
        assert!(assignments[2].study_uid.is_none());
        assert!(assignments[3].study_uid.is_none());
    }

    #[test]
    fn sync_scroll_applies_to_all_viewports() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        engine.assign_study(0, "study-1", "series-1");
        engine.assign_study(1, "study-2", "series-2");
        engine.set_scroll_sync(true);

        engine.sync_scroll(5);
        assert_eq!(engine.assignments_for(0).unwrap()[0].image_index, 5);
        assert_eq!(engine.assignments_for(1).unwrap()[0].image_index, 5);

        engine.sync_scroll(-2);
        assert_eq!(engine.assignments_for(0).unwrap()[0].image_index, 3);
        assert_eq!(engine.assignments_for(1).unwrap()[0].image_index, 3);
    }

    #[test]
    fn sync_scroll_disabled_no_op() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        engine.assign_study(0, "study-1", "series-1");
        // Scroll sync is disabled by default
        engine.sync_scroll(5);
        assert_eq!(engine.assignments_for(0).unwrap()[0].image_index, 0);
    }

    #[test]
    fn sync_window_level_applies_to_all_viewports() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        engine.set_wl_sync(true);

        engine.sync_window_level(40.0, 400.0);
        let a0 = &engine.assignments_for(0).unwrap()[0];
        let a1 = &engine.assignments_for(1).unwrap()[0];
        if let crate::WindowLevelState::Explicit { center, width } = a0.viewport.window_level {
            assert_eq!(center, 40.0);
            assert_eq!(width, 400.0);
        } else {
            panic!("Expected Explicit window level");
        }
        if let crate::WindowLevelState::Explicit { center, width } = a1.viewport.window_level {
            assert_eq!(center, 40.0);
            assert_eq!(width, 400.0);
        } else {
            panic!("Expected Explicit window level");
        }
    }

    #[test]
    fn sync_window_level_ignores_invalid() {
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::SingleUp);
        engine.set_wl_sync(true);
        engine.sync_window_level(40.0, 400.0);
        engine.sync_window_level(f64::NAN, 400.0); // Should be ignored
        let a = &engine.assignments_for(0).unwrap()[0];
        if let crate::WindowLevelState::Explicit { center, .. } = a.viewport.window_level {
            assert_eq!(center, 40.0, "Should not change on NaN");
        }
    }

    #[test]
    fn comparison_lock_config() {
        let mut engine = DiagnosticLayoutEngine::new(dual_monitor_setup());
        engine.apply_layout(LayoutPreset::DualMonitor);
        engine.set_comparison_lock(Some(ComparisonLockConfig {
            reference_monitor: 0,
            synced_monitors: vec![1],
        }));
        assert!(engine.sync_state().comparison_lock.is_some());
        let lock = engine.sync_state().comparison_lock.as_ref().unwrap();
        assert_eq!(lock.reference_monitor, 0);
        assert_eq!(lock.synced_monitors, vec![1]);
    }

    #[test]
    fn monitor_info_creation() {
        let monitor = MonitorInfo::new(0, 1920, 1080);
        assert_eq!(monitor.id, 0);
        assert_eq!(monitor.width, 1920);
        assert_eq!(monitor.height, 1080);
        assert!(monitor.is_primary);
        assert!(monitor.calibration.is_none());
    }

    #[test]
    fn monitor_info_with_calibration() {
        let mut monitor = MonitorInfo::new(1, 1920, 1080);
        monitor.is_primary = false;
        monitor.calibration = Some(DisplayCalibration {
            max_luminance: 500.0,
            min_luminance: 0.5,
            gsdf_conformance: 0.98,
            calibrated_at_epoch_secs: 1700000000,
        });
        assert!(!monitor.is_primary);
        assert!(monitor.calibration.is_some());
    }

    #[test]
    fn custom_layout() {
        let custom = CustomLayout {
            name: "3-up".to_string(),
            assignments: vec![
                ViewportAssignment::new(0, 640, 1080),
                ViewportAssignment::new(0, 640, 1080),
                ViewportAssignment::new(0, 640, 1080),
            ],
        };
        let mut engine = DiagnosticLayoutEngine::new(single_monitor_setup());
        engine.apply_layout(LayoutPreset::Custom(custom));
        assert_eq!(engine.total_viewports(), 3);
    }

    #[test]
    fn empty_monitor_list() {
        let engine = DiagnosticLayoutEngine::new(vec![]);
        assert_eq!(engine.total_viewports(), 0);
        assert_eq!(engine.monitor_count(), 0);
    }
}
