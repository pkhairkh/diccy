// Auto-extracted from /home/z/diccy/crates/dicom-workflow-server/src/ups_workflow.rs
// S13-T8: Move inline tests to tests/ directories


use dicom_workflow_server::*;
use dicom_mpps::{MppsStatus, MppsUpdate};
use dicom_ups::UpsState;
use dicom_worklist::WorklistItem;

fn sample_worklist_item(step_id: &str) -> WorklistItem {
    WorklistItem {
        scheduled_step_id: step_id.to_string(),
        modality: "CT".to_string(),
        start_date: "20260101".to_string(),
        start_time: "090000".to_string(),
        requested_procedure_id: Some("PROC-1".to_string()),
        scheduled_station_ae_title: Some("MODALITY_AE".to_string()),
        patient_id: Some("PID-1".to_string()),
        accession_number: Some("ACC-1".to_string()),
    }
}

fn sample_mpps_update(status: MppsStatus) -> MppsUpdate {
    MppsUpdate {
        sop_instance_uid: "1.2.840.10008.3.1".to_string(),
        status,
        performed_step_id: "STEP-1".to_string(),
        start_date: "20260101".to_string(),
        start_time: "090100".to_string(),
        end_date: Some("20260101".to_string()),
        end_time: Some("090500".to_string()),
    }
}

#[test]
fn workflow_adapter_maps_worklist_and_mpps_lifecycle() {
    let mut adapter = UpsWorkflowAdapter::new();
    let created = adapter
        .create_from_worklist(
            &sample_worklist_item("STEP-1"),
            "1.2.840.10008.5.1.4.34.11".to_string(),
        )
        .expect("create");
    assert_eq!(created.scheduled_step_id, "STEP-1");

    let in_progress = adapter
        .apply_mpps_update(
            "1.2.840.10008.5.1.4.34.11",
            &sample_mpps_update(MppsStatus::InProgress),
        )
        .expect("in progress");
    assert_eq!(in_progress.state, UpsState::InProgress);

    let completed = adapter
        .apply_mpps_update(
            "1.2.840.10008.5.1.4.34.11",
            &sample_mpps_update(MppsStatus::Completed),
        )
        .expect("complete");
    assert_eq!(completed.state, UpsState::Completed);

    let snapshot = adapter.snapshot();
    assert_eq!(snapshot.total_workitems, 1);
    assert_eq!(snapshot.terminal_workitems, 1);
}
