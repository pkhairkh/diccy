// Auto-extracted from /home/z/diccy/crates/dicom-workflow-server/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_workflow_server::workflow_route_contract;

    #[test]
    fn workflow_route_contract_is_deterministic_and_complete() {
        let first = workflow_route_contract();
        let second = workflow_route_contract();
        assert_eq!(first, second);
        assert_eq!(first.len(), 60);
        assert!(first
            .iter()
            .any(|row| row.path_template == "/healthz" && row.method == "GET|HEAD"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/readyz" && row.method == "GET|HEAD"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/sr/documents/{sop_instance_uid}/updates"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/sr/documents/{sop_instance_uid}/review"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/sr/documents/{sop_instance_uid}/history"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/tasks/{task_id}/commit"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/metrics" && row.requires_writer_role));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/audit" && row.requires_writer_role));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/subscriptions" && row.method == "POST"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/hl7/failures"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/connectors/status"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/connectors/features"));
        assert!(first.iter().any(|row| {
            row.path_template == "/interop/connectors/rollout" && row.method == "GET|HEAD"
        }));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/connectors/rollout" && row.method == "POST"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/connectors/capabilities"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/connectors/health"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/fhir" && row.method == "POST"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/policy/quotas"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/policy/quotas/snapshot"));
        assert!(first.iter().any(|row| {
            row.path_template == "/interop/reconciliation/jobs/{job_id}/run"
                && row.requires_idempotency_key
        }));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/workflow/workitems" && row.method == "GET|HEAD"));
        assert!(first.iter().any(
            |row| row.path_template == "/workflow/workitems/{task_id}/state"
                && row.method == "POST"
        ));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/ian" && row.method == "POST"));
        assert!(first
            .iter()
            .any(|row| row.path_template == "/interop/storage-commitment/status/{task_id}"));
        assert!(
            first
                .iter()
                .any(|row| row.path_template == "/interop/hl7/ups-correlation"
                    && row.method == "POST")
        );
    }
