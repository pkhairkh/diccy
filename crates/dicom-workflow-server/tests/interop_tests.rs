// Auto-extracted from /home/z/diccy/crates/dicom-workflow-server/src/domain/interop.rs
// S13-T8: Move inline tests to tests/ directories


use dicom_workflow_server::{classify_interop_route, InteropRoute};

#[test]
fn classify_interop_route_parses_reconciliation_run_paths() {
    let parsed = classify_interop_route("/interop/reconciliation/jobs/recon-001/run");
    assert_eq!(
        parsed,
        Some(InteropRoute::ReconciliationJobRun {
            job_id: "recon-001"
        })
    );
}

#[test]
fn classify_interop_route_rejects_unsupported_suffixes() {
    assert_eq!(
        classify_interop_route("/interop/reconciliation/jobs/recon-001/invalid"),
        None
    );
    assert_eq!(classify_interop_route("/interop/hl7/failures/extra"), None);
}
