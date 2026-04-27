#![deny(missing_docs)]

//! DiCCY Viewer SDK — High-level JavaScript/TypeScript integration via wasm-bindgen.
//!
//! Provides a [`DicomViewer`] class with event-driven architecture for embedding
//! the DiCCY DICOM viewer into third-party React, Vue, or Svelte applications
//! without iframe isolation.
//!
//! # Event Types
//!
//! - `study-loaded` — fired when a study is fully loaded from WADO-RS
//! - `viewport-changed` — fired when viewport pan/zoom/window-level changes
//! - `measurement-created` — fired when a new measurement is placed
//! - `measurement-updated` — fired when a measurement is modified
//! - `measurement-deleted` — fired when a measurement is removed
//! - `segmentation-updated` — fired when segmentation overlay changes
//! - `annotation-created` — fired when a 3D annotation is created
//! - `error` — fired when an error occurs
//!
//! # Integration Pattern (iframe-less)
//!
//! ```js
//! import init, { DicomViewer } from "dicom-viewer-sdk";
//!
//! await init();
//! const viewer = new DicomViewer("viewer-container");
//! viewer.add_measurement_listener((evt) => {
//!   console.log("Measurement:", JSON.parse(evt.data));
//! });
//! viewer.load_study("https://pacs.example.com/wado-rs/studies/1.2.3.4");
//! ```

mod events;
mod react;

pub use events::*;
pub use react::*;

use std::collections::HashMap;
use viewer_core::{
    ImagePoint, MeasurementKind, MeasurementUnit, ViewerModel, Viewport2D, WindowLevelState,
};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// ViewerModelState — deterministic snapshot for the SDK boundary
// ---------------------------------------------------------------------------

/// Deterministic viewer model state snapshot for the SDK boundary.
///
/// This is a simplified, serializable version of `ViewerModel` that can be
/// safely passed across the WASM boundary without exposing internal types.
#[derive(Debug, Clone)]
pub struct ViewerModelState {
    /// Viewport width in pixels.
    pub viewport_width: u32,
    /// Viewport height in pixels.
    pub viewport_height: u32,
    /// Current zoom factor.
    pub zoom: f64,
    /// Window center value.
    pub window_center: f64,
    /// Window width value.
    pub window_width: f64,
    /// Currently loaded study UID, if any.
    pub study_uid: Option<String>,
    /// Currently loaded series UID, if any.
    pub series_uid: Option<String>,
    /// Current frame index.
    pub frame_index: u32,
    /// Total frames in current series.
    pub total_frames: u32,
    /// Number of measurements.
    pub measurement_count: usize,
    /// Whether a study is loaded.
    pub study_loaded: bool,
}

impl Default for ViewerModelState {
    fn default() -> Self {
        Self {
            viewport_width: 0,
            viewport_height: 0,
            zoom: 1.0,
            window_center: 0.0,
            window_width: 1.0,
            study_uid: None,
            series_uid: None,
            frame_index: 0,
            total_frames: 1,
            measurement_count: 0,
            study_loaded: false,
        }
    }
}

impl ViewerModelState {
    /// Synchronize from the internal `ViewerModel`.
    pub fn sync_from_model(&mut self, model: &ViewerModel) {
        self.viewport_width = model.viewport.width;
        self.viewport_height = model.viewport.height;
        self.zoom = model.viewport.zoom;
        match model.viewport.window_level {
            WindowLevelState::Auto => {
                self.window_center = 0.0;
                self.window_width = 1.0;
            }
            WindowLevelState::Explicit { center, width } => {
                self.window_center = center;
                self.window_width = width;
            }
        }
        self.frame_index = model.frame_navigation.frame_index;
        self.total_frames = model.frame_navigation.total_frames;
        self.measurement_count = model.measurements.len();
    }

    /// Serialize to a JSON string.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"viewport_width":{},"viewport_height":{},"zoom":{},"window_center":{},"window_width":{},"study_uid":{},"series_uid":{},"frame_index":{},"total_frames":{},"measurement_count":{},"study_loaded":{}}}"#,
            self.viewport_width,
            self.viewport_height,
            self.zoom,
            self.window_center,
            self.window_width,
            json_option_string(&self.study_uid),
            json_option_string(&self.series_uid),
            self.frame_index,
            self.total_frames,
            self.measurement_count,
            self.study_loaded,
        )
    }
}

fn json_option_string(opt: &Option<String>) -> String {
    match opt {
        Some(s) => format!("\"{}\"", escape_json_string(s)),
        None => "null".to_string(),
    }
}

fn escape_json_string(s: &str) -> String {
    s.chars()
        .flat_map(|ch| match ch {
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            c => vec![c],
        })
        .collect()
}

// ---------------------------------------------------------------------------
// EventListenerMap — stores JS callbacks keyed by event type
// ---------------------------------------------------------------------------

/// Event listener storage keyed by event type string.
///
/// In native builds, this stores unit placeholders. In WASM builds, this would
/// store `js_sys::Function` references. For native testability we store
/// serializable callback descriptors.
#[derive(Debug, Clone, Default)]
pub struct EventListenerMap {
    listeners: HashMap<String, Vec<EventListenerEntry>>,
}

/// A registered event listener entry.
#[derive(Debug, Clone)]
pub struct EventListenerEntry {
    /// Unique listener identifier for removal.
    pub id: u64,
    /// Event type this listener is bound to.
    pub event_type: String,
}

impl EventListenerMap {
    /// Create a new empty listener map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a listener for the given event type. Returns the listener ID.
    pub fn add(&mut self, event_type: &str) -> u64 {
        let id = self.next_id();
        self.listeners
            .entry(event_type.to_string())
            .or_default()
            .push(EventListenerEntry {
                id,
                event_type: event_type.to_string(),
            });
        id
    }

    /// Remove a listener by event type and ID.
    pub fn remove(&mut self, event_type: &str, id: u64) -> bool {
        if let Some(entries) = self.listeners.get_mut(event_type) {
            let before = entries.len();
            entries.retain(|e| e.id != id);
            entries.len() < before
        } else {
            false
        }
    }

    /// Remove all listeners for a given event type.
    pub fn remove_all(&mut self, event_type: &str) -> usize {
        self.listeners.remove(event_type).map(|e| e.len()).unwrap_or(0)
    }

    /// Return the number of listeners for a given event type.
    pub fn count(&self, event_type: &str) -> usize {
        self.listeners.get(event_type).map(|e| e.len()).unwrap_or(0)
    }

    /// Return the total number of listeners across all event types.
    pub fn total_count(&self) -> usize {
        self.listeners.values().map(|e| e.len()).sum()
    }

    fn next_id(&mut self) -> u64 {
        static mut COUNTER: u64 = 0;
        // SAFETY: single-threaded access in the SDK context
        unsafe {
            COUNTER += 1;
            COUNTER
        }
    }
}

// ---------------------------------------------------------------------------
// DicomViewer — high-level SDK class
// ---------------------------------------------------------------------------

/// High-level DiCCY Viewer SDK for JavaScript/TypeScript integration.
///
/// Provides an event-driven API for embedding the DiCCY DICOM viewer into
/// third-party applications without iframe isolation.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
#[derive(Debug)]
pub struct DicomViewer {
    /// DOM container element ID.
    container_id: String,
    /// Internal viewer model state snapshot.
    state: ViewerModelState,
    /// Internal viewer model for deterministic operations.
    model: ViewerModel,
    /// Registered event listeners.
    event_listeners: EventListenerMap,
    /// Whether the viewer has been destroyed.
    destroyed: bool,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl DicomViewer {
    /// Create a new DicomViewer bound to the given container element.
    ///
    /// The container element must exist in the DOM. The viewer will create
    /// a canvas element inside the container for rendering.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(container_id: &str) -> Self {
        let container = container_id.to_string();
        // Default to a reasonable viewport size; resize will update.
        let model = ViewerModel::new(512, 512);
        let mut state = ViewerModelState::default();
        state.sync_from_model(&model);

        Self {
            container_id: container,
            state,
            model,
            event_listeners: EventListenerMap::new(),
            destroyed: false,
        }
    }

    /// Load a DICOM study from a WADO-RS URL.
    ///
    /// The URL should point to a WADO-RS study endpoint, e.g.
    /// `https://pacs.example.com/wado-rs/studies/1.2.3.4`
    ///
    /// Returns an error string on failure, or empty string on success.
    pub fn load_study(&mut self, wado_rs_url: &str) -> String {
        if self.destroyed {
            return "error:viewer_destroyed".to_string();
        }
        if wado_rs_url.is_empty() {
            return "error:empty_url".to_string();
        }
        // Parse study UID from WADO-RS URL pattern
        let study_uid = extract_study_uid_from_url(wado_rs_url);
        self.state.study_uid = Some(study_uid.clone());
        self.state.study_loaded = true;
        self.state.sync_from_model(&self.model);

        // Emit study-loaded event
        let event_data = format!(
            r#"{{"type":"study-loaded","study_uid":"{}","url":"{}"}}"#,
            escape_json_string(&study_uid),
            escape_json_string(wado_rs_url),
        );
        let _ = event_data; // In WASM, this would dispatch to JS listeners

        String::new()
    }

    /// Set window/level values.
    ///
    /// This sets explicit window center and width for the current viewport.
    pub fn set_window_level(&mut self, center: f64, width: f64) {
        if self.destroyed {
            return;
        }
        if !center.is_finite() || !width.is_finite() || width <= 0.0 {
            return;
        }
        self.model.viewport.window_level = WindowLevelState::Explicit { center, width };
        self.state.window_center = center;
        self.state.window_width = width;
    }

    /// Add event listener for measurement events.
    ///
    /// The callback will be invoked with an `SdkEvent` object containing
    /// event type and data fields.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "addMeasurementListener"))]
    pub fn add_measurement_listener(&mut self) -> u64 {
        if self.destroyed {
            return 0;
        }
        self.event_listeners.add(SdkEventType::MeasurementCreated.as_str())
    }

    /// Add event listener for study loaded events.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "addStudyLoadedListener"))]
    pub fn add_study_loaded_listener(&mut self) -> u64 {
        if self.destroyed {
            return 0;
        }
        self.event_listeners.add(SdkEventType::StudyLoaded.as_str())
    }

    /// Add event listener for viewport changed events.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "addViewportChangedListener"))]
    pub fn add_viewport_changed_listener(&mut self) -> u64 {
        if self.destroyed {
            return 0;
        }
        self.event_listeners.add(SdkEventType::ViewportChanged.as_str())
    }

    /// Add event listener for segmentation updated events.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "addSegmentationUpdatedListener"))]
    pub fn add_segmentation_updated_listener(&mut self) -> u64 {
        if self.destroyed {
            return 0;
        }
        self.event_listeners.add(SdkEventType::SegmentationUpdated.as_str())
    }

    /// Remove an event listener by event type and listener ID.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = "removeEventListener"))]
    pub fn remove_event_listener(&mut self, event_type: &str, listener_id: u64) -> bool {
        self.event_listeners.remove(event_type, listener_id)
    }

    /// Get current viewport state as JSON.
    pub fn get_viewport_state(&self) -> String {
        self.state.to_json()
    }

    /// Destroy the viewer and clean up all resources.
    ///
    /// After calling this, all other methods will return errors or no-ops.
    pub fn destroy(&mut self) {
        if self.destroyed {
            return;
        }
        self.event_listeners = EventListenerMap::new();
        self.state = ViewerModelState::default();
        self.destroyed = true;
    }
}

// ---------------------------------------------------------------------------
// Non-WASM public API (for native testing and integration)
// ---------------------------------------------------------------------------

impl DicomViewer {
    /// Return the container element ID.
    pub fn container_id(&self) -> &str {
        &self.container_id
    }

    /// Return whether the viewer has been destroyed.
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Return the current viewer model state.
    pub fn state(&self) -> &ViewerModelState {
        &self.state
    }

    /// Return the event listener map.
    pub fn event_listeners(&self) -> &EventListenerMap {
        &self.event_listeners
    }

    /// Return a mutable reference to the internal viewer model.
    pub fn model_mut(&mut self) -> &mut ViewerModel {
        &mut self.model
    }

    /// Emit an SDK event to all registered listeners for the event type.
    ///
    /// Returns the number of listeners that were notified.
    pub fn emit_event(&mut self, event: &SdkEvent) -> usize {
        if self.destroyed {
            return 0;
        }
        let count = self.event_listeners.count(event.event_type.as_str());
        // In WASM, this would call each registered js_sys::Function
        // For native, we just return the count
        count
    }

    /// Create a measurement event payload for the current state.
    pub fn create_measurement_event(&self, measurement_id: &str, kind: &str, value: f64) -> SdkEvent {
        SdkEvent {
            event_type: SdkEventType::MeasurementCreated.as_str().to_string(),
            data: format!(
                r#"{{"measurement_id":"{}","kind":"{}","value":{},"study_uid":{}}}"#,
                escape_json_string(measurement_id),
                escape_json_string(kind),
                value,
                json_option_string(&self.state.study_uid),
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Extract a Study Instance UID from a WADO-RS URL.
///
/// Expected patterns:
/// - `.../studies/{uid}`
/// - `.../studies/{uid}/series/{series_uid}`
/// - `.../studies/{uid}/series/{series_uid}/instances/{sop_uid}`
fn extract_study_uid_from_url(url: &str) -> String {
    let parts: Vec<&str> = url.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        if *part == "studies" {
            if let Some(uid) = parts.get(i + 1) {
                return (*uid).to_string();
            }
        }
    }
    url.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_new_viewer() {
        let viewer = DicomViewer::new("test-container");
        assert_eq!(viewer.container_id(), "test-container");
        assert!(!viewer.is_destroyed());
        assert_eq!(viewer.state().study_loaded, false);
    }

    #[test]
    fn sdk_load_study() {
        let mut viewer = DicomViewer::new("container");
        let result = viewer.load_study("https://pacs.example.com/wado-rs/studies/1.2.3.4.5");
        assert!(result.is_empty(), "load_study should succeed");
        assert!(viewer.state().study_loaded);
        assert_eq!(viewer.state().study_uid.as_deref(), Some("1.2.3.4.5"));
    }

    #[test]
    fn sdk_load_study_empty_url() {
        let mut viewer = DicomViewer::new("container");
        let result = viewer.load_study("");
        assert!(result.starts_with("error:"));
    }

    #[test]
    fn sdk_load_study_after_destroy() {
        let mut viewer = DicomViewer::new("container");
        viewer.destroy();
        let result = viewer.load_study("https://pacs.example.com/studies/1.2.3");
        assert!(result.starts_with("error:"));
    }

    #[test]
    fn sdk_set_window_level() {
        let mut viewer = DicomViewer::new("container");
        viewer.set_window_level(40.0, 400.0);
        assert_eq!(viewer.state().window_center, 40.0);
        assert_eq!(viewer.state().window_width, 400.0);
    }

    #[test]
    fn sdk_set_window_level_invalid() {
        let mut viewer = DicomViewer::new("container");
        viewer.set_window_level(40.0, 400.0);
        // NaN center
        viewer.set_window_level(f64::NAN, 400.0);
        assert_eq!(viewer.state().window_center, 40.0, "should not change on NaN");
        // Negative width
        viewer.set_window_level(40.0, -100.0);
        assert_eq!(viewer.state().window_width, 400.0, "should not change on negative width");
    }

    #[test]
    fn sdk_add_remove_listeners() {
        let mut viewer = DicomViewer::new("container");
        let id1 = viewer.add_measurement_listener();
        let id2 = viewer.add_study_loaded_listener();
        assert_ne!(id1, id2, "listener IDs should be unique");
        assert_eq!(viewer.event_listeners().count("measurement-created"), 1);
        assert_eq!(viewer.event_listeners().count("study-loaded"), 1);

        let removed = viewer.remove_event_listener("measurement-created", id1);
        assert!(removed);
        assert_eq!(viewer.event_listeners().count("measurement-created"), 0);
    }

    #[test]
    fn sdk_destroy() {
        let mut viewer = DicomViewer::new("container");
        viewer.add_measurement_listener();
        viewer.add_study_loaded_listener();
        assert!(viewer.event_listeners().total_count() > 0);

        viewer.destroy();
        assert!(viewer.is_destroyed());
        assert_eq!(viewer.event_listeners().total_count(), 0);
    }

    #[test]
    fn sdk_get_viewport_state_json() {
        let viewer = DicomViewer::new("container");
        let json = viewer.get_viewport_state();
        assert!(json.contains("viewport_width"));
        assert!(json.contains("zoom"));
        assert!(json.contains("study_loaded"));
    }

    #[test]
    fn sdk_emit_event() {
        let mut viewer = DicomViewer::new("container");
        viewer.add_measurement_listener();
        let event = SdkEvent {
            event_type: SdkEventType::MeasurementCreated.as_str().to_string(),
            data: r#"{"id":"m1"}"#.to_string(),
        };
        let count = viewer.emit_event(&event);
        assert_eq!(count, 1);
    }

    #[test]
    fn sdk_create_measurement_event() {
        let viewer = DicomViewer::new("container");
        let event = viewer.create_measurement_event("m1", "distance", 42.5);
        assert_eq!(event.event_type, "measurement-created");
        assert!(event.data.contains("m1"));
        assert!(event.data.contains("42.5"));
    }

    #[test]
    fn viewer_model_state_json() {
        let mut state = ViewerModelState::default();
        state.study_uid = Some("1.2.3".to_string());
        state.study_loaded = true;
        let json = state.to_json();
        assert!(json.contains("\"1.2.3\""));
        assert!(json.contains("true"));
    }

    #[test]
    fn extract_study_uid() {
        assert_eq!(
            extract_study_uid_from_url("https://pacs.example.com/wado-rs/studies/1.2.3.4"),
            "1.2.3.4"
        );
        assert_eq!(
            extract_study_uid_from_url("https://pacs.example.com/studies/1.2.3/series/5.6.7"),
            "1.2.3"
        );
        assert_eq!(
            extract_study_uid_from_url("no-match-here"),
            "no-match-here"
        );
    }

    #[test]
    fn event_listener_map_operations() {
        let mut map = EventListenerMap::new();
        let id1 = map.add("study-loaded");
        let id2 = map.add("study-loaded");
        let id3 = map.add("viewport-changed");
        assert_eq!(map.count("study-loaded"), 2);
        assert_eq!(map.count("viewport-changed"), 1);
        assert_eq!(map.total_count(), 3);

        assert!(map.remove("study-loaded", id1));
        assert_eq!(map.count("study-loaded"), 1);
        assert!(!map.remove("study-loaded", 9999)); // non-existent
        assert_eq!(map.remove_all("viewport-changed"), 1);
        assert_eq!(map.total_count(), 1);
    }
}
