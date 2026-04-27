//! SDK event types and serialization.
//!
//! Defines the event-driven architecture for the DiCCY Viewer SDK.
//! Events are the primary communication channel between the viewer
//! and the embedding application.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SdkEventType — enumeration of all SDK events
// ---------------------------------------------------------------------------

/// SDK event types emitted by the viewer.
///
/// Each event type corresponds to a specific viewer state change or
/// user interaction. Embedding applications register listeners for
/// specific event types to receive notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SdkEventType {
    /// A study has been loaded from a WADO-RS URL.
    StudyLoaded,
    /// The viewport pan/zoom/window-level has changed.
    ViewportChanged,
    /// A new measurement has been created.
    MeasurementCreated,
    /// An existing measurement has been modified.
    MeasurementUpdated,
    /// A measurement has been deleted.
    MeasurementDeleted,
    /// A segmentation overlay has been updated.
    SegmentationUpdated,
    /// A 3D annotation has been created.
    AnnotationCreated,
    /// An error occurred in the viewer.
    Error,
}

impl SdkEventType {
    /// Return the string identifier for this event type.
    ///
    /// Used as the event name when dispatching to JavaScript listeners
    /// and for listener registration.
    pub fn as_str(&self) -> &'static str {
        match self {
            SdkEventType::StudyLoaded => "study-loaded",
            SdkEventType::ViewportChanged => "viewport-changed",
            SdkEventType::MeasurementCreated => "measurement-created",
            SdkEventType::MeasurementUpdated => "measurement-updated",
            SdkEventType::MeasurementDeleted => "measurement-deleted",
            SdkEventType::SegmentationUpdated => "segmentation-updated",
            SdkEventType::AnnotationCreated => "annotation-created",
            SdkEventType::Error => "error",
        }
    }

    /// Parse an event type from its string identifier.
    ///
    /// Returns `None` for unrecognized strings.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "study-loaded" => Some(SdkEventType::StudyLoaded),
            "viewport-changed" => Some(SdkEventType::ViewportChanged),
            "measurement-created" => Some(SdkEventType::MeasurementCreated),
            "measurement-updated" => Some(SdkEventType::MeasurementUpdated),
            "measurement-deleted" => Some(SdkEventType::MeasurementDeleted),
            "segmentation-updated" => Some(SdkEventType::SegmentationUpdated),
            "annotation-created" => Some(SdkEventType::AnnotationCreated),
            "error" => Some(SdkEventType::Error),
            _ => None,
        }
    }

    /// Return all defined event types in declaration order.
    pub fn all() -> &'static [SdkEventType] {
        &[
            SdkEventType::StudyLoaded,
            SdkEventType::ViewportChanged,
            SdkEventType::MeasurementCreated,
            SdkEventType::MeasurementUpdated,
            SdkEventType::MeasurementDeleted,
            SdkEventType::SegmentationUpdated,
            SdkEventType::AnnotationCreated,
            SdkEventType::Error,
        ]
    }
}

// ---------------------------------------------------------------------------
// SdkEvent — event payload dispatched to listeners
// ---------------------------------------------------------------------------

/// An SDK event dispatched to registered listeners.
///
/// Each event carries a type identifier and a JSON data payload
/// containing event-specific information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SdkEvent {
    /// The event type identifier.
    pub event_type: String,
    /// JSON-encoded event data payload.
    pub data: String,
}

impl SdkEvent {
    /// Create a new SDK event with the given type and data.
    pub fn new(event_type: SdkEventType, data: impl Into<String>) -> Self {
        Self {
            event_type: event_type.as_str().to_string(),
            data: data.into(),
        }
    }

    /// Create a study-loaded event.
    pub fn study_loaded(study_uid: &str, wado_url: &str) -> Self {
        Self::new(
            SdkEventType::StudyLoaded,
            format!(
                r#"{{"study_uid":"{}","wado_url":"{}"}}"#,
                escape_json(study_uid),
                escape_json(wado_url),
            ),
        )
    }

    /// Create a viewport-changed event.
    pub fn viewport_changed(zoom: f64, center_x: f64, center_y: f64, window_center: f64, window_width: f64) -> Self {
        Self::new(
            SdkEventType::ViewportChanged,
            format!(
                r#"{{"zoom":{},"center_x":{},"center_y":{},"window_center":{},"window_width":{}}}"#,
                zoom, center_x, center_y, window_center, window_width,
            ),
        )
    }

    /// Create a measurement-created event.
    pub fn measurement_created(measurement_id: &str, kind: &str, value: f64, unit: &str) -> Self {
        Self::new(
            SdkEventType::MeasurementCreated,
            format!(
                r#"{{"measurement_id":"{}","kind":"{}","value":{},"unit":"{}"}}"#,
                escape_json(measurement_id),
                escape_json(kind),
                value,
                escape_json(unit),
            ),
        )
    }

    /// Create a measurement-updated event.
    pub fn measurement_updated(measurement_id: &str, value: f64) -> Self {
        Self::new(
            SdkEventType::MeasurementUpdated,
            format!(
                r#"{{"measurement_id":"{}","value":{}}}"#,
                escape_json(measurement_id),
                value,
            ),
        )
    }

    /// Create a measurement-deleted event.
    pub fn measurement_deleted(measurement_id: &str) -> Self {
        Self::new(
            SdkEventType::MeasurementDeleted,
            format!(r#"{{"measurement_id":"{}"}}"#, escape_json(measurement_id)),
        )
    }

    /// Create a segmentation-updated event.
    pub fn segmentation_updated(segmentation_id: &str, label_count: usize) -> Self {
        Self::new(
            SdkEventType::SegmentationUpdated,
            format!(
                r#"{{"segmentation_id":"{}","label_count":{}}}"#,
                escape_json(segmentation_id),
                label_count,
            ),
        )
    }

    /// Create an error event.
    pub fn error(code: &str, message: &str) -> Self {
        Self::new(
            SdkEventType::Error,
            format!(
                r#"{{"code":"{}","message":"{}"}}"#,
                escape_json(code),
                escape_json(message),
            ),
        )
    }

    /// Serialize this event to a JSON string.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"type":"{}","data":{}}}"#,
            self.event_type,
            if self.data.starts_with('{') || self.data.starts_with('[') {
                self.data.clone()
            } else {
                format!("\"{}\"", escape_json(&self.data))
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Event builder for complex event data
// ---------------------------------------------------------------------------

/// Builder for constructing SDK event payloads.
#[derive(Debug, Clone, Default)]
pub struct SdkEventDataBuilder {
    fields: Vec<(String, String)>,
}

impl SdkEventDataBuilder {
    /// Create a new event data builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a string field.
    pub fn string_field(mut self, key: &str, value: &str) -> Self {
        self.fields.push((key.to_string(), format!("\"{}\"", escape_json(value))));
        self
    }

    /// Add a numeric field.
    pub fn number_field(mut self, key: &str, value: f64) -> Self {
        self.fields.push((key.to_string(), format!("{}", value)));
        self
    }

    /// Add a boolean field.
    pub fn bool_field(mut self, key: &str, value: bool) -> Self {
        self.fields.push((key.to_string(), value.to_string()));
        self
    }

    /// Build the JSON string.
    pub fn build(self) -> String {
        let fields: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!("\"{}\":{}", escape_json(k), v))
            .collect();
        format!("{{{}}}", fields.join(","))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Escape a string for JSON embedding.
fn escape_json(s: &str) -> String {
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_type_as_str_roundtrip() {
        for et in SdkEventType::all() {
            let s = et.as_str();
            let parsed = SdkEventType::from_str_opt(s);
            assert_eq!(parsed, Some(*et), "roundtrip failed for {:?}", et);
        }
    }

    #[test]
    fn event_type_from_str_unknown() {
        assert_eq!(SdkEventType::from_str_opt("nonexistent"), None);
    }

    #[test]
    fn sdk_event_study_loaded() {
        let event = SdkEvent::study_loaded("1.2.3.4", "https://pacs.example.com/studies/1.2.3.4");
        assert_eq!(event.event_type, "study-loaded");
        assert!(event.data.contains("1.2.3.4"));
    }

    #[test]
    fn sdk_event_viewport_changed() {
        let event = SdkEvent::viewport_changed(2.0, 256.0, 256.0, 40.0, 400.0);
        assert_eq!(event.event_type, "viewport-changed");
        assert!(event.data.contains("2") || event.data.contains("2.0"));
        assert!(event.data.contains("40") || event.data.contains("40.0"));
    }

    #[test]
    fn sdk_event_measurement_created() {
        let event = SdkEvent::measurement_created("m1", "distance", 42.5, "mm");
        assert_eq!(event.event_type, "measurement-created");
        assert!(event.data.contains("\"m1\""));
        assert!(event.data.contains("42.5"));
    }

    #[test]
    fn sdk_event_measurement_updated() {
        let event = SdkEvent::measurement_updated("m1", 50.0);
        assert_eq!(event.event_type, "measurement-updated");
        assert!(event.data.contains("50") || event.data.contains("50.0"));
    }

    #[test]
    fn sdk_event_measurement_deleted() {
        let event = SdkEvent::measurement_deleted("m1");
        assert_eq!(event.event_type, "measurement-deleted");
    }

    #[test]
    fn sdk_event_segmentation_updated() {
        let event = SdkEvent::segmentation_updated("seg1", 5);
        assert_eq!(event.event_type, "segmentation-updated");
        assert!(event.data.contains("5"));
    }

    #[test]
    fn sdk_event_error() {
        let event = SdkEvent::error("LOAD_FAILED", "Network timeout");
        assert_eq!(event.event_type, "error");
        assert!(event.data.contains("LOAD_FAILED"));
    }

    #[test]
    fn sdk_event_to_json() {
        let event = SdkEvent::measurement_created("m1", "distance", 10.0, "px");
        let json = event.to_json();
        assert!(json.contains("\"type\":\"measurement-created\""));
        assert!(json.contains("\"data\":"));
    }

    #[test]
    fn event_data_builder() {
        let data = SdkEventDataBuilder::new()
            .string_field("id", "test-1")
            .number_field("value", 42.5)
            .bool_field("active", true)
            .build();
        assert!(data.contains("\"id\":\"test-1\""));
        assert!(data.contains("\"value\":42.5"));
        assert!(data.contains("\"active\":true"));
    }

    #[test]
    fn escape_json_special_chars() {
        let escaped = escape_json("hello\nworld\"test\\end");
        assert_eq!(escaped, "hello\\nworld\\\"test\\\\end");
    }
}
