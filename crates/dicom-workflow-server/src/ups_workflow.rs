//! UPS workflow adapter bridging Worklist and MPPS lifecycles.

use dicom_core::Result;
use dicom_mpps::{MppsStatus, MppsUpdate};
use dicom_ups::{UpsCommandAdapter, UpsState, UpsWorkitem};
use dicom_worklist::WorklistItem;

/// Worklist-to-UPS bridge event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowWorklistBridgeEvent {
    /// Worklist scheduled step id.
    pub scheduled_step_id: String,
    /// UPS instance UID created for this worklist item.
    pub ups_instance_uid: String,
}

/// MPPS-to-UPS bridge event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowMppsBridgeEvent {
    /// MPPS SOP Instance UID source event.
    pub mpps_sop_instance_uid: String,
    /// UPS instance UID affected by the update.
    pub ups_instance_uid: String,
    /// UPS state after applying the update.
    pub state: UpsState,
}

/// UPS workflow event emitted by workflow adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsWorkflowEvent {
    /// UPS instance UID.
    pub ups_instance_uid: String,
    /// UPS state after operation.
    pub state: UpsState,
    /// Source label for transition.
    pub source: &'static str,
}

/// Aggregate workflow snapshot for UPS orchestration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsWorkflowSnapshot {
    /// Total UPS workitems tracked.
    pub total_workitems: usize,
    /// Number of terminal UPS workitems.
    pub terminal_workitems: usize,
}

/// Workflow adapter for UPS lifecycle orchestration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpsWorkflowAdapter {
    ups: UpsCommandAdapter,
    events: Vec<UpsWorkflowEvent>,
}

impl UpsWorkflowAdapter {
    /// Create an empty adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a UPS workitem for a Worklist item.
    pub fn create_from_worklist(
        &mut self,
        item: &WorklistItem,
        ups_instance_uid: String,
    ) -> Result<WorkflowWorklistBridgeEvent> {
        let workitem = self
            .ups
            .create(ups_instance_uid.clone(), item.scheduled_step_id.clone())?;
        self.events.push(UpsWorkflowEvent {
            ups_instance_uid: workitem.ups_instance_uid.clone(),
            state: workitem.state,
            source: "worklist.create",
        });
        Ok(WorkflowWorklistBridgeEvent {
            scheduled_step_id: item.scheduled_step_id.clone(),
            ups_instance_uid,
        })
    }

    /// Apply MPPS status to a UPS workitem.
    pub fn apply_mpps_update(
        &mut self,
        ups_instance_uid: &str,
        update: &MppsUpdate,
    ) -> Result<WorkflowMppsBridgeEvent> {
        let workitem: UpsWorkitem = match update.status {
            MppsStatus::InProgress => self.ups.start(ups_instance_uid)?,
            MppsStatus::Completed => self.ups.complete(ups_instance_uid)?,
            MppsStatus::Discontinued => self.ups.cancel(ups_instance_uid)?,
        };
        self.events.push(UpsWorkflowEvent {
            ups_instance_uid: workitem.ups_instance_uid.clone(),
            state: workitem.state,
            source: "mpps.apply",
        });
        Ok(WorkflowMppsBridgeEvent {
            mpps_sop_instance_uid: update.sop_instance_uid.clone(),
            ups_instance_uid: workitem.ups_instance_uid,
            state: workitem.state,
        })
    }

    /// Return deterministic UPS workflow snapshot.
    pub fn snapshot(&self) -> UpsWorkflowSnapshot {
        let total = self.ups.store().workitems().len();
        let terminal = self
            .ups
            .store()
            .workitems()
            .into_iter()
            .filter(|item| {
                matches!(
                    item.state,
                    UpsState::Canceled | UpsState::Completed | UpsState::Failed
                )
            })
            .count();
        UpsWorkflowSnapshot {
            total_workitems: total,
            terminal_workitems: terminal,
        }
    }

    /// Borrow emitted workflow events.
    pub fn events(&self) -> &[UpsWorkflowEvent] {
        &self.events
    }

    /// Borrow underlying UPS adapter.
    pub fn ups(&self) -> &UpsCommandAdapter {
        &self.ups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
