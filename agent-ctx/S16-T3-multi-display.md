# S16-T3: Multi-Monitor Diagnostic Display Layout

## Task: Implement multi-monitor diagnostic display layout engine in viewer-core

### Completed Implementation

1. **Created `/home/z/diccy/crates/viewer-core/src/multi_display.rs`**
   - `DiagnosticLayoutEngine` — main orchestrator for multi-monitor layouts
   - `MonitorInfo` — display monitor information with optional calibration
   - `DisplayCalibration` — GSDF conformance and luminance data
   - `LayoutPreset` enum: SingleUp, DualMonitor, QuadUp, OnePlusTwo, MammographyMqsa, Custom
   - `ViewportAssignment` — per-monitor viewport with study/series binding
   - `CrossMonitorSyncState` — scroll/WL/zoom/pan synchronization flags
   - `ComparisonLockConfig` — reference/comparison monitor locking
   - `MammographyMqsaLayout` — MQSA-compliant CC/MLO layout structure
   - `MammographyView` enum: CC, MLO, Magnification
   - `Laterality` enum: Left, Right, Bilateral

2. **Updated `/home/z/diccy/crates/viewer-core/src/lib.rs`**
   - Added `pub mod multi_display;`
   - Re-exported key types: DiagnosticLayoutEngine, LayoutPreset, MonitorInfo, etc.

### Key Features
- **Layout Presets**: SingleUp, DualMonitor, QuadUp, OnePlusTwo, MammographyMqsa, Custom
- **MQSA Compliance**: CC views on left monitor, MLO on right, priors below
- **Synchronized Operations**: scroll_sync, wl_sync, zoom_sync, pan_sync
- **Comparison Lock**: reference monitor drives synchronized monitors
- **Single-monitor fallback**: All layouts degrade gracefully to single monitor
- **Custom layouts**: User-defined viewport assignments

### Test Results
- 19 multi_display tests passing
- Tests cover: all layout presets, MQSA compliance, viewport assignment, scroll sync, window/level sync, comparison lock, monitor info, custom layouts, empty monitors
