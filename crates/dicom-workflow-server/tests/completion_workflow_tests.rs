// Auto-extracted from /home/z/diccy/crates/dicom-workflow-server/src/completion_workflow.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_workflow_server::*;

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
