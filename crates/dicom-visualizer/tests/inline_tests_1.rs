// Auto-extracted from /home/z/diccy/crates/dicom-visualizer/src/lib.rs
// S13-T8: Move inline tests to tests/ directories

use dicom_visualizer::*;
use dicom_core::{Dataset, Element, ErrorKind, Value, Vr};

fn context_dataset(study: &str, series: &str, instance: &str) -> Dataset {
    let mut dataset = Dataset::new();
    dataset.insert(Element::new(TAG_STUDY_UID, Vr::Ui, Value::Uid(study.to_string())).unwrap());
    dataset
        .insert(Element::new(TAG_SERIES_UID, Vr::Ui, Value::Uid(series.to_string())).unwrap());
    dataset.insert(
        Element::new(TAG_INSTANCE_UID, Vr::Ui, Value::Uid(instance.to_string())).unwrap(),
    );
    dataset
}

#[test]
fn export_plan_rejects_duplicate_measurement_ids() {
    let active = extract_export_context(&context_dataset("1.2.3", "1.2.3.4", "1.2.3.4.5"))
        .expect("context");
    let plan = ExportPlan {
        active_context: active.clone(),
        bindings: vec![
            MeasurementBinding {
                measurement_id: "m-1".to_string(),
                source_context: active.clone(),
            },
            MeasurementBinding {
                measurement_id: "m-1".to_string(),
                source_context: active,
            },
        ],
    };

    let err = validate_export_plan(&plan).expect_err("duplicate id must fail");
    assert!(matches!(err.kind(), ErrorKind::IntegrityError { .. }));
}
