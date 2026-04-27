#![deny(missing_docs)]

//! Unified Procedure Step (UPS) deterministic state machine and command adapters.

use dicom_core::{validate_uid_strict, Error, ErrorKind, Result, Tag};
use std::collections::BTreeMap;

const TAG_UPS_UID: Tag = Tag(0x0008, 0x0018);

/// UPS lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsState {
    /// Workitem is scheduled and not yet started.
    Scheduled,
    /// Workitem is currently in progress.
    InProgress,
    /// Workitem was canceled.
    Canceled,
    /// Workitem completed successfully.
    Completed,
    /// Workitem failed.
    Failed,
}

/// UPS transition command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpsTransition {
    /// Start a scheduled item.
    Start,
    /// Cancel an item.
    Cancel,
    /// Complete an in-progress item.
    Complete,
    /// Mark an in-progress item failed.
    Fail,
}

/// UPS workitem model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsWorkitem {
    /// UPS instance UID.
    pub ups_instance_uid: String,
    /// Procedure step label.
    pub procedure_step_label: String,
    /// Current state.
    pub state: UpsState,
    /// Monotonic revision.
    pub revision: u64,
}

/// UPS event log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsEvent {
    /// UPS instance UID.
    pub ups_instance_uid: String,
    /// Previous state.
    pub from_state: UpsState,
    /// New state.
    pub to_state: UpsState,
    /// Revision after transition.
    pub revision: u64,
}

/// In-memory deterministic UPS store.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpsStore {
    items: BTreeMap<String, UpsWorkitem>,
    events: Vec<UpsEvent>,
}

impl UpsStore {
    /// Create an empty UPS store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a UPS workitem in scheduled state.
    pub fn create_workitem(
        &mut self,
        ups_instance_uid: String,
        procedure_step_label: String,
    ) -> Result<UpsWorkitem> {
        validate_uid_strict(TAG_UPS_UID, &ups_instance_uid)?;
        if self.items.contains_key(&ups_instance_uid) {
            return Err(integrity_error("duplicate UPS instance UID"));
        }
        let workitem = UpsWorkitem {
            ups_instance_uid: ups_instance_uid.clone(),
            procedure_step_label,
            state: UpsState::Scheduled,
            revision: 1,
        };
        self.items.insert(ups_instance_uid, workitem.clone());
        Ok(workitem)
    }

    /// Read a UPS workitem.
    pub fn get_workitem(&self, ups_instance_uid: &str) -> Option<&UpsWorkitem> {
        self.items.get(ups_instance_uid)
    }

    /// Update mutable UPS metadata without changing state.
    pub fn update_label(
        &mut self,
        ups_instance_uid: &str,
        procedure_step_label: String,
    ) -> Result<UpsWorkitem> {
        let item = self
            .items
            .get_mut(ups_instance_uid)
            .ok_or_else(|| decode_error("missing UPS workitem"))?;
        item.procedure_step_label = procedure_step_label;
        item.revision = item.revision.saturating_add(1);
        Ok(item.clone())
    }

    /// Apply a UPS transition.
    pub fn transition(
        &mut self,
        ups_instance_uid: &str,
        transition: UpsTransition,
    ) -> Result<UpsWorkitem> {
        let item = self
            .items
            .get_mut(ups_instance_uid)
            .ok_or_else(|| decode_error("missing UPS workitem"))?;
        let previous = item.state;
        let next = match (item.state, transition) {
            (UpsState::Scheduled, UpsTransition::Start) => UpsState::InProgress,
            (UpsState::Scheduled, UpsTransition::Cancel) => UpsState::Canceled,
            (UpsState::InProgress, UpsTransition::Complete) => UpsState::Completed,
            (UpsState::InProgress, UpsTransition::Fail) => UpsState::Failed,
            (UpsState::InProgress, UpsTransition::Cancel) => UpsState::Canceled,
            _ => {
                return Err(decode_error("invalid UPS state transition"));
            }
        };
        item.state = next;
        item.revision = item.revision.saturating_add(1);
        let event = UpsEvent {
            ups_instance_uid: item.ups_instance_uid.clone(),
            from_state: previous,
            to_state: item.state,
            revision: item.revision,
        };
        self.events.push(event);
        Ok(item.clone())
    }

    /// Return UPS event history in deterministic insertion order.
    pub fn events(&self) -> &[UpsEvent] {
        &self.events
    }

    /// Return all workitems in deterministic key order.
    pub fn workitems(&self) -> Vec<&UpsWorkitem> {
        self.items.values().collect()
    }

    /// Replay a deterministic UPS store from event history and snapshot.
    pub fn replay(snapshot: Vec<UpsWorkitem>, events: &[UpsEvent]) -> Result<Self> {
        let mut store = Self::new();
        for item in snapshot {
            validate_uid_strict(TAG_UPS_UID, &item.ups_instance_uid)?;
            store.items.insert(item.ups_instance_uid.clone(), item);
        }
        for event in events {
            let item = store
                .items
                .get_mut(&event.ups_instance_uid)
                .ok_or_else(|| decode_error("event references unknown UPS workitem"))?;
            if item.state != event.from_state {
                return Err(integrity_error("UPS replay state mismatch"));
            }
            item.state = event.to_state;
            item.revision = event.revision;
            store.events.push(event.clone());
        }
        Ok(store)
    }
}

/// UPS command adapter exposing Create/Get/Update/Cancel entrypoints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpsCommandAdapter {
    store: UpsStore,
}

impl UpsCommandAdapter {
    /// Create an empty adapter.
    pub fn new() -> Self {
        Self {
            store: UpsStore::new(),
        }
    }

    /// Create command.
    pub fn create(
        &mut self,
        ups_instance_uid: String,
        procedure_step_label: String,
    ) -> Result<UpsWorkitem> {
        self.store
            .create_workitem(ups_instance_uid, procedure_step_label)
    }

    /// Get command.
    pub fn get(&self, ups_instance_uid: &str) -> Result<UpsWorkitem> {
        self.store
            .get_workitem(ups_instance_uid)
            .cloned()
            .ok_or_else(|| decode_error("missing UPS workitem"))
    }

    /// Update command.
    pub fn update(
        &mut self,
        ups_instance_uid: &str,
        procedure_step_label: String,
    ) -> Result<UpsWorkitem> {
        self.store
            .update_label(ups_instance_uid, procedure_step_label)
    }

    /// Cancel command.
    pub fn cancel(&mut self, ups_instance_uid: &str) -> Result<UpsWorkitem> {
        self.store
            .transition(ups_instance_uid, UpsTransition::Cancel)
    }

    /// Start command.
    pub fn start(&mut self, ups_instance_uid: &str) -> Result<UpsWorkitem> {
        self.store
            .transition(ups_instance_uid, UpsTransition::Start)
    }

    /// Complete command.
    pub fn complete(&mut self, ups_instance_uid: &str) -> Result<UpsWorkitem> {
        self.store
            .transition(ups_instance_uid, UpsTransition::Complete)
    }

    /// Fail command.
    pub fn fail(&mut self, ups_instance_uid: &str) -> Result<UpsWorkitem> {
        self.store.transition(ups_instance_uid, UpsTransition::Fail)
    }

    /// Borrow underlying store.
    pub fn store(&self) -> &UpsStore {
        &self.store
    }
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom_ups".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn integrity_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IntegrityError {
            detail: detail.into(),
        },
        "integrity error",
    )
    .into()
}
