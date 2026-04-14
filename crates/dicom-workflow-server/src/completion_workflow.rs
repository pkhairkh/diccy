//! IAN + Storage Commitment + HL7 completion workflow adapters.

use std::collections::{BTreeMap, BTreeSet};

/// Completion event source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionEventSource {
    /// Instance Availability Notification ingestion.
    Ian,
    /// Storage Commitment lifecycle event.
    StorageCommitment,
    /// HL7 ingress signal mapped to workflow completion state.
    Hl7,
}

/// Completion outcome classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// Completion succeeded.
    Success,
    /// Completion failed.
    Failure,
}

/// HL7 signal mapped into completion workflow events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hl7WorkflowSignal {
    /// Message class (ADT/ORM/ORU/SIU or mapped alias).
    pub message_class: String,
    /// Correlation ID.
    pub correlation_id: String,
    /// Outcome inferred from signal.
    pub outcome: CompletionOutcome,
}

/// Completion workflow event record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionEvent {
    /// Stable event id for dedupe.
    pub event_id: String,
    /// Source class.
    pub source: CompletionEventSource,
    /// Correlation UID (UPS, SOP, or transaction UID).
    pub correlation_uid: String,
    /// Outcome.
    pub outcome: CompletionOutcome,
}

/// Deterministic completion workflow adapter with deduplicated ingest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompletionWorkflowAdapter {
    seen_event_ids: BTreeSet<String>,
    events: Vec<CompletionEvent>,
    by_correlation_uid: BTreeMap<String, CompletionOutcome>,
}

impl CompletionWorkflowAdapter {
    /// Create empty adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest IAN event with dedupe.
    pub fn ingest_ian(
        &mut self,
        event_id: String,
        sop_instance_uid: String,
        outcome: CompletionOutcome,
    ) -> bool {
        self.ingest_event(CompletionEvent {
            event_id,
            source: CompletionEventSource::Ian,
            correlation_uid: sop_instance_uid,
            outcome,
        })
    }

    /// Ingest Storage Commitment event with dedupe.
    pub fn ingest_storage_commitment(
        &mut self,
        event_id: String,
        transaction_uid: String,
        outcome: CompletionOutcome,
    ) -> bool {
        self.ingest_event(CompletionEvent {
            event_id,
            source: CompletionEventSource::StorageCommitment,
            correlation_uid: transaction_uid,
            outcome,
        })
    }

    /// Ingest HL7 signal with deterministic mapping.
    pub fn ingest_hl7_signal(&mut self, signal: Hl7WorkflowSignal) -> bool {
        self.ingest_event(CompletionEvent {
            event_id: format!("hl7:{}", signal.correlation_id),
            source: CompletionEventSource::Hl7,
            correlation_uid: signal.correlation_id,
            outcome: signal.outcome,
        })
    }

    fn ingest_event(&mut self, event: CompletionEvent) -> bool {
        if !self.seen_event_ids.insert(event.event_id.clone()) {
            return false;
        }
        self.by_correlation_uid
            .insert(event.correlation_uid.clone(), event.outcome);
        self.events.push(event);
        true
    }

    /// Return deduplicated events in deterministic insertion order.
    pub fn events(&self) -> &[CompletionEvent] {
        &self.events
    }

    /// Return latest outcome for a correlation UID.
    pub fn outcome_for(&self, correlation_uid: &str) -> Option<CompletionOutcome> {
        self.by_correlation_uid.get(correlation_uid).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ian_ingest_is_deduplicated() {
        let mut adapter = CompletionWorkflowAdapter::new();
        assert!(adapter.ingest_ian(
            "ian:1".to_string(),
            "1.2.840.10008.5.1".to_string(),
            CompletionOutcome::Success
        ));
        assert!(!adapter.ingest_ian(
            "ian:1".to_string(),
            "1.2.840.10008.5.1".to_string(),
            CompletionOutcome::Success
        ));
        assert_eq!(adapter.events().len(), 1);
    }

    #[test]
    fn storage_commitment_and_hl7_update_completion_index() {
        let mut adapter = CompletionWorkflowAdapter::new();
        assert!(adapter.ingest_storage_commitment(
            "stgc:1".to_string(),
            "1.2.840.10008.9.1".to_string(),
            CompletionOutcome::Success
        ));
        assert!(adapter.ingest_hl7_signal(Hl7WorkflowSignal {
            message_class: "ORU".to_string(),
            correlation_id: "1.2.840.10008.9.1".to_string(),
            outcome: CompletionOutcome::Failure,
        }));
        assert_eq!(
            adapter.outcome_for("1.2.840.10008.9.1"),
            Some(CompletionOutcome::Failure)
        );
    }
}
