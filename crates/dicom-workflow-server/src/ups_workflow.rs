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
