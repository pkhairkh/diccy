#![deny(missing_docs)]

//! Packaged workflow runtime for durable MWL and MPPS services.

use dicom_core::{Dataset, Element, Error, ErrorKind, Limits, Tag, Value, Vr};
use dicom_env_contract::{
    dicom_workflow_env_contract, parse_bool, parse_optional_string, parse_string_non_empty,
    parse_u64, parse_usize, validate_envelope_version, NumericBounds,
    DEFAULT_DICOM_ENVELOPE_VERSION, DICOM_ENVELOPE_VERSION, DICOM_WORKFLOW_ENV_PREFIX,
    SUPPORTED_DICOM_ENVELOPE_VERSIONS,
};
use dicom_mpps::{IngestOutcome as MppsIngestOutcome, MppsService, MppsServiceConfig, MppsStatus};
use dicom_ups::{UpsCommandAdapter, UpsState, UpsTransition};
use dicom_workflow_server::{
    prepare_persistence_file_with_diagnostics, rotate_file, workflow_policy_diagnostics,
    workflow_recovery_diagnostic_with_sr, CompletionOutcome, CompletionWorkflowAdapter,
    SrAuthContext, SrCreateRequest, SrLifecycleHistoryRecord, SrLifecycleStatus,
    SrLifecycleTransitionOutcome, SrLifecycleTransitionRequest, SrUpdateEnvelope, SrWorkflowStore,
    SrWriteOutcomeKind,
};
use dicom_worklist::{validate_worklist_item, WorklistQuery, WorklistStore};
use pack_sr::{Code, SrAuthoringContentItem};
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "domain/mod.rs"]
mod domain;
#[path = "adapters/observability.rs"]
mod observability;
#[path = "application/runtime_config.rs"]
mod runtime_config;

use domain::{interop, mpps, sr, task, tenant_policy as tenant_policy_domain};
use observability::{WorkflowLogLevel, WorkflowObservability};
use runtime_config::WorkflowRuntimeConfig;

mod config;
mod handlers;
mod hl7;
mod http;
mod routing;
mod sr_handlers;

use config::*;
use handlers::*;
use hl7::*;
use http::*;
use routing::*;
use sr_handlers::*;

fn main() -> std::io::Result<()> {
    let contract = dicom_workflow_env_contract(cfg!(test));

    let envelope_version = validate_envelope_version(
        WORKFLOW_SERVICE_NAME,
        DICOM_ENVELOPE_VERSION,
        SUPPORTED_DICOM_ENVELOPE_VERSIONS,
        DEFAULT_DICOM_ENVELOPE_VERSION,
    )?;

    if has_flag("--print-env-contract") {
        println!("{}", contract.snapshot_json(&envelope_version));
        return Ok(());
    }

    contract.validate()?;

    let config = WorkflowRuntimeConfig::from_env()?;
    let observability =
        WorkflowObservability::from_env(DICOM_WORKFLOW_ENV_PREFIX, "dicom-workflow-server")?;
    let bind = config.bind;
    let listener = TcpListener::bind(&bind)?;
    let limits = Arc::new(Limits::default());

    let worklist_state_path = config.worklist_state_path;
    let mpps_state_path = config.mpps_state_path;
    let sr_state_path = config.sr_state_path;
    let sr_audit_path = config.sr_audit_path;
    let workflow_audit_path = config.workflow_audit_path;
    let snapshot_max_bytes = config.snapshot_max_bytes;
    let snapshot_max_rotated = config.snapshot_max_rotated_files;
    let audit_max_bytes = config.audit_max_bytes;
    let audit_max_rotated = config.audit_max_rotated_files;
    let worklist_preflight = prepare_persistence_file_with_diagnostics(
        &worklist_state_path,
        snapshot_max_bytes,
        snapshot_max_rotated,
        "worklist snapshot",
    )?;
    let mpps_preflight = prepare_persistence_file_with_diagnostics(
        &mpps_state_path,
        snapshot_max_bytes,
        snapshot_max_rotated,
        "mpps snapshot",
    )?;
    let sr_preflight = prepare_persistence_file_with_diagnostics(
        &sr_state_path,
        snapshot_max_bytes,
        snapshot_max_rotated,
        "sr snapshot",
    )?;
    let sr_audit_preflight = prepare_persistence_file_with_diagnostics(
        &sr_audit_path,
        snapshot_max_bytes,
        snapshot_max_rotated,
        "sr audit",
    )?;
    let workflow_audit_preflight = prepare_persistence_file_with_diagnostics(
        &workflow_audit_path,
        audit_max_bytes,
        audit_max_rotated,
        "workflow audit",
    )?;
    let callback_idempotency_path = callback_idempotency_snapshot_path(&workflow_audit_path);
    let callback_idempotency_cache = load_hl7_callback_idempotency_cache(
        &callback_idempotency_path,
        now_epoch_millis(),
        DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS,
    );
    let hl7_callback_policy = parse_hl7_callback_policy_from_env()?;
    let tenant_rate_limit_overrides_path =
        tenant_rate_limit_override_snapshot_path(&workflow_audit_path);
    let connector_rollout_path = connector_rollout_snapshot_path(&workflow_audit_path);
    let hl7_failure_queue_path = hl7_failure_queue_snapshot_path(&workflow_audit_path);
    let reconciliation_idempotency_path =
        reconciliation_run_idempotency_snapshot_path(&workflow_audit_path);

    let auth_mode_raw = config.auth_mode_raw;
    let auth_token_for_policy = config.auth_token.clone();
    let tls_cert_path = config.tls_cert_path;
    let tls_key_path = config.tls_key_path;
    let transport_raw = config.transport_security_raw;
    let policy_diagnostics = workflow_policy_diagnostics(
        auth_mode_raw.as_deref(),
        auth_token_for_policy.as_deref(),
        transport_raw.as_deref(),
    );
    let query_rate_limit = config.query_rate_limit;
    let mutation_rate_limit = config.mutation_rate_limit;
    let upload_cap_bytes = config.upload_cap_bytes;
    let mut tenant_rate_limit_overrides =
        load_tenant_rate_limit_overrides(&tenant_rate_limit_overrides_path);
    let defaults = TenantRateLimitOverride {
        query_rate_limit,
        mutation_rate_limit,
        upload_cap_bytes,
    };
    tenant_rate_limit_overrides.extend(collect_tenant_rate_limit_overrides_from_env(defaults));
    persist_tenant_rate_limit_overrides(
        &tenant_rate_limit_overrides_path,
        &tenant_rate_limit_overrides,
    );
    set_tenant_rate_limit_override_cache(tenant_rate_limit_overrides);
    let anomaly_alert_threshold = config.anomaly_alert_threshold;
    let audit_export_limit = config.audit_export_limit;
    let audit_rate_window_ms = config.audit_rate_window_ms;
    let denylist_routes = config.denylist_routes;
    let auth_mode = config.auth_mode;
    let transport_security = config.transport_security;
    if transport_security == "tls" {
        validate_tls_secret_path(tls_cert_path.as_deref(), tls_key_path.as_deref())?;
    }

    let hl7_connector_registry = config.hl7_connector_registry;
    let hl7_connector_feature_flags = config.hl7_connector_feature_flags;
    let hl7_connector_rollout_percents = load_hl7_connector_rollout_state(
        &connector_rollout_path,
        &config.hl7_connector_rollout_percents,
    );
    let hl7_connector_plugins = config.hl7_connector_plugins;
    let hl7_transport = config.hl7_transport;
    let hl7_failures = load_hl7_failure_queue(&hl7_failure_queue_path, MAX_HL7_FAILURES);
    let hl7_failure_seq = hl7_failure_sequence_seed(&hl7_failures);
    let hl7_event_seq = hl7_event_sequence_seed(&hl7_failures);
    let reconciliation_run_idempotency =
        load_reconciliation_run_idempotency_cache(&reconciliation_idempotency_path);

    let state = RuntimeState {
        worklist: WorklistStore::open((*limits).clone(), &worklist_state_path)
            .map_err(|err| IoError::other(format!("failed to open worklist snapshot: {err}")))?,
        mpps: MppsService::with_persistence(
            MppsServiceConfig {
                limits: (*limits).clone(),
                audit: None,
            },
            &mpps_state_path,
        )
        .map_err(|err| IoError::other(format!("failed to open MPPS snapshot: {err}")))?,
        sr: SrWorkflowStore::open((*limits).clone(), &sr_state_path, &sr_audit_path)
            .map_err(|err| IoError::other(format!("failed to open SR snapshot: {err}")))?,
        mpps_idempotency: BTreeMap::new(),
        tasks: BTreeMap::new(),
        task_id_sequence: 1,
        task_idempotency: reconciliation_run_idempotency,
        audit_path: workflow_audit_path,
        audit_rate_window_ms,
        query_rate_limit,
        mutation_rate_limit,
        upload_cap_bytes,
        rate_windows: BTreeMap::new(),
        anomaly_alert_threshold,
        audit_max_bytes,
        audit_max_rotated_files: audit_max_rotated,
        audit_export_limit,
        denylist_routes,
        tenant_worklist: BTreeMap::new(),
        tenant_mpps: BTreeMap::new(),
        tenant_sr: BTreeMap::new(),
        tenant_tasks: BTreeMap::new(),
        metrics: BTreeMap::new(),
        hl7: Hl7RuntimeState {
            subscriptions: BTreeMap::new(),
            failures: hl7_failures,
            ups: UpsCommandAdapter::new(),
            completion: CompletionWorkflowAdapter::new(),
            ian_events: BTreeMap::new(),
            storage_commitment_status: BTreeMap::new(),
            hl7_ups_correlation: BTreeMap::new(),
            subscription_seq: 0,
            failure_seq: hl7_failure_seq,
            event_seq: hl7_event_seq,
            replay_cache: BTreeMap::new(),
            connector_registry: hl7_connector_registry,
            connector_feature_flags: hl7_connector_feature_flags,
            connector_rollout_percent: hl7_connector_rollout_percents,
            connector_plugins: hl7_connector_plugins,
            callback_delivery_idempotency: callback_idempotency_cache,
            callback_idempotency_ttl_ms: DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS,
            callback_idempotency_path,
            callback_max_attempts: hl7_callback_policy.max_attempts,
            callback_circuit_breaker_failure_threshold: hl7_callback_policy
                .circuit_failure_threshold,
            callback_circuit_breaker_base_backoff_ms: hl7_callback_policy.circuit_base_backoff_ms,
            callback_circuit_breaker_max_backoff_ms: hl7_callback_policy.circuit_max_backoff_ms,
            connector_callback_failure_streak: BTreeMap::new(),
            connector_circuit_open_until_ms: BTreeMap::new(),
            reconciliation_jobs: BTreeMap::new(),
            reconciliation_seq: 0,
        },
    };
    let recovery = workflow_recovery_diagnostic_with_sr(&state.worklist, &state.mpps, &state.sr);

    let shared_state = Arc::new(Mutex::new(state));
    if hl7_transport.mllp_enabled {
        if let Some(bind) = hl7_transport.mllp_bind.as_deref() {
            let mllp_state = Arc::clone(&shared_state);
            let mllp_limits = Arc::clone(&limits);
            let mllp_bind = bind.to_string();
            thread::spawn(move || {
                if let Err(err) = run_hl7_mllp_listener(&mllp_bind, mllp_state, mllp_limits) {
                    eprintln!("dicom-workflow-server MLLP listener stopped: {err}");
                }
            });
        }
    }

    if let (Some(file_drop_dir), Some(file_drop_done_dir), Some(file_drop_error_dir)) = (
        hl7_transport.file_drop_dir,
        hl7_transport.file_drop_done_dir,
        hl7_transport.file_drop_error_dir,
    ) {
        let file_drop_state = Arc::clone(&shared_state);
        let file_drop_limits = Arc::clone(&limits);
        let poll_interval_ms = hl7_transport.file_drop_poll_interval_ms;
        thread::spawn(move || {
            run_hl7_file_drop_worker(
                file_drop_dir,
                file_drop_done_dir,
                file_drop_error_dir,
                poll_interval_ms,
                file_drop_state,
                file_drop_limits,
            );
        });
    }
    observability.log(
        WorkflowLogLevel::Info,
        &format!("dicom-workflow-server listening on {bind}"),
    );
    observability.log(
        WorkflowLogLevel::Info,
        &format!(
            "dicom-workflow-server policy: auth={}, transport={transport_security}, auth_defaulted={}, transport_defaulted={}, token_mode_active={}, fail_closed={}, worklist={worklist_state_path}, mpps={mpps_state_path}, snapshot_max_bytes={snapshot_max_bytes}, snapshot_max_rotated={snapshot_max_rotated}, worklist_parent_created={}, worklist_rotated={}, worklist_destructive_rollover={}, mpps_parent_created={}, mpps_rotated={}, mpps_destructive_rollover={}, recovered_worklist={}, recovered_mpps={}, recovered_sr={}",
            auth_mode_label(&auth_mode),
            policy_diagnostics.auth_defaulted,
            policy_diagnostics.transport_defaulted,
            policy_diagnostics.token_mode_active,
            policy_diagnostics.fail_closed(),
            worklist_preflight.parent_created,
            worklist_preflight.rotated,
            worklist_preflight.destructive_rollover,
            mpps_preflight.parent_created,
            mpps_preflight.rotated,
            mpps_preflight.destructive_rollover,
            recovery.worklist_items,
            recovery.mpps_updates,
            recovery.sr_documents,
        ),
    );
    observability.log(
        WorkflowLogLevel::Info,
        &format!(
            "dicom-workflow-server sr: state={sr_state_path}, audit={sr_audit_path}, sr_parent_created={}, sr_rotated={}, sr_destructive_rollover={}, sr_audit_parent_created={}, sr_audit_rotated={}, sr_audit_destructive_rollover={}, workflow_audit_parent_created={}, workflow_audit_rotated={}, workflow_audit_destructive_rollover={}",
            sr_preflight.parent_created,
            sr_preflight.rotated,
            sr_preflight.destructive_rollover,
            sr_audit_preflight.parent_created,
            sr_audit_preflight.rotated,
            sr_audit_preflight.destructive_rollover,
            workflow_audit_preflight.parent_created,
            workflow_audit_preflight.rotated,
            workflow_audit_preflight.destructive_rollover,
        ),
    );
    observability.emit_telemetry(
        "service_start",
        &[
            ("bind", &bind),
            ("profile", "workflow"),
            ("transport_security", transport_security),
            ("auth_mode", auth_mode_label(&auth_mode)),
        ],
    );

    for incoming in listener.incoming() {
        match incoming {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                let state = Arc::clone(&shared_state);
                let auth_mode = auth_mode.clone();
                let limits = Arc::clone(&limits);
                let auth_token_for_connection = auth_token_for_policy.clone();
                thread::spawn(move || {
                    let _ = handle_connection(
                        &mut stream,
                        &state,
                        &limits,
                        &auth_mode,
                        transport_security,
                        auth_token_for_connection.as_deref(),
                    );
                });
            }
            Err(err) => {
                observability.log(WorkflowLogLevel::Warn, &format!("accept error: {err}"));
                observability
                    .emit_telemetry("listener_accept_error", &[("error", &err.to_string())]);
            }
        }
    }
    Ok(())
}

fn has_flag(name: &str) -> bool {
    env::args().skip(1).any(|arg| arg == name)
}

#[cfg(all(test, feature = "workflow-main-tests"))]
mod tests;
