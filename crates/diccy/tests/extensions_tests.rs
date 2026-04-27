// Auto-extracted from /home/z/diccy/crates/diccy/src/extensions.rs
// S13-T8: Move inline tests to tests/ directories

use diccy::*;
use std::collections::BTreeMap;

#[test]
fn registry_rejects_duplicate_extension_ids() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(SampleEchoExtension))
        .expect("first");
    let err = registry
        .register(Box::new(SampleEchoExtension))
        .expect_err("duplicate");
    assert_eq!(err.code(), "DVF.INTEGRITY.ERROR");
}

#[test]
fn registry_executes_extensions_in_registration_order() {
    let mut registry = ExtensionRegistry::new();
    registry
        .register(Box::new(SampleEchoExtension))
        .expect("register");
    let mut payload = BTreeMap::new();
    payload.insert("measurement_mm".to_string(), "12.5".to_string());
    let event = ExtensionEvent {
        kind: ExtensionEventKind::MeasurementCaptured,
        payload,
        study_instance_uid: Some("1.2.3".to_string()),
        series_instance_uid: None,
        sop_instance_uid: None,
    };
    let rows = registry.run_all(&event).expect("run");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].extension_id, "sample.echo");
    assert_eq!(rows[0].result.status, ExtensionStatus::Accepted);
    assert_eq!(
        rows[0].result.outputs.get("payload.measurement_mm"),
        Some(&"12.5".to_string())
    );
}
