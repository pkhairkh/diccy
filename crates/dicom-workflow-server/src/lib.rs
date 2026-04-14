#![deny(missing_docs)]

//! Runtime contracts for workflow startup policy, persistence preflight,
//! and deterministic restart-recovery diagnostics.

mod completion_workflow;
mod sr_workflow;
mod ups_workflow;

use dicom_mpps::MppsService;
use dicom_worklist::WorklistStore;
use std::fs::{self, OpenOptions};
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::path::{Path, PathBuf};

pub use completion_workflow::{
    CompletionEvent, CompletionEventSource, CompletionOutcome, CompletionWorkflowAdapter,
    Hl7WorkflowSignal,
};
pub use sr_workflow::{
    sr_endpoint_contract, sr_workflow_architecture, SrAuditRecord, SrAuthContext, SrCreateRequest,
    SrEndpointContract, SrLifecycleHistoryRecord, SrLifecycleStatus, SrLifecycleTransitionOutcome,
    SrLifecycleTransitionRequest, SrUpdateEnvelope, SrWorkflowArchitecture, SrWorkflowStore,
    SrWriteOutcome, SrWriteOutcomeKind,
};
pub use ups_workflow::{
    UpsWorkflowAdapter, UpsWorkflowEvent, UpsWorkflowSnapshot, WorkflowMppsBridgeEvent,
    WorkflowWorklistBridgeEvent,
};

/// Workflow route contract row for endpoint matrix generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowRouteContract {
    /// HTTP method.
    pub method: &'static str,
    /// Route path template.
    pub path_template: &'static str,
    /// Human operation label.
    pub operation: &'static str,
    /// Whether write operations require SR writer policy check.
    pub requires_writer_role: bool,
    /// Whether request idempotency-key header is required.
    pub requires_idempotency_key: bool,
    /// Whether body content type is required for this route.
    pub content_type: Option<&'static str>,
}

/// Deterministic workflow route contract matrix.
pub fn workflow_route_contract() -> &'static [WorkflowRouteContract] {
    &[
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/healthz",
            operation: "server health check",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/readyz",
            operation: "server readiness check",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/worklist/items",
            operation: "query worklist",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/worklist/items",
            operation: "upsert worklist item",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/mpps/updates",
            operation: "query mpps update snapshots",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/mpps/updates",
            operation: "ingest mpps update",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/mpps/updates/{sop_instance_uid}",
            operation: "get latest mpps update by sop uid",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/mpps/updates/{sop_instance_uid}/status",
            operation: "read mpps update status",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/mpps/updates/{sop_instance_uid}/status",
            operation: "update mpps status",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/sr/documents",
            operation: "list sr documents",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents",
            operation: "create sr document",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/sr/documents/{sop_instance_uid}",
            operation: "get sr document",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/updates",
            operation: "append sr update",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/review",
            operation: "review sr document",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/finalize",
            operation: "finalize sr document",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/commit",
            operation: "commit sr document",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/sr/documents/{sop_instance_uid}/cancel",
            operation: "cancel sr document",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/sr/documents/{sop_instance_uid}/history",
            operation: "read sr lifecycle history",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/tasks",
            operation: "list tasks",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks",
            operation: "create task",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/tasks/{task_id}",
            operation: "get task by id",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/start",
            operation: "start task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/pause",
            operation: "pause task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/resume",
            operation: "resume task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/complete",
            operation: "complete task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/review",
            operation: "review task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/commit",
            operation: "commit task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/tasks/{task_id}/cancel",
            operation: "cancel task",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/workitems",
            operation: "search ups workitems",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/workitems",
            operation: "create ups workitem",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/workitems/{task_id}",
            operation: "retrieve ups workitem",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/workitems/{task_id}",
            operation: "update ups workitem",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/workitems/{task_id}/state",
            operation: "read ups workitem state",
            requires_writer_role: false,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/workitems/{task_id}/state",
            operation: "change ups workitem state",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/workflow/workitems/{task_id}/cancel",
            operation: "cancel ups workitem",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/ian",
            operation: "list ian events",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/ian",
            operation: "ingest ian event",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/storage-commitment/status",
            operation: "list storage commitment status",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/storage-commitment/status/{task_id}",
            operation: "read storage commitment transaction status",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/storage-commitment/status",
            operation: "ingest storage commitment status",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/hl7/ups-correlation",
            operation: "list hl7 ups correlations",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/hl7/ups-correlation",
            operation: "upsert hl7 ups correlation",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/hl7",
            operation: "ingest interoperability event",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/hl7/failures",
            operation: "list hl7 failures",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/connectors/status",
            operation: "read hl7 connector status dashboard",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/connectors/features",
            operation: "read hl7 connector feature flags and rollout",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/connectors/rollout",
            operation: "list connector rollout percentages",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/connectors/rollout",
            operation: "update connector rollout percentage",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/connectors/capabilities",
            operation: "discover connector capabilities",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/connectors/health",
            operation: "read connector health checks",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/subscriptions",
            operation: "list hl7 subscriptions",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/interop/reconciliation/jobs",
            operation: "list study reconciliation jobs",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/reconciliation/jobs",
            operation: "create study reconciliation job",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/reconciliation/jobs/{job_id}/run",
            operation: "run study reconciliation job",
            requires_writer_role: true,
            requires_idempotency_key: true,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/subscriptions",
            operation: "create hl7 subscription",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "POST",
            path_template: "/interop/fhir",
            operation: "ingest fhir event",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: Some("application/x-www-form-urlencoded"),
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/policy/quotas",
            operation: "read tenant quota policy overrides",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/policy/quotas/snapshot",
            operation: "export tenant quota policy snapshot",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/metrics",
            operation: "read tenant-aware workflow metrics",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
        WorkflowRouteContract {
            method: "GET|HEAD",
            path_template: "/workflow/audit",
            operation: "read workflow audit trail",
            requires_writer_role: true,
            requires_idempotency_key: false,
            content_type: None,
        },
    ]
}

/// Workflow startup diagnostics for environment-derived auth/transport policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowPolicyDiagnostics {
    /// Effective auth mode label (`deny_all`, `allow_all`, or `token`).
    pub auth_mode_label: &'static str,
    /// Effective transport security label (`tls` or `insecure`).
    pub transport_security: &'static str,
    /// True when token mode is active with a non-empty configured token.
    pub token_mode_active: bool,
    /// True when auth mode was invalid and fell back.
    pub auth_defaulted: bool,
    /// True when transport mode was invalid and fell back.
    pub transport_defaulted: bool,
}

impl WorkflowPolicyDiagnostics {
    /// Return whether the resulting posture fails closed for insecure/unauthenticated paths.
    pub fn fail_closed(&self) -> bool {
        self.auth_mode_label == "deny_all" || self.transport_security == "insecure"
    }
}

/// Resolve workflow runtime policy diagnostics from optional raw environment values.
pub fn workflow_policy_diagnostics(
    auth_mode_raw: Option<&str>,
    auth_token_raw: Option<&str>,
    transport_raw: Option<&str>,
) -> WorkflowPolicyDiagnostics {
    let (auth_mode_label, auth_defaulted, token_mode_active) = match auth_mode_raw {
        Some("allow_all") => ("allow_all", false, false),
        Some("deny_all") | None => ("deny_all", false, false),
        Some("token") => {
            let token_is_set = auth_token_raw
                .map(str::trim)
                .is_some_and(|token| !token.is_empty());
            if token_is_set {
                ("token", false, true)
            } else {
                ("deny_all", true, false)
            }
        }
        Some(_) => ("deny_all", true, false),
    };

    let (transport_security, transport_defaulted) = match transport_raw {
        Some("tls") => ("tls", false),
        Some("insecure") | None => ("insecure", false),
        Some(_) => ("insecure", true),
    };

    WorkflowPolicyDiagnostics {
        auth_mode_label,
        transport_security,
        token_mode_active,
        auth_defaulted,
        transport_defaulted,
    }
}

/// Preflight diagnostics for workflow persistence artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistencePreflightDiagnostic {
    /// Human label for the artifact.
    pub label: String,
    /// Artifact file path.
    pub path: String,
    /// Whether parent directory creation was needed.
    pub parent_created: bool,
    /// Whether open/sync readiness checks succeeded.
    pub file_ready: bool,
    /// Whether rollover rotation occurred.
    pub rotated: bool,
    /// Configured retained rotation depth.
    pub max_rotated_files: usize,
    /// Whether rollover used destructive zero-retention mode.
    pub destructive_rollover: bool,
}

/// Ensure workflow persistence file readiness and return deterministic diagnostics.
pub fn prepare_persistence_file_with_diagnostics(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<PersistencePreflightDiagnostic> {
    let path = Path::new(path);
    let mut parent_created = false;
    if let Some(parent) = path.parent() {
        parent_created = !parent.exists();
        fs::create_dir_all(parent)?;
    }

    let mut rotated = false;
    if let Ok(meta) = fs::metadata(path) {
        if meta.is_dir() {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                format!(
                    "{label} path must reference a file, not a directory; choose a file target path"
                ),
            ));
        }
        if meta.len() > max_bytes {
            rotate_file(path, max_rotated_files)?;
            rotated = true;
        }
    }

    let file = OpenOptions::new().create(true).append(true).open(path)?;
    file.sync_all()?;

    Ok(PersistencePreflightDiagnostic {
        label: label.to_string(),
        path: path.display().to_string(),
        parent_created,
        file_ready: true,
        rotated,
        max_rotated_files,
        destructive_rollover: rotated && max_rotated_files == 0,
    })
}

/// Rotate a persistence artifact with bounded suffix depth.
pub fn rotate_file(path: &Path, max_rotated_files: usize) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if max_rotated_files == 0 {
        fs::remove_file(path)?;
        return Ok(());
    }

    let oldest = path_with_suffix(path, max_rotated_files);
    if oldest.exists() {
        fs::remove_file(&oldest)?;
    }

    for index in (1..=max_rotated_files).rev() {
        let src = if index == 1 {
            path.to_path_buf()
        } else {
            path_with_suffix(path, index - 1)
        };
        let dst = path_with_suffix(path, index);
        if src.exists() {
            fs::rename(src, dst)?;
        }
    }
    Ok(())
}

/// Build a deterministic rotation suffix path (`.1`, `.2`, ...).
pub fn path_with_suffix(path: &Path, suffix_index: usize) -> PathBuf {
    let mut os = path.as_os_str().to_os_string();
    os.push(format!(".{suffix_index}"));
    PathBuf::from(os)
}

/// Deterministic workflow restart-recovery summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRecoveryDiagnostic {
    /// Number of recovered worklist entries.
    pub worklist_items: usize,
    /// Number of recovered MPPS updates.
    pub mpps_updates: usize,
    /// Number of recovered SR documents.
    pub sr_documents: usize,
    /// True when persistence is path-backed for both workflow services.
    pub durable: bool,
}

/// Build deterministic recovery diagnostics for workflow services.
pub fn workflow_recovery_diagnostic(
    worklist: &WorklistStore,
    mpps: &MppsService,
) -> WorkflowRecoveryDiagnostic {
    WorkflowRecoveryDiagnostic {
        worklist_items: worklist.len(),
        mpps_updates: mpps.len(),
        sr_documents: 0,
        durable: true,
    }
}

/// Build deterministic recovery diagnostics including SR workflow persistence.
pub fn workflow_recovery_diagnostic_with_sr(
    worklist: &WorklistStore,
    mpps: &MppsService,
    sr: &SrWorkflowStore,
) -> WorkflowRecoveryDiagnostic {
    WorkflowRecoveryDiagnostic {
        worklist_items: worklist.len(),
        mpps_updates: mpps.len(),
        sr_documents: sr.documents().len(),
        durable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::workflow_route_contract;

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
}
