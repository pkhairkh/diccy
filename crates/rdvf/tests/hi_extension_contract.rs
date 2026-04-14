use rdvf::{
    ExtensionEvent, ExtensionEventKind, ExtensionRegistry, ExtensionStatus, SampleEchoExtension,
};
use std::collections::BTreeMap;

#[test]
fn sample_extension_roundtrip_is_deterministic() {
    // REQ-HI-173, REQ-HI-177
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(SampleEchoExtension))
        .expect("register");
    let mut payload = BTreeMap::new();
    payload.insert("tool".to_string(), "length".to_string());
    payload.insert("value_mm".to_string(), "25.0".to_string());
    let event = ExtensionEvent {
        kind: ExtensionEventKind::MeasurementCaptured,
        payload,
        study_instance_uid: Some("1.2.3".to_string()),
        series_instance_uid: Some("1.2.3.4".to_string()),
        sop_instance_uid: Some("1.2.3.4.5".to_string()),
    };

    let first = registry.run_all(&event).expect("first run");
    let second = registry.run_all(&event).expect("second run");
    assert_eq!(first, second);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].result.status, ExtensionStatus::Accepted);
    assert_eq!(
        first[0].result.outputs.get("payload.value_mm"),
        Some(&"25.0".to_string())
    );
}
