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

const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
const TAG_SCHEDULED_STATION_AE_TITLE: Tag = Tag(0x0040, 0x0001);
const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
const TAG_REQUESTED_PROCEDURE_ID: Tag = Tag(0x0040, 0x1001);
const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);

const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);
const TAG_END_DATE: Tag = Tag(0x0040, 0x0250);
const TAG_END_TIME: Tag = Tag(0x0040, 0x0251);
const DEFAULT_WORKFLOW_PAGE_SIZE: usize = 50;
const MAX_WORKFLOW_PAGE_SIZE: usize = 500;
const DEFAULT_UPLOAD_CAP_BYTES: u64 = 8 * 1024 * 1024;
const DEFAULT_QUERY_RATE_LIMIT: u64 = 300;
const DEFAULT_MUTATION_RATE_LIMIT: u64 = 120;
const DEFAULT_AUDIT_MAX_BYTES: u64 = 8 * 1024 * 1024;
const DEFAULT_AUDIT_MAX_ROTATED_FILES: usize = 4;
const DEFAULT_AUDIT_EXPORT_LIMIT: usize = 256;
const DEFAULT_AUDIT_PATH: &str = "./state/workflow/workflow.audit.log";
const DEFAULT_RATE_LIMIT_WINDOW_MS: u64 = 15_000;
const DEFAULT_ANOMALY_ALERT_THRESHOLD: u64 = 6;
const DEFAULT_WORKFLOW_MAX_PAIR_COUNT: usize = 256;
const DEFAULT_WORKFLOW_DENYLIST_PATHS: &[&str] = &[
    "/workflow/tasks/{task_id}/cancel",
    "/workflow/tasks/{task_id}/commit",
    "/mpps/updates/{sop_instance_uid}/status",
];
const WORKITEM_COLLECTION_PATH: &str = "/workflow/workitems";
const WORKITEM_ITEM_PREFIX: &str = "/workflow/workitems/";
const INTEROP_IAN_PATH: &str = "/interop/ian";
const INTEROP_STORAGE_COMMITMENT_STATUS_PATH: &str = "/interop/storage-commitment/status";
const INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX: &str = "/interop/storage-commitment/status/";
const INTEROP_HL7_UPS_CORRELATION_PATH: &str = "/interop/hl7/ups-correlation";
const MAX_MPPS_IDEMPOTENCY_ENTRIES: usize = 128;
const MAX_TASK_IDEMPOTENCY_ENTRIES: usize = 128;
const MAX_HL7_FAILURES: usize = 256;
const MAX_HL7_FAILURE_EXCERPT_BYTES: usize = 180;
const MAX_HL7_SUBSCRIPTIONS: usize = 64;
const MAX_HL7_IDEMPOTENCY_ENTRIES: usize = 128;
const MAX_HL7_CONNECTOR_REGISTRY: usize = 64;
const DEFAULT_TENANT_TASK_QUOTA: usize = 512;
const DEFAULT_TENANT_SUBSCRIPTION_QUOTA: usize = 64;
const DEFAULT_HL7_CONNECTOR_ROLLOUT_PERCENT: u64 = 100;
const HL7_CONNECTOR_FEATURE_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_";
const HL7_CONNECTOR_ROLLOUT_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_";
const HL7_CONNECTOR_PLUGIN_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_";
const HL7_CONNECTOR_VERSION_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_";
const HL7_CONNECTOR_COMPAT_MIN_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_";
const HL7_CONNECTOR_COMPAT_MAX_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_";
const DEFAULT_HL7_FILE_DROP_POLL_INTERVAL_MS: u64 = 3000;
const HL7_MLLP_START_BYTE: u8 = 0x0b;
const HL7_MLLP_END_BYTES: [u8; 2] = [0x1c, 0x0d];
const MAX_WORKFLOW_CALLBACK_ATTEMPTS: u32 = 3;
const CALLBACK_CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 2;
const CALLBACK_CIRCUIT_BREAKER_BASE_BACKOFF_MS: u64 = 15_000;
const CALLBACK_CIRCUIT_BREAKER_MAX_BACKOFF_MS: u64 = 300_000;
const DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS: u64 = 30 * 60 * 1000;
const HL7_CALLBACK_MAX_ATTEMPTS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS";
const HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV: &str =
    "DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD";
const HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS";
const HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS";
const MAX_RECONCILIATION_JOBS: usize = 128;
const MAX_RECONCILIATION_JOBS_PER_TENANT: usize = 32;
const RECONCILIATION_INTERVAL_FLOOR_SECONDS: u64 = 60;
const RECONCILIATION_INTERVAL_CEILING_SECONDS: u64 = 86_400;
const RECONCILIATION_RUN_IDEMPOTENCY_PREFIX: &str = "reconciliation-run:";
const WORKFLOW_ADMIN_API_VERSION: &str = "v1";
const WEBHOOK_AUTH_STRATEGY_ENV: &str = "DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY";
const WEBHOOK_AUTH_SECRET_ENV: &str = "DICOM_WORKFLOW_WEBHOOK_AUTH_SHARED_SECRET";
const WORKFLOW_SERVICE_NAME: &str = "dicom-workflow-server";
const WORKFLOW_SECRET_AUTH_TOKEN_FILE_HINTS: &[&str] = &["auth_token", "workflow_auth_token"];
const WORKFLOW_SECRET_TLS_CERT_FILE_HINTS: &[&str] =
    &["tls_cert", "tls_cert.pem", "workflow_tls_cert"];
const WORKFLOW_SECRET_TLS_KEY_FILE_HINTS: &[&str] = &["tls_key", "tls_key.pem", "workflow_tls_key"];
const HL7_CONNECTOR_ENV_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_";
const TENANT_ID_DEFAULT: &str = "tenant-default";
static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Policy and identity context derived from request headers.
#[derive(Debug, Clone)]
struct WorkflowActorContext {
    principal: Option<String>,
    role: Option<String>,
    tenant: String,
}

impl WorkflowActorContext {
    fn can_write(&self) -> bool {
        self.role.as_deref().is_some_and(workflow_role_is_writer)
    }
}

/// Fixed-window request throttle for a policy scope.
#[derive(Debug, Clone, Copy)]
struct RequestWindow {
    window_start_ms: u64,
    count: u64,
}

impl RequestWindow {
    fn empty() -> Self {
        Self {
            window_start_ms: 0,
            count: 0,
        }
    }
}

/// Tenant policy object for baseline rate limits and quotas.
#[derive(Debug, Clone, Copy)]
struct TenantWorkflowPolicy {
    query_rate_limit: u64,
    mutation_rate_limit: u64,
    upload_cap_bytes: u64,
    task_quota: usize,
    subscription_quota: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TenantQuotaOverride {
    task_quota: usize,
    subscription_quota: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TenantRateLimitOverride {
    query_rate_limit: u64,
    mutation_rate_limit: u64,
    upload_cap_bytes: u64,
}

/// Lightweight mutable per-tenant/route anomaly and operations metrics.
#[derive(Debug, Clone, Copy, Default)]
struct TenantOperationMetrics {
    read_operations: u64,
    mutation_operations: u64,
    query_operations: u64,
    denied_operations: u64,
    anomaly_operations: u64,
}

/// Runtime operational audit event written to append-only file.
#[derive(Debug, Clone)]
struct WorkflowAuditEvent {
    ts_ms: u64,
    tenant: String,
    principal_hash: String,
    request_id_hash: String,
    previous_audit_hash: String,
    audit_hash: String,
    route: String,
    method: String,
    operation: String,
    outcome: String,
    scope: String,
    status: u16,
    anomaly: bool,
}

static TENANT_RATE_LIMIT_OVERRIDE_CACHE: OnceLock<
    Mutex<BTreeMap<String, TenantRateLimitOverride>>,
> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hl7ConnectorFeatureFlag {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
struct ConnectorPluginMetadata {
    plugin_path: String,
    adapter_version: String,
    compatible_min: String,
    compatible_max: String,
}

#[derive(Debug, Clone, Copy)]
enum NormalizedConnectorAdapterKind {
    Dimse,
    HisRis,
    Generic,
}

impl NormalizedConnectorAdapterKind {
    fn as_label(self) -> &'static str {
        match self {
            Self::Dimse => "dimse",
            Self::HisRis => "his_ris",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderProfile {
    Azure,
    AwsHealthImaging,
    Orthanc,
    Dcm4chee,
    Generic,
}

impl ProviderProfile {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "azure" => Some(Self::Azure),
            "aws_health_imaging" | "aws" | "ahi" => Some(Self::AwsHealthImaging),
            "orthanc" => Some(Self::Orthanc),
            "dcm4chee" => Some(Self::Dcm4chee),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Azure => "azure",
            Self::AwsHealthImaging => "aws_health_imaging",
            Self::Orthanc => "orthanc",
            Self::Dcm4chee => "dcm4chee",
            Self::Generic => "generic",
        }
    }

    fn capabilities(self) -> ProviderCapabilityMap {
        match self {
            Self::Azure => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: false,
                workitem: true,
            },
            Self::AwsHealthImaging => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: false,
                workitem: false,
            },
            Self::Orthanc => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: false,
            },
            Self::Dcm4chee => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: false,
            },
            Self::Generic => ProviderCapabilityMap {
                retrieve: true,
                search: true,
                store: true,
                delete: true,
                workitem: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProviderCapabilityMap {
    retrieve: bool,
    search: bool,
    store: bool,
    delete: bool,
    workitem: bool,
}

#[derive(Debug, Clone)]
struct NormalizedConnectorAdapter {
    alias: String,
    target: String,
    adapter_kind: NormalizedConnectorAdapterKind,
    plugin: Option<ConnectorPluginMetadata>,
}

impl Hl7ConnectorFeatureFlag {
    fn parse(raw: &str, alias: &str) -> std::io::Result<Self> {
        let name = format!("DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_{alias}");
        let normalized = raw.trim();
        match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" | "enabled" => Ok(Self::Enabled),
            "0" | "false" | "no" | "off" | "disabled" => Ok(Self::Disabled),
            _ => Err(environment_parse_error(
                &name,
                normalized,
                "must be a boolean-like value",
            )),
        }
    }

    fn as_bool(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// HL7 sink adapter for outbound interoperability notifications.
#[derive(Debug, Clone)]
struct Hl7Sink {
    /// Transport kind.
    kind: Hl7SinkKind,
    /// Target URI/topic.
    target: String,
}

#[derive(Debug, Clone)]
enum Hl7SinkKind {
    Webhook,
    MessageBus,
    Custom(String),
}

impl Hl7SinkKind {
    fn as_label(&self) -> &'static str {
        match self {
            Hl7SinkKind::Webhook => "webhook",
            Hl7SinkKind::MessageBus => "message_bus",
            Hl7SinkKind::Custom(_) => "custom",
        }
    }
}

/// Deterministic interop subscription state.
#[derive(Debug, Clone)]
struct Hl7Subscription {
    /// Stable subscription identifier.
    id: String,
    /// Adapter source name.
    source: String,
    /// Event filters (e.g., `adt`, `orm`, `oru`, `task`, `mpps`, or `sr`).
    event_filter: Vec<String>,
    /// Selected sink.
    sink: Hl7Sink,
    /// Total events delivered to this subscription endpoint.
    delivered_events: u64,
    /// Creation timestamp.
    created_at_ms: u64,
    /// Last event timestamp.
    last_event_ms: u64,
}

/// Supported HL7 message class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hl7MessageClass {
    Adt,
    Orm,
    Oru,
    Siu,
}

impl Hl7MessageClass {
    fn as_label(&self) -> &'static str {
        match self {
            Hl7MessageClass::Adt => "ADT",
            Hl7MessageClass::Orm => "ORM",
            Hl7MessageClass::Oru => "ORU",
            Hl7MessageClass::Siu => "SIU",
        }
    }
}

fn parse_hl7_message_class(raw: &str) -> Option<Hl7MessageClass> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "ADT" | "A01" | "A04" => Some(Hl7MessageClass::Adt),
        "ORM" | "ORM_O01" | "ORM-O01" => Some(Hl7MessageClass::Orm),
        "ORU" | "ORU_R01" | "ORU-R01" => Some(Hl7MessageClass::Oru),
        "SIU" | "SIU_S12" | "SIU-S12" | "S12" => Some(Hl7MessageClass::Siu),
        _ => None,
    }
}

fn hl7_field<'a>(field: Option<&'a &'a str>) -> Option<&'a str> {
    field
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// Failure quarantine entry for malformed HL7 payloads.
#[derive(Debug, Clone)]
struct Hl7FailureRecord {
    /// Deterministic record identifier.
    id: String,
    /// Source adapter (provider).
    source: String,
    /// Parsed HL7 message type, or `invalid`.
    message_type: String,
    /// Human-readable failure reason.
    reason: String,
    /// Optional payload excerpt.
    payload_excerpt: String,
    /// Event timestamp.
    created_at_ms: u64,
    /// Failure scope (e.g., `ingest` or `callback`).
    scope: String,
    /// Callback subscription identifier when scope is `callback`.
    subscription_id: String,
    /// Event identifier used for failure correlation.
    event_id: String,
    /// Correlation identifier for the inbound interface request.
    correlation_id: String,
    /// Monotonic sequence number of the inbound interface event.
    sequence: u64,
    /// Last attempt number attempted.
    attempt: u32,
    /// Maximum attempts allowed before dead-lettering.
    max_attempts: u32,
}

#[derive(Debug, Clone)]
struct StudyReconciliationJob {
    id: String,
    tenant: String,
    source: String,
    target_endpoint: String,
    interval_seconds: u64,
    runs_enqueued: u64,
    runs_completed: u64,
    last_run_at_ms: u64,
    created_at_ms: u64,
    enabled: bool,
}

#[derive(Debug, Clone)]
struct IanEventRecord {
    event_id: String,
    sop_instance_uid: String,
    outcome: CompletionOutcome,
    ingested_at_ms: u64,
}

#[derive(Debug, Clone)]
struct StorageCommitmentStatusRecord {
    transaction_uid: String,
    outcome: CompletionOutcome,
    updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct Hl7CallbackPolicy {
    max_attempts: u32,
    circuit_failure_threshold: u32,
    circuit_base_backoff_ms: u64,
    circuit_max_backoff_ms: u64,
}

/// Runtime state for HL7 transport and notification hooks.
#[derive(Debug, Default)]
struct Hl7RuntimeState {
    subscriptions: BTreeMap<String, Hl7Subscription>,
    failures: VecDeque<Hl7FailureRecord>,
    ups: UpsCommandAdapter,
    completion: CompletionWorkflowAdapter,
    ian_events: BTreeMap<String, IanEventRecord>,
    storage_commitment_status: BTreeMap<String, StorageCommitmentStatusRecord>,
    hl7_ups_correlation: BTreeMap<String, String>,
    subscription_seq: u64,
    failure_seq: u64,
    event_seq: u64,
    replay_cache: BTreeMap<String, CachedMppsRequest>,
    connector_registry: BTreeMap<String, String>,
    connector_feature_flags: BTreeMap<String, Hl7ConnectorFeatureFlag>,
    connector_rollout_percent: BTreeMap<String, u64>,
    connector_plugins: BTreeMap<String, ConnectorPluginMetadata>,
    callback_delivery_idempotency: BTreeMap<String, u64>,
    callback_idempotency_ttl_ms: u64,
    callback_idempotency_path: String,
    callback_max_attempts: u32,
    callback_circuit_breaker_failure_threshold: u32,
    callback_circuit_breaker_base_backoff_ms: u64,
    callback_circuit_breaker_max_backoff_ms: u64,
    connector_callback_failure_streak: BTreeMap<String, u32>,
    connector_circuit_open_until_ms: BTreeMap<String, u64>,
    reconciliation_jobs: BTreeMap<String, StudyReconciliationJob>,
    reconciliation_seq: u64,
}

/// Workflow runtime auth mode.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WorkflowAuthMode {
    DenyAll,
    AllowAll,
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WebhookAuthStrategy {
    None,
    HmacSha256,
}

impl WorkflowAuthMode {
    fn parse(raw: Option<&str>) -> std::io::Result<Self> {
        let mode = raw.unwrap_or("deny_all");
        let mode = match mode {
            "allow_all" => Self::AllowAll,
            "deny_all" => Self::DenyAll,
            "token" => Self::Token,
            _ => {
                return Err(environment_parse_error(
                    "DICOM_WORKFLOW_AUTH_MODE",
                    mode,
                    "supported values are allow_all | deny_all | token",
                ))
            }
        };
        Ok(mode)
    }
}

/// Shared workflow runtime state.
struct RuntimeState {
    worklist: WorklistStore,
    mpps: MppsService,
    sr: SrWorkflowStore,
    mpps_idempotency: BTreeMap<String, CachedMppsRequest>,
    tasks: BTreeMap<String, ProcedureTask>,
    task_id_sequence: u64,
    task_idempotency: BTreeMap<String, CachedMppsRequest>,
    hl7: Hl7RuntimeState,
    audit_path: String,
    audit_rate_window_ms: u64,
    query_rate_limit: u64,
    mutation_rate_limit: u64,
    upload_cap_bytes: u64,
    audit_max_bytes: u64,
    audit_max_rotated_files: usize,
    rate_windows: BTreeMap<String, RequestWindow>,
    anomaly_alert_threshold: u64,
    audit_export_limit: usize,
    denylist_routes: Vec<String>,
    tenant_worklist: BTreeMap<String, Vec<String>>,
    tenant_mpps: BTreeMap<String, Vec<String>>,
    tenant_sr: BTreeMap<String, Vec<String>>,
    tenant_tasks: BTreeMap<String, Vec<String>>,
    metrics: BTreeMap<String, TenantOperationMetrics>,
}

#[derive(Debug, Clone)]
struct Hl7TransportConfig {
    mllp_enabled: bool,
    mllp_bind: Option<String>,
    file_drop_dir: Option<PathBuf>,
    file_drop_done_dir: Option<PathBuf>,
    file_drop_error_dir: Option<PathBuf>,
    file_drop_poll_interval_ms: u64,
}

#[derive(Debug, Clone)]
struct CachedMppsRequest {
    signature: String,
    response: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum TaskStatus {
    Scheduled,
    InProgress,
    OnHold,
    Completed,
    Reviewed,
    Committed,
    Discontinued,
}

#[derive(Debug, Clone)]
struct ProcedureTask {
    task_id: String,
    scheduled_step_id: String,
    tenant: String,
    requested_procedure_id: Option<String>,
    status: TaskStatus,
    worker: Option<String>,
    created_at_ms: u64,
    updated_at_ms: u64,
}

/// Parsed HTTP request.
struct HttpRequest {
    method: String,
    path: String,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_string()
}

fn workflow_actor_context(headers: &BTreeMap<String, String>) -> WorkflowActorContext {
    let principal = headers
        .get("x-sr-principal")
        .or_else(|| headers.get("x-auth-principal"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let role = headers
        .get("x-sr-role")
        .or_else(|| headers.get("x-auth-role"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let tenant = tenant_policy_domain::TENANT_HEADER_CANDIDATES
        .iter()
        .find_map(|header| headers.get(*header))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| TENANT_ID_DEFAULT.to_string());

    WorkflowActorContext {
        principal,
        role,
        tenant,
    }
}

fn workflow_event_source(actor: &WorkflowActorContext) -> String {
    actor
        .principal
        .clone()
        .unwrap_or_else(|| "workflow".to_string())
}

fn tenant_env_suffix(raw_tenant: &str) -> String {
    let trimmed = normalize_identifier(raw_tenant);
    if trimmed.is_empty() {
        return TENANT_ID_DEFAULT.to_ascii_uppercase().replace('-', "_");
    }
    trimmed
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn tenant_quota_override_for_tenant(tenant: &str) -> Option<TenantQuotaOverride> {
    let suffix = tenant_env_suffix(tenant);
    let task_key = format!(
        "{}{}",
        tenant_policy_domain::TENANT_TASK_QUOTA_ENV_PREFIX,
        suffix
    );
    let subscription_key = format!(
        "{}{}",
        tenant_policy_domain::TENANT_SUBSCRIPTION_QUOTA_ENV_PREFIX,
        suffix
    );
    let task_quota = env::var(&task_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0);
    let subscription_quota = env::var(&subscription_key)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0);

    if task_quota.is_none() && subscription_quota.is_none() {
        return None;
    }

    Some(TenantQuotaOverride {
        task_quota: task_quota.unwrap_or(DEFAULT_TENANT_TASK_QUOTA),
        subscription_quota: subscription_quota.unwrap_or(DEFAULT_TENANT_SUBSCRIPTION_QUOTA),
    })
}

fn collect_tenant_quota_overrides_from_env() -> BTreeMap<String, TenantQuotaOverride> {
    let mut task_overrides: BTreeMap<String, usize> = BTreeMap::new();
    let mut subscription_overrides: BTreeMap<String, usize> = BTreeMap::new();
    for (key, value) in env::vars() {
        if let Some(suffix) = key.strip_prefix(tenant_policy_domain::TENANT_TASK_QUOTA_ENV_PREFIX) {
            if let Ok(parsed) = value.trim().parse::<usize>() {
                if parsed > 0 {
                    let _ = task_overrides.insert(suffix.to_ascii_lowercase(), parsed);
                }
            }
            continue;
        }
        if let Some(suffix) =
            key.strip_prefix(tenant_policy_domain::TENANT_SUBSCRIPTION_QUOTA_ENV_PREFIX)
        {
            if let Ok(parsed) = value.trim().parse::<usize>() {
                if parsed > 0 {
                    let _ = subscription_overrides.insert(suffix.to_ascii_lowercase(), parsed);
                }
            }
        }
    }

    let mut out = BTreeMap::new();
    for (tenant_key, task_quota) in &task_overrides {
        let subscription_quota = subscription_overrides
            .get(tenant_key)
            .copied()
            .unwrap_or(DEFAULT_TENANT_SUBSCRIPTION_QUOTA);
        let _ = out.insert(
            tenant_key.clone(),
            TenantQuotaOverride {
                task_quota: *task_quota,
                subscription_quota,
            },
        );
    }
    for (tenant_key, subscription_quota) in &subscription_overrides {
        let _ = out
            .entry(tenant_key.clone())
            .or_insert(TenantQuotaOverride {
                task_quota: DEFAULT_TENANT_TASK_QUOTA,
                subscription_quota: *subscription_quota,
            });
    }
    out
}

fn tenant_rate_limit_override_cache() -> &'static Mutex<BTreeMap<String, TenantRateLimitOverride>> {
    TENANT_RATE_LIMIT_OVERRIDE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn set_tenant_rate_limit_override_cache(overrides: BTreeMap<String, TenantRateLimitOverride>) {
    if let Ok(mut cache) = tenant_rate_limit_override_cache().lock() {
        *cache = overrides;
    }
}

fn collect_tenant_rate_limit_overrides_from_env(
    defaults: TenantRateLimitOverride,
) -> BTreeMap<String, TenantRateLimitOverride> {
    let mut out = BTreeMap::new();
    for (key, value) in env::vars() {
        let parsed = match value.trim().parse::<u64>() {
            Ok(parsed) if parsed > 0 => parsed,
            _ => continue,
        };
        if let Some(suffix) =
            key.strip_prefix(tenant_policy_domain::TENANT_QUERY_RATE_LIMIT_ENV_PREFIX)
        {
            let tenant_key = suffix.to_ascii_lowercase();
            let entry = out.entry(tenant_key).or_insert(defaults);
            entry.query_rate_limit = parsed;
            continue;
        }
        if let Some(suffix) =
            key.strip_prefix(tenant_policy_domain::TENANT_MUTATION_RATE_LIMIT_ENV_PREFIX)
        {
            let tenant_key = suffix.to_ascii_lowercase();
            let entry = out.entry(tenant_key).or_insert(defaults);
            entry.mutation_rate_limit = parsed;
            continue;
        }
        if let Some(suffix) =
            key.strip_prefix(tenant_policy_domain::TENANT_UPLOAD_CAP_BYTES_ENV_PREFIX)
        {
            let tenant_key = suffix.to_ascii_lowercase();
            let entry = out.entry(tenant_key).or_insert(defaults);
            entry.upload_cap_bytes = parsed;
        }
    }
    out
}

fn tenant_rate_limit_override_for_tenant(tenant: &str) -> Option<TenantRateLimitOverride> {
    let tenant_key = tenant_env_suffix(tenant).to_ascii_lowercase();
    let cache = tenant_rate_limit_override_cache().lock().ok()?;
    cache.get(&tenant_key).copied()
}

fn tenant_rate_limit_overrides_snapshot() -> BTreeMap<String, TenantRateLimitOverride> {
    tenant_rate_limit_override_cache()
        .lock()
        .map(|cache| cache.clone())
        .unwrap_or_default()
}

fn tenant_policy(state: &RuntimeState, tenant: &str) -> TenantWorkflowPolicy {
    let mut policy = TenantWorkflowPolicy {
        query_rate_limit: state.query_rate_limit,
        mutation_rate_limit: state.mutation_rate_limit,
        upload_cap_bytes: state.upload_cap_bytes,
        task_quota: DEFAULT_TENANT_TASK_QUOTA,
        subscription_quota: DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
    };
    if let Some(override_limits) = tenant_rate_limit_override_for_tenant(tenant) {
        policy.query_rate_limit = override_limits.query_rate_limit;
        policy.mutation_rate_limit = override_limits.mutation_rate_limit;
        policy.upload_cap_bytes = override_limits.upload_cap_bytes;
    }
    if let Some(override_limits) = tenant_quota_override_for_tenant(tenant) {
        policy.task_quota = override_limits.task_quota;
        policy.subscription_quota = override_limits.subscription_quota;
    }
    policy
}

fn secret_dir_root(raw_secret_dir: Option<&str>) -> std::io::Result<Option<PathBuf>> {
    let raw_secret_dir = match raw_secret_dir {
        Some(raw) => raw.trim(),
        None => return Ok(None),
    };
    if raw_secret_dir.is_empty() {
        return Ok(None);
    }

    let root = Path::new(raw_secret_dir);
    if root
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(IoError::other(
            "DICOM_WORKFLOW_SECRET_DIR must not contain parent-directory components",
        ));
    }
    let canonical = fs::canonicalize(root).map_err(|error| {
        IoError::new(
            error.kind(),
            format!("DICOM_WORKFLOW_SECRET_DIR must be an existing directory: {raw_secret_dir}"),
        )
    })?;
    if !canonical.is_dir() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("DICOM_WORKFLOW_SECRET_DIR must be a directory: {raw_secret_dir}"),
        ));
    }
    Ok(Some(canonical))
}

fn resolve_secret_path(secret_dir: Option<&Path>, raw_path: &str) -> std::io::Result<PathBuf> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "secret path must be non-empty",
        ));
    }

    if trimmed
        .split('/')
        .any(|segment| segment == ".." || segment == "." || segment.is_empty())
        && Path::new(trimmed).is_relative()
    {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "secret path contains disallowed relative segments",
        ));
    }
    if Path::new(trimmed)
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "secret path contains unsupported traversal component",
        ));
    }

    let candidate = if Path::new(trimmed).is_absolute() {
        PathBuf::from(trimmed)
    } else if let Some(root) = secret_dir {
        root.join(trimmed)
    } else {
        PathBuf::from(trimmed)
    };

    let canonical_candidate = fs::canonicalize(&candidate)?;
    if let Some(root) = secret_dir {
        if !canonical_candidate.starts_with(root) {
            return Err(IoError::new(
                IoErrorKind::PermissionDenied,
                "secret path must resolve within DICOM_WORKFLOW_SECRET_DIR",
            ));
        }
    }
    Ok(canonical_candidate)
}

fn auth_denied_error(detail: &str) -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.AUTH_DENIED",
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-auth".to_string(),
            detail: detail.to_string(),
        },
        "auth_denied",
    )
    .into()
}

fn route_path_normalize(raw_path: &str) -> Result<String, Box<Error>> {
    let trimmed = raw_path.trim();
    if trimmed.is_empty() {
        return Err(decode_error("invalid route path"));
    }
    if !trimmed.starts_with('/') {
        return Err(decode_error("invalid route path"));
    }

    let mut normalized = trimmed.trim_end_matches('/').to_string();
    if normalized.is_empty() {
        return Err(decode_error("invalid route path"));
    }

    if normalized == "/tasks" {
        normalized = "/workflow/tasks".to_string();
    } else if normalized == "/ups" {
        normalized = "/workflow/tasks".to_string();
    } else if normalized == "/workitems" {
        normalized = WORKITEM_COLLECTION_PATH.to_string();
    } else if normalized.starts_with("/tasks/") {
        normalized = format!("/workflow{}", &normalized["/tasks".len()..]);
    } else if normalized.starts_with("/ups/") {
        normalized = format!("/workflow{}", &normalized["/ups".len()..]);
    } else if normalized.starts_with("/workitems/") {
        normalized = format!(
            "{}{}",
            WORKITEM_COLLECTION_PATH,
            &normalized["/workitems".len()..]
        );
    }
    Ok(normalized)
}

fn workflow_role_is_writer(role: &str) -> bool {
    matches!(role, "writer" | "admin" | "operator")
}

fn route_id_to_tenant_index(
    tenant_indexes: &mut BTreeMap<String, Vec<String>>,
    tenant: &str,
    id: &str,
) {
    let bucket = tenant_indexes.entry(tenant.to_string()).or_default();
    if !bucket.iter().any(|existing| existing == id) {
        bucket.push(id.to_string());
    }
}

fn tenant_indexes_contains(
    tenant_indexes: &BTreeMap<String, Vec<String>>,
    tenant: &str,
    id: &str,
) -> bool {
    tenant_indexes
        .get(tenant)
        .is_some_and(|entries| entries.iter().any(|entry| entry == id))
}

fn tenant_of_id(tenant_indexes: &BTreeMap<String, Vec<String>>, id: &str) -> Option<String> {
    for (tenant, entries) in tenant_indexes {
        if entries.iter().any(|entry| entry == id) {
            return Some(tenant.clone());
        }
    }
    None
}

fn actor_has_resource_access(
    actor: &WorkflowActorContext,
    indexes: &BTreeMap<String, Vec<String>>,
    resource_id: &str,
) -> bool {
    tenant_indexes_contains(indexes, &actor.tenant, resource_id)
}

fn touch_tenant_index(state: &mut RuntimeState, tenant: &str) {
    let _ = state.tenant_worklist.entry(tenant.to_string()).or_default();
    let _ = state.tenant_mpps.entry(tenant.to_string()).or_default();
    let _ = state.tenant_sr.entry(tenant.to_string()).or_default();
    let _ = state.tenant_tasks.entry(tenant.to_string()).or_default();
}

fn ensure_tenant_indexes_initialized(state: &mut RuntimeState) {
    if !state.tenant_worklist.is_empty()
        && !state.tenant_mpps.is_empty()
        && !state.tenant_sr.is_empty()
        && !state.tenant_tasks.is_empty()
    {
        return;
    }

    state.tenant_worklist.clear();
    state.tenant_mpps.clear();
    state.tenant_sr.clear();
    state.tenant_tasks.clear();

    let default_tenant = TENANT_ID_DEFAULT.to_string();
    touch_tenant_index(state, &default_tenant);

    for task in state.tasks.values() {
        let tenant = if task.tenant.is_empty() {
            default_tenant.clone()
        } else {
            task.tenant.clone()
        };
        route_id_to_tenant_index(&mut state.tenant_tasks, &tenant, &task.task_id);
    }

    let worklist_ids: Vec<_> = state
        .worklist
        .query(&WorklistQuery::default())
        .ok()
        .map(|items| {
            items
                .into_iter()
                .filter_map(|item| {
                    item.get(TAG_SPS_SEQUENCE)
                        .and_then(|element| match &element.value {
                            Value::Sequence(entries) => entries.first(),
                            _ => None,
                        })
                        .and_then(|entry| entry.get_str(TAG_SPS_ID))
                        .map(|value| value.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for step_id in worklist_ids {
        route_id_to_tenant_index(&mut state.tenant_worklist, &default_tenant, &step_id);
    }

    let mpps_ids = state.mpps.all_updates();
    for update in mpps_ids {
        route_id_to_tenant_index(
            &mut state.tenant_mpps,
            &default_tenant,
            &update.sop_instance_uid,
        );
    }

    for document in state.sr.documents() {
        route_id_to_tenant_index(
            &mut state.tenant_sr,
            &default_tenant,
            &document.provenance.sop_instance_uid,
        );
    }
}

fn load_secret_value(
    secret_var: &str,
    secret_path_var: &str,
    secret_dir: Option<&str>,
    fallback_files: &[&str],
) -> std::io::Result<Option<String>> {
    if let Ok(raw) = env::var(secret_var) {
        let value = raw.trim().to_string();
        if !value.is_empty() {
            return Ok(Some(value));
        }
    }
    if let Ok(path) = env::var(secret_path_var) {
        let value = path.trim();
        if !value.is_empty() {
            let root = secret_dir_root(secret_dir)?;
            let resolved = match root.as_ref() {
                Some(root) => resolve_secret_path(Some(root.as_path()), value)?,
                None => resolve_secret_path(None, value)?,
            };
            let loaded = fs::read_to_string(resolved)?.trim().to_string();
            if !loaded.is_empty() {
                return Ok(Some(loaded));
            }
        }
    }
    let secret_dir = match secret_dir_root(secret_dir)? {
        Some(value) => value,
        None => return Ok(None),
    };
    for fallback in fallback_files {
        let candidate = secret_dir.join(fallback);
        if candidate.exists() {
            let resolved =
                resolve_secret_path(Some(secret_dir.as_path()), &candidate.to_string_lossy())?;
            let loaded = fs::read_to_string(resolved)?.trim().to_string();
            if !loaded.is_empty() {
                return Ok(Some(loaded));
            }
        }
    }
    Ok(None)
}

fn secret_path_with_default(
    env_var: &str,
    fallback_files: &[&str],
    secret_dir: Option<&str>,
) -> std::io::Result<Option<String>> {
    let direct = env::var(env_var).ok().map(|raw| raw.trim().to_string());
    if let Some(path) = direct.filter(|value| !value.is_empty()) {
        let resolved = match secret_dir_root(secret_dir)?.as_ref() {
            Some(dir) => resolve_secret_path(Some(dir.as_path()), &path)?,
            None => resolve_secret_path(None, path.as_str())?,
        };
        return Ok(Some(resolved.to_string_lossy().to_string()));
    }
    let Some(secret_dir) = secret_dir else {
        return Ok(None);
    };
    let Some(secret_dir) = secret_dir_root(Some(secret_dir))? else {
        return Ok(None);
    };
    if !secret_dir.is_dir() {
        return Ok(None);
    }
    for fallback in fallback_files {
        let candidate = secret_dir.join(fallback);
        if candidate.exists() {
            let resolved =
                resolve_secret_path(Some(secret_dir.as_path()), &candidate.to_string_lossy())?;
            return Ok(Some(resolved.to_string_lossy().to_string()));
        }
    }
    Ok(None)
}

fn hash_text(value: &str) -> String {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn parse_denylist_routes(raw_routes: Option<String>) -> Vec<String> {
    let mut denylist: Vec<String> = raw_routes
        .map(|raw| {
            raw.split([',', ';', '\n', ' ', '\t'])
                .map(|entry| entry.trim())
                .filter(|entry| !entry.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();

    if denylist.is_empty() {
        denylist.extend(
            DEFAULT_WORKFLOW_DENYLIST_PATHS
                .iter()
                .map(|template| template.to_string()),
        );
    }

    denylist.sort_unstable();
    denylist.dedup();
    denylist
}

fn environment_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!(
            "service={WORKFLOW_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}",
        ),
    )
}

fn parse_log_level(name: &str) -> std::io::Result<WorkflowLogLevel> {
    match parse_optional_string(WORKFLOW_SERVICE_NAME, name)? {
        Some(raw) => WorkflowLogLevel::parse(&raw).ok_or_else(|| {
            environment_parse_error(name, raw.trim(), "must be off/error/warn/info/debug/trace")
        }),
        None => Ok(WorkflowLogLevel::Info),
    }
}

fn lookup_identifier_from_map(values: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(value) = values.get(*key) {
            let normalized = normalize_identifier(value);
            if !normalized.is_empty() {
                return Some(normalized);
            }
        }
    }

    for (key, value) in values {
        let normalized_key = key.to_ascii_lowercase().replace('_', "");
        let normalized_value = normalize_identifier(value);
        if normalized_value.is_empty() {
            continue;
        }
        for alias in keys {
            let alias_normalized = alias.to_ascii_lowercase().replace('_', "");
            if normalized_key == alias_normalized {
                return Some(normalized_value);
            }
        }
    }

    None
}

fn required_identifier_from_map(
    values: &BTreeMap<String, String>,
    keys: &[&str],
    context: &str,
) -> Result<String, Box<Error>> {
    lookup_identifier_from_map(values, keys)
        .ok_or_else(|| decode_error(format!("missing required identifier: {context}")))
}

fn normalize_connector_adapter_kind(alias: &str, target: &str) -> NormalizedConnectorAdapterKind {
    let alias = alias.to_ascii_lowercase();
    let target = target.to_ascii_lowercase();
    if alias.contains("dimse") || target.starts_with("dimse://") {
        return NormalizedConnectorAdapterKind::Dimse;
    }
    if alias.contains("his")
        || alias.contains("ris")
        || target.contains("/his")
        || target.contains("/ris")
    {
        return NormalizedConnectorAdapterKind::HisRis;
    }
    NormalizedConnectorAdapterKind::Generic
}

fn normalized_connector_descriptors(state: &RuntimeState) -> Vec<NormalizedConnectorAdapter> {
    let mut out = Vec::new();
    for (alias, target) in &state.hl7.connector_registry {
        let plugin = state.hl7.connector_plugins.get(alias).cloned();
        out.push(NormalizedConnectorAdapter {
            alias: alias.clone(),
            target: target.clone(),
            adapter_kind: normalize_connector_adapter_kind(alias, target),
            plugin,
        });
    }
    out
}

fn parse_hl7_connector_plugin_path(var_name: &str, raw: &str) -> std::io::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must be a non-empty path"),
        ));
    }
    let path = Path::new(trimmed);
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must not contain parent-directory traversal segments"),
        ));
    }
    if !path.exists() {
        return Err(IoError::new(
            IoErrorKind::NotFound,
            format!("{var_name} must point to an existing file: {trimmed}"),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        IoError::new(
            error.kind(),
            format!("{var_name} must resolve to an existing file: {trimmed}"),
        )
    })?;
    if !canonical.is_file() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must be a file path: {trimmed}"),
        ));
    }
    Ok(canonical.to_string_lossy().to_string())
}

fn parse_semver_triplet(raw: &str) -> std::io::Result<(u64, u64, u64)> {
    let trimmed = raw.trim();
    let mut parts = trimmed.split('.');
    let major = parts
        .next()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "missing major version"))?
        .parse::<u64>()
        .map_err(|_| IoError::new(IoErrorKind::InvalidInput, "invalid major version"))?;
    let minor = parts
        .next()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "missing minor version"))?
        .parse::<u64>()
        .map_err(|_| IoError::new(IoErrorKind::InvalidInput, "invalid minor version"))?;
    let patch = parts
        .next()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "missing patch version"))?
        .parse::<u64>()
        .map_err(|_| IoError::new(IoErrorKind::InvalidInput, "invalid patch version"))?;
    if parts.next().is_some() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "semantic version must have exactly three components",
        ));
    }
    Ok((major, minor, patch))
}

fn validate_hl7_connector_plugin_compatibility(
    alias: &str,
    adapter_version: &str,
    compatible_min: &str,
    compatible_max: &str,
) -> std::io::Result<()> {
    let version = parse_semver_triplet(adapter_version).map_err(|err| {
        IoError::new(
            IoErrorKind::InvalidInput,
            format!("connector {alias} adapter_version {adapter_version} is invalid semver: {err}"),
        )
    })?;
    let min = parse_semver_triplet(compatible_min).map_err(|err| {
        IoError::new(
            IoErrorKind::InvalidInput,
            format!("connector {alias} compatible_min {compatible_min} is invalid semver: {err}"),
        )
    })?;
    let max = parse_semver_triplet(compatible_max).map_err(|err| {
        IoError::new(
            IoErrorKind::InvalidInput,
            format!("connector {alias} compatible_max {compatible_max} is invalid semver: {err}"),
        )
    })?;
    if min > max {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!(
                "connector {alias} compatibility range is invalid: min {compatible_min} > max {compatible_max}"
            ),
        ));
    }
    if version < min || version > max {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!(
                "connector {alias} adapter_version {adapter_version} is outside compatible range [{compatible_min}, {compatible_max}]"
            ),
        ));
    }
    Ok(())
}

fn parse_hl7_callback_policy_from_env() -> std::io::Result<Hl7CallbackPolicy> {
    let max_attempts = parse_u64(
        WORKFLOW_SERVICE_NAME,
        HL7_CALLBACK_MAX_ATTEMPTS_ENV,
        MAX_WORKFLOW_CALLBACK_ATTEMPTS as u64,
        NumericBounds::with_range(1, 10),
    )? as u32;
    let circuit_failure_threshold = parse_u64(
        WORKFLOW_SERVICE_NAME,
        HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV,
        CALLBACK_CIRCUIT_BREAKER_FAILURE_THRESHOLD as u64,
        NumericBounds::with_range(1, 10),
    )? as u32;
    let circuit_base_backoff_ms = parse_u64(
        WORKFLOW_SERVICE_NAME,
        HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV,
        CALLBACK_CIRCUIT_BREAKER_BASE_BACKOFF_MS,
        NumericBounds::with_range(100, 3_600_000),
    )?;
    let circuit_max_backoff_ms = parse_u64(
        WORKFLOW_SERVICE_NAME,
        HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV,
        CALLBACK_CIRCUIT_BREAKER_MAX_BACKOFF_MS,
        NumericBounds::with_range(1_000, 86_400_000),
    )?;
    if circuit_base_backoff_ms > circuit_max_backoff_ms {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!(
                "{HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV} must be <= {HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV}"
            ),
        ));
    }
    Ok(Hl7CallbackPolicy {
        max_attempts,
        circuit_failure_threshold,
        circuit_base_backoff_ms,
        circuit_max_backoff_ms,
    })
}

fn parse_hl7_connector_plugins() -> std::io::Result<BTreeMap<String, ConnectorPluginMetadata>> {
    let mut plugins = BTreeMap::new();
    let mut collected = 0usize;
    for (key, value) in env::vars() {
        if !key.starts_with(HL7_CONNECTOR_PLUGIN_PREFIX) {
            continue;
        }
        let alias = normalize_connector_alias(&key[HL7_CONNECTOR_PLUGIN_PREFIX.len()..].trim());
        if alias.is_empty() {
            continue;
        }
        if collected >= MAX_HL7_CONNECTOR_REGISTRY {
            break;
        }
        let plugin_path = parse_hl7_connector_plugin_path(&key, &value)?;
        let version_key = format!(
            "{HL7_CONNECTOR_VERSION_PREFIX}{}",
            alias.to_ascii_uppercase()
        );
        let min_key = format!(
            "{HL7_CONNECTOR_COMPAT_MIN_PREFIX}{}",
            alias.to_ascii_uppercase()
        );
        let max_key = format!(
            "{HL7_CONNECTOR_COMPAT_MAX_PREFIX}{}",
            alias.to_ascii_uppercase()
        );
        let adapter_version = env::var(&version_key).unwrap_or_else(|_| "1.0.0".to_string());
        let compatible_min = env::var(&min_key).unwrap_or_else(|_| adapter_version.clone());
        let compatible_max = env::var(&max_key).unwrap_or_else(|_| adapter_version.clone());
        validate_hl7_connector_plugin_compatibility(
            &alias,
            &adapter_version,
            &compatible_min,
            &compatible_max,
        )?;
        plugins.insert(
            alias.clone(),
            ConnectorPluginMetadata {
                plugin_path,
                adapter_version,
                compatible_min,
                compatible_max,
            },
        );
        collected += 1;
    }
    Ok(plugins)
}

fn parse_hl7_connector_registry() -> BTreeMap<String, String> {
    let mut connectors = BTreeMap::new();
    let mut collected = 0usize;
    for (key, value) in env::vars() {
        if !key.starts_with(HL7_CONNECTOR_ENV_PREFIX) {
            continue;
        }
        if key.starts_with(HL7_CONNECTOR_FEATURE_PREFIX)
            || key.starts_with(HL7_CONNECTOR_ROLLOUT_PREFIX)
            || key.starts_with(HL7_CONNECTOR_PLUGIN_PREFIX)
            || key.starts_with(HL7_CONNECTOR_VERSION_PREFIX)
            || key.starts_with(HL7_CONNECTOR_COMPAT_MIN_PREFIX)
            || key.starts_with(HL7_CONNECTOR_COMPAT_MAX_PREFIX)
        {
            continue;
        }
        let alias = normalize_connector_alias(&key[HL7_CONNECTOR_ENV_PREFIX.len()..].trim());
        let destination = value.trim().to_string();
        if alias.is_empty() || destination.is_empty() {
            continue;
        }
        if collected >= MAX_HL7_CONNECTOR_REGISTRY {
            break;
        }
        connectors.insert(alias, destination);
        collected += 1;
    }
    connectors
}

fn parse_hl7_connector_feature_flags() -> std::io::Result<BTreeMap<String, Hl7ConnectorFeatureFlag>>
{
    let mut feature_flags = BTreeMap::new();
    let mut collected = 0usize;
    for (key, value) in env::vars() {
        if !key.starts_with(HL7_CONNECTOR_FEATURE_PREFIX) {
            continue;
        }
        let alias = normalize_connector_alias(&key[HL7_CONNECTOR_FEATURE_PREFIX.len()..].trim());
        if alias.is_empty() {
            continue;
        }
        if collected >= MAX_HL7_CONNECTOR_REGISTRY {
            break;
        }
        let flag = Hl7ConnectorFeatureFlag::parse(&value, &alias)?;
        feature_flags.insert(alias, flag);
        collected += 1;
    }
    Ok(feature_flags)
}

fn parse_hl7_connector_rollout_percents() -> std::io::Result<BTreeMap<String, u64>> {
    let mut rollout_percents = BTreeMap::new();
    let mut collected = 0usize;
    for (key, value) in env::vars() {
        if !key.starts_with(HL7_CONNECTOR_ROLLOUT_PREFIX) {
            continue;
        }
        let alias = normalize_connector_alias(&key[HL7_CONNECTOR_ROLLOUT_PREFIX.len()..].trim());
        if alias.is_empty() {
            continue;
        }
        if collected >= MAX_HL7_CONNECTOR_REGISTRY {
            break;
        }
        let parsed = value.trim().parse::<u64>().map_err(|_| {
            environment_parse_error(&key, value.trim(), "must be a non-negative integer")
        })?;
        if parsed > 100 {
            return Err(environment_parse_error(
                &key,
                value.trim(),
                "must be within 0..=100",
            ));
        }
        rollout_percents.insert(alias, parsed);
        collected += 1;
    }
    Ok(rollout_percents)
}

fn parse_hl7_transport_dir(var_name: &str, raw: &str) -> std::io::Result<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must be a non-empty directory path"),
        ));
    }
    if Path::new(trimmed)
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must not contain parent-directory traversal segments"),
        ));
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        return Err(IoError::new(
            IoErrorKind::NotFound,
            format!("{var_name} must point to an existing directory: {trimmed}"),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        IoError::new(
            error.kind(),
            format!("{var_name} must resolve to an existing directory: {trimmed}"),
        )
    })?;
    if !canonical.is_dir() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            format!("{var_name} must be a directory: {trimmed}"),
        ));
    }
    Ok(canonical)
}

fn parse_hl7_mllp_bind(raw: &str) -> std::io::Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_BIND must be a host:port value",
        ));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_BIND must not contain whitespace",
        ));
    }
    let (_, raw_port) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_BIND must include a host and port (for example 127.0.0.1:2575)",
        ))?;
    if !raw_port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_BIND requires a numeric TCP port",
        ));
    }
    let _port = raw_port.parse::<u16>().map_err(|_| {
        IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_BIND port must be 0-65535",
        )
    })?;
    Ok(trimmed.to_string())
}

fn parse_hl7_transport_config() -> std::io::Result<Hl7TransportConfig> {
    let mllp_enabled = parse_bool(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_HL7_MLLP_ENABLED",
        false,
    )?;
    let mllp_bind = parse_optional_string(WORKFLOW_SERVICE_NAME, "DICOM_WORKFLOW_HL7_MLLP_BIND")?
        .map(|raw| parse_hl7_mllp_bind(&raw))
        .transpose()?;

    if mllp_enabled && mllp_bind.is_none() {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "DICOM_WORKFLOW_HL7_MLLP_ENABLED requires DICOM_WORKFLOW_HL7_MLLP_BIND",
        ));
    }

    let file_drop_dir =
        match parse_optional_string(WORKFLOW_SERVICE_NAME, "DICOM_WORKFLOW_HL7_FILE_DROP_DIR")? {
            Some(raw) if raw.trim().is_empty() => {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    "DICOM_WORKFLOW_HL7_FILE_DROP_DIR must be non-empty",
                ));
            }
            Some(raw) => Some(parse_hl7_transport_dir(
                "DICOM_WORKFLOW_HL7_FILE_DROP_DIR",
                &raw,
            )?),
            None => None,
        };
    let file_drop_done_dir = match parse_optional_string(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR",
    )? {
        Some(raw) if raw.trim().is_empty() => {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR must be non-empty",
            ));
        }
        Some(raw) => Some(parse_hl7_transport_dir(
            "DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR",
            &raw,
        )?),
        None => None,
    };
    let file_drop_error_dir = match parse_optional_string(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR",
    )? {
        Some(raw) if raw.trim().is_empty() => {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR must be non-empty",
            ));
        }
        Some(raw) => Some(parse_hl7_transport_dir(
            "DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR",
            &raw,
        )?),
        None => None,
    };

    let file_drop_dir_count = usize::from(file_drop_dir.is_some())
        + usize::from(file_drop_done_dir.is_some())
        + usize::from(file_drop_error_dir.is_some());
    if file_drop_dir_count > 0 && file_drop_dir_count < 3 {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "HL7 file-drop requires DICOM_WORKFLOW_HL7_FILE_DROP_DIR, DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR, and DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR",
        ));
    }

    let file_drop_poll_interval_ms = parse_u64(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS",
        DEFAULT_HL7_FILE_DROP_POLL_INTERVAL_MS,
        NumericBounds::at_least(1),
    )?;

    Ok(Hl7TransportConfig {
        mllp_enabled,
        mllp_bind,
        file_drop_dir,
        file_drop_done_dir,
        file_drop_error_dir,
        file_drop_poll_interval_ms,
    })
}

fn normalize_connector_alias(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase();
    if let Some(prefix) = normalized.strip_suffix("_star") {
        format!("{}*", prefix.replace('_', ""))
    } else {
        normalized.replace('_', "")
    }
}

fn resolve_hl7_connector_alias(
    connectors: &BTreeMap<String, String>,
    alias: &str,
) -> Option<String> {
    let requested = normalize_connector_alias(alias);
    if let Some(value) = connectors.get(&requested) {
        return Some(value.clone());
    }

    let mut best_match: Option<(usize, String)> = None;
    for (pattern, value) in connectors {
        if !pattern.ends_with('*') {
            continue;
        }

        let prefix = &pattern[..pattern.len() - 1];
        if requested.starts_with(prefix) {
            if best_match
                .as_ref()
                .is_none_or(|(len, _)| prefix.len() > *len)
            {
                best_match = Some((prefix.len(), value.clone()));
            }
        }
    }

    best_match.map(|(_, value)| value)
}

fn resolve_hl7_connector_setting<T: Copy>(
    settings: &BTreeMap<String, T>,
    alias: &str,
    default: T,
) -> T {
    let requested = normalize_connector_alias(alias);
    if let Some(value) = settings.get(&requested) {
        return *value;
    }

    let mut best_match: Option<(usize, T)> = None;
    for (pattern, value) in settings {
        if !pattern.ends_with('*') {
            continue;
        }

        let prefix = &pattern[..pattern.len() - 1];
        if requested.starts_with(prefix) {
            if best_match.is_none_or(|(length, _)| prefix.len() > length) {
                best_match = Some((prefix.len(), *value));
            }
        }
    }
    best_match.map(|(_, value)| value).unwrap_or(default)
}

fn resolve_hl7_connector_rollout_percent(
    rollout_percents: &BTreeMap<String, u64>,
    alias: &str,
) -> u64 {
    resolve_hl7_connector_setting(
        rollout_percents,
        alias,
        DEFAULT_HL7_CONNECTOR_ROLLOUT_PERCENT,
    )
}

fn hl7_connector_rollout_bucket(event_id: &str, alias: &str) -> u64 {
    let raw = hash_text(&format!("{alias}|{event_id}"));
    u64::from_str_radix(&raw[..16], 16).unwrap_or_default() % 100
}

fn hl7_connector_rollout_allows(event_id: &str, alias: &str, rollout_percent: u64) -> bool {
    if rollout_percent >= 100 {
        return true;
    }
    if rollout_percent == 0 {
        return false;
    }
    hl7_connector_rollout_bucket(event_id, alias) < rollout_percent
}

fn normalize_route_identifier(raw: &str) -> Result<String, Box<Error>> {
    let normalized = normalize_identifier(raw);
    if normalized.is_empty() || normalized.contains('/') {
        return Err(decode_error("invalid identifier in route"));
    }
    if normalized.contains(' ') {
        return Err(decode_error("identifier contains embedded whitespace"));
    }
    Ok(normalized)
}

fn resolve_hl7_connector_feature_flag(
    feature_flags: &BTreeMap<String, Hl7ConnectorFeatureFlag>,
    alias: &str,
) -> bool {
    resolve_hl7_connector_setting(feature_flags, alias, Hl7ConnectorFeatureFlag::Enabled).as_bool()
}

fn json_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Start the workflow server loop.
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

fn handle_connection(
    stream: &mut TcpStream,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    auth_mode: &WorkflowAuthMode,
    transport_security: &'static str,
    auth_token: Option<&str>,
) -> std::io::Result<()> {
    let request_bytes = read_http_request(stream, limits).inspect_err(|err| {
        let response = http_error_response(
            400,
            "Bad Request",
            "request_read_failed",
            &err.to_string(),
            false,
        );
        let _ = stream.write_all(&response);
    })?;

    let request = match parse_http_request(&request_bytes, limits) {
        Ok(request) => request,
        Err(err) => {
            let response =
                http_error_response(400, "Bad Request", "request_parse_failed", &err, false);
            stream.write_all(&response)?;
            return Ok(());
        }
    };

    let head_only = request.method == "HEAD";

    if !authorize(&request, auth_mode, transport_security, auth_token) {
        let response = http_error_response(
            403,
            "Forbidden",
            "auth_denied",
            "request denied by auth policy",
            head_only,
        );
        stream.write_all(&response)?;
        return Ok(());
    }

    let response = match route_request(&request, state, limits) {
        Ok(response) => http_success_response(response, head_only),
        Err(err) => {
            let (status, label) = status_for_error(&err);
            http_error_response(status, label, err.code, &err.message, head_only)
        }
    };
    stream.write_all(&response)?;
    Ok(())
}

fn route_request(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let request_path = route_path_normalize(&request.path)?;
    let actor = workflow_actor_context(&request.headers);
    let contract = workflow_contract_for_request(&request.method, &request_path);
    let is_mutation = request.method != "GET" && request.method != "HEAD";

    let policy_anomaly = {
        let mut store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        ensure_tenant_indexes_initialized(&mut store);
        let policy_anomaly = match contract {
            Some(contract) => {
                enforce_route_policy(&mut store, &actor, request, &contract, &request_path)?
            }
            None => false,
        };
        if let Some(contract) = &contract {
            touch_tenant_index(&mut store, &actor.tenant);
            let metrics = store.metrics.entry(actor.tenant.clone()).or_default();
            metrics.read_operations = metrics.read_operations.saturating_add(1);
            if is_mutation {
                metrics.mutation_operations = metrics.mutation_operations.saturating_add(1);
            } else {
                metrics.query_operations = metrics.query_operations.saturating_add(1);
            }
            let _ = contract;
        }
        policy_anomaly
    };

    let response = route_request_core(&request_path, request, state, limits, &actor);
    let status = match &response {
        Ok(WorkflowResponse::Json(code, _)) => *code,
        Err(err) => status_for_error(err).0,
    };
    let is_anomaly = status == 429 || policy_anomaly;
    {
        let mut store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut outcome = "ok".to_string();
        if let Some(contract) = &contract {
            let metrics = store.metrics.entry(actor.tenant.clone()).or_default();
            if status == 429 {
                metrics.anomaly_operations = metrics.anomaly_operations.saturating_add(1);
            }
            if status >= 400 {
                metrics.denied_operations = metrics.denied_operations.saturating_add(1);
                outcome = "error".to_string();
            }
            let _ = contract;
        }
        if let Some(contract) = &contract {
            let _ = store_append_workflow_audit(
                &mut store,
                &actor,
                request,
                contract.operation,
                contract.path_template,
                status,
                &outcome,
                is_anomaly,
            );
        }
    }
    response
}

fn workflow_provider_profile_from_env() -> Result<ProviderProfile, Box<Error>> {
    match env::var("DICOM_WORKFLOW_PROVIDER_PROFILE") {
        Ok(raw) => ProviderProfile::parse(raw.trim()).ok_or_else(|| {
            decode_error(
                "invalid DICOM_WORKFLOW_PROVIDER_PROFILE; supported values: azure | aws_health_imaging | orthanc | dcm4chee | generic",
            )
        }),
        Err(env::VarError::NotPresent) => Ok(ProviderProfile::Generic),
        Err(env::VarError::NotUnicode(_)) => {
            Err(decode_error("DICOM_WORKFLOW_PROVIDER_PROFILE must be valid UTF-8"))
        }
    }
}

fn blocked_provider_profile_operation(
    method: &str,
    path: &str,
    profile: ProviderProfile,
) -> Option<&'static str> {
    let capabilities = profile.capabilities();
    if path == WORKITEM_COLLECTION_PATH || path.starts_with(WORKITEM_ITEM_PREFIX) {
        if !capabilities.workitem {
            return Some("workitem");
        }
        return None;
    }

    match method {
        "GET" | "HEAD" => {
            if path.contains("/search") {
                if !capabilities.search {
                    Some("search")
                } else {
                    None
                }
            } else if !capabilities.retrieve {
                Some("retrieve")
            } else {
                None
            }
        }
        "POST" | "PUT" | "PATCH" => {
            if !capabilities.store {
                Some("store")
            } else {
                None
            }
        }
        "DELETE" => {
            if !capabilities.delete {
                Some("delete")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn route_request_core(
    request_path: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    actor: &WorkflowActorContext,
) -> Result<WorkflowResponse, Box<Error>> {
    let provider_profile = workflow_provider_profile_from_env()?;
    if let Some(operation) =
        blocked_provider_profile_operation(&request.method, request_path, provider_profile)
    {
        return Err(decode_error(format!(
            "provider profile '{}' does not support '{}' operations",
            provider_profile.label(),
            operation,
        )));
    }

    if request_path == WORKITEM_COLLECTION_PATH || request_path.starts_with(WORKITEM_ITEM_PREFIX) {
        return handle_workitem_routes(request_path, request, state, limits, actor);
    }
    if request_path == INTEROP_IAN_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_ian_list(request, state),
            "POST" => handle_ian_ingest(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == INTEROP_STORAGE_COMMITMENT_STATUS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_storage_commitment_status_list(request, state),
            "POST" => handle_storage_commitment_status_upsert(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some(transaction_uid) =
        request_path.strip_prefix(INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX)
    {
        let transaction_uid = normalize_route_identifier(transaction_uid)?;
        if transaction_uid.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        return match request.method.as_str() {
            "GET" | "HEAD" => {
                handle_storage_commitment_status_get(&transaction_uid, request, state)
            }
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == INTEROP_HL7_UPS_CORRELATION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_hl7_ups_correlation_list(request, state),
            "POST" => handle_hl7_ups_correlation_upsert(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if request_path == task::TASK_COLLECTION_PATH
        || request_path.starts_with(task::TASK_ITEM_PREFIX)
    {
        if request_path == task::TASK_COLLECTION_PATH {
            return match request.method.as_str() {
                "GET" | "HEAD" => handle_task_list(request, state, limits),
                "POST" => handle_task_create(request, state, limits),
                _ => Err(decode_error("unsupported route")),
            };
        }
        let rest = request_path.trim_start_matches(task::TASK_ITEM_PREFIX);
        if rest.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((task_id, suffix)) = rest.split_once('/') {
            if suffix.is_empty() {
                return Err(decode_error("unsupported route"));
            }
            let task_id = normalize_route_identifier(task_id)?;
            return match suffix {
                "start" | "pause" | "resume" | "complete" | "review" | "commit" | "cancel" => {
                    handle_task_control(&task_id, suffix, request, state)
                }
                _ => Err(decode_error("unsupported route")),
            };
        }
        let task_id = normalize_route_identifier(rest)?;
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_task_get(&task_id, request, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if let Some(rest) = request_path.strip_prefix(mpps::MPPS_ITEM_PREFIX) {
        if rest.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((sop_uid, suffix)) = rest.split_once('/') {
            let sop_uid = normalize_route_identifier(sop_uid)?;
            if suffix != "status" {
                return Err(decode_error("unsupported route"));
            }
            return match request.method.as_str() {
                "GET" | "HEAD" => handle_mpps_get_status(&sop_uid, actor, state),
                "POST" => handle_mpps_update_status(&sop_uid, request, actor, state),
                _ => Err(decode_error("unsupported route")),
            };
        }
        let sop_uid = normalize_route_identifier(rest)?;
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_mpps_get_single(&sop_uid, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    if request_path == sr::SR_COLLECTION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_sr_list(request, actor, state, limits),
            "POST" => handle_sr_create(request, actor, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some(interop_route) = interop::classify_interop_route(request_path) {
        return match interop_route {
            interop::InteropRoute::Hl7Failures => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_failures(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::Hl7Ingest => match request.method.as_str() {
                "POST" => handle_hl7_ingest(request, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorStatus => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_status_dashboard(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorFeatures => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_features(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorRollout => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_rollout_list(state),
                "POST" => handle_hl7_connector_rollout_update(request, actor, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorCapabilities => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_capabilities(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ConnectorHealth => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_connector_health(state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::Subscriptions => match request.method.as_str() {
                "GET" | "HEAD" => handle_hl7_subscriptions_list(state),
                "POST" => handle_hl7_subscriptions_create(request, state),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::FhirIngest => match request.method.as_str() {
                "POST" => handle_fhir_ingest(request, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ReconciliationJobsCollection => match request.method.as_str() {
                "GET" | "HEAD" => handle_reconciliation_jobs_list(request, actor, state),
                "POST" => handle_reconciliation_jobs_create(request, actor, state, limits),
                _ => Err(decode_error("unsupported route")),
            },
            interop::InteropRoute::ReconciliationJobRun { job_id } => match request.method.as_str()
            {
                "POST" => handle_reconciliation_jobs_run(job_id, request, actor, state),
                _ => Err(decode_error("unsupported route")),
            },
        };
    }
    if let Some(sop_uid) = request_path.strip_prefix(sr::SR_ITEM_PREFIX) {
        if sop_uid.is_empty() {
            return Err(decode_error("unsupported route"));
        }
        if let Some((sop_uid, suffix)) = sop_uid.split_once('/') {
            let sop_uid = normalize_route_identifier(sop_uid)?;
            if suffix.is_empty() || suffix.contains('/') {
                return Err(decode_error("unsupported route"));
            }
            return match (request.method.as_str(), suffix) {
                ("POST", "updates") => handle_sr_update(&sop_uid, request, actor, state, limits),
                ("GET" | "HEAD", "history") => handle_sr_history(&sop_uid, actor, state),
                ("POST", "review") => {
                    handle_sr_transition("review", &sop_uid, request, actor, state)
                }
                ("POST", "finalize") => {
                    handle_sr_transition("finalize", &sop_uid, request, actor, state)
                }
                ("POST", "commit") => {
                    handle_sr_transition("commit", &sop_uid, request, actor, state)
                }
                ("POST", "cancel") => {
                    handle_sr_transition("cancel", &sop_uid, request, actor, state)
                }
                _ => Err(decode_error("unsupported route")),
            };
        }
        let sop_uid = normalize_route_identifier(sop_uid)?;
        if sop_uid.contains('/') {
            return Err(decode_error("unsupported route"));
        }
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_sr_get(&sop_uid, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_tenant_quotas(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_SNAPSHOT_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_tenant_quota_snapshot(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_METRICS_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_metrics(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if request_path == tenant_policy_domain::WORKFLOW_AUDIT_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workflow_audit(request, actor, state),
            _ => Err(decode_error("unsupported route")),
        };
    }

    match (request.method.as_str(), request_path) {
        ("GET", "/healthz") | ("HEAD", "/healthz") => Ok(WorkflowResponse::Json(
            200,
            "{\"status\":\"ok\"}".to_string(),
        )),
        ("GET", "/readyz") | ("HEAD", "/readyz") => Ok(WorkflowResponse::Json(
            200,
            "{\"status\":\"ready\"}".to_string(),
        )),
        ("GET", "/worklist/items") | ("HEAD", "/worklist/items") => {
            handle_worklist_query(request, state, limits)
        }
        ("POST", "/worklist/items") => handle_worklist_upsert(request, state, limits),
        ("GET", mpps::MPPS_COLLECTION_PATH) | ("HEAD", mpps::MPPS_COLLECTION_PATH) => {
            handle_mpps_get(request, actor, state, limits)
        }
        ("POST", mpps::MPPS_COLLECTION_PATH) => handle_mpps_ingest(request, actor, state, limits),
        _ => Err(decode_error("unsupported route")),
    }
}

fn workflow_contract_for_request(
    method: &str,
    path: &str,
) -> Option<dicom_workflow_server::WorkflowRouteContract> {
    use dicom_workflow_server::workflow_route_contract;
    let method = method.to_ascii_uppercase();
    for contract in workflow_route_contract() {
        if !method_matches(method.as_str(), contract.method) {
            continue;
        }
        if route_path_matches_contract(path, contract.path_template) {
            return Some(*contract);
        }
    }
    None
}

fn method_matches(method: &str, route_methods: &str) -> bool {
    route_methods
        .split('|')
        .any(|candidate| candidate.eq_ignore_ascii_case(method))
}

fn route_path_matches_contract(path: &str, template: &str) -> bool {
    let path_parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let template_parts: Vec<&str> = template
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    if path_parts.len() != template_parts.len() {
        return false;
    }
    path_parts
        .iter()
        .zip(template_parts.iter())
        .all(|(path_part, template_part)| {
            if template_part.starts_with('{') && template_part.ends_with('}') {
                return true;
            }
            path_part == template_part
        })
}

fn is_route_denied(templates: &[String], path: &str) -> bool {
    templates
        .iter()
        .any(|template| route_path_matches_contract(path, template))
}

fn render_route_policy_enforcement_failure_log(
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    policy: &str,
    detail: &str,
) -> String {
    format!(
        "{{\"event\":\"workflow_route_policy_enforcement_failure\",\"tenant\":\"{}\",\"route\":\"{}\",\"policy\":\"{}\",\"correlation_id\":\"{}\",\"detail\":\"{}\"}}",
        escape_json(&actor.tenant),
        escape_json(contract.path_template),
        escape_json(policy),
        escape_json(&request_id_from_headers(&request.headers)),
        escape_json(detail),
    )
}

fn log_route_policy_enforcement_failure(
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    policy: &str,
    detail: &str,
) {
    eprintln!(
        "{}",
        render_route_policy_enforcement_failure_log(actor, request, contract, policy, detail)
    );
}

fn enforce_route_policy(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    contract: &dicom_workflow_server::WorkflowRouteContract,
    request_path: &str,
) -> Result<bool, Box<Error>> {
    if is_route_denied(&state.denylist_routes, request_path) {
        log_route_policy_enforcement_failure(
            actor,
            request,
            contract,
            "denylist_route",
            "request denied by workflow operation deny-list",
        );
        return Err(auth_denied_error(
            "request denied by workflow operation deny-list",
        ));
    }

    let route = contract;
    if route.requires_writer_role && !actor.can_write() {
        log_route_policy_enforcement_failure(
            actor,
            request,
            contract,
            "writer_role_required",
            "writer role required",
        );
        return Err(auth_denied_error("writer role required"));
    }
    if route.requires_idempotency_key {
        let key = request
            .headers
            .get("x-idempotency-key")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty());
        if key.is_none() {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "idempotency_key_required",
                "missing required x-idempotency-key header",
            );
            return Err(decode_error("missing required x-idempotency-key header"));
        }
    }

    let policy = tenant_policy(state, &actor.tenant);
    if request.method != "GET" && request.method != "HEAD" {
        if request.body.len() as u64 > policy.upload_cap_bytes {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "upload_cap_bytes",
                &format!(
                    "observed body {} exceeds upload cap {}",
                    request.body.len(),
                    policy.upload_cap_bytes
                ),
            );
            return Err(limit_exceeded(
                "workflow_upload_cap_bytes",
                request.body.len() as u64,
                policy.upload_cap_bytes,
            ));
        }
    }

    let anomaly = if request.method == "GET" || request.method == "HEAD" {
        enforce_rate_limit(state, actor, request, request_path, policy.query_rate_limit).map_err(
            |err| {
                log_route_policy_enforcement_failure(
                    actor,
                    request,
                    contract,
                    "query_rate_limit",
                    &err.message.clone(),
                );
                err
            },
        )?
    } else {
        enforce_rate_limit(
            state,
            actor,
            request,
            request_path,
            policy.mutation_rate_limit,
        )
        .map_err(|err| {
            log_route_policy_enforcement_failure(
                actor,
                request,
                contract,
                "mutation_rate_limit",
                &err.message.clone(),
            );
            err
        })?
    };
    Ok(anomaly)
}

fn enforce_rate_limit(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    request_path: &str,
    limit: u64,
) -> Result<bool, Box<Error>> {
    if limit == 0 {
        return Ok(false);
    }
    let route = route_request_scope_path(request_path, request.method.as_str());
    let key = format!(
        "{}|{}|{}|{}",
        actor.tenant,
        actor
            .principal
            .clone()
            .unwrap_or_else(|| "anonymous".to_string()),
        request.method,
        route
    );
    let now_ms = now_epoch_millis();
    let window_ms = state.audit_rate_window_ms;
    let bucket = state
        .rate_windows
        .entry(key)
        .or_insert_with(RequestWindow::empty);
    if bucket.window_start_ms == 0 || now_ms.saturating_sub(bucket.window_start_ms) >= window_ms {
        bucket.window_start_ms = now_ms;
        bucket.count = 0;
    }
    bucket.count = bucket.count.saturating_add(1);
    let anomaly = if state.anomaly_alert_threshold > 0 {
        bucket.count >= state.anomaly_alert_threshold
    } else {
        false
    };
    if bucket.count > limit {
        return Err(Error::from_kind(
            ErrorKind::LimitExceeded {
                limit_name: "workflow_rate_limit",
                observed: bucket.count,
                allowed: limit,
            },
            "rate limit exceeded",
        )
        .into());
    }
    Ok(anomaly && bucket.count < limit)
}

fn route_request_scope_path(path: &str, method: &str) -> &'static str {
    match (method, path) {
        ("GET", "/healthz") | ("HEAD", "/healthz") => "healthz",
        ("POST", "/worklist/items") | ("GET", "/worklist/items") | ("HEAD", "/worklist/items") => {
            "worklist/items"
        }
        ("POST", mpps::MPPS_COLLECTION_PATH)
        | ("GET", mpps::MPPS_COLLECTION_PATH)
        | ("HEAD", mpps::MPPS_COLLECTION_PATH) => "mpps/updates",
        (method, path) if path.starts_with(mpps::MPPS_ITEM_PREFIX) => match method {
            "POST" => "mpps/updates/{sop}/status",
            _ => "mpps/updates/{sop}",
        },
        (_, path) if path.starts_with(sr::SR_ITEM_PREFIX) => {
            if path.ends_with("/updates") {
                "sr/documents/{sop}/updates"
            } else if path.ends_with("/history") {
                "sr/documents/{sop}/history"
            } else if path.ends_with("/review") {
                "sr/documents/{sop}/review"
            } else if path.ends_with("/finalize") {
                "sr/documents/{sop}/finalize"
            } else if path.ends_with("/commit") {
                "sr/documents/{sop}/commit"
            } else if path.ends_with("/cancel") {
                "sr/documents/{sop}/cancel"
            } else {
                "sr/documents/{sop}"
            }
        }
        (method, sr::SR_COLLECTION_PATH) => {
            if method == "POST" {
                "sr/documents"
            } else {
                "sr/documents"
            }
        }
        (method, WORKITEM_COLLECTION_PATH) => {
            if method == "POST" {
                "workflow/workitems"
            } else {
                "workflow/workitems"
            }
        }
        (_, path) if path.starts_with(task::TASK_ITEM_PREFIX) => {
            if path == task::TASK_COLLECTION_PATH {
                "workflow/tasks"
            } else if path.ends_with("/start") {
                "workflow/tasks/{task_id}/start"
            } else if path.ends_with("/pause") {
                "workflow/tasks/{task_id}/pause"
            } else if path.ends_with("/resume") {
                "workflow/tasks/{task_id}/resume"
            } else if path.ends_with("/complete") {
                "workflow/tasks/{task_id}/complete"
            } else if path.ends_with("/review") {
                "workflow/tasks/{task_id}/review"
            } else if path.ends_with("/commit") {
                "workflow/tasks/{task_id}/commit"
            } else if path.ends_with("/cancel") {
                "workflow/tasks/{task_id}/cancel"
            } else {
                "workflow/tasks/{task_id}"
            }
        }
        (_, path) if path.starts_with(WORKITEM_ITEM_PREFIX) => {
            if path == WORKITEM_COLLECTION_PATH {
                "workflow/workitems"
            } else if path.ends_with("/state") {
                "workflow/workitems/{task_id}/state"
            } else if path.ends_with("/cancel") {
                "workflow/workitems/{task_id}/cancel"
            } else if path.ends_with("/search") {
                "workflow/workitems/search"
            } else {
                "workflow/workitems/{task_id}"
            }
        }
        (method, INTEROP_IAN_PATH) => {
            if method == "POST" {
                "interop/ian"
            } else {
                "interop/ian"
            }
        }
        (method, INTEROP_STORAGE_COMMITMENT_STATUS_PATH) => {
            if method == "POST" {
                "interop/storage-commitment/status"
            } else {
                "interop/storage-commitment/status"
            }
        }
        (_, path) if path.starts_with(INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX) => {
            "interop/storage-commitment/status/{task_id}"
        }
        (method, INTEROP_HL7_UPS_CORRELATION_PATH) => {
            if method == "POST" {
                "interop/hl7/ups-correlation"
            } else {
                "interop/hl7/ups-correlation"
            }
        }
        ("POST", interop::INTEROP_HL7_PATH) => "interop/hl7",
        (method, interop::INTEROP_HL7_FAILURES_PATH) if method == "GET" || method == "HEAD" => {
            "interop/hl7/failures"
        }
        (method, interop::INTEROP_CONNECTORS_STATUS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/status"
        }
        (method, interop::INTEROP_CONNECTORS_FEATURES_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/features"
        }
        (method, interop::INTEROP_CONNECTORS_ROLLOUT_PATH) => {
            if method == "POST" {
                "interop/connectors/rollout/update"
            } else {
                "interop/connectors/rollout"
            }
        }
        (method, interop::INTEROP_CONNECTORS_CAPABILITIES_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/capabilities"
        }
        (method, interop::INTEROP_CONNECTORS_HEALTH_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "interop/connectors/health"
        }
        (method, interop::INTEROP_SUBSCRIPTIONS_PATH) => {
            if method == "POST" {
                "interop/subscriptions"
            } else {
                "interop/subscriptions"
            }
        }
        ("POST", interop::INTEROP_FHIR_PATH) => "interop/fhir",
        (_, path) if path.starts_with(interop::INTEROP_RECONCILIATION_JOBS_PREFIX) => {
            if path.ends_with("/run") {
                "interop/reconciliation/jobs/{job_id}/run"
            } else if path == interop::INTEROP_RECONCILIATION_JOBS_PREFIX {
                "interop/reconciliation/jobs"
            } else {
                "interop/reconciliation/jobs/{job_id}/run"
            }
        }
        (method, tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/policy/quotas"
        }
        (method, tenant_policy_domain::WORKFLOW_TENANT_QUOTAS_SNAPSHOT_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/policy/quotas/snapshot"
        }
        (method, tenant_policy_domain::WORKFLOW_METRICS_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/metrics"
        }
        (method, tenant_policy_domain::WORKFLOW_AUDIT_PATH)
            if method == "GET" || method == "HEAD" =>
        {
            "workflow/audit"
        }
        _ => "unknown",
    }
}

fn store_append_workflow_audit(
    state: &mut RuntimeState,
    actor: &WorkflowActorContext,
    request: &HttpRequest,
    operation: &str,
    route: &str,
    status: u16,
    outcome: &str,
    anomaly: bool,
) {
    let _ = outcome;
    let event = WorkflowAuditEvent {
        ts_ms: now_epoch_millis(),
        tenant: actor.tenant.clone(),
        principal_hash: hash_text(actor.principal.as_deref().unwrap_or("anonymous")),
        request_id_hash: hash_text(&request_id_from_headers(&request.headers)),
        previous_audit_hash: String::new(),
        audit_hash: String::new(),
        route: route.to_string(),
        method: request.method.clone(),
        operation: operation.to_string(),
        outcome: outcome.to_string(),
        scope: request
            .query
            .get("scope")
            .cloned()
            .unwrap_or_else(|| "default".to_string()),
        status,
        anomaly,
    };
    let _ = append_workflow_audit_event(
        &state.audit_path,
        state.audit_max_bytes,
        state.audit_max_rotated_files,
        state.audit_export_limit,
        &event,
    );
}

fn append_workflow_audit_event(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    _export_limit: usize,
    event: &WorkflowAuditEvent,
) {
    if max_bytes == 0 {
        return;
    }
    let _ = rotate_audit_if_needed(path, max_bytes, max_rotated_files);
    let previous_audit_hash = read_workflow_audit_previous_hash(path);
    let event = WorkflowAuditEvent {
        previous_audit_hash: previous_audit_hash.clone(),
        audit_hash: workflow_audit_line_hash(&previous_audit_hash, &event),
        ..event.clone()
    };
    let line = format!(
        "{{\"ts_ms\":{},\"tenant\":\"{}\",\"principal_hash\":\"{}\",\"request_id_hash\":\"{}\",\"route\":\"{}\",\"method\":\"{}\",\"operation\":\"{}\",\"outcome\":\"{}\",\"scope\":\"{}\",\"status\":{},\"anomaly\":{},\"previous_audit_hash\":\"{}\",\"audit_hash\":\"{}\"}}\n",
        event.ts_ms,
        escape_json(&event.tenant),
        escape_json(&event.principal_hash),
        escape_json(&event.request_id_hash),
        escape_json(&event.route),
        escape_json(&event.method),
        escape_json(&event.operation),
        escape_json(&event.outcome),
        escape_json(&event.scope),
        event.status,
        if event.anomaly { "true" } else { "false" },
        escape_json(&event.previous_audit_hash),
        escape_json(&event.audit_hash),
    );
    if let Ok(mut audit_file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = audit_file.write_all(line.as_bytes());
        let _ = audit_file.sync_all();
    }
}

fn workflow_audit_line_hash(previous_audit_hash: &str, event: &WorkflowAuditEvent) -> String {
    let anomaly = if event.anomaly { "true" } else { "false" };
    hash_text(&format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        event.ts_ms,
        previous_audit_hash,
        event.tenant,
        event.principal_hash,
        event.request_id_hash,
        event.route,
        event.method,
        event.operation,
        event.outcome,
        event.scope,
        event.status,
        anomaly,
    ))
}

fn read_workflow_audit_previous_hash(path: &str) -> String {
    let content = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return String::new(),
    };
    content
        .lines()
        .rev()
        .find_map(|line| parse_workflow_audit_json_field(line, "audit_hash"))
        .unwrap_or_default()
}

fn parse_workflow_audit_json_field(line: &str, key: &str) -> Option<String> {
    let token = format!("\\\"{}\\\":\\\"", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let end = rest.find('"')?;
    Some(rest.get(0..end)?.to_string())
}

fn parse_workflow_audit_json_u64_field(line: &str, key: &str) -> Option<u64> {
    let token = format!("\\\"{}\\\":", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let trimmed = rest.trim_start();
    let mut len = 0usize;
    for byte in trimmed.bytes() {
        if !byte.is_ascii_digit() {
            break;
        }
        len = len.saturating_add(1);
    }
    if len == 0 {
        return None;
    }
    trimmed.get(0..len)?.parse().ok()
}

fn parse_workflow_audit_json_bool_field(line: &str, key: &str) -> Option<bool> {
    let token = format!("\"{}\":", key);
    let start = line.find(&token)?;
    let start = start + token.len();
    let rest = line.get(start..)?;
    let trimmed = rest.trim_start();
    if trimmed.starts_with("true") {
        Some(true)
    } else if trimmed.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_workflow_audit_line(line: &str) -> Option<WorkflowAuditEvent> {
    let ts_ms = parse_workflow_audit_json_u64_field(line, "ts_ms")?;
    let status = parse_workflow_audit_json_u64_field(line, "status")?;
    let anomaly = parse_workflow_audit_json_bool_field(line, "anomaly")?;
    if status > u16::MAX as u64 {
        return None;
    }
    Some(WorkflowAuditEvent {
        ts_ms,
        tenant: parse_workflow_audit_json_field(line, "tenant")?,
        principal_hash: parse_workflow_audit_json_field(line, "principal_hash")?,
        request_id_hash: parse_workflow_audit_json_field(line, "request_id_hash")?,
        previous_audit_hash: parse_workflow_audit_json_field(line, "previous_audit_hash")?,
        audit_hash: parse_workflow_audit_json_field(line, "audit_hash")?,
        route: parse_workflow_audit_json_field(line, "route")?,
        method: parse_workflow_audit_json_field(line, "method")?,
        operation: parse_workflow_audit_json_field(line, "operation")?,
        outcome: parse_workflow_audit_json_field(line, "outcome")?,
        scope: parse_workflow_audit_json_field(line, "scope")?,
        status: status as u16,
        anomaly,
    })
}

#[derive(Debug, Clone)]
struct WorkflowAuditVerificationFailure {
    line: usize,
    detail: String,
}

fn verify_workflow_audit_chain(lines: &[String]) -> Vec<WorkflowAuditVerificationFailure> {
    let mut failures = Vec::new();
    let mut previous_audit_hash = String::new();
    for (index, line) in lines.iter().enumerate() {
        let line_no = index + 1;
        let event = match parse_workflow_audit_line(line) {
            Some(event) => event,
            None => {
                failures.push(WorkflowAuditVerificationFailure {
                    line: line_no,
                    detail: "parse failure for workflow audit line".to_string(),
                });
                previous_audit_hash.clear();
                continue;
            }
        };
        if event.previous_audit_hash != previous_audit_hash {
            failures.push(WorkflowAuditVerificationFailure {
                line: line_no,
                detail: format!(
                    "previous hash mismatch at line {line_no}: expected {previous_audit_hash}, got {}",
                    event.previous_audit_hash
                ),
            });
        }
        let expected_hash = workflow_audit_line_hash(&previous_audit_hash, &event);
        if event.audit_hash != expected_hash {
            failures.push(WorkflowAuditVerificationFailure {
                line: line_no,
                detail: format!("audit hash mismatch at line {line_no}"),
            });
        }
        previous_audit_hash = event.audit_hash;
    }
    failures
}

fn rotate_audit_if_needed(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
) -> std::io::Result<()> {
    let path = Path::new(path);
    let Some(parent) = path.parent() else {
        return Err(IoError::new(
            IoErrorKind::InvalidInput,
            "workflow audit path must include a file parent",
        ));
    };
    std::fs::create_dir_all(parent)?;
    let metadata = fs::metadata(path);
    if let Ok(meta) = metadata {
        if meta.len() > max_bytes {
            rotate_file(path, max_rotated_files)?;
        }
    }
    Ok(())
}

fn render_workflow_audit_payload(lines: &[String]) -> String {
    let mut out = String::from("[");
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(line);
    }
    out.push(']');
    out
}

fn render_workflow_audit_payload_with_verification(
    lines: &[String],
    failures: &[WorkflowAuditVerificationFailure],
) -> String {
    let mut violation_json = String::new();
    for (index, failure) in failures.iter().enumerate() {
        if index > 0 {
            violation_json.push(',');
        }
        violation_json.push_str(&format!(
            "{{\"line\":{},\"detail\":\"{}\"}}",
            failure.line,
            escape_json(&failure.detail),
        ));
    }
    format!(
        "{{\"verification\":{{\"ok\":{},\"failures\":[{}]}},\"events\":{}}}",
        failures.is_empty(),
        violation_json,
        render_workflow_audit_payload(lines),
    )
}

fn render_tenant_quota_override_json(override_limits: Option<TenantQuotaOverride>) -> String {
    match override_limits {
        Some(override_limits) => format!(
            "{{\"task_quota\":{},\"subscription_quota\":{}}}",
            override_limits.task_quota, override_limits.subscription_quota
        ),
        None => "null".to_string(),
    }
}

fn render_tenant_rate_override_json(override_limits: Option<TenantRateLimitOverride>) -> String {
    match override_limits {
        Some(override_limits) => format!(
            "{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes
        ),
        None => "null".to_string(),
    }
}

fn handle_workflow_tenant_quotas(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    let tenant_filter = request
        .query
        .get("tenant")
        .cloned()
        .unwrap_or_else(|| actor.tenant.clone());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant_filter != actor.tenant && tenant_filter != "*" && tenant_filter != "all"
    {
        return Err(auth_denied_error(
            "quota tenant scope outside caller tenant",
        ));
    }

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if tenant_filter == "*" || tenant_filter == "all" {
        if !is_admin {
            return Err(auth_denied_error(
                "quota tenant scope outside caller tenant",
            ));
        }
        let overrides = collect_tenant_quota_overrides_from_env();
        let rate_overrides = tenant_rate_limit_overrides_snapshot();
        let mut tenants = Vec::new();
        for (tenant_key, override_limits) in overrides {
            tenants.push(format!(
                "\"{}\":{{\"task_quota\":{},\"subscription_quota\":{}}}",
                escape_json(&tenant_key),
                override_limits.task_quota,
                override_limits.subscription_quota
            ));
        }
        let mut rate_rows = Vec::new();
        for (tenant_key, override_limits) in rate_overrides {
            rate_rows.push(format!(
                "\"{}\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
                escape_json(&tenant_key),
                override_limits.query_rate_limit,
                override_limits.mutation_rate_limit,
                override_limits.upload_cap_bytes
            ));
        }
        return Ok(WorkflowResponse::Json(
            200,
            format!(
                "{{\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"overrides\":{{{}}},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_overrides\":{{{}}}}}",
                DEFAULT_TENANT_TASK_QUOTA,
                DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
                tenants.join(","),
                store.query_rate_limit,
                store.mutation_rate_limit,
                store.upload_cap_bytes,
                rate_rows.join(","),
            ),
        ));
    }

    let effective = tenant_policy(&store, &tenant_filter);
    let override_limits = tenant_quota_override_for_tenant(&tenant_filter);
    let rate_override = tenant_rate_limit_override_for_tenant(&tenant_filter);
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"tenant\":\"{}\",\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"effective\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"override\":{},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_effective\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_override\":{}}}",
            escape_json(&tenant_filter),
            DEFAULT_TENANT_TASK_QUOTA,
            DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
            effective.task_quota,
            effective.subscription_quota,
            render_tenant_quota_override_json(override_limits),
            store.query_rate_limit,
            store.mutation_rate_limit,
            store.upload_cap_bytes,
            effective.query_rate_limit,
            effective.mutation_rate_limit,
            effective.upload_cap_bytes,
            render_tenant_rate_override_json(rate_override),
        ),
    ))
}

fn handle_workflow_tenant_quota_snapshot(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    if actor.role.as_deref() != Some("admin") {
        return Err(auth_denied_error("quota snapshot requires admin role"));
    }
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let overrides = collect_tenant_quota_overrides_from_env();
    let rate_overrides = tenant_rate_limit_overrides_snapshot();
    let mut rows = Vec::new();
    for (tenant_key, override_limits) in overrides {
        rows.push(format!(
            "\"{}\":{{\"task_quota\":{},\"subscription_quota\":{}}}",
            escape_json(&tenant_key),
            override_limits.task_quota,
            override_limits.subscription_quota,
        ));
    }
    let mut rate_rows = Vec::new();
    for (tenant_key, override_limits) in rate_overrides {
        rate_rows.push(format!(
            "\"{}\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}}",
            escape_json(&tenant_key),
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"defaults\":{{\"task_quota\":{},\"subscription_quota\":{}}},\"overrides\":{{{}}},\"rate_defaults\":{{\"query_rate_limit\":{},\"mutation_rate_limit\":{},\"upload_cap_bytes\":{}}},\"rate_overrides\":{{{}}}}}",
            now_epoch_millis(),
            DEFAULT_TENANT_TASK_QUOTA,
            DEFAULT_TENANT_SUBSCRIPTION_QUOTA,
            rows.join(","),
            store.query_rate_limit,
            store.mutation_rate_limit,
            store.upload_cap_bytes,
            rate_rows.join(","),
        ),
    ))
}

fn handle_workflow_metrics(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let tenant_filter = request
        .query
        .get("tenant")
        .cloned()
        .unwrap_or_else(|| actor.tenant.clone());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant_filter != actor.tenant && tenant_filter != "*" && tenant_filter != "all"
    {
        return Err(auth_denied_error(
            "metrics tenant scope outside caller tenant",
        ));
    }

    let metrics_snapshot = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let anomaly_alert_threshold = store.anomaly_alert_threshold;
        if tenant_filter == "*" || tenant_filter == "all" {
            let tenants: Vec<_> = store
                .metrics
                .iter()
                .map(|(tenant, counters)| {
                    format!(
                        "\"{}\":{{\"read_operations\":{},\"query_operations\":{},\"mutation_operations\":{},\"anomaly_operations\":{},\"denied_operations\":{},\"alerts\":{}}}",
                        escape_json(tenant),
                        counters.read_operations,
                        counters.query_operations,
                        counters.mutation_operations,
                        counters.anomaly_operations,
                        counters.denied_operations,
                        render_tenant_alerts_json(counters, anomaly_alert_threshold),
                        )
                })
                .collect();
            format!(
                "{{\"tenants\":{{{}}},\"anomaly_alert_threshold\":{}}}",
                tenants.join(","),
                anomaly_alert_threshold,
            )
        } else {
            let counters = store.metrics.get(&tenant_filter);
            let counters = counters.cloned().unwrap_or_default();
            format!(
                "{{\"tenant\":\"{}\",\"read_operations\":{},\"query_operations\":{},\"mutation_operations\":{},\"anomaly_operations\":{},\"denied_operations\":{},\"anomaly_alert_threshold\":{},\"alerts\":{}}}",
                escape_json(&tenant_filter),
                counters.read_operations,
                counters.query_operations,
                counters.mutation_operations,
                counters.anomaly_operations,
                counters.denied_operations,
                anomaly_alert_threshold,
                render_tenant_alerts_json(&counters, anomaly_alert_threshold),
            )
        }
    };
    Ok(WorkflowResponse::Json(200, metrics_snapshot))
}

fn render_tenant_alerts_json(
    counters: &TenantOperationMetrics,
    anomaly_alert_threshold: u64,
) -> String {
    let mut alerts = Vec::new();
    if anomaly_alert_threshold > 0 && counters.anomaly_operations >= anomaly_alert_threshold {
        alerts.push(
            "{\"type\":\"rate_burst\",\"severity\":\"warn\",\"observed\":\"true\",\"unit\":\"requests\"}"
                .to_string(),
        );
    }
    format!("[{}]", alerts.join(","))
}

fn handle_workflow_audit(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    if !actor.can_write() {
        return Err(auth_denied_error("writer role required"));
    }
    let tenant = request
        .query
        .get("tenant")
        .map(|value| value.as_str())
        .unwrap_or(actor.tenant.as_str());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant != "*" && tenant != "all" && tenant != actor.tenant {
        return Err(auth_denied_error(
            "audit tenant scope outside caller tenant",
        ));
    }

    let verify = request
        .query
        .get("verify")
        .is_some_and(|value| matches!(value.as_str(), "true" | "1"));
    let limit = request
        .query
        .get("limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
        .min({
            let store = state
                .lock()
                .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
            store.audit_export_limit
        });
    let path = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        store.audit_path.clone()
    };
    let all_lines: Vec<String> = fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(std::string::ToString::to_string)
        .collect();
    let failures = if verify {
        Some(verify_workflow_audit_chain(&all_lines))
    } else {
        None
    };

    let mut lines: Vec<String> = all_lines;
    lines.reverse();
    if tenant != "*" && tenant != "all" {
        lines = lines
            .into_iter()
            .filter(|entry| {
                let token = format!("\"tenant\":\"{}\"", escape_json(tenant));
                entry.contains(&token)
            })
            .collect();
    }
    let lines: Vec<String> = lines.into_iter().take(limit).collect();
    if let Some(failures) = failures {
        Ok(WorkflowResponse::Json(
            200,
            render_workflow_audit_payload_with_verification(&lines, &failures),
        ))
    } else {
        Ok(WorkflowResponse::Json(
            200,
            render_workflow_audit_payload(&lines),
        ))
    }
}

fn now_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |delta| delta.as_millis() as u64)
}

fn task_status_from_label(label: &str, context: &str) -> Result<TaskStatus, Box<Error>> {
    match label.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "SCHEDULED" => Ok(TaskStatus::Scheduled),
        "IN PROGRESS" | "INPROGRESS" => Ok(TaskStatus::InProgress),
        "ON HOLD" | "ONHOLD" | "HOLD" => Ok(TaskStatus::OnHold),
        "COMPLETED" => Ok(TaskStatus::Completed),
        "REVIEWED" => Ok(TaskStatus::Reviewed),
        "COMMITTED" => Ok(TaskStatus::Committed),
        "DISCONTINUED" => Ok(TaskStatus::Discontinued),
        _ => Err(decode_error(context)),
    }
}

impl TaskStatus {
    fn as_label(self) -> &'static str {
        match self {
            TaskStatus::Scheduled => "SCHEDULED",
            TaskStatus::InProgress => "IN PROGRESS",
            TaskStatus::OnHold => "ON HOLD",
            TaskStatus::Completed => "COMPLETED",
            TaskStatus::Reviewed => "REVIEWED",
            TaskStatus::Committed => "COMMITTED",
            TaskStatus::Discontinued => "DISCONTINUED",
        }
    }

    fn can_transition_to(self, next: TaskStatus) -> bool {
        match (self, next) {
            (TaskStatus::Scheduled, TaskStatus::InProgress)
            | (TaskStatus::Scheduled, TaskStatus::Discontinued) => true,
            (TaskStatus::InProgress, TaskStatus::Completed)
            | (TaskStatus::InProgress, TaskStatus::OnHold)
            | (TaskStatus::InProgress, TaskStatus::Discontinued) => true,
            (TaskStatus::OnHold, TaskStatus::InProgress)
            | (TaskStatus::OnHold, TaskStatus::Discontinued) => true,
            (TaskStatus::Completed, TaskStatus::Reviewed)
            | (TaskStatus::Completed, TaskStatus::Discontinued) => true,
            (TaskStatus::Reviewed, TaskStatus::Committed) => true,
            (TaskStatus::Completed, TaskStatus::Completed)
            | (TaskStatus::Reviewed, TaskStatus::Reviewed)
            | (TaskStatus::Committed, TaskStatus::Committed)
            | (TaskStatus::Discontinued, TaskStatus::Discontinued) => true,
            _ => false,
        }
    }
}

fn parse_task_status_filter(raw: Option<&String>) -> Result<Option<TaskStatus>, Box<Error>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    Ok(Some(task_status_from_label(
        raw,
        "invalid task status filter",
    )?))
}

fn parse_query_param_language(
    raw_query: &str,
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    if raw_query.is_empty() {
        return Ok(BTreeMap::new());
    }
    parse_query_map(raw_query, limits)
        .map_err(decode_error)
        .map(|query| {
            let mut query = query;
            query.remove("query");
            query
        })
}

fn merge_query_parameters(
    raw_params: &BTreeMap<String, String>,
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    let mut query = if let Some(raw_query) = raw_params.get("query") {
        parse_query_param_language(raw_query, limits)?
    } else {
        BTreeMap::new()
    };
    for (key, value) in raw_params {
        if key == "query" {
            continue;
        }
        query.insert(key.clone(), value.clone());
    }
    Ok(query)
}

fn task_not_found_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.TASK.NOT_FOUND",
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-task".to_string(),
            detail: "task not found".to_string(),
        },
        "task not found",
    )
    .into()
}

fn task_status_invalid_transition_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.TASK.INVALID_TRANSITION",
        ErrorKind::IntegrityError {
            detail: "invalid task status transition".to_string(),
        },
        "task status transition not allowed",
    )
    .into()
}

fn task_idempotency_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_TASK_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

fn hl7_replay_cache_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

fn hl7_callback_idempotency_limit(cache: &mut BTreeMap<String, u64>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

fn callback_idempotency_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.callback-idempotency")
}

fn hl7_callback_idempotency_prune(cache: &mut BTreeMap<String, u64>, now_ms: u64, ttl_ms: u64) {
    if ttl_ms > 0 {
        cache.retain(|_, observed_ms| now_ms.saturating_sub(*observed_ms) <= ttl_ms);
    }
    hl7_callback_idempotency_limit(cache);
}

fn load_hl7_callback_idempotency_cache(
    path: &str,
    now_ms: u64,
    ttl_ms: u64,
) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((timestamp_raw, key_raw)) = trimmed.split_once('\t') else {
            continue;
        };
        let Ok(observed_ms) = timestamp_raw.parse::<u64>() else {
            continue;
        };
        let key = key_raw.trim().to_string();
        if key.is_empty() {
            continue;
        }
        let _ = out.insert(key, observed_ms);
    }
    hl7_callback_idempotency_prune(&mut out, now_ms, ttl_ms);
    out
}

fn persist_hl7_callback_idempotency_cache(path: &str, cache: &BTreeMap<String, u64>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (key, observed_ms) in cache {
        lines.push_str(&format!("{observed_ms}\t{key}\n"));
    }
    let _ = fs::write(path, lines);
}

fn tenant_rate_limit_override_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.tenant-rate-limit-overrides")
}

fn load_tenant_rate_limit_overrides(path: &str) -> BTreeMap<String, TenantRateLimitOverride> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 4 {
            continue;
        }
        let tenant_key = parts[0].trim().to_ascii_lowercase();
        if tenant_key.is_empty() {
            continue;
        }
        let Ok(query_rate_limit) = parts[1].parse::<u64>() else {
            continue;
        };
        let Ok(mutation_rate_limit) = parts[2].parse::<u64>() else {
            continue;
        };
        let Ok(upload_cap_bytes) = parts[3].parse::<u64>() else {
            continue;
        };
        if query_rate_limit == 0 || mutation_rate_limit == 0 || upload_cap_bytes == 0 {
            continue;
        }
        let _ = out.insert(
            tenant_key,
            TenantRateLimitOverride {
                query_rate_limit,
                mutation_rate_limit,
                upload_cap_bytes,
            },
        );
    }
    out
}

fn persist_tenant_rate_limit_overrides(
    path: &str,
    overrides: &BTreeMap<String, TenantRateLimitOverride>,
) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (tenant_key, override_limits) in overrides {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            dlq_field_encode(tenant_key),
            override_limits.query_rate_limit,
            override_limits.mutation_rate_limit,
            override_limits.upload_cap_bytes
        ));
    }
    let _ = fs::write(path, lines);
}

fn connector_rollout_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.connector-rollout")
}

fn load_hl7_connector_rollout_state(
    path: &str,
    defaults: &BTreeMap<String, u64>,
) -> BTreeMap<String, u64> {
    let mut out = defaults.clone();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((alias_raw, rollout_raw)) = trimmed.split_once('\t') else {
            continue;
        };
        let alias = normalize_connector_alias(alias_raw);
        if alias.is_empty() {
            continue;
        }
        let Ok(rollout) = rollout_raw.parse::<u64>() else {
            continue;
        };
        if rollout > 100 {
            continue;
        }
        let _ = out.insert(alias, rollout);
    }
    out
}

fn persist_hl7_connector_rollout_state(path: &str, rollout: &BTreeMap<String, u64>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (alias, percent) in rollout {
        lines.push_str(&format!("{alias}\t{percent}\n"));
    }
    let _ = fs::write(path, lines);
}

fn hl7_failure_queue_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.hl7-failures.dlq")
}

fn dlq_field_encode(raw: &str) -> String {
    raw.replace('\t', " ").replace('\n', " ")
}

fn load_hl7_failure_queue(path: &str, max_entries: usize) -> VecDeque<Hl7FailureRecord> {
    let mut out = VecDeque::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 11 && parts.len() != 13 {
            continue;
        }
        let Ok(created_at_ms) = parts[5].parse::<u64>() else {
            continue;
        };
        let Ok(sequence) = parts[10].parse::<u64>() else {
            continue;
        };
        let (attempt, max_attempts) = if parts.len() == 13 {
            let Ok(attempt) = parts[11].parse::<u32>() else {
                continue;
            };
            let Ok(max_attempts) = parts[12].parse::<u32>() else {
                continue;
            };
            (attempt, max_attempts)
        } else {
            (1, MAX_WORKFLOW_CALLBACK_ATTEMPTS)
        };
        out.push_back(Hl7FailureRecord {
            id: normalize_identifier(parts[0]),
            source: normalize_identifier(parts[1]),
            message_type: normalize_identifier(parts[2]),
            reason: normalize_identifier(parts[3]),
            payload_excerpt: normalize_identifier(parts[4]),
            created_at_ms,
            scope: normalize_identifier(parts[6]),
            subscription_id: normalize_identifier(parts[7]),
            event_id: normalize_identifier(parts[8]),
            correlation_id: normalize_identifier(parts[9]),
            sequence,
            attempt,
            max_attempts,
        });
    }
    while out.len() > max_entries {
        let _ = out.pop_back();
    }
    out
}

fn persist_hl7_failure_queue(path: &str, queue: &VecDeque<Hl7FailureRecord>) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for record in queue {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            dlq_field_encode(&record.id),
            dlq_field_encode(&record.source),
            dlq_field_encode(&record.message_type),
            dlq_field_encode(&record.reason),
            dlq_field_encode(&record.payload_excerpt),
            record.created_at_ms,
            dlq_field_encode(&record.scope),
            dlq_field_encode(&record.subscription_id),
            dlq_field_encode(&record.event_id),
            dlq_field_encode(&record.correlation_id),
            record.sequence,
            record.attempt,
            record.max_attempts
        ));
    }
    let _ = fs::write(path, lines);
}

#[cfg(test)]
fn render_hl7_failure_queue_legacy_v1(queue: &VecDeque<Hl7FailureRecord>) -> String {
    let mut lines = String::new();
    for record in queue {
        lines.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            dlq_field_encode(&record.id),
            dlq_field_encode(&record.source),
            dlq_field_encode(&record.message_type),
            dlq_field_encode(&record.reason),
            dlq_field_encode(&record.payload_excerpt),
            record.created_at_ms,
            dlq_field_encode(&record.scope),
            dlq_field_encode(&record.subscription_id),
            dlq_field_encode(&record.event_id),
            dlq_field_encode(&record.correlation_id),
            record.sequence,
        ));
    }
    lines
}

fn parse_hl7_failure_id_sequence(id: &str) -> Option<u64> {
    let suffix = id.strip_prefix("hl7-fail-")?;
    suffix.parse::<u64>().ok()
}

fn hl7_failure_sequence_seed(queue: &VecDeque<Hl7FailureRecord>) -> u64 {
    queue
        .iter()
        .filter_map(|record| parse_hl7_failure_id_sequence(&record.id))
        .max()
        .unwrap_or(0)
}

fn hl7_event_sequence_seed(queue: &VecDeque<Hl7FailureRecord>) -> u64 {
    queue
        .iter()
        .map(|record| record.sequence)
        .max()
        .unwrap_or(0)
}

fn reconciliation_run_idempotency_snapshot_path(workflow_audit_path: &str) -> String {
    format!("{workflow_audit_path}.reconciliation-run-idempotency")
}

fn reconciliation_run_idempotency_limit(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_HL7_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

fn load_reconciliation_run_idempotency_cache(path: &str) -> BTreeMap<String, CachedMppsRequest> {
    let mut out = BTreeMap::new();
    let raw = match fs::read_to_string(path) {
        Ok(value) => value,
        Err(_) => return out,
    };
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(3, '\t');
        let Some(key) = parts.next() else {
            continue;
        };
        let Some(signature) = parts.next() else {
            continue;
        };
        let Some(response) = parts.next() else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || !key.starts_with(RECONCILIATION_RUN_IDEMPOTENCY_PREFIX) {
            continue;
        }
        let _ = out.insert(
            key.to_string(),
            CachedMppsRequest {
                signature: signature.to_string(),
                response: response.to_string(),
            },
        );
    }
    reconciliation_run_idempotency_limit(&mut out);
    out
}

fn persist_reconciliation_run_idempotency_cache(
    path: &str,
    cache: &BTreeMap<String, CachedMppsRequest>,
) {
    if path.trim().is_empty() {
        return;
    }
    let mut lines = String::new();
    for (key, value) in cache {
        if !key.starts_with(RECONCILIATION_RUN_IDEMPOTENCY_PREFIX) {
            continue;
        }
        lines.push_str(&format!(
            "{}\t{}\t{}\n",
            dlq_field_encode(key),
            dlq_field_encode(&value.signature),
            dlq_field_encode(&value.response)
        ));
    }
    let _ = fs::write(path, lines);
}

fn task_next_id(sequence: &mut u64) -> String {
    let next = format!("TASK-{sequence:06}");
    *sequence = sequence.saturating_add(1);
    next
}

fn render_task_json(task: &ProcedureTask) -> String {
    format!(
        "{{\"task_id\":\"{}\",\"scheduled_step_id\":\"{}\",\"requested_procedure_id\":{},\"status\":\"{}\",\"worker\":{},\"created_at_ms\":{},\"updated_at_ms\":{}}}",
        escape_json(&task.task_id),
        escape_json(&task.scheduled_step_id),
        task.requested_procedure_id
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .unwrap_or_else(|| "null".to_string()),
        task.status.as_label(),
        task.worker
            .as_ref()
            .map(|value| format!("\"{}\"", escape_json(value)))
            .unwrap_or_else(|| "null".to_string()),
        task.created_at_ms,
        task.updated_at_ms,
    )
}

fn handle_task_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query = merge_query_parameters(&request.query, limits)?;
    let status_filter = parse_task_status_filter(query.get("status"))?;
    let scheduled_step_filter = query.get("scheduled_step_id").cloned();

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut tasks: Vec<ProcedureTask> = store
        .tasks
        .iter()
        .filter(|(task_id, _)| actor_has_resource_access(&actor, &store.tenant_tasks, task_id))
        .map(|(_, task)| task.clone())
        .collect();
    drop(store);

    if let Some(status) = status_filter {
        tasks.retain(|task| task.status == status);
    }
    if let Some(step_id) = scheduled_step_filter {
        tasks.retain(|task| task.scheduled_step_id == step_id);
    }

    match query.get("sort").map(|value| value.as_str()) {
        None | Some("default") | Some("task_id") => tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id)),
        Some("status") => tasks.sort_by(|a, b| a.status.cmp(&b.status)),
        Some("scheduled_step_id") => {
            tasks.sort_by(|a, b| a.scheduled_step_id.cmp(&b.scheduled_step_id))
        }
        Some("created_at") => tasks.sort_by(|a, b| a.created_at_ms.cmp(&b.created_at_ms)),
        Some(_) => return Err(decode_error("unsupported task sort")),
    }
    if query.get("order") == Some(&"desc".to_string()) {
        tasks.reverse();
    }

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = tasks.len().min(offset.saturating_add(limit));
    let mut tasks_slice = if offset > tasks.len() {
        Vec::new()
    } else {
        tasks[offset..end].to_vec()
    };
    let mut json = String::from("[");
    for (idx, task) in tasks_slice.drain(..).enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&render_task_json(&task));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn handle_task_get(
    task_id: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    if task_id.contains('/') {
        return Err(decode_error("unsupported route"));
    }
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(task) = store.tasks.get(task_id) else {
        return Err(task_not_found_error());
    };
    if !actor_has_resource_access(&actor, &store.tenant_tasks, task_id) {
        return Err(auth_denied_error("task belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_task_json(task)))
}

fn handle_task_control(
    task_id: &str,
    action: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, task_id) {
        return Err(auth_denied_error("task belongs to another tenant"));
    }
    let Some(existing) = store.tasks.get(task_id) else {
        return Err(task_not_found_error());
    };
    let target_status = match action {
        "start" => TaskStatus::InProgress,
        "pause" => TaskStatus::OnHold,
        "resume" => TaskStatus::InProgress,
        "complete" => TaskStatus::Completed,
        "review" => TaskStatus::Reviewed,
        "commit" => TaskStatus::Committed,
        "cancel" => TaskStatus::Discontinued,
        _ => return Err(decode_error("unsupported task action")),
    };
    if !existing.status.can_transition_to(target_status) {
        return Err(task_status_invalid_transition_error());
    }
    let previous_status = existing.status;
    if previous_status == target_status {
        return Ok(WorkflowResponse::Json(
            200,
            format!(
                "{{\"outcome\":\"ok\",\"task_id\":\"{}\",\"status\":\"{}\"}}",
                escape_json(task_id),
                previous_status.as_label()
            ),
        ));
    }
    let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
    let correlation_id = request_id_from_headers(&request.headers);
    let worker = request
        .headers
        .get("x-task-worker")
        .or_else(|| request.headers.get("x-sr-principal"))
        .map(ToString::to_string);
    if let Some(task) = store.tasks.get_mut(task_id) {
        task.status = target_status;
        if let Some(worker) = worker {
            task.worker = Some(worker);
        }
        task.updated_at_ms = now_epoch_millis();
    }
    let mut payload = BTreeMap::new();
    payload.insert("event".to_string(), "task.transition".to_string());
    payload.insert("action".to_string(), action.to_string());
    payload.insert("task_id".to_string(), task_id.to_string());
    payload.insert(
        "from_status".to_string(),
        previous_status.as_label().to_string(),
    );
    payload.insert(
        "to_status".to_string(),
        target_status.as_label().to_string(),
    );
    payload.insert("tenant".to_string(), actor.tenant.clone());
    if let Some(actor_name) = actor.principal.clone() {
        payload.insert("actor".to_string(), actor_name);
    }
    publish_hl7_event(
        &mut store,
        &workflow_event_source(&actor),
        "task",
        &payload,
        &event_id,
        sequence,
        &correlation_id,
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"outcome\":\"ok\",\"task_id\":\"{}\",\"status\":\"{}\"}}",
            escape_json(task_id),
            target_status.as_label()
        ),
    ))
}

fn completion_outcome_label(outcome: CompletionOutcome) -> &'static str {
    match outcome {
        CompletionOutcome::Success => "success",
        CompletionOutcome::Failure => "failure",
    }
}

fn parse_completion_outcome(value: &str) -> Result<CompletionOutcome, Box<Error>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "success" | "ok" | "completed" | "delivered" => Ok(CompletionOutcome::Success),
        "failure" | "failed" | "error" | "timedout" | "canceled" => Ok(CompletionOutcome::Failure),
        _ => Err(decode_error("invalid completion outcome")),
    }
}

fn ups_state_label(state: UpsState) -> &'static str {
    match state {
        UpsState::Scheduled => "scheduled",
        UpsState::InProgress => "in_progress",
        UpsState::Canceled => "canceled",
        UpsState::Completed => "completed",
        UpsState::Failed => "failed",
    }
}

fn parse_ups_transition(value: &str) -> Result<UpsTransition, Box<Error>> {
    match value.trim().to_ascii_lowercase().as_str() {
        "start" => Ok(UpsTransition::Start),
        "cancel" => Ok(UpsTransition::Cancel),
        "complete" => Ok(UpsTransition::Complete),
        "fail" => Ok(UpsTransition::Fail),
        _ => Err(decode_error("unsupported ups transition action")),
    }
}

fn normalize_ups_uid(raw: &str) -> Result<String, Box<Error>> {
    let uid = normalize_route_identifier(raw)?;
    dicom_core::validate_uid_strict(TAG_SOP_INSTANCE_UID, &uid)?;
    Ok(uid)
}

fn tenant_scoped_key(tenant: &str, id: &str) -> String {
    format!("{tenant}|{id}")
}

fn tenant_scoped_suffix<'a>(key: &'a str, tenant: &str) -> Option<&'a str> {
    let prefix = format!("{tenant}|");
    key.strip_prefix(&prefix)
}

fn render_ups_workitem_json(item: &dicom_ups::UpsWorkitem) -> String {
    format!(
        "{{\"ups_instance_uid\":\"{}\",\"procedure_step_label\":\"{}\",\"state\":\"{}\",\"revision\":{}}}",
        escape_json(&item.ups_instance_uid),
        escape_json(&item.procedure_step_label),
        ups_state_label(item.state),
        item.revision,
    )
}

fn handle_workitem_routes(
    request_path: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
    _actor: &WorkflowActorContext,
) -> Result<WorkflowResponse, Box<Error>> {
    if request_path == WORKITEM_COLLECTION_PATH {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workitem_list(request, state, limits),
            "POST" => handle_workitem_create(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    let Some(rest) = request_path.strip_prefix(WORKITEM_ITEM_PREFIX) else {
        return Err(decode_error("unsupported route"));
    };
    if rest.is_empty() {
        return Err(decode_error("unsupported route"));
    }
    if rest == "search" {
        return match request.method.as_str() {
            "GET" | "HEAD" => handle_workitem_list(request, state, limits),
            _ => Err(decode_error("unsupported route")),
        };
    }
    if let Some((workitem_uid, suffix)) = rest.split_once('/') {
        let workitem_uid = normalize_ups_uid(workitem_uid)?;
        return match (request.method.as_str(), suffix) {
            ("GET", "state") | ("HEAD", "state") => {
                handle_workitem_state_get(&workitem_uid, request, state)
            }
            ("POST", "state") => {
                handle_workitem_state_update(&workitem_uid, request, state, limits)
            }
            ("POST", "cancel") => handle_workitem_cancel(&workitem_uid, request, state),
            _ => Err(decode_error("unsupported route")),
        };
    }
    let workitem_uid = normalize_ups_uid(rest)?;
    match request.method.as_str() {
        "GET" | "HEAD" => handle_workitem_get(&workitem_uid, request, state),
        "POST" => handle_workitem_update(&workitem_uid, request, state, limits),
        _ => Err(decode_error("unsupported route")),
    }
}

fn handle_workitem_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query = merge_query_parameters(&request.query, limits)?;
    let state_filter = query
        .get("state")
        .map(|value| value.trim().to_ascii_lowercase());

    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut items: Vec<String> = Vec::new();
    for item in store.hl7.ups.store().workitems() {
        if !actor_has_resource_access(&actor, &store.tenant_tasks, &item.ups_instance_uid) {
            continue;
        }
        if let Some(filter) = &state_filter {
            if ups_state_label(item.state) != filter {
                continue;
            }
        }
        items.push(render_ups_workitem_json(item));
    }

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"count\":{},\"items\":[{}]}}",
            items.len(),
            items.join(",")
        ),
    ))
}

fn handle_workitem_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let ups_instance_uid = normalize_ups_uid(required_param(&params, "ups_instance_uid")?)?;
    let procedure_step_label = required_param(&params, "procedure_step_label")?.to_string();
    let correlation_id = params.get("hl7_correlation_id").cloned();

    let request_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = format!("ups-create:{ups_instance_uid}:{idempotency_key}");

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(entry) = store.task_idempotency.get(&cache_key) {
        if entry.signature == request_signature {
            return Ok(WorkflowResponse::Json(200, entry.response.clone()));
        }
        return Err(decode_error("idempotency key replay conflict"));
    }

    let created = store
        .hl7
        .ups
        .create(ups_instance_uid.clone(), procedure_step_label)?;
    route_id_to_tenant_index(&mut store.tenant_tasks, &actor.tenant, &ups_instance_uid);
    if let Some(correlation_id) = correlation_id {
        let key = tenant_scoped_key(&actor.tenant, &correlation_id);
        let _ = store
            .hl7
            .hl7_ups_correlation
            .insert(key, ups_instance_uid.clone());
    }

    let body = render_ups_workitem_json(&created);
    let _ = store.task_idempotency.insert(
        cache_key,
        CachedMppsRequest {
            signature: request_signature,
            response: body.clone(),
        },
    );
    task_idempotency_limit(&mut store.task_idempotency);
    Ok(WorkflowResponse::Json(200, body))
}

fn handle_workitem_get(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.ups.get(workitem_uid)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

fn handle_workitem_update(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let new_label = required_param(&params, "procedure_step_label")?.to_string();

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.ups.update(workitem_uid, new_label)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

fn handle_workitem_state_get(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.ups.get(workitem_uid)?;
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"ups_instance_uid\":\"{}\",\"state\":\"{}\",\"revision\":{}}}",
            escape_json(workitem_uid),
            ups_state_label(item.state),
            item.revision,
        ),
    ))
}

fn handle_workitem_state_update(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let action = required_param(&params, "action")?;
    let transition = parse_ups_transition(action)?;

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = match transition {
        UpsTransition::Start => store.hl7.ups.start(workitem_uid)?,
        UpsTransition::Cancel => store.hl7.ups.cancel(workitem_uid)?,
        UpsTransition::Complete => store.hl7.ups.complete(workitem_uid)?,
        UpsTransition::Fail => store.hl7.ups.fail(workitem_uid)?,
    };
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

fn handle_workitem_cancel(
    workitem_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(&actor, &store.tenant_tasks, workitem_uid) {
        return Err(auth_denied_error("ups workitem belongs to another tenant"));
    }
    let item = store.hl7.ups.cancel(workitem_uid)?;
    Ok(WorkflowResponse::Json(200, render_ups_workitem_json(&item)))
}

fn handle_ian_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, record) in &store.hl7.ian_events {
        let Some(event_id) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"event_id\":\"{}\",\"sop_instance_uid\":\"{}\",\"outcome\":\"{}\",\"ingested_at_ms\":{}}}",
            escape_json(event_id),
            escape_json(&record.sop_instance_uid),
            completion_outcome_label(record.outcome),
            record.ingested_at_ms,
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

fn handle_ian_ingest(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let event_id = required_param(&params, "event_id")?.to_string();
    let sop_instance_uid = normalize_ups_uid(required_param(&params, "sop_instance_uid")?)?;
    let outcome = parse_completion_outcome(required_param(&params, "outcome")?)?;

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let inserted =
        store
            .hl7
            .completion
            .ingest_ian(event_id.clone(), sop_instance_uid.clone(), outcome);
    if inserted {
        let key = tenant_scoped_key(&actor.tenant, &event_id);
        let _ = store.hl7.ian_events.insert(
            key,
            IanEventRecord {
                event_id,
                sop_instance_uid,
                outcome,
                ingested_at_ms: now_epoch_millis(),
            },
        );
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"inserted\":{},\"outcome\":\"{}\"}}",
            inserted,
            completion_outcome_label(outcome)
        ),
    ))
}

fn handle_storage_commitment_status_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, record) in &store.hl7.storage_commitment_status {
        let Some(transaction_uid) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\",\"updated_at_ms\":{}}}",
            escape_json(transaction_uid),
            completion_outcome_label(record.outcome),
            record.updated_at_ms,
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

fn handle_storage_commitment_status_get(
    transaction_uid: &str,
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    dicom_core::validate_uid_strict(TAG_SOP_INSTANCE_UID, transaction_uid)?;
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let key = tenant_scoped_key(&actor.tenant, transaction_uid);
    let Some(record) = store.hl7.storage_commitment_status.get(&key) else {
        return Err(decode_error("storage commitment status not found"));
    };
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\",\"updated_at_ms\":{}}}",
            escape_json(transaction_uid),
            completion_outcome_label(record.outcome),
            record.updated_at_ms,
        ),
    ))
}

fn handle_storage_commitment_status_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let transaction_uid = normalize_ups_uid(required_param(&params, "transaction_uid")?)?;
    let outcome = parse_completion_outcome(required_param(&params, "outcome")?)?;

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let event_id = format!("stgc:{}", transaction_uid);
    let _ =
        store
            .hl7
            .completion
            .ingest_storage_commitment(event_id, transaction_uid.clone(), outcome);
    let key = tenant_scoped_key(&actor.tenant, &transaction_uid);
    let _ = store.hl7.storage_commitment_status.insert(
        key,
        StorageCommitmentStatusRecord {
            transaction_uid: transaction_uid.clone(),
            outcome,
            updated_at_ms: now_epoch_millis(),
        },
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"transaction_uid\":\"{}\",\"outcome\":\"{}\"}}",
            escape_json(&transaction_uid),
            completion_outcome_label(outcome),
        ),
    ))
}

fn handle_hl7_ups_correlation_list(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut payload = String::from("[");
    let mut first = true;
    for (key, ups_uid) in &store.hl7.hl7_ups_correlation {
        let Some(correlation_id) = tenant_scoped_suffix(key, &actor.tenant) else {
            continue;
        };
        if !first {
            payload.push(',');
        }
        first = false;
        payload.push_str(&format!(
            "{{\"correlation_id\":\"{}\",\"ups_instance_uid\":\"{}\"}}",
            escape_json(correlation_id),
            escape_json(ups_uid),
        ));
    }
    payload.push(']');
    Ok(WorkflowResponse::Json(200, payload))
}

fn handle_hl7_ups_correlation_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let correlation_id = required_param(&params, "correlation_id")?.to_string();
    let ups_instance_uid = normalize_ups_uid(required_param(&params, "ups_instance_uid")?)?;

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let key = tenant_scoped_key(&actor.tenant, &correlation_id);
    let _ = store
        .hl7
        .hl7_ups_correlation
        .insert(key, ups_instance_uid.clone());
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"correlation_id\":\"{}\",\"ups_instance_uid\":\"{}\"}}",
            escape_json(&correlation_id),
            escape_json(&ups_instance_uid),
        ),
    ))
}

fn handle_task_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let scheduled_step_id = required_param(&params, "scheduled_step_id")?.to_string();
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let status = params
        .get("status")
        .map(|status| task_status_from_label(status, "invalid task status"))
        .transpose()?
        .unwrap_or(TaskStatus::Scheduled);
    let provided_task_id = params.get("task_id").cloned();

    let request_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = if idempotency_key.is_empty() {
        None
    } else {
        Some(format!("task-create:{idempotency_key}"))
    };

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(key) = &cache_key {
        if let Some(entry) = store.task_idempotency.get(key) {
            if entry.signature == request_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(decode_error("idempotency key replay conflict"));
        }
    }

    let task_id = provided_task_id.unwrap_or_else(|| task_next_id(&mut store.task_id_sequence));
    let now = now_epoch_millis();
    let policy = tenant_policy(&store, &actor.tenant);

    if let Some(_task) = store.tasks.get(&task_id) {
        if !actor_has_resource_access(&actor, &store.tenant_tasks, &task_id) {
            return Err(auth_denied_error("task belongs to another tenant"));
        }
    } else {
        let existing = store
            .tenant_tasks
            .get(&actor.tenant)
            .map(|ids| ids.len())
            .unwrap_or_default();
        if existing >= policy.task_quota {
            return Err(limit_exceeded(
                "workflow_tenant_task_quota",
                existing as u64,
                policy.task_quota as u64,
            ));
        }
    }

    let outcome = if let Some(task) = store.tasks.get_mut(&task_id) {
        task.scheduled_step_id = scheduled_step_id.clone();
        task.requested_procedure_id = requested_procedure_id.clone();
        task.status = status;
        task.worker = worker.clone();
        task.updated_at_ms = now;
        "updated"
    } else {
        store.tasks.insert(
            task_id.clone(),
            ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status,
                worker,
                tenant: actor.tenant.clone(),
                created_at_ms: now,
                updated_at_ms: now,
            },
        );
        route_id_to_tenant_index(&mut store.tenant_tasks, &actor.tenant, &task_id);
        "inserted"
    };

    let body = format!(
        "{{\"outcome\":\"{}\",\"task_id\":\"{}\"}}",
        outcome, task_id,
    );

    if let Some(key) = cache_key {
        store.task_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: request_signature,
                response: body.clone(),
            },
        );
        task_idempotency_limit(&mut store.task_idempotency);
    }

    Ok(WorkflowResponse::Json(200, body))
}

fn handle_worklist_query(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let query_params = merge_query_parameters(&request.query, limits)?;
    let worklist_query = WorklistQuery {
        modality: query_params.get("modality").cloned(),
        scheduled_step_id: query_params.get("scheduled_step_id").cloned(),
        patient_id: lookup_identifier_from_map(
            &query_params,
            &["patient_id", "PatientID", "patientId", "Patient_Id"],
        ),
        requested_procedure_id: query_params.get("requested_procedure_id").cloned(),
    };
    let status_filter = parse_worklist_status_filter(query_params.get("status"))?;

    let (rows, status_by_step) = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mpps = store.mpps.all_updates();
        let mut status_by_step = BTreeMap::new();
        for update in mpps {
            if !actor_has_resource_access(&actor, &store.tenant_mpps, &update.sop_instance_uid) {
                continue;
            }
            let status = mpps_status_label(update.status).to_string();
            match status_by_step.get_mut(&update.performed_step_id) {
                Some((current_uid, current_status)) => {
                    if update.sop_instance_uid > *current_uid {
                        *current_uid = update.sop_instance_uid.clone();
                        *current_status = status;
                    }
                }
                None => {
                    status_by_step.insert(
                        update.performed_step_id.clone(),
                        (update.sop_instance_uid.clone(), status),
                    );
                }
            }
        }
        let rows = store.worklist.query(&worklist_query)?;
        (rows, status_by_step)
    };

    let mut items: Vec<WorklistItemWithStatus> = Vec::new();
    for row in rows {
        let item = validate_worklist_item(&row, &Limits::default())?;
        {
            let store = state
                .lock()
                .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
            if !actor_has_resource_access(&actor, &store.tenant_worklist, &item.scheduled_step_id) {
                continue;
            }
        }
        let status = status_by_step
            .get(&item.scheduled_step_id)
            .map(|(_, status)| status.clone())
            .unwrap_or_else(|| "SCHEDULED".to_string());
        if let Some(filter) = &status_filter {
            if status != *filter {
                continue;
            }
        }
        items.push(WorklistItemWithStatus { item, status });
    }

    apply_worklist_sort(
        &mut items,
        query_params.get("sort"),
        query_params.get("order"),
    )?;

    let (offset, limit) =
        parse_pagination(query_params.get("page"), query_params.get("page_size"))?;
    let end = items.len().min(offset.saturating_add(limit));

    if offset > items.len() {
        items.clear();
    } else {
        items = items[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, item) in items.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"scheduled_step_id\":\"");
        json.push_str(&escape_json(&item.item.scheduled_step_id));
        json.push_str("\",\"modality\":\"");
        json.push_str(&escape_json(&item.item.modality));
        json.push_str("\",\"start_date\":\"");
        json.push_str(&escape_json(&item.item.start_date));
        json.push_str("\",\"start_time\":\"");
        json.push_str(&escape_json(&item.item.start_time));
        if let Some(value) = item.item.requested_procedure_id.as_deref() {
            json.push_str("\",\"requested_procedure_id\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.scheduled_station_ae_title.as_deref() {
            json.push_str("\",\"scheduled_station_ae_title\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.patient_id.as_deref() {
            json.push_str("\",\"patient_id\":\"");
            json.push_str(&escape_json(&value));
        }
        if let Some(value) = item.item.accession_number.as_deref() {
            json.push_str("\",\"accession_number\":\"");
            json.push_str(&escape_json(&value));
        }
        json.push_str("\",\"status\":\"");
        json.push_str(&escape_json(&item.status));
        json.push_str("\"}");
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn apply_worklist_sort(
    items: &mut Vec<WorklistItemWithStatus>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None | Some("start_date,start_time,scheduled_step_id,modality") | Some("default") => {
            if descending {
                items.sort_by(|a, b| {
                    (
                        b.item.start_date.as_str(),
                        b.item.start_time.as_str(),
                        b.item.scheduled_step_id.as_str(),
                        b.item.modality.as_str(),
                    )
                        .cmp(&(
                            a.item.start_date.as_str(),
                            a.item.start_time.as_str(),
                            a.item.scheduled_step_id.as_str(),
                            a.item.modality.as_str(),
                        ))
                });
            } else {
                items.sort_by(|a, b| {
                    (
                        a.item.start_date.as_str(),
                        a.item.start_time.as_str(),
                        a.item.scheduled_step_id.as_str(),
                        a.item.modality.as_str(),
                    )
                        .cmp(&(
                            b.item.start_date.as_str(),
                            b.item.start_time.as_str(),
                            b.item.scheduled_step_id.as_str(),
                            b.item.modality.as_str(),
                        ))
                });
            }
        }
        Some("status") => {
            if descending {
                items.sort_by(|a, b| b.status.cmp(&a.status));
            } else {
                items.sort_by(|a, b| a.status.cmp(&b.status));
            }
        }
        Some("scheduled_step_id") => {
            if descending {
                items.sort_by(|a, b| b.item.scheduled_step_id.cmp(&a.item.scheduled_step_id));
            } else {
                items.sort_by(|a, b| a.item.scheduled_step_id.cmp(&b.item.scheduled_step_id));
            }
        }
        Some("modality") => {
            if descending {
                items.sort_by(|a, b| b.item.modality.cmp(&a.item.modality));
            } else {
                items.sort_by(|a, b| a.item.modality.cmp(&b.item.modality));
            }
        }
        Some("patient_id") => {
            if descending {
                items.sort_by(|a, b| b.item.patient_id.cmp(&a.item.patient_id));
            } else {
                items.sort_by(|a, b| a.item.patient_id.cmp(&b.item.patient_id));
            }
        }
        Some("accession_number") => {
            if descending {
                items.sort_by(|a, b| b.item.accession_number.cmp(&a.item.accession_number));
            } else {
                items.sort_by(|a, b| a.item.accession_number.cmp(&b.item.accession_number));
            }
        }
        Some(_) => Err(decode_error("unsupported worklist sort"))?,
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct WorklistItemWithStatus {
    item: dicom_worklist::WorklistItem,
    status: String,
}

fn parse_worklist_status_filter(status: Option<&String>) -> Result<Option<String>, Box<Error>> {
    let Some(raw) = status else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Err(decode_error("invalid worklist status filter"));
    }
    let normalized = raw.trim().replace('_', " ").to_ascii_uppercase();
    let allowed = match normalized.as_str() {
        "IN PROGRESS" | "INPROGRESS" => "IN PROGRESS",
        "COMPLETED" => "COMPLETED",
        "DISCONTINUED" => "DISCONTINUED",
        "SCHEDULED" | "PENDING" | "UNCLAIMED" => "SCHEDULED",
        _ => return Err(decode_error("invalid worklist status filter")),
    };
    Ok(Some(allowed.to_string()))
}

fn parse_pagination(
    page: Option<&String>,
    page_size: Option<&String>,
) -> Result<(usize, usize), Box<Error>> {
    let size = match page_size {
        Some(value) => {
            parse_usize_param("page_size", value, 1, Some(MAX_WORKFLOW_PAGE_SIZE), false)?
        }
        None => DEFAULT_WORKFLOW_PAGE_SIZE,
    };
    let page = match page {
        Some(value) => parse_usize_param("page", value, 1, None, true)?,
        None => 1,
    };
    let offset = page.saturating_sub(1).saturating_mul(size);
    Ok((offset, size))
}

fn parse_usize_param(
    name: &'static str,
    value: &str,
    min: usize,
    max: Option<usize>,
    _allow_unbounded: bool,
) -> Result<usize, Box<Error>> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| decode_error(format!("invalid {name} parameter")))?;
    if parsed < min {
        return Err(decode_error(format!("invalid {name} parameter")));
    }
    if let Some(max) = max {
        if parsed > max {
            return Err(limit_exceeded(name, parsed as u64, max as u64));
        }
    }
    Ok(parsed)
}

fn mpps_status_label(status: MppsStatus) -> &'static str {
    match status {
        MppsStatus::InProgress => "IN PROGRESS",
        MppsStatus::Completed => "COMPLETED",
        MppsStatus::Discontinued => "DISCONTINUED",
    }
}

fn handle_worklist_upsert(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let actor = workflow_actor_context(&request.headers);
    let params = parse_form_map(&request.body, limits)?;
    let scheduled_step_id = required_param(&params, "scheduled_step_id")?.to_string();
    let dataset = build_worklist_dataset(&params)?;
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(owner_tenant) = tenant_of_id(&store.tenant_worklist, &scheduled_step_id) {
        if owner_tenant != actor.tenant {
            return Err(auth_denied_error("worklist item belongs to another tenant"));
        }
    }
    let outcome = store.worklist.upsert_dataset(&dataset)?;
    route_id_to_tenant_index(
        &mut store.tenant_worklist,
        &actor.tenant,
        &scheduled_step_id,
    );
    let body = format!(
        "{{\"outcome\":\"{}\"}}",
        match outcome {
            dicom_worklist::UpsertOutcome::Inserted => "inserted",
            dicom_worklist::UpsertOutcome::Updated => "updated",
            dicom_worklist::UpsertOutcome::Duplicate => "duplicate",
        }
    );
    Ok(WorkflowResponse::Json(200, body))
}

fn handle_mpps_get(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    handle_mpps_list(request, actor, state, limits)
}

fn handle_mpps_get_single(
    sop_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(update) = store.mpps.get(sop_uid) else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_mpps_update_json(update)))
}

fn handle_mpps_get_status(
    sop_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(update) = store.mpps.get(sop_uid) else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    Ok(WorkflowResponse::Json(200, render_mpps_update_json(update)))
}

fn handle_mpps_update_status(
    sop_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, &Limits::default())?;
    let requested_status = parse_mpps_status(
        required_param(&params, "status")?,
        "invalid MPPS status transition status",
    )?;
    let payload_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = if idempotency_key.is_empty() {
        None
    } else {
        Some(format!("mpps-status:{sop_uid}:{idempotency_key}"))
    };

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(current) = store.mpps.get(sop_uid).cloned() else {
        return Err(mpps_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant_mpps, sop_uid) {
        return Err(auth_denied_error("mpps belongs to another tenant"));
    }
    if let Some(key) = &cache_key {
        if let Some(entry) = store.mpps_idempotency.get(key) {
            if entry.signature == payload_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(mpps_idempotency_conflict_error());
        }
    }

    let mut update_params = BTreeMap::new();
    update_params.insert("sop_instance_uid".to_string(), sop_uid.to_string());
    update_params.insert(
        "performed_step_id".to_string(),
        current.performed_step_id.clone(),
    );
    update_params.insert("start_date".to_string(), current.start_date.clone());
    update_params.insert("start_time".to_string(), current.start_time.clone());
    update_params.insert(
        "status".to_string(),
        mpps_status_label(requested_status).to_string(),
    );
    if let Some(end_date) = params.get("end_date") {
        update_params.insert("end_date".to_string(), end_date.to_string());
    } else if let Some(end_date) = current.end_date {
        update_params.insert("end_date".to_string(), end_date);
    }
    if let Some(end_time) = params.get("end_time") {
        update_params.insert("end_time".to_string(), end_time.to_string());
    } else if let Some(end_time) = current.end_time {
        update_params.insert("end_time".to_string(), end_time);
    }

    let outcome = store.mpps.ingest(&build_mpps_dataset(&update_params)?)?;
    if current.status != requested_status {
        let mut payload = BTreeMap::new();
        payload.insert("event".to_string(), "mpps.transition".to_string());
        payload.insert("sop_instance_uid".to_string(), sop_uid.to_string());
        payload.insert(
            "from_status".to_string(),
            mpps_status_label(current.status).to_string(),
        );
        payload.insert(
            "to_status".to_string(),
            mpps_status_label(requested_status).to_string(),
        );
        payload.insert(
            "performed_step_id".to_string(),
            current.performed_step_id.clone(),
        );
        payload.insert("tenant".to_string(), actor.tenant.clone());
        if let Some(actor_name) = actor.principal.clone() {
            payload.insert("actor".to_string(), actor_name);
        }
        let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
        let correlation_id = request_id_from_headers(&request.headers);
        publish_hl7_event(
            &mut store,
            &workflow_event_source(&actor),
            "mpps",
            &payload,
            &event_id,
            sequence,
            &correlation_id,
        );
    }
    route_id_to_tenant_index(&mut store.tenant_mpps, &actor.tenant, sop_uid);
    let body = render_mpps_ingest_outcome_json(&outcome);
    if let Some(key) = cache_key {
        store.mpps_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: payload_signature,
                response: body.clone(),
            },
        );
        enforce_mpps_idempotency_capacity(&mut store.mpps_idempotency);
    }
    Ok(WorkflowResponse::Json(200, body))
}

fn handle_mpps_list(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let query = merge_query_parameters(&request.query, limits)?;
    let mut updates = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut updates = if let Some(sop_uid) = request.query.get("sop_instance_uid") {
            match store.mpps.get(sop_uid) {
                Some(update) => vec![update.clone()],
                None => Vec::new(),
            }
        } else {
            store.mpps.all_updates()
        };
        updates.retain(|update| {
            actor_has_resource_access(actor, &store.tenant_mpps, &update.sop_instance_uid)
        });
        updates
    };

    if let Some(status) = parse_mpps_status_filter(query.get("status"))? {
        updates.retain(|update| mpps_status_label(update.status) == status);
    }
    if let Some(performed_step_id) = query.get("performed_step_id") {
        updates.retain(|update| update.performed_step_id == *performed_step_id);
    }

    apply_mpps_sort(&mut updates, query.get("sort"), query.get("order"))?;

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = updates.len().min(offset.saturating_add(limit));
    if offset > updates.len() {
        updates.clear();
    } else {
        updates = updates[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, update) in updates.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"sop_instance_uid\":\"");
        json.push_str(&escape_json(&update.sop_instance_uid));
        json.push_str("\",\"status\":\"");
        json.push_str(match update.status {
            MppsStatus::InProgress => "IN PROGRESS",
            MppsStatus::Completed => "COMPLETED",
            MppsStatus::Discontinued => "DISCONTINUED",
        });
        json.push_str("\",\"performed_step_id\":\"");
        json.push_str(&escape_json(&update.performed_step_id));
        json.push_str("\",\"start_date\":\"");
        json.push_str(&escape_json(&update.start_date));
        json.push_str("\",\"start_time\":\"");
        json.push_str(&escape_json(&update.start_time));
        if let Some(end_date) = &update.end_date {
            json.push_str("\",\"end_date\":\"");
            json.push_str(&escape_json(end_date));
        }
        if let Some(end_time) = &update.end_time {
            json.push_str("\",\"end_time\":\"");
            json.push_str(&escape_json(end_time));
        }
        json.push_str("\"}");
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn apply_mpps_sort(
    updates: &mut Vec<dicom_mpps::MppsUpdate>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None | Some("default") | Some("sop_instance_uid") => {
            updates.sort_by(|a, b| a.sop_instance_uid.cmp(&b.sop_instance_uid));
            if descending {
                updates.reverse();
            }
        }
        Some("status") => {
            updates.sort_by(|a, b| mpps_status_label(a.status).cmp(mpps_status_label(b.status)));
            if descending {
                updates.reverse();
            }
        }
        Some("performed_step_id") => {
            updates.sort_by(|a, b| a.performed_step_id.cmp(&b.performed_step_id));
            if descending {
                updates.reverse();
            }
        }
        Some("start_date") => {
            updates.sort_by(|a, b| a.start_date.cmp(&b.start_date));
            if descending {
                updates.reverse();
            }
        }
        Some("start_time") => {
            updates.sort_by(|a, b| a.start_time.cmp(&b.start_time));
            if descending {
                updates.reverse();
            }
        }
        Some("end_date") => {
            updates.sort_by(|a, b| {
                (a.end_date.as_deref(), a.end_time.as_deref())
                    .cmp(&(b.end_date.as_deref(), b.end_time.as_deref()))
            });
            if descending {
                updates.reverse();
            }
        }
        Some(_) => Err(decode_error("unsupported MPPS sort"))?,
    }
    Ok(())
}

fn handle_hl7_ingest(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .map(std::string::String::as_str);
    let correlation_id = request_id_from_headers(&request.headers);
    let response = run_hl7_ingest_payload(
        &request.body,
        idempotency_key,
        &correlation_id,
        state,
        limits,
    )?;
    Ok(WorkflowResponse::Json(200, response))
}

fn fhir_ingest_enabled() -> Result<bool, Box<Error>> {
    parse_bool(
        WORKFLOW_SERVICE_NAME,
        "DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
        false,
    )
    .map_err(|err| {
        io_error(
            "invalid DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
            err.to_string(),
        )
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FhirIngestRequest {
    tenant: String,
    resource_type: String,
    source_system: String,
    content_sha256: Option<String>,
    dry_run: bool,
    validated_required_fields: Vec<String>,
}

fn is_supported_fhir_resource_type(resource_type: &str) -> bool {
    matches!(
        resource_type,
        "Bundle" | "Patient" | "Encounter" | "Observation" | "DiagnosticReport"
    )
}

fn parse_fhir_ingest_request(
    params: &BTreeMap<String, String>,
) -> Result<FhirIngestRequest, Box<Error>> {
    let tenant = params
        .get("tenant")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TENANT_ID_DEFAULT.to_string());
    let resource_type = params
        .get("resource_type")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Bundle".to_string());
    if !is_supported_fhir_resource_type(resource_type.as_str()) {
        return Err(decode_error("unsupported fhir resource_type"));
    }
    let source_system = params
        .get("source_system")
        .or_else(|| params.get("source"))
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    let content_sha256 = params
        .get("content_sha256")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty());
    if let Some(sha256) = content_sha256.as_deref() {
        let is_hex = sha256.chars().all(|ch| ch.is_ascii_hexdigit());
        if sha256.len() != 64 || !is_hex {
            return Err(decode_error(
                "content_sha256 must be 64 lowercase/uppercase hex characters",
            ));
        }
    }
    let dry_run = match params
        .get("dry_run")
        .map(|value| value.to_ascii_lowercase())
    {
        Some(value) if value == "true" || value == "1" => true,
        Some(value) if value == "false" || value == "0" => false,
        Some(_) => return Err(decode_error("invalid dry_run flag")),
        None => false,
    };
    let validated_required_fields =
        validate_fhir_resource_specific_required_fields(resource_type.as_str(), params)?;
    Ok(FhirIngestRequest {
        tenant,
        resource_type,
        source_system,
        content_sha256,
        dry_run,
        validated_required_fields,
    })
}

fn render_fhir_ingest_response(request: &FhirIngestRequest) -> String {
    format!(
        "{{\"status\":\"accepted\",\"endpoint\":\"/interop/fhir\",\"tenant\":\"{}\",\"resource_type\":\"{}\",\"source_system\":\"{}\",\"accepted\":true,\"dry_run\":{},\"validation\":{{\"profile\":\"fhir-minimal-v1\",\"content_sha256_present\":{},\"required_fields\":{}}},\"mode\":\"typed\"}}",
        escape_json(&request.tenant),
        escape_json(&request.resource_type),
        escape_json(&request.source_system),
        if request.dry_run { "true" } else { "false" },
        if request.content_sha256.is_some() {
            "true"
        } else {
            "false"
        },
        render_string_array(&request.validated_required_fields),
    )
}

fn required_fhir_field(
    params: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) -> Result<String, Box<Error>> {
    params
        .get(key)
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| decode_error(format!("missing required fhir field: {label}")))
}

fn required_fhir_positive_integer(
    params: &BTreeMap<String, String>,
    key: &str,
    label: &str,
) -> Result<u64, Box<Error>> {
    let value = required_fhir_field(params, key, label)?;
    let parsed = value
        .parse::<u64>()
        .map_err(|_| decode_error(format!("invalid fhir numeric field: {label}")))?;
    if parsed == 0 {
        return Err(decode_error(format!(
            "invalid fhir numeric field: {label} must be > 0"
        )));
    }
    Ok(parsed)
}

fn validate_fhir_resource_specific_required_fields(
    resource_type: &str,
    params: &BTreeMap<String, String>,
) -> Result<Vec<String>, Box<Error>> {
    let mut required = Vec::new();
    match resource_type {
        "Bundle" => {
            let _ = required_fhir_field(params, "bundle_type", "bundle_type")?;
            let _ = required_fhir_positive_integer(params, "entry_count", "entry_count")?;
            required.push("bundle_type".to_string());
            required.push("entry_count".to_string());
        }
        "Patient" => {
            let _ = required_fhir_field(params, "patient_id", "patient_id")?;
            let _ = required_fhir_field(params, "patient_name", "patient_name")?;
            required.push("patient_id".to_string());
            required.push("patient_name".to_string());
        }
        "Encounter" => {
            let _ = required_fhir_field(params, "encounter_id", "encounter_id")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            required.push("encounter_id".to_string());
            required.push("subject_id".to_string());
        }
        "Observation" => {
            let _ = required_fhir_field(params, "observation_code", "observation_code")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            let _ = required_fhir_field(params, "observed_at", "observed_at")?;
            required.push("observation_code".to_string());
            required.push("subject_id".to_string());
            required.push("observed_at".to_string());
        }
        "DiagnosticReport" => {
            let _ = required_fhir_field(params, "report_code", "report_code")?;
            let _ = required_fhir_field(params, "subject_id", "subject_id")?;
            let _ = required_fhir_field(params, "issued_at", "issued_at")?;
            required.push("report_code".to_string());
            required.push("subject_id".to_string());
            required.push("issued_at".to_string());
        }
        _ => {}
    }
    Ok(required)
}

fn handle_fhir_ingest(
    request: &HttpRequest,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    if !fhir_ingest_enabled()? {
        return Ok(WorkflowResponse::Json(
            404,
            render_interop_error_json(
                "DVF.WORKFLOW.FHIR.DISABLED",
                "FHIR ingestion is disabled",
                "set DICOM_WORKFLOW_FHIR_INGEST_ENABLED=true to enable the typed ingest endpoint",
            ),
        ));
    }
    let params = parse_form_map(&request.body, limits)?;
    let typed_request = parse_fhir_ingest_request(&params)?;
    Ok(WorkflowResponse::Json(
        202,
        render_fhir_ingest_response(&typed_request),
    ))
}

fn run_hl7_mllp_listener(
    bind: &str,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) -> std::io::Result<()> {
    let listener = TcpListener::bind(bind)?;
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let state = Arc::clone(&state);
                let limits = Arc::clone(&limits);
                thread::spawn(move || {
                    if let Err(err) =
                        run_hl7_mllp_connection(stream, Arc::clone(&state), Arc::clone(&limits))
                    {
                        eprintln!("dicom-workflow-server MLLP connection failed: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("dicom-workflow-server MLLP accept failed: {err}");
            }
        }
    }
    Ok(())
}

fn run_hl7_mllp_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) -> std::io::Result<()> {
    let mut read_buffer = Vec::new();
    let mut incoming = [0u8; 8192];
    loop {
        let size = stream.read(&mut incoming)?;
        if size == 0 {
            return Ok(());
        }
        read_buffer.extend_from_slice(&incoming[..size]);
        while let Some(frame) = take_next_hl7_mllp_frame(&mut read_buffer, limits.max_input_bytes)?
        {
            let correlation_id = fallback_request_id();
            let (ack, error_code): (&str, Option<String>) =
                match run_hl7_ingest_payload(&frame, None, &correlation_id, &state, &limits) {
                    Ok(_) => ("AA", None),
                    Err(err) => {
                        let (ack, code) = classify_hl7_mllp_nack(err.as_ref());
                        (ack, Some(code))
                    }
                };
            let response = build_mllp_ack(ack, error_code.as_deref());
            stream.write_all(&response)?;
        }
    }
}

fn take_next_hl7_mllp_frame(
    read_buffer: &mut Vec<u8>,
    max_input_bytes: u64,
) -> std::io::Result<Option<Vec<u8>>> {
    let Some(start_idx) = read_buffer
        .iter()
        .position(|value| *value == HL7_MLLP_START_BYTE)
    else {
        if read_buffer.len() as u64 > max_input_bytes {
            return Err(IoError::new(
                IoErrorKind::InvalidInput,
                "HL7 MLLP frame exceeded max_input_bytes",
            ));
        }
        return Ok(None);
    };

    if start_idx > 0 {
        read_buffer.drain(0..start_idx);
    }

    let mut end_idx = None;
    let mut cursor = 1;
    while cursor + 1 < read_buffer.len() {
        if read_buffer[cursor] == HL7_MLLP_END_BYTES[0]
            && read_buffer[cursor + 1] == HL7_MLLP_END_BYTES[1]
        {
            end_idx = Some(cursor);
            break;
        }
        cursor += 1;
    }

    match end_idx {
        Some(cursor) => {
            if cursor < 1 {
                read_buffer.drain(0..2);
                return Ok(Some(Vec::new()));
            }
            if (cursor as u64 - 1) > max_input_bytes {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    "HL7 MLLP frame exceeded max_input_bytes",
                ));
            }

            let frame = read_buffer[1..cursor].to_vec();
            read_buffer.drain(0..(cursor + HL7_MLLP_END_BYTES.len()));
            Ok(Some(frame))
        }
        None => {
            if (read_buffer.len() as u64 - 1) > max_input_bytes {
                return Err(IoError::new(
                    IoErrorKind::InvalidInput,
                    "HL7 MLLP frame exceeded max_input_bytes",
                ));
            }
            Ok(None)
        }
    }
}

fn classify_hl7_mllp_nack(err: &Error) -> (&'static str, String) {
    if err.code == "DVF.WORKFLOW.SR.AUTH_DENIED" {
        ("AR", err.code.to_string())
    } else {
        ("AE", err.code.to_string())
    }
}

fn build_mllp_ack(ack: &str, error_code: Option<&str>) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.push(HL7_MLLP_START_BYTE);
    frame.extend_from_slice(format!("MSA|{ack}|ACK\r").as_bytes());
    if let Some(code) = error_code.map(str::trim).filter(|value| !value.is_empty()) {
        frame.extend_from_slice(format!("ERR|{code}\r").as_bytes());
    }
    frame.extend_from_slice(&HL7_MLLP_END_BYTES);
    frame
}

fn run_hl7_file_drop_worker(
    drop_dir: PathBuf,
    done_dir: PathBuf,
    error_dir: PathBuf,
    poll_interval_ms: u64,
    state: Arc<Mutex<RuntimeState>>,
    limits: Arc<Limits>,
) {
    loop {
        if let Err(err) = run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &state, &limits)
        {
            eprintln!("dicom-workflow-server HL7 file-drop worker error: {err}");
        }
        thread::sleep(Duration::from_millis(poll_interval_ms.max(1)));
    }
}

fn run_hl7_file_drop_once(
    drop_dir: &Path,
    done_dir: &Path,
    error_dir: &Path,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Arc<Limits>,
) -> std::io::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(drop_dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            files.push(entry.path());
        }
    }
    files.sort();

    for path in files {
        let body = fs::read(&path)?;
        let correlation_id = fallback_request_id();
        match run_hl7_ingest_payload(&body, None, &correlation_id, state, limits) {
            Ok(_) => move_hl7_drop_file(&path, done_dir)?,
            Err(_) => move_hl7_drop_file(&path, error_dir)?,
        };
    }

    Ok(())
}

fn move_hl7_drop_file(source_path: &Path, destination_dir: &Path) -> std::io::Result<()> {
    let source_file_name = source_path
        .file_name()
        .ok_or_else(|| IoError::new(IoErrorKind::InvalidInput, "invalid hl7 drop file name"))?;
    let file_name = source_file_name.to_string_lossy().to_string();
    let mut destination = destination_dir.join(&file_name);
    if destination.exists() {
        let mut index = 0usize;
        loop {
            index = index.saturating_add(1);
            let staged = if let Some(stem) = Path::new(&file_name).file_stem() {
                if let Some(ext) = Path::new(&file_name).extension() {
                    destination_dir.join(format!(
                        "{}.{index}.{}",
                        stem.to_string_lossy(),
                        ext.to_string_lossy()
                    ))
                } else {
                    destination_dir.join(format!("{file_name}.{index}"))
                }
            } else {
                destination_dir.join(format!("{file_name}.{index}"))
            };
            if !staged.exists() {
                destination = staged;
                break;
            }
        }
    }
    if fs::rename(source_path, &destination).is_err() {
        fs::copy(source_path, &destination)?;
        fs::remove_file(source_path)?;
    }
    Ok(())
}

fn hl7_replay_cache_key(
    idempotency_key: Option<&str>,
    source: &str,
    message_type: &Hl7MessageClass,
    params: &BTreeMap<String, String>,
    signature: &str,
) -> String {
    if let Some(value) = idempotency_key
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return format!("{}|{}:idempotency:{value}", source, message_type.as_label());
    }

    if let Some(message_control_id) = params
        .get("message_control_id")
        .or_else(|| params.get("message_id"))
        .filter(|value| !value.trim().is_empty())
    {
        return format!(
            "{}|{}:control:{}",
            source,
            message_type.as_label(),
            message_control_id
        );
    }

    if let Some(task_id) = params
        .get("task_id")
        .filter(|value| !value.trim().is_empty())
    {
        return format!("{}|{}:task:{task_id}", source, message_type.as_label());
    }

    if let Some(sop_instance_uid) = params
        .get("sop_instance_uid")
        .or_else(|| params.get("sop_uid"))
        .or_else(|| params.get("SOPInstanceUID"))
        .filter(|value| !value.trim().is_empty())
    {
        return format!(
            "{}|{}:sop:{sop_instance_uid}",
            source,
            message_type.as_label()
        );
    }

    format!("{}|{}:sig:{signature}", source, message_type.as_label())
}

fn run_hl7_ingest_payload(
    raw_body: &[u8],
    idempotency_key: Option<&str>,
    correlation_id: &str,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<String, Box<Error>> {
    let params = parse_hl7_payload(raw_body, limits)?;
    let source = normalize_identifier(params.get("source").unwrap_or(&"interop".to_string()));
    let message_type_raw = required_param(&params, "message_type")?;
    let message_class = parse_hl7_message_class(message_type_raw)
        .ok_or_else(|| decode_error("unsupported hl7 message type"))?;
    let payload_signature = build_request_signature(&params);
    let replay_key = hl7_replay_cache_key(
        idempotency_key,
        &source,
        &message_class,
        &params,
        &payload_signature,
    );

    let correlation_id = {
        let value = correlation_id.trim();
        if value.is_empty() {
            fallback_request_id()
        } else {
            value.to_string()
        }
    };

    let mut state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(entry) = state.hl7.replay_cache.get(&replay_key) {
        if entry.signature == payload_signature.as_str() {
            return Ok(entry.response.clone());
        }
        return Err(decode_error("hl7 replay payload mismatch"));
    }

    let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut state);
    let tenant = params
        .get("tenant")
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| TENANT_ID_DEFAULT.to_string());
    let ack = match message_class {
        Hl7MessageClass::Adt => handle_hl7_adt_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Orm => handle_hl7_orm_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Oru => handle_hl7_oru_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
        Hl7MessageClass::Siu => handle_hl7_siu_event(
            &source,
            &tenant,
            &event_id,
            &payload_signature,
            &params,
            &mut state,
        ),
    };
    let ack = match ack {
        Ok(response) => {
            publish_hl7_event(
                &mut state,
                &source,
                &message_class.as_label(),
                &params,
                &event_id,
                sequence,
                &correlation_id,
            );
            state.hl7.replay_cache.insert(
                replay_key,
                CachedMppsRequest {
                    signature: payload_signature,
                    response: response.clone(),
                },
            );
            hl7_replay_cache_limit(&mut state.hl7.replay_cache);
            response
        }
        Err(err) => {
            let failed_reason = err.to_string();
            let raw_body = String::from_utf8(raw_body.to_vec())
                .unwrap_or_else(|_| "[invalid utf8]".to_string());
            publish_hl7_failure(
                &mut state,
                &source,
                &message_class.as_label(),
                &raw_body,
                &failed_reason,
                &correlation_id,
                &event_id,
                sequence,
            );
            return Err(err);
        }
    };
    Ok(ack)
}

fn parse_hl7_payload(body: &[u8], limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    if body.len() as u64 > limits.max_input_bytes {
        return Err(limit_exceeded(
            "max_input_bytes",
            body.len() as u64,
            limits.max_input_bytes,
        ));
    }

    if let Ok(params) = parse_form_map(body, limits) {
        if let Some(raw_type) = params.get("message_type") {
            if parse_hl7_message_class(raw_type).is_some() {
                return Ok(params);
            }
        }
    }
    parse_hl7_raw_message_payload(body, limits)
}

fn parse_hl7_raw_message_payload(
    body: &[u8],
    limits: &Limits,
) -> Result<BTreeMap<String, String>, Box<Error>> {
    let body =
        std::str::from_utf8(body).map_err(|_| decode_error("hl7 payload is not valid UTF-8"))?;
    let mut source = String::new();
    let mut message_type = String::new();
    let mut message_control = String::new();
    let mut patient_id = String::new();
    let mut order_id = String::new();
    let mut performer = String::new();
    let mut status = String::new();
    let mut study = String::new();
    let mut series = String::new();
    let sop = String::new();
    let mut report = String::new();

    for raw_segment in body.split(|c| c == '\r' || c == '\n') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(rest) = segment.strip_prefix("MSH|") {
            let fields: Vec<&str> = rest.split('|').collect();
            message_type = hl7_field(fields.get(8)).unwrap_or("ADT").to_string();
            source = hl7_field(fields.get(2))
                .or_else(|| hl7_field(fields.get(0)))
                .unwrap_or("interop")
                .to_string();
            message_control = hl7_field(fields.get(9)).unwrap_or("MSG1").to_string();
        } else if let Some(raw) = segment.strip_prefix("PID|") {
            let fields: Vec<&str> = raw.split('|').collect();
            patient_id = hl7_field(fields.get(2)).unwrap_or("").to_string();
        } else if let Some(raw) = segment.strip_prefix("ORC|") {
            let fields: Vec<&str> = raw.split('|').collect();
            order_id = hl7_field(fields.get(1)).unwrap_or("").to_string();
            status = hl7_field(fields.get(4)).unwrap_or("").to_string();
            performer = hl7_field(fields.get(5)).unwrap_or("").to_string();
        } else if let Some(raw) = segment.strip_prefix("OBR|") {
            let fields: Vec<&str> = raw.split('|').collect();
            if order_id.is_empty() {
                order_id = hl7_field(fields.get(2)).unwrap_or("").to_string();
            }
            if study.is_empty() {
                study = hl7_field(fields.get(0)).unwrap_or("").to_string();
            }
            if series.is_empty() {
                series = hl7_field(fields.get(1)).unwrap_or("").to_string();
            }
            if status.is_empty() {
                status = hl7_field(fields.get(4)).unwrap_or("").to_string();
            }
            if report.is_empty() {
                report = hl7_field(fields.get(7)).unwrap_or("").to_string();
            }
            if report.is_empty() {
                report = hl7_field(fields.get(5)).unwrap_or("").to_string();
            }
        } else if let Some(raw) = segment.strip_prefix("OBX|") {
            if report.is_empty() {
                let fields: Vec<&str> = raw.split('|').collect();
                report = hl7_field(fields.get(4)).unwrap_or("").to_string();
            }
        }
    }

    let message_class = parse_hl7_message_class(&message_type)
        .ok_or_else(|| decode_error("unsupported hl7 message type"))?;
    let source = normalize_identifier(&source);
    let message_type = message_class.as_label().to_string();
    let mut payload = BTreeMap::new();
    let seed = if !order_id.is_empty() {
        normalize_identifier(&order_id)
    } else if !patient_id.is_empty() {
        normalize_identifier(&patient_id)
    } else if !message_control.is_empty() {
        normalize_identifier(&message_control)
    } else {
        "MSG-0001".to_string()
    };
    if source.is_empty() {
        return Err(decode_error("unsupported hl7 payload source"));
    }
    enforce_ascii_and_limits(&source, limits)?;
    payload.insert("source".to_string(), source);
    enforce_ascii_and_limits(&message_type, limits)?;
    payload.insert("message_type".to_string(), message_type);

    let scheduled_step_id = if !seed.is_empty() {
        format!("STEP-{seed}")
    } else {
        format!("STEP-{}", normalize_identifier(&message_control))
    };
    enforce_ascii_and_limits(&scheduled_step_id, limits)?;
    payload.insert("scheduled_step_id".to_string(), scheduled_step_id);

    let status_label = normalize_hl7_status(&status)
        .unwrap_or(TaskStatus::Scheduled)
        .as_label()
        .to_string();
    if !message_control.is_empty() {
        let message_control_id = normalize_identifier(&message_control);
        enforce_ascii_and_limits(&message_control_id, limits)?;
        payload.insert("message_control_id".to_string(), message_control_id);
    }
    enforce_ascii_and_limits(&status_label, limits)?;
    payload.insert("status".to_string(), status_label);

    if !patient_id.is_empty() {
        let patient_id = normalize_identifier(&patient_id);
        enforce_ascii_and_limits(&patient_id, limits)?;
        payload.insert("patient_id".to_string(), patient_id);
    }

    if !order_id.is_empty() {
        let requested_procedure_id = normalize_identifier(&order_id);
        enforce_ascii_and_limits(&requested_procedure_id, limits)?;
        payload.insert("requested_procedure_id".to_string(), requested_procedure_id);
    }

    if !performer.is_empty() {
        let worker = normalize_identifier(&performer);
        enforce_ascii_and_limits(&worker, limits)?;
        payload.insert("worker".to_string(), worker);
    }

    if !study.is_empty() {
        let study_instance_uid = normalize_identifier(&study);
        enforce_ascii_and_limits(&study_instance_uid, limits)?;
        payload.insert("study_instance_uid".to_string(), study_instance_uid);
    }

    if !series.is_empty() {
        let series_instance_uid = normalize_identifier(&series);
        enforce_ascii_and_limits(&series_instance_uid, limits)?;
        payload.insert("series_instance_uid".to_string(), series_instance_uid);
    }

    if !sop.is_empty() {
        let sop_instance_uid = normalize_identifier(&sop);
        enforce_ascii_and_limits(&sop_instance_uid, limits)?;
        payload.insert("sop_instance_uid".to_string(), sop_instance_uid);
    }

    if !report.is_empty() {
        let report_text = normalize_identifier(&report);
        enforce_ascii_and_limits(&report_text, limits)?;
        payload.insert("report_text".to_string(), report_text);
    }

    Ok(payload)
}
fn handle_hl7_failures(state: &Arc<Mutex<RuntimeState>>) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut records: Vec<&Hl7FailureRecord> = state.hl7.failures.iter().collect();
    records.reverse();
    let mut json = String::from("[");
    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"id\":\"{}\",\"source\":\"{}\",\"message_type\":\"{}\",\"reason\":\"{}\",\"payload_excerpt\":\"{}\",\"created_at_ms\":{},\"scope\":\"{}\",\"subscription_id\":\"{}\",\"event_id\":\"{}\",\"correlation_id\":\"{}\",\"sequence\":{},\"attempt\":{},\"max_attempts\":{}}}",
            escape_json(&record.id),
            escape_json(&record.source),
            escape_json(&record.message_type),
            escape_json(&record.reason),
            escape_json(&record.payload_excerpt),
            record.created_at_ms,
            escape_json(&record.scope),
            escape_json(&record.subscription_id),
            escape_json(&record.event_id),
            escape_json(&record.correlation_id),
            record.sequence,
            record.attempt,
            record.max_attempts,
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn handle_hl7_connector_status_dashboard(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connector_subscriptions: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_delivered_events: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_last_event_ms: BTreeMap<String, u64> = BTreeMap::new();
    let mut connector_callback_failures: BTreeMap<String, u64> = BTreeMap::new();
    let mut custom_connector_subscriptions = 0u64;
    let mut webhook_connector_subscriptions = 0u64;
    let mut message_bus_connector_subscriptions = 0u64;
    let mut wildcard_subscriptions = 0u64;
    let mut subscription_to_connector: BTreeMap<String, String> = BTreeMap::new();

    for subscription in state.hl7.subscriptions.values() {
        match &subscription.sink.kind {
            Hl7SinkKind::Custom(connector_alias) => {
                custom_connector_subscriptions = custom_connector_subscriptions.saturating_add(1);
                let subscription_count = connector_subscriptions
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *subscription_count = subscription_count.saturating_add(1);

                let delivered_events = connector_delivered_events
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *delivered_events = delivered_events.saturating_add(subscription.delivered_events);

                let last_event = connector_last_event_ms
                    .entry(connector_alias.clone())
                    .or_insert(0);
                *last_event = (*last_event).max(subscription.last_event_ms);
                let _ = subscription_to_connector
                    .insert(subscription.id.clone(), connector_alias.clone());
            }
            Hl7SinkKind::Webhook => {
                webhook_connector_subscriptions = webhook_connector_subscriptions.saturating_add(1);
            }
            Hl7SinkKind::MessageBus => {
                message_bus_connector_subscriptions =
                    message_bus_connector_subscriptions.saturating_add(1);
            }
        }

        if subscription.source == "*" {
            wildcard_subscriptions = wildcard_subscriptions.saturating_add(1);
        }
    }

    let mut failure_total = 0u64;
    let mut failure_ingest = 0u64;
    let mut failure_callback = 0u64;
    for failure in state.hl7.failures.iter() {
        failure_total = failure_total.saturating_add(1);
        match failure.scope.as_str() {
            "ingest" => {
                failure_ingest = failure_ingest.saturating_add(1);
            }
            "callback" => {
                failure_callback = failure_callback.saturating_add(1);
                if let Some(connector_alias) =
                    subscription_to_connector.get(&failure.subscription_id)
                {
                    let failures = connector_callback_failures
                        .entry(connector_alias.clone())
                        .or_insert(0);
                    *failures = failures.saturating_add(1);
                }
            }
            _ => {}
        }
    }

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let alias = &descriptor.alias;
        let target = &descriptor.target;
        let subscription_count = connector_subscriptions
            .get(alias)
            .copied()
            .unwrap_or_default();
        let delivered_events = connector_delivered_events
            .get(alias)
            .copied()
            .unwrap_or_default();
        let last_event_ms = connector_last_event_ms
            .get(alias)
            .copied()
            .unwrap_or_default();
        let callback_failures = connector_callback_failures
            .get(alias)
            .copied()
            .unwrap_or_default();
        let plugin_json = descriptor.plugin.as_ref().map_or_else(
            || "null".to_string(),
            |plugin| {
                format!(
                    "{{\"path\":\"{}\",\"version\":\"{}\",\"compatible_min\":\"{}\",\"compatible_max\":\"{}\"}}",
                    escape_json(&plugin.plugin_path),
                    escape_json(&plugin.adapter_version),
                    escape_json(&plugin.compatible_min),
                    escape_json(&plugin.compatible_max),
                )
            },
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"target\":\"{}\",\"adapter_kind\":\"{}\",\"subscription_count\":{},\"delivered_events\":{},\"last_event_ms\":{},\"callback_failures\":{},\"plugin\":{}}}",
            escape_json(alias),
            escape_json(target),
            descriptor.adapter_kind.as_label(),
            subscription_count,
            delivered_events,
            last_event_ms,
            callback_failures,
            plugin_json,
        ));
    }

    let mut json = String::new();
    json.push('{');
    json.push_str(&format!(
        "\"generated_at_ms\":{},\"connector_count\":{},",
        now_epoch_millis(),
        state.hl7.connector_registry.len()
    ));
    json.push_str("\"connectors\":[");
    for (index, connector) in connectors.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(connector);
    }
    json.push_str("]");
    json.push_str(&format!(
        "\"summary\":{{\"subscription_count\":{},\"custom_connector_subscriptions\":{},\"webhook_subscriptions\":{},\"message_bus_subscriptions\":{},\"wildcard_subscriptions\":{},\"failures\":{{\"total\":{},\"ingest\":{},\"callback\":{}}}}}",
        state.hl7.subscriptions.len(),
        custom_connector_subscriptions,
        webhook_connector_subscriptions,
        message_bus_connector_subscriptions,
        wildcard_subscriptions,
        failure_total,
        failure_ingest,
        failure_callback
    ));
    json.push('}');
    Ok(WorkflowResponse::Json(200, json))
}

fn handle_hl7_connector_features(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let alias = &descriptor.alias;
        let target = &descriptor.target;
        let feature_enabled =
            resolve_hl7_connector_feature_flag(&state.hl7.connector_feature_flags, alias);
        let rollout_percent =
            resolve_hl7_connector_rollout_percent(&state.hl7.connector_rollout_percent, alias);
        let plugin_json = descriptor.plugin.as_ref().map_or_else(
            || "null".to_string(),
            |plugin| {
                format!(
                    "{{\"path\":\"{}\",\"version\":\"{}\",\"compatible_min\":\"{}\",\"compatible_max\":\"{}\"}}",
                    escape_json(&plugin.plugin_path),
                    escape_json(&plugin.adapter_version),
                    escape_json(&plugin.compatible_min),
                    escape_json(&plugin.compatible_max),
                )
            },
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"target\":\"{}\",\"adapter_kind\":\"{}\",\"feature_enabled\":{},\"rollout_percent\":{},\"plugin\":{}}}",
            escape_json(alias),
            escape_json(target),
            descriptor.adapter_kind.as_label(),
            feature_enabled,
            rollout_percent,
            plugin_json,
        ));
    }

    let mut json = String::new();
    json.push('{');
    json.push_str(&format!(
        "\"generated_at_ms\":{},\"connector_count\":{},",
        now_epoch_millis(),
        state.hl7.connector_registry.len()
    ));
    json.push_str("\"connectors\":[");
    for (index, connector) in connectors.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(connector);
    }
    json.push(']');
    json.push('}');
    Ok(WorkflowResponse::Json(200, json))
}

fn handle_hl7_connector_rollout_list(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let rollout_percent = resolve_hl7_connector_rollout_percent(
            &state.hl7.connector_rollout_percent,
            &descriptor.alias,
        );
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"rollout_percent\":{},\"percent\":{}}}",
            escape_json(&descriptor.alias),
            rollout_percent,
            rollout_percent,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connector_count\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            state.hl7.connector_registry.len(),
            connectors.join(","),
        ),
    ))
}

fn handle_hl7_connector_rollout_update(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    enforce_admin_api_version(request)?;
    if actor.role.as_deref() != Some("admin") {
        return Err(auth_denied_error(
            "connector rollout update requires admin role",
        ));
    }
    let params = parse_form_map(&request.body, limits)?;
    let alias = params
        .get("alias")
        .map(|value| normalize_connector_alias(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| decode_error("missing required parameter: alias"))?;
    let rollout_percent = params
        .get("rollout_percent")
        .or_else(|| params.get("percent"))
        .ok_or_else(|| decode_error("missing required parameter: rollout_percent (or percent)"))?
        .trim()
        .parse::<u64>()
        .map_err(|_| decode_error("rollout_percent must be an integer between 0 and 100"))?;
    if rollout_percent > 100 {
        return Err(decode_error("rollout_percent must be within 0..=100"));
    }

    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !store.hl7.connector_registry.contains_key(&alias) {
        return Err(decode_error("unknown connector alias"));
    }
    let _ = store
        .hl7
        .connector_rollout_percent
        .insert(alias.clone(), rollout_percent);
    let rollout_snapshot_path = connector_rollout_snapshot_path(&store.audit_path);
    persist_hl7_connector_rollout_state(
        &rollout_snapshot_path,
        &store.hl7.connector_rollout_percent,
    );
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"status\":\"updated\",\"alias\":\"{}\",\"rollout_percent\":{},\"percent\":{},\"updated_at_ms\":{}}}",
            escape_json(&alias),
            rollout_percent,
            rollout_percent,
            now_epoch_millis(),
        ),
    ))
}

fn handle_hl7_connector_capabilities(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;

    let mut connectors = Vec::new();
    for descriptor in normalized_connector_descriptors(&state) {
        let version = descriptor
            .plugin
            .as_ref()
            .map(|plugin| plugin.adapter_version.as_str())
            .unwrap_or("1.0.0");
        let supported_message_classes = match descriptor.adapter_kind {
            NormalizedConnectorAdapterKind::HisRis => "[\"ADT\",\"ORM\",\"ORU\",\"SIU\"]",
            NormalizedConnectorAdapterKind::Dimse => "[\"C-FIND\",\"N-CREATE\",\"N-SET\"]",
            NormalizedConnectorAdapterKind::Generic => "[\"ADT\",\"ORU\"]",
        };
        connectors.push(format!(
            "{{\"alias\":\"{}\",\"adapter_kind\":\"{}\",\"version\":\"{}\",\"adapter_version\":\"{}\",\"supported_message_classes\":{}}}",
            escape_json(&descriptor.alias),
            descriptor.adapter_kind.as_label(),
            escape_json(version),
            escape_json(version),
            supported_message_classes,
        ));
    }

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connector_count\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            state.hl7.connector_registry.len(),
            connectors.join(","),
        ),
    ))
}

fn render_interop_error_json(code: &str, message: &str, detail: &str) -> String {
    format!(
        "{{\"error\":{{\"code\":\"{}\",\"message\":\"{}\",\"detail\":\"{}\"}}}}",
        escape_json(code),
        escape_json(message),
        escape_json(detail),
    )
}

fn handle_hl7_connector_health(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let descriptors = normalized_connector_descriptors(&state);
    if descriptors.is_empty() {
        return Ok(WorkflowResponse::Json(
            503,
            render_interop_error_json(
                "DVF.WORKFLOW.CONNECTOR_HEALTH.EMPTY",
                "connector health unavailable",
                "no connectors are configured",
            ),
        ));
    }
    let mut subscription_to_connector: BTreeMap<String, String> = BTreeMap::new();
    for subscription in state.hl7.subscriptions.values() {
        if let Hl7SinkKind::Custom(connector_alias) = &subscription.sink.kind {
            let _ =
                subscription_to_connector.insert(subscription.id.clone(), connector_alias.clone());
        }
    }
    let mut downstream_timeout_connectors: BTreeMap<String, bool> = BTreeMap::new();
    for failure in &state.hl7.failures {
        if failure.scope != "callback" {
            continue;
        }
        if !failure.reason.to_ascii_lowercase().contains("timeout") {
            continue;
        }
        if let Some(connector_alias) = subscription_to_connector.get(&failure.subscription_id) {
            let _ = downstream_timeout_connectors.insert(connector_alias.clone(), true);
        }
    }

    let mut rows = Vec::new();
    for descriptor in descriptors {
        let is_timeout = downstream_timeout_connectors
            .get(&descriptor.alias)
            .copied()
            .unwrap_or(false);
        let (status, reason) = if is_timeout {
            ("downstream_timeout", "recent callback timeout failures")
        } else if descriptor.target.trim().is_empty() {
            ("degraded", "connector target is empty")
        } else {
            ("healthy", "ok")
        };
        rows.push(format!(
            "{{\"alias\":\"{}\",\"adapter_kind\":\"{}\",\"target\":\"{}\",\"status\":\"{}\",\"reason\":\"{}\"}}",
            escape_json(&descriptor.alias),
            descriptor.adapter_kind.as_label(),
            escape_json(&descriptor.target),
            status,
            reason,
        ));
    }
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"generated_at_ms\":{},\"connectors\":[{}]}}",
            now_epoch_millis(),
            rows.join(","),
        ),
    ))
}

fn handle_hl7_subscriptions_list(
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut subscriptions: Vec<&Hl7Subscription> = state.hl7.subscriptions.values().collect();
    subscriptions.sort_by(|a, b| a.id.cmp(&b.id));
    let mut json = String::from("[");
    for (index, subscription) in subscriptions.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str(&format!(
            "\"id\":\"{}\",\"source\":\"{}\",\"event_filter\":{},\"sink_kind\":\"{}\",\"sink_target\":\"{}\",\"created_at_ms\":{},\"delivered_events\":{},\"last_event_ms\":{}",
            escape_json(&subscription.id),
            escape_json(&subscription.source),
            render_string_array(&subscription.event_filter),
            escape_json(subscription.sink.kind.as_label()),
            escape_json(&subscription.sink.target),
            subscription.created_at_ms,
            subscription.delivered_events,
            subscription.last_event_ms,
        ));
        json.push('}');
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn handle_hl7_subscriptions_create(
    request: &HttpRequest,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, &Limits::default())?;
    let source = normalize_identifier(required_param(&params, "source")?);
    let raw_kind = normalize_identifier(
        params
            .get("sink_kind")
            .or_else(|| params.get("sink"))
            .ok_or_else(|| decode_error("missing required parameter: sink_kind"))?,
    );
    let sink_kind = match raw_kind.to_ascii_lowercase().as_str() {
        "webhook" | "http" => Hl7SinkKind::Webhook,
        "message_bus" | "bus" | "messagebus" => Hl7SinkKind::MessageBus,
        "custom" | "connector" => {
            let connector = normalize_identifier(
                params
                    .get("sink_connector")
                    .or_else(|| params.get("connector"))
                    .ok_or_else(|| decode_error("missing required parameter: sink_connector"))?,
            );
            if connector.is_empty() {
                return Err(decode_error("missing required parameter: sink_connector"));
            }
            let registered = {
                let state = state
                    .lock()
                    .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
                resolve_hl7_connector_alias(&state.hl7.connector_registry, &connector).is_some()
            };
            if !registered {
                return Err(decode_error("unknown sink connector"));
            }
            Hl7SinkKind::Custom(connector.to_ascii_lowercase())
        }
        _ => return Err(decode_error("unsupported sink kind")),
    };
    let sink_target = normalize_identifier(
        params
            .get("sink_target")
            .or_else(|| params.get("target"))
            .ok_or_else(|| decode_error("missing required parameter: sink_target"))?,
    );
    if sink_target.is_empty() {
        return Err(decode_error("missing required parameter: sink_target"));
    }

    let event_filter = parse_hl7_event_filter(
        params
            .get("event_filter")
            .map(|value| value.as_str())
            .or_else(|| params.get("event").map(|value| value.as_str()))
            .unwrap_or("all"),
    )?;
    if event_filter.is_empty() {
        return Err(decode_error("missing required hl7 event filter"));
    }

    let mut state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let policy = tenant_policy(&state, TENANT_ID_DEFAULT);
    if state.hl7.subscriptions.len() >= MAX_HL7_SUBSCRIPTIONS {
        return Err(decode_error("subscription capacity exceeded"));
    }
    let source_subscriptions = state
        .hl7
        .subscriptions
        .values()
        .filter(|entry| entry.source == source || source == "*")
        .count();
    if source_subscriptions >= policy.subscription_quota {
        return Err(limit_exceeded(
            "workflow_subscription_quota",
            source_subscriptions as u64,
            policy.subscription_quota as u64,
        ));
    }
    state.hl7.subscription_seq = state.hl7.subscription_seq.saturating_add(1);
    let id = format!("sub-{0:05}", state.hl7.subscription_seq);
    let created_at_ms = now_epoch_millis();
    let subscription = Hl7Subscription {
        id: id.clone(),
        source,
        event_filter,
        sink: Hl7Sink {
            kind: sink_kind,
            target: sink_target,
        },
        delivered_events: 0,
        created_at_ms,
        last_event_ms: 0,
    };
    state
        .hl7
        .subscriptions
        .insert(id.clone(), subscription.clone());

    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"id\":\"{}\",\"source\":\"{}\",\"event_filter\":{},\"sink_kind\":\"{}\",\"sink_target\":\"{}\",\"created_at_ms\":{}}}",
            escape_json(&id),
            escape_json(&subscription.source),
            render_string_array(&subscription.event_filter),
            escape_json(subscription.sink.kind.as_label()),
            escape_json(&subscription.sink.target),
            created_at_ms
        ),
    ))
}

fn handle_reconciliation_jobs_list(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let tenant_filter = request
        .query
        .get("tenant")
        .cloned()
        .unwrap_or_else(|| actor.tenant.clone());
    let is_admin = actor.role.as_deref() == Some("admin");
    if !is_admin && tenant_filter != actor.tenant && tenant_filter != "*" && tenant_filter != "all"
    {
        return Err(auth_denied_error(
            "reconciliation tenant scope outside caller tenant",
        ));
    }

    let state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let mut jobs: Vec<&StudyReconciliationJob> = state
        .hl7
        .reconciliation_jobs
        .values()
        .filter(|job| {
            if tenant_filter == "*" || tenant_filter == "all" {
                return is_admin;
            }
            job.tenant == tenant_filter
        })
        .collect();
    jobs.sort_by(|a, b| a.id.cmp(&b.id));
    let mut json = String::from("[");
    for (index, job) in jobs.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"id\":\"{}\",\"tenant\":\"{}\",\"source\":\"{}\",\"target_endpoint\":\"{}\",\"interval_seconds\":{},\"enabled\":{},\"runs_enqueued\":{},\"runs_completed\":{},\"created_at_ms\":{},\"last_run_at_ms\":{}}}",
            escape_json(&job.id),
            escape_json(&job.tenant),
            escape_json(&job.source),
            escape_json(&job.target_endpoint),
            job.interval_seconds,
            if job.enabled { "true" } else { "false" },
            job.runs_enqueued,
            job.runs_completed,
            job.created_at_ms,
            job.last_run_at_ms,
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn parse_reconciliation_interval(raw: Option<&str>) -> Result<u64, Box<Error>> {
    let interval_seconds = raw
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(300);
    if !(RECONCILIATION_INTERVAL_FLOOR_SECONDS..=RECONCILIATION_INTERVAL_CEILING_SECONDS)
        .contains(&interval_seconds)
    {
        return Err(decode_error(format!(
            "interval_seconds must be within {}..={}",
            RECONCILIATION_INTERVAL_FLOOR_SECONDS, RECONCILIATION_INTERVAL_CEILING_SECONDS
        )));
    }
    Ok(interval_seconds)
}

fn handle_reconciliation_jobs_create(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let source = normalize_identifier(required_param(&params, "source")?);
    let target_endpoint = normalize_identifier(
        params
            .get("target_endpoint")
            .or_else(|| params.get("target"))
            .ok_or_else(|| decode_error("missing required parameter: target_endpoint"))?,
    );
    if target_endpoint.is_empty() {
        return Err(decode_error("missing required parameter: target_endpoint"));
    }
    let interval_seconds =
        parse_reconciliation_interval(params.get("interval_seconds").map(String::as_str))?;
    let mut state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if state.hl7.reconciliation_jobs.len() >= MAX_RECONCILIATION_JOBS {
        return Err(limit_exceeded(
            "workflow_reconciliation_jobs",
            state.hl7.reconciliation_jobs.len() as u64,
            MAX_RECONCILIATION_JOBS as u64,
        ));
    }
    let tenant_jobs = state
        .hl7
        .reconciliation_jobs
        .values()
        .filter(|job| job.tenant == actor.tenant)
        .count();
    if tenant_jobs >= MAX_RECONCILIATION_JOBS_PER_TENANT {
        return Err(limit_exceeded(
            "workflow_reconciliation_jobs_per_tenant",
            tenant_jobs as u64,
            MAX_RECONCILIATION_JOBS_PER_TENANT as u64,
        ));
    }
    state.hl7.reconciliation_seq = state.hl7.reconciliation_seq.saturating_add(1);
    let id = format!("recon-{0:05}", state.hl7.reconciliation_seq);
    let now_ms = now_epoch_millis();
    let job = StudyReconciliationJob {
        id: id.clone(),
        tenant: actor.tenant.clone(),
        source,
        target_endpoint: target_endpoint.clone(),
        interval_seconds,
        runs_enqueued: 0,
        runs_completed: 0,
        last_run_at_ms: 0,
        created_at_ms: now_ms,
        enabled: true,
    };
    state
        .hl7
        .reconciliation_jobs
        .insert(id.clone(), job.clone());
    Ok(WorkflowResponse::Json(
        200,
        format!(
            "{{\"id\":\"{}\",\"tenant\":\"{}\",\"source\":\"{}\",\"target_endpoint\":\"{}\",\"interval_seconds\":{},\"enabled\":{}}}",
            escape_json(&job.id),
            escape_json(&job.tenant),
            escape_json(&job.source),
            escape_json(&job.target_endpoint),
            job.interval_seconds,
            if job.enabled { "true" } else { "false" },
        ),
    ))
}

fn handle_reconciliation_jobs_run(
    job_id: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let id = normalize_route_identifier(job_id)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .map(|value| normalize_identifier(value))
        .unwrap_or_default();
    if idempotency_key.is_empty() {
        return Err(decode_error("missing required x-idempotency-key header"));
    }
    let mut state = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let idempotency_cache_key = format!(
        "{RECONCILIATION_RUN_IDEMPOTENCY_PREFIX}{}:{}:{}",
        actor.tenant, id, idempotency_key
    );
    if let Some(cached) = state.task_idempotency.get(&idempotency_cache_key) {
        let replay = cached.response.replacen(
            "\"idempotency_replay\":false",
            "\"idempotency_replay\":true",
            1,
        );
        return Ok(WorkflowResponse::Json(200, replay));
    }
    let job = state
        .hl7
        .reconciliation_jobs
        .get_mut(&id)
        .ok_or_else(|| decode_error("reconciliation job not found"))?;
    if actor.role.as_deref() != Some("admin") && job.tenant != actor.tenant {
        return Err(auth_denied_error(
            "reconciliation job belongs to another tenant",
        ));
    }
    if !job.enabled {
        return Err(decode_error("reconciliation job is disabled"));
    }
    job.runs_enqueued = job.runs_enqueued.saturating_add(1);
    job.last_run_at_ms = now_epoch_millis();
    job.runs_completed = job.runs_completed.saturating_add(1);
    let response = format!(
        "{{\"id\":\"{}\",\"run_status\":\"ok\",\"runs_enqueued\":{},\"runs_completed\":{},\"last_run_at_ms\":{},\"idempotency_replay\":false}}",
        escape_json(&job.id),
        job.runs_enqueued,
        job.runs_completed,
        job.last_run_at_ms,
    );
    let response_signature = hash_text(&format!(
        "{}|{}|{}|{}|{}",
        job.id, job.runs_enqueued, job.runs_completed, job.last_run_at_ms, actor.tenant
    ));
    let _ = state.task_idempotency.insert(
        idempotency_cache_key,
        CachedMppsRequest {
            signature: response_signature,
            response: response.clone(),
        },
    );
    task_idempotency_limit(&mut state.task_idempotency);
    let reconciliation_idempotency_path =
        reconciliation_run_idempotency_snapshot_path(&state.audit_path);
    persist_reconciliation_run_idempotency_cache(
        &reconciliation_idempotency_path,
        &state.task_idempotency,
    );
    Ok(WorkflowResponse::Json(200, response))
}

fn publish_hl7_failure(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    raw_body: &str,
    reason: &str,
    correlation_id: &str,
    event_id: &str,
    sequence: u64,
) {
    publish_hl7_failure_record(
        state,
        source,
        message_type,
        raw_body,
        reason,
        "ingest",
        "",
        0,
        0,
        event_id,
        sequence,
        correlation_id,
    );
}

fn publish_hl7_failure_record(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    raw_body: &str,
    reason: &str,
    scope: &str,
    subscription_id: &str,
    attempt: u32,
    max_attempts: u32,
    event_id: &str,
    sequence: u64,
    correlation_id: &str,
) {
    let excerpt = if raw_body.len() > MAX_HL7_FAILURE_EXCERPT_BYTES {
        &raw_body[..MAX_HL7_FAILURE_EXCERPT_BYTES]
    } else {
        raw_body
    };
    state.hl7.failure_seq = state.hl7.failure_seq.saturating_add(1);
    state.hl7.failures.push_front(Hl7FailureRecord {
        id: format!("hl7-fail-{0:06}", state.hl7.failure_seq),
        source: normalize_identifier(source),
        message_type: normalize_identifier(message_type),
        reason: normalize_identifier(reason),
        payload_excerpt: excerpt.to_string(),
        created_at_ms: now_epoch_millis(),
        scope: normalize_identifier(scope),
        subscription_id: normalize_identifier(subscription_id),
        event_id: normalize_identifier(event_id),
        correlation_id: normalize_identifier(correlation_id),
        sequence,
        attempt,
        max_attempts,
    });
    while state.hl7.failures.len() > MAX_HL7_FAILURES {
        let _ = state.hl7.failures.pop_back();
    }
    let failure_queue_path = hl7_failure_queue_snapshot_path(&state.audit_path);
    persist_hl7_failure_queue(&failure_queue_path, &state.hl7.failures);
}

fn next_hl7_event_id_with_sequence(state: &mut RuntimeState) -> (String, u64) {
    let next = state.hl7.event_seq.saturating_add(1);
    state.hl7.event_seq = next;
    (format!("HL7-{0:06}", next), next)
}

fn render_callback_payload_excerpt(
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
) -> String {
    let mut excerpt = format!("event_type={message_type},source={source}");
    for (key, value) in payload.iter().take(4) {
        excerpt.push(',');
        excerpt.push_str(key);
        excerpt.push('=');
        excerpt.push_str(value);
    }
    if excerpt.len() > MAX_HL7_FAILURE_EXCERPT_BYTES {
        excerpt[..MAX_HL7_FAILURE_EXCERPT_BYTES].to_string()
    } else {
        excerpt
    }
}

fn publish_hl7_callback_failure(
    state: &mut RuntimeState,
    subscription: &Hl7Subscription,
    message_type: &str,
    payload: &BTreeMap<String, String>,
    event_id: &str,
    correlation_id: &str,
    sequence: u64,
    attempt: u32,
    max_attempts: u32,
) {
    let reason = format!("callback delivery failed after attempt {attempt} of {max_attempts}");
    let raw_body = render_callback_payload_excerpt(message_type, &subscription.source, payload);
    publish_hl7_failure_record(
        state,
        &subscription.source,
        message_type,
        &raw_body,
        &reason,
        "callback",
        &subscription.id,
        attempt,
        max_attempts,
        event_id,
        sequence,
        correlation_id,
    );
}

fn webhook_auth_strategy_from_env() -> Result<WebhookAuthStrategy, &'static str> {
    let raw = env::var(WEBHOOK_AUTH_STRATEGY_ENV).unwrap_or_else(|_| "none".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(WebhookAuthStrategy::None),
        "hmac-sha256" | "hmac_sha256" => Ok(WebhookAuthStrategy::HmacSha256),
        _ => Err("unsupported webhook auth strategy"),
    }
}

fn resolve_webhook_sink_signature(
    subscription: &Hl7Subscription,
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
    correlation_id: &str,
    sequence: u64,
) -> Result<Option<String>, &'static str> {
    let strategy = webhook_auth_strategy_from_env()?;
    match strategy {
        WebhookAuthStrategy::None => Ok(None),
        WebhookAuthStrategy::HmacSha256 => {
            let secret = env::var(WEBHOOK_AUTH_SECRET_ENV)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .ok_or("webhook auth strategy hmac-sha256 requires shared secret")?;
            let excerpt = render_callback_payload_excerpt(message_type, source, payload);
            Ok(Some(hash_text(&format!(
                "{}|{}|{}|{}|{}|{}|{}",
                secret,
                subscription.sink.target,
                message_type,
                source,
                correlation_id,
                sequence,
                excerpt
            ))))
        }
    }
}

fn publish_hl7_event_attempt(
    subscription: &Hl7Subscription,
    message_type: &str,
    source: &str,
    payload: &BTreeMap<String, String>,
    correlation_id: &str,
    sequence: u64,
) -> Result<(), &'static str> {
    if matches!(subscription.sink.kind, Hl7SinkKind::Webhook)
        && (subscription.sink.target.starts_with("http://")
            || subscription.sink.target.starts_with("https://"))
    {
        let _ = resolve_webhook_sink_signature(
            subscription,
            message_type,
            source,
            payload,
            correlation_id,
            sequence,
        )?;
    }
    if subscription
        .sink
        .target
        .to_ascii_lowercase()
        .contains("callback-fail")
    {
        return Err("simulated callback failure");
    }
    Ok(())
}

fn callback_circuit_breaker_backoff_ms(
    consecutive_failures: u32,
    failure_threshold: u32,
    base_backoff_ms: u64,
    max_backoff_ms: u64,
) -> u64 {
    let exponent = consecutive_failures.saturating_sub(failure_threshold);
    let shifted = base_backoff_ms
        .checked_shl(exponent.min(8))
        .unwrap_or(max_backoff_ms);
    shifted.min(max_backoff_ms)
}

fn publish_hl7_event(
    state: &mut RuntimeState,
    source: &str,
    message_type: &str,
    params: &BTreeMap<String, String>,
    event_id: &str,
    sequence: u64,
    correlation_id: &str,
) {
    let source = normalize_identifier(source);
    let message_type = normalize_identifier(message_type).to_ascii_lowercase();
    let now_ms = now_epoch_millis();
    hl7_callback_idempotency_prune(
        &mut state.hl7.callback_delivery_idempotency,
        now_ms,
        state.hl7.callback_idempotency_ttl_ms,
    );
    persist_hl7_callback_idempotency_cache(
        &state.hl7.callback_idempotency_path,
        &state.hl7.callback_delivery_idempotency,
    );
    let subscriptions: Vec<Hl7Subscription> = state.hl7.subscriptions.values().cloned().collect();
    for subscription in subscriptions {
        if !subscription.event_filter.iter().any(|candidate| {
            candidate == "all"
                || candidate == "*"
                || candidate == message_type.as_str()
                || (candidate == "workflow"
                    && matches!(message_type.as_str(), "task" | "mpps" | "sr"))
        }) {
            continue;
        }
        if subscription.source != source && subscription.source != "*" {
            continue;
        }
        let callback_idempotency_key = format!("{event_id}|{}", subscription.id);
        if state
            .hl7
            .callback_delivery_idempotency
            .contains_key(&callback_idempotency_key)
        {
            continue;
        }
        let connector_alias = match &subscription.sink.kind {
            Hl7SinkKind::Custom(alias) => Some(normalize_connector_alias(alias)),
            _ => None,
        };
        if let Some(alias) = connector_alias.as_deref() {
            if let Some(open_until_ms) = state
                .hl7
                .connector_circuit_open_until_ms
                .get(alias)
                .copied()
            {
                if open_until_ms > now_ms {
                    continue;
                }
                state.hl7.connector_circuit_open_until_ms.remove(alias);
            }
            let feature_enabled =
                resolve_hl7_connector_feature_flag(&state.hl7.connector_feature_flags, alias);
            if !feature_enabled {
                continue;
            }
            let rollout_percent =
                resolve_hl7_connector_rollout_percent(&state.hl7.connector_rollout_percent, alias);
            if !hl7_connector_rollout_allows(event_id, alias, rollout_percent) {
                continue;
            }
        }
        let mut delivered = false;
        for attempt in 1..=state.hl7.callback_max_attempts {
            if publish_hl7_event_attempt(
                &subscription,
                message_type.as_str(),
                source.as_str(),
                params,
                correlation_id,
                sequence,
            )
            .is_ok()
            {
                delivered = true;
                break;
            }
            if attempt == state.hl7.callback_max_attempts {
                publish_hl7_callback_failure(
                    state,
                    &subscription,
                    message_type.as_str(),
                    params,
                    event_id,
                    correlation_id,
                    sequence,
                    attempt,
                    state.hl7.callback_max_attempts,
                );
                if let Some(alias) = connector_alias.as_deref() {
                    let failure_streak = state
                        .hl7
                        .connector_callback_failure_streak
                        .entry(alias.to_string())
                        .or_insert(0);
                    *failure_streak = failure_streak.saturating_add(1);
                    if *failure_streak >= state.hl7.callback_circuit_breaker_failure_threshold {
                        let backoff_ms = callback_circuit_breaker_backoff_ms(
                            *failure_streak,
                            state.hl7.callback_circuit_breaker_failure_threshold,
                            state.hl7.callback_circuit_breaker_base_backoff_ms,
                            state.hl7.callback_circuit_breaker_max_backoff_ms,
                        );
                        let _ = state
                            .hl7
                            .connector_circuit_open_until_ms
                            .insert(alias.to_string(), now_ms.saturating_add(backoff_ms));
                    }
                }
            }
        }
        if delivered {
            if let Some(alias) = connector_alias.as_deref() {
                state.hl7.connector_callback_failure_streak.remove(alias);
                state.hl7.connector_circuit_open_until_ms.remove(alias);
            }
            if let Some(subscription_state) = state.hl7.subscriptions.get_mut(&subscription.id) {
                subscription_state.delivered_events =
                    subscription_state.delivered_events.saturating_add(1);
                subscription_state.last_event_ms = now_ms;
            }
            state
                .hl7
                .callback_delivery_idempotency
                .insert(callback_idempotency_key, now_ms);
            hl7_callback_idempotency_prune(
                &mut state.hl7.callback_delivery_idempotency,
                now_ms,
                state.hl7.callback_idempotency_ttl_ms,
            );
            persist_hl7_callback_idempotency_cache(
                &state.hl7.callback_idempotency_path,
                &state.hl7.callback_delivery_idempotency,
            );
        }
    }
}

fn parse_hl7_event_filter(raw: &str) -> Result<Vec<String>, Box<Error>> {
    let mut out = Vec::new();
    for candidate in raw.split(',') {
        let token = normalize_identifier(candidate);
        if token.is_empty() {
            continue;
        }
        let token = token.to_ascii_lowercase();
        match token.as_str() {
            "adt" | "orm" | "oru" | "siu" | "task" | "mpps" | "sr" | "workflow" | "all" | "*" => {
                out.push(token)
            }
            _ => return Err(decode_error("unsupported hl7 event filter")),
        }
    }
    if out.is_empty() {
        out.push("all".to_string());
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn render_string_array(values: &[String]) -> String {
    let mut out = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(&escape_json(value));
        out.push('"');
    }
    out.push(']');
    out
}

fn normalize_hl7_status(raw: &str) -> Option<TaskStatus> {
    match raw.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "SCHEDULED" | "PENDING" | "ADMIT" | "REGISTER" | "CREATE" => Some(TaskStatus::Scheduled),
        "IN PROGRESS" | "INPROGRESS" | "START" | "RUN" => Some(TaskStatus::InProgress),
        "ON HOLD" | "ONHOLD" | "HOLD" => Some(TaskStatus::OnHold),
        "COMPLETED" => Some(TaskStatus::Completed),
        "REVIEWED" => Some(TaskStatus::Reviewed),
        "COMMITTED" => Some(TaskStatus::Committed),
        "DISCONTINUED" | "CANCELED" | "CANCELLED" | "DISCHARGE" => Some(TaskStatus::Discontinued),
        _ => None,
    }
}

fn parse_hl7_task_id(
    params: &BTreeMap<String, String>,
    source: &str,
) -> Result<String, Box<Error>> {
    if let Some(value) = params
        .get("task_id")
        .or_else(|| params.get("task"))
        .cloned()
    {
        let value = normalize_identifier(&value);
        if !value.is_empty() {
            return Ok(value);
        }
    }
    if let Some(step) = params.get("scheduled_step_id").cloned() {
        let sanitized = normalize_identifier(&step);
        if !sanitized.is_empty() {
            return Ok(format!("STEP-{sanitized}"));
        }
    }
    if let Some(patient) = params.get("patient_id") {
        let value = normalize_identifier(patient);
        if !value.is_empty() {
            return Ok(format!("PAT-{value}"));
        }
    }
    if let Some(accession) = params.get("accession_number") {
        let value = normalize_identifier(accession);
        if !value.is_empty() {
            return Ok(format!("ACC-{value}"));
        }
    }
    Ok(format!("HL7-{source}"))
}

fn handle_hl7_adt_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let task_id = parse_hl7_task_id(params, source)?;
    let scheduled_step_id = required_identifier_from_map(
        params,
        &["scheduled_step_id", "ScheduledStepID", "step_id"],
        "scheduled_step_id",
    )?;
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let now = now_epoch_millis();
    let requested_status = params
        .get("status")
        .and_then(|raw| normalize_hl7_status(raw))
        .unwrap_or(TaskStatus::Scheduled);

    let created = match state.tasks.entry(task_id.clone()) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let task = entry.get_mut();
            if task.tenant != tenant {
                return Err(auth_denied_error(
                    "hl7 task update denied because tenant is not owner",
                ));
            }
            if task.status != requested_status {
                if !task.status.can_transition_to(requested_status) {
                    return Err(task_status_invalid_transition_error());
                }
                task.status = requested_status;
            }
            task.scheduled_step_id = scheduled_step_id;
            task.requested_procedure_id = requested_procedure_id;
            if worker.is_some() {
                task.worker = worker;
            }
            task.updated_at_ms = now;
            false
        }
        std::collections::btree_map::Entry::Vacant(v) => {
            let _ = v.insert(ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status: requested_status,
                worker,
                tenant: tenant.to_string(),
                created_at_ms: now,
                updated_at_ms: now,
            });
            route_id_to_tenant_index(&mut state.tenant_tasks, tenant, &task_id);
            true
        }
    };
    let status = state.tasks[&task_id].status.as_label().to_string();
    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ADT\",\"task_id\":\"{}\",\"task_status\":\"{}\",\"outcome\":\"{}\",\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&task_id),
        escape_json(&status),
        if created { "inserted" } else { "updated" },
        escape_json(payload_signature)
    ))
}

fn handle_hl7_orm_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let task_id = parse_hl7_task_id(params, source)?;
    let scheduled_step_id = required_identifier_from_map(
        params,
        &["scheduled_step_id", "ScheduledStepID", "step_id"],
        "scheduled_step_id",
    )?;
    let requested_procedure_id = params.get("requested_procedure_id").cloned();
    let worker = params.get("worker").cloned();
    let now = now_epoch_millis();
    let requested_status = params
        .get("status")
        .and_then(|raw| normalize_hl7_status(raw))
        .unwrap_or(TaskStatus::Scheduled);

    let created = match state.tasks.entry(task_id.clone()) {
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let task = entry.get_mut();
            if task.tenant != tenant {
                return Err(auth_denied_error(
                    "hl7 task update denied because tenant is not owner",
                ));
            }
            if task.status != requested_status {
                if !task.status.can_transition_to(requested_status) {
                    return Err(task_status_invalid_transition_error());
                }
                task.status = requested_status;
            }
            task.scheduled_step_id = scheduled_step_id;
            task.requested_procedure_id = requested_procedure_id;
            if worker.is_some() {
                task.worker = worker;
            }
            task.updated_at_ms = now;
            false
        }
        std::collections::btree_map::Entry::Vacant(v) => {
            let _ = v.insert(ProcedureTask {
                task_id: task_id.clone(),
                scheduled_step_id,
                requested_procedure_id,
                status: requested_status,
                worker,
                tenant: tenant.to_string(),
                created_at_ms: now,
                updated_at_ms: now,
            });
            route_id_to_tenant_index(&mut state.tenant_tasks, tenant, &task_id);
            true
        }
    };
    let status = state.tasks[&task_id].status.as_label().to_string();

    let mut mpps_outcome = None;
    if let (Some(sop_instance_uid), Some(performed_step_id), Some(start_date), Some(start_time)) = (
        params.get("sop_instance_uid"),
        params.get("performed_step_id"),
        params.get("start_date"),
        params.get("start_time"),
    ) {
        let status = normalize_hl7_status(
            params
                .get("mpps_status")
                .unwrap_or(&"IN_PROGRESS".to_string()),
        )
        .unwrap_or(TaskStatus::InProgress);
        let mpps_status = if status == TaskStatus::Completed {
            MppsStatus::Completed
        } else if status == TaskStatus::Discontinued {
            MppsStatus::Discontinued
        } else {
            MppsStatus::InProgress
        };
        let mut update = BTreeMap::new();
        update.insert("sop_instance_uid".to_string(), sop_instance_uid.to_string());
        update.insert(
            "performed_step_id".to_string(),
            performed_step_id.to_string(),
        );
        update.insert(
            "status".to_string(),
            mpps_status_label(mpps_status).to_string(),
        );
        update.insert("start_date".to_string(), start_date.to_string());
        update.insert("start_time".to_string(), start_time.to_string());
        let dataset = build_mpps_dataset(&update)?;
        let outcome = state.mpps.ingest(&dataset)?;
        route_id_to_tenant_index(&mut state.tenant_mpps, tenant, sop_instance_uid);
        mpps_outcome = Some(render_mpps_ingest_outcome_json(&outcome));
    }

    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ORM\",\"task_id\":\"{}\",\"task_status\":\"{}\",\"mpps_outcome\":{},\"outcome\":\"{}\",\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&task_id),
        escape_json(&status),
        mpps_outcome.unwrap_or_else(|| "\"not_configured\"".to_string()),
        if created { "inserted" } else { "updated" },
        escape_json(payload_signature)
    ))
}

fn handle_hl7_oru_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let study_instance_uid = required_identifier_from_map(
        params,
        &["study_instance_uid", "study_uid", "StudyInstanceUID"],
        "study_instance_uid",
    )?;
    let series_instance_uid = required_identifier_from_map(
        params,
        &["series_instance_uid", "series_uid", "SeriesInstanceUID"],
        "series_instance_uid",
    )?;
    let sop_instance_uid = required_identifier_from_map(
        params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID"],
        "sop_instance_uid",
    )?;
    let report = params
        .get("report_text")
        .or_else(|| params.get("text"))
        .unwrap_or(&"Interop generated report".to_string())
        .to_string();
    let observed = parse_u64_or_default(params.get("authored_epoch_ms"));
    let auth = SrAuthContext {
        principal: Some(source.to_string()),
        can_write: true,
    };
    let item = SrAuthoringContentItem::Text {
        concept: Code {
            code_value: "121071".to_string(),
            scheme: "DCM".to_string(),
            meaning: "Finding".to_string(),
        },
        text: report.clone(),
        referenced_sop_instance_uid: None,
    };
    let outcome = if let Some(current) = state.sr.get(&sop_instance_uid) {
        state.sr.update(
            SrUpdateEnvelope {
                sop_instance_uid: sop_instance_uid.to_string(),
                expected_version: current.version,
                item,
                observer: Some(source.to_string()),
                known_referenced_sop_instance_uids: Vec::new(),
                idempotency_key: event_id.to_string(),
                request_id: Some(format!("hl7:{event_id}")),
            },
            &auth,
        )?
    } else {
        state.sr.create(
            SrCreateRequest {
                study_instance_uid: study_instance_uid.clone(),
                series_instance_uid: series_instance_uid.clone(),
                sop_instance_uid: sop_instance_uid.to_string(),
                observer: source.to_string(),
                authored_epoch_ms: observed.max(1),
                item,
                known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
                idempotency_key: event_id.to_string(),
                request_id: Some(format!("hl7:{event_id}")),
            },
            &auth,
        )?
    };
    let outcome_label = match outcome.kind {
        SrWriteOutcomeKind::Created => "created",
        SrWriteOutcomeKind::Updated => "updated",
        SrWriteOutcomeKind::Duplicate => "duplicate",
    };
    route_id_to_tenant_index(&mut state.tenant_sr, tenant, &sop_instance_uid);
    Ok(format!(
        "{{\"ack\":\"AA\",\"event_id\":\"{}\",\"message_type\":\"ORU\",\"sop_instance_uid\":\"{}\",\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"outcome\":\"{}\",\"version\":{},\"idempotency_replay\":{},\"signature\":\"{}\"}}",
        escape_json(event_id),
        escape_json(&sop_instance_uid),
        escape_json(&study_instance_uid),
        escape_json(&series_instance_uid),
        outcome_label,
        outcome.version,
        if outcome.idempotency_replay { "true" } else { "false" },
        escape_json(payload_signature),
    ))
}

fn handle_hl7_siu_event(
    source: &str,
    tenant: &str,
    event_id: &str,
    payload_signature: &str,
    params: &BTreeMap<String, String>,
    state: &mut RuntimeState,
) -> Result<String, Box<Error>> {
    let adt_ack = handle_hl7_adt_event(source, tenant, event_id, payload_signature, params, state)?;
    Ok(adt_ack.replace("\"message_type\":\"ADT\"", "\"message_type\":\"SIU\""))
}

fn parse_u64_or_default(raw: Option<&String>) -> u64 {
    raw.and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(now_epoch_millis)
}

fn handle_mpps_ingest(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let sop_instance_uid = required_identifier_from_map(
        &params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
    )?;
    let payload_signature = build_request_signature(&params);
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let cache_key = if idempotency_key.is_empty() {
        None
    } else {
        Some(format!("mpps-update:{idempotency_key}"))
    };
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if let Some(owner_tenant) = tenant_of_id(&store.tenant_mpps, &sop_instance_uid) {
        if owner_tenant != actor.tenant {
            return Err(auth_denied_error("mpps belongs to another tenant"));
        }
    }
    let dataset = build_mpps_dataset(&params)?;
    if let Some(key) = &cache_key {
        if let Some(entry) = store.mpps_idempotency.get(key) {
            if entry.signature == payload_signature {
                return Ok(WorkflowResponse::Json(200, entry.response.clone()));
            }
            return Err(mpps_idempotency_conflict_error());
        }
    }
    let outcome = store.mpps.ingest(&dataset)?;
    let body = render_mpps_ingest_outcome_json(&outcome);
    route_id_to_tenant_index(&mut store.tenant_mpps, &actor.tenant, &sop_instance_uid);
    if let Some(key) = cache_key {
        store.mpps_idempotency.insert(
            key,
            CachedMppsRequest {
                signature: payload_signature,
                response: body.clone(),
            },
        );
        enforce_mpps_idempotency_capacity(&mut store.mpps_idempotency);
    }
    Ok(WorkflowResponse::Json(200, body))
}

fn render_mpps_ingest_outcome_json(outcome: &dicom_mpps::IngestOutcome) -> String {
    let outcome = match outcome {
        MppsIngestOutcome::Inserted => "inserted",
        MppsIngestOutcome::Updated => "updated",
        MppsIngestOutcome::Duplicate => "duplicate",
    };
    format!("{{\"outcome\":\"{outcome}\"}}")
}

fn render_mpps_update_json(update: &dicom_mpps::MppsUpdate) -> String {
    let mut json = String::new();
    json.push_str("{\"sop_instance_uid\":\"");
    json.push_str(&escape_json(&update.sop_instance_uid));
    json.push_str("\",\"status\":\"");
    json.push_str(mpps_status_label(update.status));
    json.push_str("\",\"performed_step_id\":\"");
    json.push_str(&escape_json(&update.performed_step_id));
    json.push_str("\",\"start_date\":\"");
    json.push_str(&escape_json(&update.start_date));
    json.push_str("\",\"start_time\":\"");
    json.push_str(&escape_json(&update.start_time));
    if let Some(end_date) = &update.end_date {
        json.push_str("\",\"end_date\":\"");
        json.push_str(&escape_json(end_date));
    }
    if let Some(end_time) = &update.end_time {
        json.push_str("\",\"end_time\":\"");
        json.push_str(&escape_json(end_time));
    }
    json.push('}');
    json
}

fn parse_mpps_status(raw: &str, message: &str) -> Result<MppsStatus, Box<Error>> {
    match raw.trim().replace('_', " ").to_ascii_uppercase().as_str() {
        "IN PROGRESS" => Ok(MppsStatus::InProgress),
        "COMPLETED" => Ok(MppsStatus::Completed),
        "DISCONTINUED" => Ok(MppsStatus::Discontinued),
        "INPROGRESS" => Ok(MppsStatus::InProgress),
        _ => Err(decode_error(message)),
    }
}

fn parse_mpps_status_filter(raw: Option<&String>) -> Result<Option<&'static str>, Box<Error>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let normalized = raw.trim().replace('_', " ").to_ascii_uppercase();
    let status = match normalized.as_str() {
        "IN PROGRESS" | "INPROGRESS" => "IN PROGRESS",
        "COMPLETED" => "COMPLETED",
        "DISCONTINUED" => "DISCONTINUED",
        "SCHEDULED" | "PENDING" | "UNCLAIMED" => "SCHEDULED",
        _ => return Err(decode_error("invalid MPPS status filter")),
    };
    Ok(Some(status))
}

fn mpps_not_found_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.MPPS.NOT_FOUND",
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-mpps".to_string(),
            detail: "MPPS instance not found".to_string(),
        },
        "MPPS instance not found",
    )
    .into()
}

fn mpps_idempotency_conflict_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.MPPS.IDEMPOTENCY_CONFLICT",
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-mpps".to_string(),
            detail: "idempotency key replay conflict".to_string(),
        },
        "idempotency key replay conflict",
    )
    .into()
}

fn build_request_signature(params: &BTreeMap<String, String>) -> String {
    let mut output = String::new();
    let mut first = true;
    for (key, value) in params {
        if !first {
            output.push('&');
        }
        first = false;
        output.push_str(key);
        output.push('=');
        output.push_str(value);
    }
    output
}

fn enforce_mpps_idempotency_capacity(cache: &mut BTreeMap<String, CachedMppsRequest>) {
    while cache.len() > MAX_MPPS_IDEMPOTENCY_ENTRIES {
        let Some(oldest) = cache.keys().next().cloned() else {
            break;
        };
        let _ = cache.remove(&oldest);
    }
}

fn handle_sr_list(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let query = merge_query_parameters(&request.query, limits)?;
    let mut docs: Vec<_> = {
        let store = state
            .lock()
            .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
        let mut docs: Vec<_> = Vec::new();
        for doc in store.sr.documents() {
            if !actor_has_resource_access(actor, &store.tenant_sr, &doc.provenance.sop_instance_uid)
            {
                continue;
            }
            let status = store
                .sr
                .lifecycle_status(&doc.provenance.sop_instance_uid)
                .unwrap_or(SrLifecycleStatus::Draft);
            docs.push((doc.clone(), status));
        }
        docs
    };
    if let Some(study_uid) = lookup_identifier_from_map(
        &query,
        &[
            "study_uid",
            "study_instance_uid",
            "StudyInstanceUID",
            "StudyUID",
            "study_uid",
        ],
    ) {
        docs.retain(|(doc, _status)| doc.provenance.study_instance_uid == *study_uid);
    }
    if let Some(series_uid) = lookup_identifier_from_map(
        &query,
        &[
            "series_uid",
            "series_instance_uid",
            "SeriesInstanceUID",
            "SeriesUID",
        ],
    ) {
        docs.retain(|(doc, _status)| doc.provenance.series_instance_uid == *series_uid);
    }
    if let Some(observer) = query.get("observer") {
        docs.retain(|(doc, _status)| doc.provenance.observer == *observer);
    }
    apply_sr_sort(&mut docs, query.get("sort"), query.get("order"))?;

    let (offset, limit) = parse_pagination(query.get("page"), query.get("page_size"))?;
    let end = docs.len().min(offset.saturating_add(limit));
    if offset > docs.len() {
        docs.clear();
    } else {
        docs = docs[offset..end].to_vec();
    }

    let mut json = String::from("[");
    for (idx, (doc, status)) in docs.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"sop_instance_uid\":\"{}\",\"observer\":\"{}\",\"version\":{},\"item_count\":{},\"lifecycle_status\":\"{}\"}}",
            escape_json(&doc.provenance.study_instance_uid),
            escape_json(&doc.provenance.series_instance_uid),
            escape_json(&doc.provenance.sop_instance_uid),
            escape_json(&doc.provenance.observer),
            doc.version,
            doc.items.len(),
            status.as_label()
        ));
    }
    json.push(']');
    Ok(WorkflowResponse::Json(200, json))
}

fn apply_sr_sort(
    docs: &mut Vec<(pack_sr::SrAuthoredDocument, SrLifecycleStatus)>,
    sort: Option<&String>,
    order: Option<&String>,
) -> Result<(), Box<Error>> {
    let descending = matches!(order.map(|value| value.as_str()), Some("desc"));
    match sort.map(|value| value.as_str()) {
        None
        | Some("default")
        | Some("study_instance_uid,series_instance_uid,sop_instance_uid") => {
            docs.sort_by(|a, b| {
                (
                    a.0.provenance.study_instance_uid.as_str(),
                    a.0.provenance.series_instance_uid.as_str(),
                    a.0.provenance.sop_instance_uid.as_str(),
                )
                    .cmp(&(
                        b.0.provenance.study_instance_uid.as_str(),
                        b.0.provenance.series_instance_uid.as_str(),
                        b.0.provenance.sop_instance_uid.as_str(),
                    ))
            });
            if descending {
                docs.reverse();
            }
        }
        Some("study_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .study_instance_uid
                    .cmp(&b.0.provenance.study_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("series_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .series_instance_uid
                    .cmp(&b.0.provenance.series_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("sop_instance_uid") => {
            docs.sort_by(|a, b| {
                a.0.provenance
                    .sop_instance_uid
                    .cmp(&b.0.provenance.sop_instance_uid)
            });
            if descending {
                docs.reverse();
            }
        }
        Some("observer") => {
            docs.sort_by(|a, b| a.0.provenance.observer.cmp(&b.0.provenance.observer));
            if descending {
                docs.reverse();
            }
        }
        Some("version") => {
            docs.sort_by(|a, b| a.0.version.cmp(&b.0.version));
            if descending {
                docs.reverse();
            }
        }
        Some("item_count") => {
            docs.sort_by(|a, b| a.0.items.len().cmp(&b.0.items.len()));
            if descending {
                docs.reverse();
            }
        }
        Some(_) => Err(decode_error("unsupported SR sort"))?,
    }
    Ok(())
}

fn handle_sr_get(
    sop_instance_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let Some(doc) = store.sr.get(sop_instance_uid).cloned() else {
        return Err(sr_not_found_error());
    };
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid) {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let status = store
        .sr
        .lifecycle_status(sop_instance_uid)
        .unwrap_or(SrLifecycleStatus::Draft);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_document_json(&doc, status),
    ))
}

fn handle_sr_create(
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let item = parse_sr_item_from_params(&params)?;
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    let sop_instance_uid = required_identifier_from_map(
        &params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
    )?;
    if !actor_has_resource_access(actor, &store.tenant_sr, &sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, &sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let outcome = store.sr.create(
        SrCreateRequest {
            study_instance_uid: required_identifier_from_map(
                &params,
                &[
                    "study_instance_uid",
                    "study_uid",
                    "StudyInstanceUID",
                    "StudyUID",
                ],
                "study_instance_uid",
            )?,
            series_instance_uid: required_identifier_from_map(
                &params,
                &[
                    "series_instance_uid",
                    "series_uid",
                    "SeriesInstanceUID",
                    "SeriesUID",
                ],
                "series_instance_uid",
            )?,
            sop_instance_uid: sop_instance_uid.clone(),
            observer: required_param(&params, "observer")?.to_string(),
            authored_epoch_ms: required_param(&params, "authored_epoch_ms")?
                .parse::<u64>()
                .map_err(|_| decode_error("invalid authored_epoch_ms"))?,
            item,
            known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
            idempotency_key,
            request_id: Some(request_id_from_headers(&request.headers)),
        },
        &auth,
    )?;
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, &sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_write_outcome_json(&outcome),
    ))
}

fn handle_sr_update(
    sop_instance_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
    limits: &Limits,
) -> Result<WorkflowResponse, Box<Error>> {
    let params = parse_form_map(&request.body, limits)?;
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let item = parse_sr_item_from_params(&params)?;
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let outcome = store.sr.update(
        SrUpdateEnvelope {
            sop_instance_uid: sop_instance_uid.to_string(),
            expected_version: required_param(&params, "expected_version")?
                .parse::<u64>()
                .map_err(|_| decode_error("invalid expected_version"))?,
            item,
            observer: params.get("observer").cloned(),
            known_referenced_sop_instance_uids: parse_known_refs(params.get("known_refs")),
            idempotency_key,
            request_id: Some(request_id_from_headers(&request.headers)),
        },
        &auth,
    )?;
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_write_outcome_json(&outcome),
    ))
}

fn handle_sr_transition(
    action: &str,
    sop_instance_uid: &str,
    request: &HttpRequest,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let idempotency_key = request
        .headers
        .get("x-idempotency-key")
        .cloned()
        .unwrap_or_default();
    let auth = sr_auth_context(request);
    let mut store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid)
        && tenant_of_id(&store.tenant_sr, sop_instance_uid).is_some()
    {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let previous_status = store.sr.lifecycle_status(sop_instance_uid);
    let correlation_id = request_id_from_headers(&request.headers);

    let transition = SrLifecycleTransitionRequest {
        sop_instance_uid: sop_instance_uid.to_string(),
        idempotency_key,
        request_id: Some(correlation_id.clone()),
    };
    let outcome = match action {
        "review" => store.sr.review(transition, &auth),
        "finalize" => store.sr.finalize(transition, &auth),
        "commit" => store.sr.commit(transition, &auth),
        "cancel" => store.sr.cancel(transition, &auth),
        _ => return Err(decode_error("unsupported sr transition")),
    }?;
    if !outcome.idempotency_replay {
        let from_status = previous_status
            .unwrap_or(SrLifecycleStatus::Draft)
            .as_label();
        let mut payload = BTreeMap::new();
        payload.insert("event".to_string(), "sr.transition".to_string());
        payload.insert("sop_instance_uid".to_string(), sop_instance_uid.to_string());
        payload.insert("action".to_string(), action.to_string());
        payload.insert("from_status".to_string(), from_status.to_string());
        payload.insert(
            "to_status".to_string(),
            outcome.status.as_label().to_string(),
        );
        payload.insert("version".to_string(), outcome.version.to_string());
        payload.insert("tenant".to_string(), actor.tenant.clone());
        if let Some(actor_name) = actor.principal.clone() {
            payload.insert("actor".to_string(), actor_name);
        }
        let (event_id, sequence) = next_hl7_event_id_with_sequence(&mut store);
        publish_hl7_event(
            &mut store,
            &workflow_event_source(&actor),
            "sr",
            &payload,
            &event_id,
            sequence,
            &correlation_id,
        );
    }
    route_id_to_tenant_index(&mut store.tenant_sr, &actor.tenant, sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_transition_outcome_json(&outcome),
    ))
}

fn handle_sr_history(
    sop_instance_uid: &str,
    actor: &WorkflowActorContext,
    state: &Arc<Mutex<RuntimeState>>,
) -> Result<WorkflowResponse, Box<Error>> {
    let store = state
        .lock()
        .map_err(|_| io_error("workflow state lock poisoned", "mutex poison"))?;
    if store.sr.get(sop_instance_uid).is_none() {
        return Err(sr_not_found_error());
    }
    if !actor_has_resource_access(actor, &store.tenant_sr, sop_instance_uid) {
        return Err(auth_denied_error("sr belongs to another tenant"));
    }
    let history = store.sr.lifecycle_history(sop_instance_uid);
    Ok(WorkflowResponse::Json(
        200,
        render_sr_history_json(&history),
    ))
}

fn parse_sr_item_from_params(
    params: &BTreeMap<String, String>,
) -> Result<SrAuthoringContentItem, Box<Error>> {
    let concept = Code {
        code_value: required_param(params, "concept_code_value")?.to_string(),
        scheme: required_param(params, "concept_scheme")?.to_string(),
        meaning: required_param(params, "concept_meaning")?.to_string(),
    };
    let referenced = params
        .get("referenced_sop_instance_uid")
        .filter(|value| !value.trim().is_empty())
        .cloned();
    match required_param(params, "item_kind")? {
        "num" => Ok(SrAuthoringContentItem::Num {
            concept,
            value: required_param(params, "num_value")?
                .parse::<f64>()
                .map_err(|_| decode_error("invalid num_value"))?,
            units: Code {
                code_value: required_param(params, "units_code_value")?.to_string(),
                scheme: required_param(params, "units_scheme")?.to_string(),
                meaning: required_param(params, "units_meaning")?.to_string(),
            },
            referenced_sop_instance_uid: referenced,
        }),
        "text" => Ok(SrAuthoringContentItem::Text {
            concept,
            text: required_param(params, "text_value")?.to_string(),
            referenced_sop_instance_uid: referenced,
        }),
        "code" => Ok(SrAuthoringContentItem::Code {
            concept,
            value: Code {
                code_value: required_param(params, "value_code_value")?.to_string(),
                scheme: required_param(params, "value_scheme")?.to_string(),
                meaning: required_param(params, "value_meaning")?.to_string(),
            },
            referenced_sop_instance_uid: referenced,
        }),
        _ => Err(decode_error("unsupported item_kind")),
    }
}

fn parse_known_refs(value: Option<&String>) -> Vec<String> {
    let mut refs = value
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    refs.sort();
    refs.dedup();
    refs
}

fn sr_auth_context(request: &HttpRequest) -> SrAuthContext {
    let principal = request
        .headers
        .get("x-sr-principal")
        .cloned()
        .or_else(|| request.headers.get("x-auth-principal").cloned())
        .filter(|value| !value.trim().is_empty());
    let role = request
        .headers
        .get("x-sr-role")
        .or_else(|| request.headers.get("x-auth-role"))
        .map(|value| value.as_str())
        .unwrap_or("");
    let can_write = workflow_role_is_writer(role);
    SrAuthContext {
        principal,
        can_write,
    }
}

fn render_sr_write_outcome_json(outcome: &dicom_workflow_server::SrWriteOutcome) -> String {
    let kind = match outcome.kind {
        SrWriteOutcomeKind::Created => "created",
        SrWriteOutcomeKind::Updated => "updated",
        SrWriteOutcomeKind::Duplicate => "duplicate",
    };
    format!(
        "{{\"outcome\":\"{}\",\"sop_instance_uid\":\"{}\",\"version\":{},\"idempotency_replay\":{}}}",
        kind,
        escape_json(&outcome.sop_instance_uid),
        outcome.version,
        if outcome.idempotency_replay {
            "true"
        } else {
            "false"
        }
    )
}

fn render_sr_transition_outcome_json(outcome: &SrLifecycleTransitionOutcome) -> String {
    let status = outcome.status.as_label();
    format!(
        "{{\"operation\":\"{}\",\"sop_instance_uid\":\"{}\",\"status\":\"{}\",\"version\":{},\"idempotency_replay\":{}}}",
        escape_json(outcome.operation),
        escape_json(&outcome.sop_instance_uid),
        status,
        outcome.version,
        if outcome.idempotency_replay { "true" } else { "false" }
    )
}

fn render_sr_history_json(history: &[SrLifecycleHistoryRecord]) -> String {
    let mut json = String::from("[");
    for (idx, entry) in history.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"at_epoch_ms\":{},\"action\":\"{}\",\"status\":\"{}\",\"version\":{}}}",
            entry.at_epoch_ms,
            escape_json(entry.action),
            entry.status.as_label(),
            entry.version
        ));
    }
    json.push(']');
    json
}

fn render_sr_document_json(
    document: &pack_sr::SrAuthoredDocument,
    status: SrLifecycleStatus,
) -> String {
    let mut json = String::new();
    json.push_str("{\"provenance\":{");
    json.push_str(&format!(
        "\"study_instance_uid\":\"{}\",\"series_instance_uid\":\"{}\",\"sop_instance_uid\":\"{}\",\"observer\":\"{}\",\"authored_epoch_ms\":{},\"lifecycle_status\":\"{}\"",
        escape_json(&document.provenance.study_instance_uid),
        escape_json(&document.provenance.series_instance_uid),
        escape_json(&document.provenance.sop_instance_uid),
        escape_json(&document.provenance.observer),
        document.provenance.authored_epoch_ms,
        status.as_label()
    ));
    json.push_str("},\"version\":");
    json.push_str(&document.version.to_string());
    json.push_str(",\"items\":[");
    for (idx, item) in document.items.iter().enumerate() {
        if idx > 0 {
            json.push(',');
        }
        json.push_str(&render_sr_item_json(item));
    }
    json.push_str("]}");
    json
}

fn render_sr_item_json(item: &SrAuthoringContentItem) -> String {
    match item {
        SrAuthoringContentItem::Num {
            concept,
            value,
            units,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"num\",\"concept\":{},\"value\":{},\"units\":{},\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            value,
            render_code_json(units),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
        SrAuthoringContentItem::Text {
            concept,
            text,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"text\",\"concept\":{},\"text\":\"{}\",\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            escape_json(text),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
        SrAuthoringContentItem::Code {
            concept,
            value,
            referenced_sop_instance_uid,
        } => format!(
            "{{\"kind\":\"code\",\"concept\":{},\"value\":{},\"referenced_sop_instance_uid\":{}}}",
            render_code_json(concept),
            render_code_json(value),
            render_optional_str(referenced_sop_instance_uid.as_deref())
        ),
    }
}

fn render_code_json(code: &Code) -> String {
    format!(
        "{{\"code_value\":\"{}\",\"scheme\":\"{}\",\"meaning\":\"{}\"}}",
        escape_json(&code.code_value),
        escape_json(&code.scheme),
        escape_json(&code.meaning)
    )
}

fn render_optional_str(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json(value)))
        .unwrap_or_else(|| "null".to_string())
}

fn build_worklist_dataset(params: &BTreeMap<String, String>) -> Result<Dataset, Box<Error>> {
    let scheduled_step_id = required_param(params, "scheduled_step_id")?;
    let modality = required_param(params, "modality")?;
    let start_date = required_param(params, "start_date")?;
    let start_time = required_param(params, "start_time")?;

    let mut item = Dataset::new();
    item.insert(Element {
        tag: TAG_SPS_ID,
        vr: Vr::Sh,
        value: Value::Str(scheduled_step_id.to_string()),
    });
    item.insert(Element {
        tag: TAG_MODALITY,
        vr: Vr::Cs,
        value: Value::Str(modality.to_string()),
    });
    item.insert(Element {
        tag: TAG_SPS_START_DATE,
        vr: Vr::Da,
        value: Value::Str(start_date.to_string()),
    });
    item.insert(Element {
        tag: TAG_SPS_START_TIME,
        vr: Vr::Tm,
        value: Value::Str(start_time.to_string()),
    });

    if let Some(value) = params.get("requested_procedure_id") {
        item.insert(Element {
            tag: TAG_REQUESTED_PROCEDURE_ID,
            vr: Vr::Sh,
            value: Value::Str(value.to_string()),
        });
    }
    if let Some(value) = params.get("scheduled_station_ae_title") {
        item.insert(Element {
            tag: TAG_SCHEDULED_STATION_AE_TITLE,
            vr: Vr::Ae,
            value: Value::Str(value.to_string()),
        });
    }

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_SPS_SEQUENCE,
        vr: Vr::Sq,
        value: Value::Sequence(vec![item]),
    });

    if let Some(value) = lookup_identifier_from_map(
        params,
        &["patient_id", "PatientID", "PatientId", "patientId"],
    ) {
        dataset.insert(Element {
            tag: TAG_PATIENT_ID,
            vr: Vr::Lo,
            value: Value::Str(value),
        });
    }
    if let Some(value) = lookup_identifier_from_map(
        params,
        &[
            "accession_number",
            "AccessionNumber",
            "accessionNo",
            "accession_number_raw",
        ],
    ) {
        dataset.insert(Element {
            tag: TAG_ACCESSION_NUMBER,
            vr: Vr::Sh,
            value: Value::Str(value),
        });
    }

    Ok(dataset)
}

fn build_mpps_dataset(params: &BTreeMap<String, String>) -> Result<Dataset, Box<Error>> {
    let sop_instance_uid = required_identifier_from_map(
        params,
        &["sop_instance_uid", "sop_uid", "SOPInstanceUID", "SOP_UID"],
        "sop_instance_uid",
    )?;
    let status = required_param(params, "status")?;
    let performed_step_id = required_param(params, "performed_step_id")?;
    let start_date = required_param(params, "start_date")?;
    let start_time = required_param(params, "start_time")?;

    let mut dataset = Dataset::new();
    dataset.insert(Element {
        tag: TAG_SOP_INSTANCE_UID,
        vr: Vr::Ui,
        value: Value::Uid(sop_instance_uid.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_STATUS,
        vr: Vr::Cs,
        value: Value::Str(status.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_PERFORMED_STEP_ID,
        vr: Vr::Sh,
        value: Value::Str(performed_step_id.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_START_DATE,
        vr: Vr::Da,
        value: Value::Str(start_date.to_string()),
    });
    dataset.insert(Element {
        tag: TAG_START_TIME,
        vr: Vr::Tm,
        value: Value::Str(start_time.to_string()),
    });

    if let Some(value) = params.get("end_date") {
        dataset.insert(Element {
            tag: TAG_END_DATE,
            vr: Vr::Da,
            value: Value::Str(value.to_string()),
        });
    }
    if let Some(value) = params.get("end_time") {
        dataset.insert(Element {
            tag: TAG_END_TIME,
            vr: Vr::Tm,
            value: Value::Str(value.to_string()),
        });
    }

    Ok(dataset)
}

fn required_param<'a>(
    params: &'a BTreeMap<String, String>,
    key: &str,
) -> Result<&'a str, Box<Error>> {
    let value = params
        .get(key)
        .ok_or_else(|| decode_error(format!("missing required parameter: {key}")))?;
    if value.is_empty() {
        return Err(decode_error(format!("empty required parameter: {key}")));
    }
    Ok(value)
}

fn parse_form_map(body: &[u8], limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    let text =
        std::str::from_utf8(body).map_err(|_| decode_error("form body is not valid UTF-8"))?;
    parse_pairs(text, limits)
}

fn parse_query_map(query: &str, limits: &Limits) -> Result<BTreeMap<String, String>, String> {
    parse_pairs(query, limits).map_err(|err| err.to_string())
}

fn parse_pairs(text: &str, limits: &Limits) -> Result<BTreeMap<String, String>, Box<Error>> {
    let mut out = BTreeMap::new();
    let max_pairs = limits
        .max_dataset_elements
        .min(DEFAULT_WORKFLOW_MAX_PAIR_COUNT as u64);
    let mut pair_count = 0usize;
    if text.is_empty() {
        return Ok(out);
    }
    for pair in text.split('&') {
        if pair.is_empty() {
            continue;
        }
        pair_count = pair_count.saturating_add(1);
        if pair_count as u64 > max_pairs {
            return Err(limit_exceeded(
                "max_workflow_pair_count",
                pair_count as u64,
                max_pairs,
            ));
        }
        let (raw_key, raw_value) = pair
            .split_once('=')
            .ok_or_else(|| decode_error("invalid key-value pair"))?;
        let key = percent_decode(raw_key).ok_or_else(|| decode_error("invalid key encoding"))?;
        let value =
            percent_decode(raw_value).ok_or_else(|| decode_error("invalid value encoding"))?;
        if key.is_empty() {
            return Err(decode_error("empty key is not allowed"));
        }
        enforce_ascii_and_limits(&key, limits)?;
        enforce_ascii_and_limits(&value, limits)?;
        out.insert(key, value);
    }
    Ok(out)
}

fn enforce_ascii_and_limits(value: &str, limits: &Limits) -> Result<(), Box<Error>> {
    if !value.is_ascii() {
        return Err(decode_error("request values must be ASCII"));
    }
    if value.len() as u64 > limits.max_string_bytes {
        return Err(limit_exceeded(
            "max_string_bytes",
            value.len() as u64,
            limits.max_string_bytes,
        ));
    }
    Ok(())
}

fn percent_decode(input: &str) -> Option<String> {
    let mut out = String::with_capacity(input.len());
    let mut bytes = input.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'+' => out.push(' '),
            b'%' => {
                let hi = bytes.next()?;
                let lo = bytes.next()?;
                let decoded = (from_hex(hi)? << 4) | from_hex(lo)?;
                out.push(decoded as char);
            }
            _ => out.push(byte as char),
        }
    }
    Some(out)
}

fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_http_request(bytes: &[u8], limits: &Limits) -> Result<HttpRequest, String> {
    let (head_end, body_start) =
        header_offsets(bytes).ok_or_else(|| "missing header terminator".to_string())?;
    let head =
        std::str::from_utf8(&bytes[..head_end]).map_err(|_| "headers are not UTF-8".to_string())?;
    let mut lines = head.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "missing request line".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let target = request_parts
        .next()
        .ok_or_else(|| "missing target".to_string())?;

    if method != "GET" && method != "HEAD" && method != "POST" {
        return Err("unsupported method".to_string());
    }

    if method != "POST" && bytes.len() > body_start {
        return Err("GET/HEAD requests must not include a body".to_string());
    }

    let mut headers = BTreeMap::new();
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| "invalid header format".to_string())?;
        let key = name.trim().to_ascii_lowercase();
        let value = value.trim().to_string();
        if key.len() as u64 > limits.max_string_bytes
            || value.len() as u64 > limits.max_string_bytes
        {
            return Err("header length limit exceeded".to_string());
        }
        headers.insert(key, value);
    }

    let body = bytes[body_start..].to_vec();

    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (
            path.to_string(),
            parse_query_map(query, limits).map_err(|err| err.to_string())?,
        ),
        None => (target.to_string(), BTreeMap::new()),
    };

    if path.len() as u64 > limits.max_string_bytes {
        return Err("path length limit exceeded".to_string());
    }

    Ok(HttpRequest {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn requested_admin_api_version(request: &HttpRequest) -> Option<String> {
    request
        .headers
        .get("x-workflow-admin-api-version")
        .or_else(|| request.query.get("api_version"))
        .map(|value| normalize_identifier(value))
        .filter(|value| !value.is_empty())
}

fn enforce_admin_api_version(request: &HttpRequest) -> Result<(), Box<Error>> {
    let Some(version) = requested_admin_api_version(request) else {
        return Ok(());
    };
    if version == WORKFLOW_ADMIN_API_VERSION || version == "1" {
        return Ok(());
    }
    Err(decode_error(format!(
        "unsupported admin api_version: {version} (supported: v1, 1)"
    )))
}

fn request_id_from_headers(headers: &BTreeMap<String, String>) -> String {
    let Some(raw) = headers.get("x-request-id") else {
        return fallback_request_id();
    };
    let value = raw.trim();
    if value.is_empty() {
        fallback_request_id()
    } else {
        value.to_string()
    }
}

fn fallback_request_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let counter = REQUEST_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("request-{}-{}", now.as_nanos(), counter)
}

fn read_http_request(stream: &mut TcpStream, limits: &Limits) -> std::io::Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 8192];
    let mut total_len: Option<usize> = None;
    let hard_cap = limits
        .max_input_bytes
        .saturating_add(limits.max_string_bytes) as usize;

    loop {
        let n = stream.read(&mut temp)?;
        if n == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..n]);

        if buffer.len() > hard_cap {
            return Err(IoError::new(IoErrorKind::InvalidData, "request too large"));
        }

        if total_len.is_none() {
            if let Some((head_end, body_start)) = header_offsets(&buffer) {
                let head = std::str::from_utf8(&buffer[..head_end]).map_err(|_| {
                    IoError::new(IoErrorKind::InvalidData, "headers are not valid UTF-8")
                })?;
                let body_len = content_length(head).unwrap_or(0);
                total_len = Some(body_start.saturating_add(body_len));
            }
        }

        if let Some(total) = total_len {
            if buffer.len() >= total {
                buffer.truncate(total);
                return Ok(buffer);
            }
        }
    }

    if total_len.is_some() {
        return Err(IoError::new(
            IoErrorKind::UnexpectedEof,
            "incomplete HTTP request body",
        ));
    }
    Err(IoError::new(
        IoErrorKind::InvalidData,
        "missing HTTP header terminator",
    ))
}

fn header_offsets(buffer: &[u8]) -> Option<(usize, usize)> {
    if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
        return Some((pos, pos + 4));
    }
    buffer
        .windows(2)
        .position(|w| w == b"\n\n")
        .map(|pos| (pos, pos + 2))
}

fn content_length(head: &str) -> Option<usize> {
    for line in head.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
}

fn authorize(
    request: &HttpRequest,
    mode: &WorkflowAuthMode,
    transport_security: &str,
    auth_token: Option<&str>,
) -> bool {
    if transport_security == "insecure" {
        return false;
    }
    match mode {
        WorkflowAuthMode::AllowAll => true,
        WorkflowAuthMode::DenyAll => false,
        WorkflowAuthMode::Token => request
            .headers
            .get("x-rdvf-token")
            .is_some_and(|value| Some(value.as_str()) == auth_token),
    }
}

fn auth_mode_from_env(
    auth_token_raw: Option<&str>,
    auth_mode_raw: Option<&str>,
) -> std::io::Result<WorkflowAuthMode> {
    let parsed = WorkflowAuthMode::parse(auth_mode_raw)?;
    if matches!(parsed, WorkflowAuthMode::Token)
        && !auth_token_raw.is_some_and(|token| !token.trim().is_empty())
    {
        return Err(environment_parse_error(
            "DICOM_WORKFLOW_AUTH_MODE",
            auth_mode_raw.unwrap_or("token"),
            "DICOM_WORKFLOW_AUTH_TOKEN must be set when auth mode is token",
        ));
    }
    Ok(parsed)
}

fn auth_mode_label(mode: &WorkflowAuthMode) -> &'static str {
    match mode {
        WorkflowAuthMode::AllowAll => "allow_all",
        WorkflowAuthMode::DenyAll => "deny_all",
        WorkflowAuthMode::Token => "token",
    }
}

fn validate_tls_secret_path(
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> std::io::Result<()> {
    match (cert_path, key_path) {
        (None, None) => Err(IoError::other(
            "transport requires TLS but DICOM_WORKFLOW_TLS_CERT_PATH and DICOM_WORKFLOW_TLS_KEY_PATH are both unset",
        )),
        (Some(_), None) | (None, Some(_)) => Err(IoError::other(
            "transport requires TLS but DICOM_WORKFLOW_TLS_CERT_PATH and DICOM_WORKFLOW_TLS_KEY_PATH must both be set",
        )),
        (Some(cert_path), Some(key_path)) => {
            if cert_path.trim().is_empty() || key_path.trim().is_empty() {
                return Err(IoError::other(
                    "transport requires TLS but TLS material path values must be non-empty",
                ));
            }
            let cert_meta = fs::metadata(cert_path)?;
            if !cert_meta.is_file() {
                return Err(IoError::other(format!(
                    "TLS cert path is not a file: {cert_path}"
                )));
            }
            let key_meta = fs::metadata(key_path)?;
            if !key_meta.is_file() {
                return Err(IoError::other(format!(
                    "TLS key path is not a file: {key_path}"
                )));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
fn prepare_persistence_file(
    path: &str,
    max_bytes: u64,
    max_rotated_files: usize,
    label: &str,
) -> std::io::Result<()> {
    let _ = prepare_persistence_file_with_diagnostics(path, max_bytes, max_rotated_files, label)?;
    Ok(())
}

fn transport_security_from_env(raw_security: Option<&str>) -> &'static str {
    match raw_security {
        Some("tls") => "tls",
        Some("insecure") | None => "insecure",
        Some(_) => "insecure",
    }
}

enum WorkflowResponse {
    Json(u16, String),
}

#[derive(Clone, Copy)]
struct WorkflowRouteErrorInvariant {
    route_group: &'static str,
    error_code: &'static str,
    http_status: u16,
    http_label: &'static str,
}

const WORKFLOW_ROUTE_ERROR_INVARIANTS: &[WorkflowRouteErrorInvariant] = &[
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.AUTH_DENIED",
        http_status: 403,
        http_label: "Forbidden",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.NOT_FOUND",
        http_status: 404,
        http_label: "Not Found",
    },
    WorkflowRouteErrorInvariant {
        route_group: "mpps",
        error_code: "DVF.WORKFLOW.MPPS.NOT_FOUND",
        http_status: 404,
        http_label: "Not Found",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.VERSION_CONFLICT",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.ALREADY_EXISTS",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.IDEMPOTENCY_CONFLICT",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "sr",
        error_code: "DVF.WORKFLOW.SR.INVALID_TRANSITION",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "mpps",
        error_code: "DVF.WORKFLOW.MPPS.IDEMPOTENCY_CONFLICT",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "task",
        error_code: "DVF.WORKFLOW.TASK.NOT_FOUND",
        http_status: 404,
        http_label: "Not Found",
    },
    WorkflowRouteErrorInvariant {
        route_group: "task",
        error_code: "DVF.WORKFLOW.TASK.INVALID_TRANSITION",
        http_status: 409,
        http_label: "Conflict",
    },
    WorkflowRouteErrorInvariant {
        route_group: "http",
        error_code: "DVF.WORKFLOW.HTTP.DECODE_ERROR",
        http_status: 400,
        http_label: "Bad Request",
    },
];

fn workflow_route_error_invariant(error_code: &str) -> Option<WorkflowRouteErrorInvariant> {
    WORKFLOW_ROUTE_ERROR_INVARIANTS
        .iter()
        .find(|invariant| invariant.error_code == error_code)
        .copied()
}

fn status_for_error(error: &Error) -> (u16, &'static str) {
    if let Some(invariant) = workflow_route_error_invariant(error.code) {
        let _route_group = invariant.route_group;
        return (invariant.http_status, invariant.http_label);
    }
    match &error.kind {
        ErrorKind::LimitExceeded { limit_name, .. }
            if (*limit_name).contains("rate") || *limit_name == "workflow_rate_limit" =>
        {
            (429, "Too Many Requests")
        }
        ErrorKind::LimitExceeded { .. } => (413, "Payload Too Large"),
        ErrorKind::DecodeError { .. }
        | ErrorKind::MissingRequiredTag { .. }
        | ErrorKind::InvalidTagValue { .. }
        | ErrorKind::UnsupportedSopClass { .. }
        | ErrorKind::UnsupportedTransferSyntax { .. }
        | ErrorKind::InvalidGeometry { .. }
        | ErrorKind::InvalidPixelTransform { .. } => (400, "Bad Request"),
        ErrorKind::IntegrityError { .. } => (409, "Conflict"),
        ErrorKind::IoError { .. } | ErrorKind::InternalError { .. } => {
            (500, "Internal Server Error")
        }
    }
}

fn http_success_response(response: WorkflowResponse, head_only: bool) -> Vec<u8> {
    match response {
        WorkflowResponse::Json(status, body) => {
            let status_label = if status == 200 { "OK" } else { "Response" };
            http_response_bytes(
                status,
                status_label,
                "application/json",
                body.as_bytes(),
                head_only,
            )
        }
    }
}

fn http_error_response(
    status: u16,
    status_label: &str,
    code: &str,
    message: &str,
    head_only: bool,
) -> Vec<u8> {
    let message = redact_diagnostic_message(message);
    let body = format!(
        "{{\"error\":\"{}\",\"message\":\"{}\"}}",
        escape_json(code),
        escape_json(&message),
    )
    .into_bytes();
    http_response_bytes(status, status_label, "application/json", &body, head_only)
}

fn http_response_bytes(
    status: u16,
    status_label: &str,
    content_type: &str,
    body: &[u8],
    head_only: bool,
) -> Vec<u8> {
    let mut response = Vec::new();
    response.extend_from_slice(format!("HTTP/1.1 {status} {status_label}\r\n").as_bytes());
    response.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
    response.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    response.extend_from_slice(b"Connection: close\r\n\r\n");
    if !head_only {
        response.extend_from_slice(body);
    }
    response
}

fn limit_exceeded(limit_name: &'static str, observed: u64, allowed: u64) -> Box<Error> {
    Error::from_kind(
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        },
        "limit exceeded",
    )
    .into()
}

fn decode_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server".to_string(),
            detail: detail.into(),
        },
        "decode error",
    )
    .into()
}

fn sr_not_found_error() -> Box<Error> {
    Error::new(
        "DVF.WORKFLOW.SR.NOT_FOUND",
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-sr".to_string(),
            detail: "requested SR document not found".to_string(),
        },
        "requested SR document not found",
    )
    .into()
}

fn io_error(context: impl Into<String>, detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::IoError {
            detail: format!("{}: {}", context.into(), detail.into()),
        },
        "i/o error",
    )
    .into()
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn redact_diagnostic_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_token(token: &str) -> String {
    let (prefix, candidate) = match token.split_once('=') {
        Some((left, right)) if !left.is_empty() => (Some(left), right),
        _ => (None, token),
    };
    if prefix.is_some_and(looks_like_sensitive_key) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_PII]"),
            None => "[REDACTED_PII]".to_string(),
        };
    }
    let trimmed = candidate.trim_matches(|ch: char| ",;()[]{}\"'".contains(ch));
    if looks_like_email(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_EMAIL]"),
            None => "[REDACTED_EMAIL]".to_string(),
        };
    }
    if looks_like_uid(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_UID]"),
            None => "[REDACTED_UID]".to_string(),
        };
    }
    if looks_like_path(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_PATH]"),
            None => "[REDACTED_PATH]".to_string(),
        };
    }
    if looks_like_host(trimmed) {
        return match prefix {
            Some(key) => format!("{key}=[REDACTED_HOST]"),
            None => "[REDACTED_HOST]".to_string(),
        };
    }
    token.to_string()
}

fn looks_like_uid(value: &str) -> bool {
    if value.len() < 7 {
        return false;
    }
    let mut part_count = 0usize;
    for part in value.split('.') {
        if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        part_count = part_count.saturating_add(1);
    }
    part_count >= 3
}

fn looks_like_path(value: &str) -> bool {
    value.contains('/') || value.contains('\\')
}

fn looks_like_host(value: &str) -> bool {
    let Some((host, port)) = value.rsplit_once(':') else {
        return false;
    };
    if host.is_empty() || port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-')
}

fn looks_like_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn looks_like_sensitive_key(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "patient"
            | "patient_id"
            | "patient_name"
            | "mrn"
            | "email"
            | "ssn"
            | "dob"
            | "birth_date"
            | "phone"
            | "person_name"
    )
}

#[cfg(all(test, feature = "workflow-main-tests"))]
mod tests {
    use super::*;
    use dicom_workflow_server::{path_with_suffix, workflow_route_contract, WorkflowRouteContract};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.{ext}"))
    }

    fn cleanup_with_rotations(path: &Path, max_rotations: usize) {
        let _ = fs::remove_file(path);
        for index in 1..=max_rotations + 1 {
            let _ = fs::remove_file(path_with_suffix(path, index));
        }
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }

    fn with_env_var<F, R>(name: &str, value: Option<&str>, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = std::env::var(name).ok();
        match value {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        let result = f();
        match previous {
            Some(raw) => std::env::set_var(name, raw),
            None => std::env::remove_var(name),
        }
        result
    }

    fn with_env_vars<F, R>(pairs: &[(&str, Option<&str>)], f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let previous = pairs
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect::<Vec<_>>();
        for (name, value) in pairs {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        let result = f();
        for (name, value) in previous {
            match value {
                Some(raw) => std::env::set_var(name, raw),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    #[test]
    fn parse_u64_uses_default_when_unset() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", None, || {
            let value = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("default");
            assert_eq!(value, 300);
        });
    }

    #[test]
    fn parse_u64_rejects_invalid_value() {
        with_env_var(
            "DICOM_WORKFLOW_TEST_RATE_LIMIT",
            Some("not-a-number"),
            || {
                let err = parse_u64(
                    WORKFLOW_SERVICE_NAME,
                    "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                    300,
                    NumericBounds::at_least(1),
                )
                .expect_err("invalid value must fail");
                assert_eq!(err.kind(), IoErrorKind::InvalidInput);
                assert!(err.to_string().contains("DICOM_WORKFLOW_TEST_RATE_LIMIT"));
            },
        );
    }

    #[test]
    fn parse_u64_accepts_positive_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("456"), || {
            let value = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("positive value");
            assert_eq!(value, 456);
        });
    }

    #[test]
    fn parse_u64_rejects_out_of_range_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("0"), || {
            let err = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_u64_rejects_negative_value() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("-1"), || {
            let err = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("invalid value"));
        });
    }

    #[test]
    fn parse_u64_is_idempotent_for_repeated_invocations() {
        with_env_var("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("456"), || {
            let first = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("first parse");
            let second = parse_u64(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_RATE_LIMIT",
                300,
                NumericBounds::at_least(1),
            )
            .expect("second parse");
            assert_eq!(first, second);
        });
    }

    #[test]
    fn parse_usize_is_idempotent_for_repeated_invocations() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("6"), || {
            let first = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("first parse");
            let second = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("second parse");
            assert_eq!(first, second);
        });
    }

    #[test]
    fn parse_usize_uses_default_when_unset() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", None, || {
            let value = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("default");
            assert_eq!(value, 2);
        });
    }

    #[test]
    fn parse_usize_rejects_invalid_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("bad"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("invalid value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("DICOM_WORKFLOW_TEST_ROTATIONS"));
        });
    }

    #[test]
    fn parse_usize_accepts_positive_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("6"), || {
            let value = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect("positive value");
            assert_eq!(value, 6);
        });
    }

    #[test]
    fn parse_usize_rejects_out_of_range_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("0"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("zero should fail minimum bound");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be >= 1"));
        });
    }

    #[test]
    fn parse_usize_rejects_negative_value() {
        with_env_var("DICOM_WORKFLOW_TEST_ROTATIONS", Some("-1"), || {
            let err = parse_usize(
                WORKFLOW_SERVICE_NAME,
                "DICOM_WORKFLOW_TEST_ROTATIONS",
                2,
                NumericBounds::at_least(1),
            )
            .expect_err("negative value must fail");
            assert_eq!(err.kind(), IoErrorKind::InvalidInput);
            assert!(err.to_string().contains("invalid value"));
        });
    }

    #[test]
    fn workflow_runtime_config_from_env_is_idempotent() {
        with_env_vars(
            &[
                ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", Some("451")),
                ("DICOM_WORKFLOW_MUTATION_RATE_LIMIT", Some("129")),
                ("DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT", Some("64")),
                ("DICOM_WORKFLOW_DENYLIST_PATHS", Some("/test|/other")),
            ],
            || {
                let first = WorkflowRuntimeConfig::from_env().expect("first config");
                let second = WorkflowRuntimeConfig::from_env().expect("second config");
                assert_eq!(first.query_rate_limit, second.query_rate_limit);
                assert_eq!(first.mutation_rate_limit, second.mutation_rate_limit);
                assert_eq!(first.audit_export_limit, second.audit_export_limit);
                assert_eq!(first.denylist_routes, second.denylist_routes);
                assert_eq!(
                    first.hl7_transport.mllp_enabled,
                    second.hl7_transport.mllp_enabled
                );
                assert_eq!(
                    first.hl7_transport.mllp_bind,
                    second.hl7_transport.mllp_bind
                );
                assert_eq!(
                    first.hl7_transport.file_drop_dir,
                    second.hl7_transport.file_drop_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_done_dir,
                    second.hl7_transport.file_drop_done_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_error_dir,
                    second.hl7_transport.file_drop_error_dir
                );
                assert_eq!(
                    first.hl7_transport.file_drop_poll_interval_ms,
                    second.hl7_transport.file_drop_poll_interval_ms
                );
            },
        );
    }

    #[test]
    fn parse_log_level_rejects_invalid_value_from_env() {
        let err = with_env_var("DICOM_WORKFLOW_LOG_LEVEL", Some("invalid"), || {
            parse_log_level("DICOM_WORKFLOW_LOG_LEVEL").expect_err("invalid log level must fail")
        });
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_WORKFLOW_LOG_LEVEL"));
    }

    #[test]
    fn test_only_env_vars_do_not_change_production_rate_defaults() {
        let config = with_env_vars(
            &[
                ("DICOM_WORKFLOW_TEST_RATE_LIMIT", Some("9999")),
                ("DICOM_WORKFLOW_TEST_ROTATIONS", Some("9999")),
                ("DICOM_WORKFLOW_QUERY_RATE_LIMIT", None),
                ("DICOM_WORKFLOW_MUTATION_RATE_LIMIT", None),
            ],
            || WorkflowRuntimeConfig::from_env(),
        )
        .expect("runtime config defaults");
        assert_eq!(config.query_rate_limit, DEFAULT_QUERY_RATE_LIMIT);
        assert_eq!(config.mutation_rate_limit, DEFAULT_MUTATION_RATE_LIMIT);
    }

    #[test]
    fn parse_hl7_transport_config_requires_bind_when_enabled() {
        let err = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("true")),
                ("DICOM_WORKFLOW_HL7_MLLP_BIND", None),
            ],
            || parse_hl7_transport_config(),
        )
        .expect_err("MLLP enabled without bind must fail");
        assert!(err
            .to_string()
            .contains("DICOM_WORKFLOW_HL7_MLLP_ENABLED requires DICOM_WORKFLOW_HL7_MLLP_BIND"));
    }

    #[test]
    fn parse_hl7_transport_config_rejects_invalid_mllp_bind() {
        let err = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("true")),
                ("DICOM_WORKFLOW_HL7_MLLP_BIND", Some("bad:bind")),
            ],
            || parse_hl7_transport_config(),
        )
        .expect_err("invalid bind must fail");
        assert!(err.to_string().contains("requires a numeric TCP port"));
    }

    #[test]
    fn parse_hl7_transport_config_supports_file_drop_paths() {
        let drop_dir = temp_file_path("interop_drop", "in");
        let done_dir = temp_file_path("interop_drop", "done");
        let error_dir = temp_file_path("interop_drop", "err");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");
        let drop_dir = drop_dir.to_string_lossy().to_string();
        let done_dir = done_dir.to_string_lossy().to_string();
        let error_dir = error_dir.to_string_lossy().to_string();

        let config = with_env_vars(
            &[
                ("DICOM_WORKFLOW_HL7_MLLP_ENABLED", Some("false")),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_DIR", Some(&drop_dir)),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR", Some(&done_dir)),
                ("DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR", Some(&error_dir)),
                (
                    "DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS",
                    Some("4000"),
                ),
            ],
            || parse_hl7_transport_config(),
        )
        .expect("valid file-drop config");
        assert!(config.file_drop_dir.is_some());
        assert!(config.file_drop_done_dir.is_some());
        assert!(config.file_drop_error_dir.is_some());
        assert_eq!(config.file_drop_poll_interval_ms, 4000);

        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn parse_hl7_raw_message_payload_defaults_status_to_scheduled() {
        let body = b"MSH|^~\\&|pre|his|rdr|rdr|20260201||x|ADT|MSG0001|P|2.3\rPID|1||PAT-1||||\r";
        let params =
            parse_hl7_raw_message_payload(body, &Limits::default()).expect("parse raw HL7 payload");
        assert_eq!(params.get("status"), Some(&"SCHEDULED".to_string()));
        assert_eq!(params.get("patient_id"), Some(&"PAT-1".to_string()));
        assert_eq!(params.get("source"), Some(&"his".to_string()));
    }

    #[test]
    fn take_next_hl7_mllp_frame_extracts_payloads() {
        let mut buffer = Vec::from(b"noise\x0bMSH|^~\\&|HIS||RDR|\x1c\x0dAFTER");
        let frame = take_next_hl7_mllp_frame(&mut buffer, 1024).expect("extract mllp frame");
        assert_eq!(frame, Some(b"MSH|^~\\&|HIS||RDR|".to_vec()));
        assert_eq!(buffer, b"AFTER");
    }

    #[test]
    fn build_mllp_ack_wraps_frame_with_control_bytes() {
        let frame = build_mllp_ack("AA", None);
        assert_eq!(frame.first(), Some(&HL7_MLLP_START_BYTE));
        assert_eq!(
            &frame[frame.len() - HL7_MLLP_END_BYTES.len()..],
            HL7_MLLP_END_BYTES.as_slice()
        );
        assert_eq!(
            std::str::from_utf8(&frame[1..frame.len() - HL7_MLLP_END_BYTES.len()]).expect("utf8"),
            "MSA|AA|ACK\r"
        );
    }

    #[test]
    fn build_mllp_nack_includes_deterministic_error_code() {
        let frame = build_mllp_ack("AE", Some("DVF.WORKFLOW.HTTP.DECODE_ERROR"));
        assert_eq!(
            std::str::from_utf8(&frame[1..frame.len() - HL7_MLLP_END_BYTES.len()]).expect("utf8"),
            "MSA|AE|ACK\rERR|DVF.WORKFLOW.HTTP.DECODE_ERROR\r"
        );
    }

    #[test]
    fn classify_hl7_mllp_nack_maps_error_classes_deterministically() {
        let auth = auth_denied_error("forbidden");
        let decode = decode_error("bad payload");
        let not_found = not_found_error("missing", "DVF.WORKFLOW.MPPS.NOT_FOUND");

        let (auth_ack, auth_code) = classify_hl7_mllp_nack(auth.as_ref());
        let (decode_ack, decode_code) = classify_hl7_mllp_nack(decode.as_ref());
        let (nf_ack, nf_code) = classify_hl7_mllp_nack(not_found.as_ref());

        assert_eq!(auth_ack, "AR");
        assert_eq!(auth_code, "DVF.WORKFLOW.SR.AUTH_DENIED");
        assert_eq!(decode_ack, "AE");
        assert_eq!(decode_code, "DVF.WORKFLOW.HTTP.DECODE_ERROR");
        assert_eq!(nf_ack, "AE");
        assert_eq!(nf_code, "DVF.WORKFLOW.MPPS.NOT_FOUND");
    }

    #[test]
    fn hl7_dual_mode_ingest_deduplicates_under_concurrent_delivery() {
        let worklist_path = temp_file_path("interop_dual_mode_dedupe_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_dual_mode_dedupe_mpps", "snapshot");
        let sr_path = temp_file_path("interop_dual_mode_dedupe_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_dual_mode_dedupe_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let drop_dir = temp_file_path("interop_dual_mode_dedupe", "drop");
        let done_dir = temp_file_path("interop_dual_mode_dedupe", "done");
        let error_dir = temp_file_path("interop_dual_mode_dedupe", "error");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let limits = Arc::new(Limits::default());
        let payload = b"source=his&message_type=ADT&message_control_id=MSG-DUAL-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-1".to_vec();
        fs::write(drop_dir.join("dedupe.hl7"), &payload).expect("write drop payload");

        let shared_for_mllp = Arc::clone(&shared);
        let limits_for_mllp = Arc::clone(&limits);
        let payload_for_mllp = payload.clone();
        let ingest_handle = thread::spawn(move || {
            run_hl7_ingest_payload(
                &payload_for_mllp,
                None,
                "corr-dual-mllp",
                &shared_for_mllp,
                limits_for_mllp.as_ref(),
            )
        });

        run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &shared, &limits)
            .expect("run file-drop once");
        let ingest_result = ingest_handle.join().expect("join mllp ingest thread");
        assert!(ingest_result.is_ok());

        {
            let state = shared.lock().expect("state lock after dual-mode dedupe");
            assert_eq!(state.hl7.event_seq, 1);
            assert_eq!(state.hl7.replay_cache.len(), 1);
            assert!(state.hl7.failures.is_empty());
        }

        let done_count = fs::read_dir(&done_dir)
            .expect("read done directory")
            .count();
        let error_count = fs::read_dir(&error_dir)
            .expect("read error directory")
            .count();
        assert_eq!(done_count, 1);
        assert_eq!(error_count, 0);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn hl7_dual_mode_conflict_resolution_routes_divergent_payload_to_error_dir() {
        let worklist_path = temp_file_path("interop_dual_mode_conflict_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_dual_mode_conflict_mpps", "snapshot");
        let sr_path = temp_file_path("interop_dual_mode_conflict_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_dual_mode_conflict_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let drop_dir = temp_file_path("interop_dual_mode_conflict", "drop");
        let done_dir = temp_file_path("interop_dual_mode_conflict", "done");
        let error_dir = temp_file_path("interop_dual_mode_conflict", "error");
        fs::create_dir_all(&drop_dir).expect("create drop directory");
        fs::create_dir_all(&done_dir).expect("create done directory");
        fs::create_dir_all(&error_dir).expect("create error directory");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let limits = Arc::new(Limits::default());
        let accepted_payload = b"source=his&message_type=ADT&message_control_id=MSG-CONFLICT-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-1".to_vec();
        run_hl7_ingest_payload(
            &accepted_payload,
            None,
            "corr-conflict-primary",
            &shared,
            limits.as_ref(),
        )
        .expect("seed accepted replay payload");

        let divergent_payload = b"source=his&message_type=ADT&message_control_id=MSG-CONFLICT-1&scheduled_step_id=STEP-100&status=SCHEDULED&patient_id=P-2".to_vec();
        fs::write(drop_dir.join("conflict.hl7"), &divergent_payload)
            .expect("write conflict payload");
        run_hl7_file_drop_once(&drop_dir, &done_dir, &error_dir, &shared, &limits)
            .expect("run file-drop once");

        let done_count = fs::read_dir(&done_dir)
            .expect("read done directory")
            .count();
        let error_count = fs::read_dir(&error_dir)
            .expect("read error directory")
            .count();
        assert_eq!(done_count, 0);
        assert_eq!(error_count, 1);
        {
            let state = shared
                .lock()
                .expect("state lock after dual-mode conflict resolution");
            assert_eq!(state.hl7.event_seq, 1);
            assert_eq!(state.hl7.replay_cache.len(), 1);
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        let _ = fs::remove_dir_all(&drop_dir);
        let _ = fs::remove_dir_all(&done_dir);
        let _ = fs::remove_dir_all(&error_dir);
    }

    #[test]
    fn parse_denylist_routes_falls_back_to_defaults_when_empty() {
        let denied = parse_denylist_routes(Some(" ".to_string()));
        assert_eq!(denied, *DEFAULT_WORKFLOW_DENYLIST_PATHS);
    }

    #[test]
    fn parse_denylist_routes_splits_multiple_delimiters_and_dedups() {
        let denied = parse_denylist_routes(Some(
            "/a,/b;/c\n/c; /d\t/e "
                .to_string()
                .replace("/e", "/d")
                .into(),
        ));
        assert_eq!(denied, vec!["/a", "/b", "/c", "/d"]);
    }

    #[test]
    fn denylist_precedence_applies_before_role_and_tenant_scope_checks() {
        let worklist_path = temp_file_path("denylist_precedence_worklist", "snapshot");
        let mpps_path = temp_file_path("denylist_precedence_mpps", "snapshot");
        let sr_path = temp_file_path("denylist_precedence_sr", "snapshot");
        let sr_audit_path = temp_file_path("denylist_precedence_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut store = shared.lock().expect("state lock");
            store.denylist_routes = vec!["/workflow/audit".to_string()];
        }

        let mut viewer_headers = BTreeMap::new();
        viewer_headers.insert("x-sr-principal".to_string(), "viewer-user".to_string());
        viewer_headers.insert("x-sr-role".to_string(), "viewer".to_string());
        viewer_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());
        let denied_viewer = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-b".to_string())]),
            headers: viewer_headers,
            body: Vec::new(),
        };
        let viewer_err = route_request(&denied_viewer, &shared, &limits)
            .expect_err("denylist should fail first");
        assert_eq!(viewer_err.code, "DVF.WORKFLOW.SR.AUTH_DENIED");
        assert!(viewer_err
            .to_string()
            .contains("workflow operation deny-list"));

        let mut admin_headers = BTreeMap::new();
        admin_headers.insert("x-sr-principal".to_string(), "admin-user".to_string());
        admin_headers.insert("x-sr-role".to_string(), "admin".to_string());
        admin_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());
        let denied_admin = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-b".to_string())]),
            headers: admin_headers,
            body: Vec::new(),
        };
        let admin_err =
            route_request(&denied_admin, &shared, &limits).expect_err("denylist should fail first");
        assert_eq!(admin_err.code, "DVF.WORKFLOW.SR.AUTH_DENIED");
        assert!(admin_err
            .to_string()
            .contains("workflow operation deny-list"));

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_callback_idempotency_prune_enforces_ttl() {
        let mut cache = BTreeMap::new();
        let _ = cache.insert("old".to_string(), 1_000);
        let _ = cache.insert("fresh".to_string(), 9_500);
        hl7_callback_idempotency_prune(&mut cache, 10_000, 1_000);
        assert!(!cache.contains_key("old"));
        assert!(cache.contains_key("fresh"));
    }

    #[test]
    fn hl7_callback_idempotency_cache_load_filters_stale_entries() {
        let path = temp_file_path("hl7_callback_idempotency", "cache");
        fs::write(&path, "1000\told\n9500\tfresh\nbad-line\n")
            .expect("write callback idempotency cache fixture");
        let loaded = load_hl7_callback_idempotency_cache(&path.to_string_lossy(), 10_000, 1_000);
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("fresh"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn webhook_auth_strategy_hmac_requires_shared_secret_for_third_party_destinations() {
        let _guard = ENV_LOCK.lock().expect("env lock for webhook auth");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        let previous_secret = env::var(WEBHOOK_AUTH_SECRET_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "hmac-sha256");
        env::remove_var(WEBHOOK_AUTH_SECRET_ENV);

        let subscription = Hl7Subscription {
            id: "sub-webhook".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()],
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let err = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &BTreeMap::new(),
            "corr-1",
            1,
        )
        .expect_err("hmac strategy without shared secret must fail");
        assert_eq!(
            err,
            "webhook auth strategy hmac-sha256 requires shared secret"
        );

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
        if let Some(raw) = previous_secret {
            env::set_var(WEBHOOK_AUTH_SECRET_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_SECRET_ENV);
        }
    }

    #[test]
    fn webhook_auth_strategy_invalid_value_fails_closed() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for webhook auth invalid strategy");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "invalid-strategy");

        let subscription = Hl7Subscription {
            id: "sub-webhook-invalid".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()],
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let err = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &BTreeMap::new(),
            "corr-1",
            1,
        )
        .expect_err("invalid strategy must fail closed");
        assert_eq!(err, "unsupported webhook auth strategy");

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
    }

    #[test]
    fn webhook_auth_strategy_hmac_generates_deterministic_signature() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for webhook auth deterministic signature");
        let previous_strategy = env::var(WEBHOOK_AUTH_STRATEGY_ENV).ok();
        let previous_secret = env::var(WEBHOOK_AUTH_SECRET_ENV).ok();
        env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, "hmac-sha256");
        env::set_var(WEBHOOK_AUTH_SECRET_ENV, "shared-secret");

        let subscription = Hl7Subscription {
            id: "sub-webhook".to_string(),
            source: "workflow".to_string(),
            event_filter: vec!["task".to_string()],
            sink: Hl7Sink {
                kind: Hl7SinkKind::Webhook,
                target: "https://third-party.example/callback".to_string(),
            },
            delivered_events: 0,
            created_at_ms: 0,
            last_event_ms: 0,
        };
        let mut payload = BTreeMap::new();
        let _ = payload.insert("task_id".to_string(), "TASK-001".to_string());
        let first = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &payload,
            "corr-1",
            1,
        )
        .expect("signature")
        .expect("hmac strategy returns signature");
        let second = resolve_webhook_sink_signature(
            &subscription,
            "task",
            "workflow",
            &payload,
            "corr-1",
            1,
        )
        .expect("signature")
        .expect("hmac strategy returns signature");
        assert_eq!(first, second);
        assert!(!first.is_empty());

        if let Some(raw) = previous_strategy {
            env::set_var(WEBHOOK_AUTH_STRATEGY_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_STRATEGY_ENV);
        }
        if let Some(raw) = previous_secret {
            env::set_var(WEBHOOK_AUTH_SECRET_ENV, raw);
        } else {
            env::remove_var(WEBHOOK_AUTH_SECRET_ENV);
        }
    }

    #[test]
    fn parse_hl7_connector_registry_supports_prefix_precedence() {
        let _guard = ENV_LOCK.lock().expect("env lock for connector registry");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR").ok(),
            ),
        ];
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP",
            "https://default.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A",
            "https://exact.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR",
            "https://fallback.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR",
            "https://a-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR",
            "https://ax-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR",
            "https://a1-prefix.corp/connect",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR",
            "https://east-prefix.corp/connect",
        );

        let registry = parse_hl7_connector_registry();
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_alice"),
            Some("https://a-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_axyz"),
            Some("https://ax-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_a1bravo"),
            Some("https://a1-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_east_lab"),
            Some("https://east-prefix.corp/connect".to_string())
        );
        assert_eq!(
            resolve_hl7_connector_alias(&registry, "corp_a"),
            Some("https://exact.corp/connect".to_string())
        );
        assert_eq!(resolve_hl7_connector_alias(&registry, "other"), None);

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn parse_hl7_connector_plugins_rejects_adapter_version_outside_compatibility_range() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector plugin compatibility");
        let plugin_path = temp_file_path("connector_plugin_compat", "wasm");
        fs::write(&plugin_path, "mock plugin").expect("write plugin fixture");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS").ok(),
            ),
        ];
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            plugin_path.to_string_lossy().to_string(),
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
            "2.0.0",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
            "2.1.0",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
            "3.0.0",
        );

        let err = parse_hl7_connector_plugins()
            .expect_err("version outside compatible range should fail");
        assert!(err
            .to_string()
            .contains("outside compatible range [2.1.0, 3.0.0]"));

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = fs::remove_file(plugin_path);
    }

    #[test]
    fn parse_hl7_connector_plugins_validates_compatibility_version_matrix() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector plugin matrix");
        let plugin_path = temp_file_path("connector_plugin_matrix", "wasm");
        fs::write(&plugin_path, "mock plugin").expect("write plugin fixture");

        let keys = [
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
            "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
        ];
        let mut prev = BTreeMap::new();
        for key in keys {
            let _ = prev.insert(key, std::env::var(key).ok());
        }
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS",
            plugin_path.to_string_lossy().to_string(),
        );

        let cases = [
            ("2.1.0", "2.1.0", "3.0.0", true),
            ("2.5.4", "2.1.0", "3.0.0", true),
            ("3.0.0", "2.1.0", "3.0.0", true),
            ("2.0.9", "2.1.0", "3.0.0", false),
            ("3.0.1", "2.1.0", "3.0.0", false),
            ("2.5.0", "3.0.0", "2.1.0", false),
        ];
        for (version, min, max, should_pass) in cases {
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS",
                version,
            );
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS",
                min,
            );
            std::env::set_var(
                "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS",
                max,
            );

            let parsed = parse_hl7_connector_plugins();
            if should_pass {
                let plugins = parsed.expect("matrix case should pass compatibility validation");
                let plugin = plugins
                    .get("enterprisehis")
                    .expect("enterprisehis plugin should be present");
                assert_eq!(plugin.adapter_version, version);
                assert_eq!(plugin.compatible_min, min);
                assert_eq!(plugin.compatible_max, max);
            } else {
                assert!(
                    parsed.is_err(),
                    "matrix case should fail compatibility validation"
                );
            }
        }

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
        let _ = fs::remove_file(plugin_path);
    }

    #[test]
    fn parse_hl7_connector_registry_skips_feature_and_rollout_env_vars() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for connector registry filtering");
        let prev = [
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS").ok(),
            ),
            (
                "DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS",
                std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS").ok(),
            ),
        ];

        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
            "https://connectors.enterprise-his.example/interop",
        );
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS",
            "false",
        );
        std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", "37");

        let registry = parse_hl7_connector_registry();
        let feature_flags = parse_hl7_connector_feature_flags().expect("parse feature flags");
        let rollout_percents =
            parse_hl7_connector_rollout_percents().expect("parse rollout percents");
        assert_eq!(
            registry.get("enterprise_his"),
            Some(&"https://connectors.enterprise-his.example/interop".to_string())
        );
        assert_eq!(
            feature_flags
                .get("enterprise_his")
                .expect("enterprise feature flag")
                .as_bool(),
            false
        );
        assert_eq!(rollout_percents.get("enterprise_his"), Some(&37));

        for (name, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(name, raw);
            } else {
                std::env::remove_var(name);
            }
        }
    }

    #[test]
    fn parse_hl7_connector_rollout_percent_rejects_values_above_100() {
        let _guard = ENV_LOCK.lock().expect("env lock for rollout bounds");
        let previous = std::env::var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS").ok();
        std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", "101");

        let err =
            parse_hl7_connector_rollout_percents().expect_err("rollout above 100 should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        match previous {
            Some(raw) => {
                std::env::set_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS", raw)
            }
            None => std::env::remove_var("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS"),
        }
    }

    #[test]
    fn parse_hl7_callback_policy_allows_operator_overrides() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for callback policy overrides");
        let keys = [
            HL7_CALLBACK_MAX_ATTEMPTS_ENV,
            HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV,
            HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV,
            HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV,
        ];
        let mut prev = BTreeMap::new();
        for key in keys {
            let _ = prev.insert(key, std::env::var(key).ok());
        }

        std::env::set_var(HL7_CALLBACK_MAX_ATTEMPTS_ENV, "5");
        std::env::set_var(HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV, "3");
        std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, "20000");
        std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, "240000");

        let policy =
            parse_hl7_callback_policy_from_env().expect("callback policy override should parse");
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.circuit_failure_threshold, 3);
        assert_eq!(policy.circuit_base_backoff_ms, 20000);
        assert_eq!(policy.circuit_max_backoff_ms, 240000);

        for (key, value) in prev {
            if let Some(raw) = value {
                std::env::set_var(key, raw);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn parse_hl7_callback_policy_rejects_invalid_backoff_window() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for callback policy backoff");
        let prev_base = std::env::var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV).ok();
        let prev_max = std::env::var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV).ok();

        std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, "500000");
        std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, "100000");

        let err = parse_hl7_callback_policy_from_env()
            .expect_err("base backoff greater than max should fail");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        assert!(err
            .to_string()
            .contains(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV));

        if let Some(raw) = prev_base {
            std::env::set_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV, raw);
        } else {
            std::env::remove_var(HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV);
        }
        if let Some(raw) = prev_max {
            std::env::set_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV, raw);
        } else {
            std::env::remove_var(HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV);
        }
    }

    #[test]
    fn hl7_failure_queue_round_trip_preserves_record_order() {
        let dlq_path = temp_file_path("hl7_dlq_order", "snapshot");
        let mut queue = VecDeque::new();
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000010".to_string(),
            source: "his".to_string(),
            message_type: "ADT".to_string(),
            reason: "parse_error".to_string(),
            payload_excerpt: "msh".to_string(),
            created_at_ms: 10,
            scope: "ingest".to_string(),
            subscription_id: String::new(),
            event_id: "HL7-000100".to_string(),
            correlation_id: "corr-10".to_string(),
            sequence: 100,
            attempt: 1,
            max_attempts: 3,
        });
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000009".to_string(),
            source: "his".to_string(),
            message_type: "ORM".to_string(),
            reason: "decode_error".to_string(),
            payload_excerpt: "pid".to_string(),
            created_at_ms: 9,
            scope: "callback".to_string(),
            subscription_id: "sub-1".to_string(),
            event_id: "HL7-000099".to_string(),
            correlation_id: "corr-9".to_string(),
            sequence: 99,
            attempt: 3,
            max_attempts: 3,
        });
        persist_hl7_failure_queue(&dlq_path.to_string_lossy(), &queue);

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        assert_eq!(
            loaded.front().map(|record| record.id.as_str()),
            Some("hl7-fail-000010")
        );
        assert_eq!(
            loaded.back().map(|record| record.id.as_str()),
            Some("hl7-fail-000009")
        );

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn hl7_failure_queue_migrates_legacy_v1_rows_to_current_schema_defaults() {
        let dlq_path = temp_file_path("hl7_dlq_legacy_v1", "snapshot");
        let legacy =
            "hl7-fail-000001\this\tADT\tparse_error\tmsh\t101\tingest\t\tHL7-000001\tcorr-1\t7\n";
        fs::write(&dlq_path, legacy).expect("write legacy v1 queue");

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        let record = loaded.front().expect("one legacy row should load");
        assert_eq!(record.id, "hl7_fail_000001");
        assert_eq!(record.sequence, 7);
        assert_eq!(record.attempt, 1);
        assert_eq!(record.max_attempts, MAX_WORKFLOW_CALLBACK_ATTEMPTS);

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn hl7_failure_queue_rollback_projection_preserves_core_fields() {
        let dlq_path = temp_file_path("hl7_dlq_rollback_projection", "snapshot");
        let mut queue = VecDeque::new();
        queue.push_back(Hl7FailureRecord {
            id: "hl7-fail-000021".to_string(),
            source: "his".to_string(),
            message_type: "ORU".to_string(),
            reason: "timeout".to_string(),
            payload_excerpt: "obr".to_string(),
            created_at_ms: 401,
            scope: "callback".to_string(),
            subscription_id: "sub-44".to_string(),
            event_id: "HL7-000021".to_string(),
            correlation_id: "corr-21".to_string(),
            sequence: 21,
            attempt: 4,
            max_attempts: 5,
        });
        let legacy_payload = render_hl7_failure_queue_legacy_v1(&queue);
        fs::write(&dlq_path, legacy_payload).expect("write rollback-projected queue");

        let loaded = load_hl7_failure_queue(&dlq_path.to_string_lossy(), MAX_HL7_FAILURES);
        let record = loaded.front().expect("one rollback row should load");
        assert_eq!(record.message_type, "oru");
        assert_eq!(record.scope, "callback");
        assert_eq!(record.sequence, 21);
        assert_eq!(record.attempt, 1);
        assert_eq!(record.max_attempts, MAX_WORKFLOW_CALLBACK_ATTEMPTS);

        cleanup_with_rotations(&dlq_path, 0);
    }

    #[test]
    fn reconciliation_idempotency_snapshot_filters_non_reconciliation_entries() {
        let snapshot_path = temp_file_path("recon_idempotency_snapshot", "state");
        let mut cache = BTreeMap::new();
        let _ = cache.insert(
            "task:create:tenant-a:1".to_string(),
            CachedMppsRequest {
                signature: "task-signature".to_string(),
                response: "{\"task\":\"ok\"}".to_string(),
            },
        );
        let _ = cache.insert(
            "reconciliation-run:tenant-a:recon-1:key-1".to_string(),
            CachedMppsRequest {
                signature: "recon-signature".to_string(),
                response: "{\"recon\":\"ok\"}".to_string(),
            },
        );

        persist_reconciliation_run_idempotency_cache(&snapshot_path.to_string_lossy(), &cache);
        let loaded = load_reconciliation_run_idempotency_cache(&snapshot_path.to_string_lossy());

        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("reconciliation-run:tenant-a:recon-1:key-1"));
        assert!(!loaded.contains_key("task:create:tenant-a:1"));

        cleanup_with_rotations(&snapshot_path, 0);
    }

    fn build_sr_workflow_state(
        worklist_path: &Path,
        mpps_path: &Path,
        sr_path: &Path,
        sr_audit_path: &Path,
    ) -> Arc<Mutex<RuntimeState>> {
        let limits = Limits::default();
        set_tenant_rate_limit_override_cache(BTreeMap::new());
        let audit_path = sr_audit_path.to_string_lossy().to_string();
        let callback_idempotency_path = callback_idempotency_snapshot_path(&audit_path);
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState {
                subscriptions: BTreeMap::new(),
                failures: VecDeque::new(),
                ups: UpsCommandAdapter::new(),
                completion: CompletionWorkflowAdapter::new(),
                ian_events: BTreeMap::new(),
                storage_commitment_status: BTreeMap::new(),
                hl7_ups_correlation: BTreeMap::new(),
                subscription_seq: 0,
                failure_seq: 0,
                event_seq: 0,
                replay_cache: BTreeMap::new(),
                connector_registry: BTreeMap::new(),
                connector_feature_flags: BTreeMap::new(),
                connector_rollout_percent: BTreeMap::new(),
                connector_plugins: BTreeMap::new(),
                callback_delivery_idempotency: BTreeMap::new(),
                callback_idempotency_ttl_ms: DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS,
                callback_idempotency_path,
                callback_max_attempts: MAX_WORKFLOW_CALLBACK_ATTEMPTS,
                callback_circuit_breaker_failure_threshold:
                    CALLBACK_CIRCUIT_BREAKER_FAILURE_THRESHOLD,
                callback_circuit_breaker_base_backoff_ms: CALLBACK_CIRCUIT_BREAKER_BASE_BACKOFF_MS,
                callback_circuit_breaker_max_backoff_ms: CALLBACK_CIRCUIT_BREAKER_MAX_BACKOFF_MS,
                connector_callback_failure_streak: BTreeMap::new(),
                connector_circuit_open_until_ms: BTreeMap::new(),
                reconciliation_jobs: BTreeMap::new(),
                reconciliation_seq: 0,
            },
            audit_path,
            audit_rate_window_ms: DEFAULT_RATE_LIMIT_WINDOW_MS,
            query_rate_limit: DEFAULT_QUERY_RATE_LIMIT,
            mutation_rate_limit: DEFAULT_MUTATION_RATE_LIMIT,
            upload_cap_bytes: DEFAULT_UPLOAD_CAP_BYTES,
            audit_max_bytes: DEFAULT_AUDIT_MAX_BYTES,
            audit_max_rotated_files: DEFAULT_AUDIT_MAX_ROTATED_FILES,
            rate_windows: BTreeMap::new(),
            anomaly_alert_threshold: DEFAULT_ANOMALY_ALERT_THRESHOLD,
            audit_export_limit: DEFAULT_AUDIT_EXPORT_LIMIT,
            denylist_routes: DEFAULT_WORKFLOW_DENYLIST_PATHS
                .iter()
                .map(|path| path.to_string())
                .collect(),
            tenant_worklist: BTreeMap::new(),
            tenant_mpps: BTreeMap::new(),
            tenant_sr: BTreeMap::new(),
            tenant_tasks: BTreeMap::new(),
            metrics: BTreeMap::new(),
        };
        Arc::new(Mutex::new(state))
    }

    #[test]
    fn percent_decode_decodes_form_values() {
        // REQ-HTTP-301: query/body value decoding is deterministic.
        assert_eq!(percent_decode("A%20B+X"), Some("A B X".to_string()));
    }

    #[test]
    fn parse_pairs_rejects_non_ascii() {
        // REQ-HTTP-301: non-ASCII inputs fail closed.
        let err = parse_pairs("modality=%E2%98%83", &Limits::default()).expect_err("error");
        assert!(matches!(err.kind, ErrorKind::DecodeError { .. }));
    }

    #[test]
    fn parse_pairs_enforces_workflow_pair_cap() {
        // REQ-HTTP-301: workflow query/body parsing is bounded by max_workflow_pair_count.
        let mut pairs = Vec::new();
        for index in 0..300 {
            pairs.push(format!("k{index}=v"));
        }
        let err = parse_pairs(&pairs.join("&"), &Limits::default()).expect_err("error");
        match err.kind {
            ErrorKind::LimitExceeded {
                limit_name,
                observed,
                ..
            } => {
                assert_eq!(limit_name, "max_workflow_pair_count");
                assert_eq!(observed, 257);
            }
            _ => panic!("expected LimitExceeded"),
        }
    }

    #[test]
    fn sr_auth_context_accepts_operator_role_alias_header() {
        // REQ-AUTH-300: workflow auth role checks accept operator/admin writer-equivalent role aliases.
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: {
                let mut headers = BTreeMap::new();
                headers.insert("x-auth-principal".to_string(), "alice".to_string());
                headers.insert("x-auth-role".to_string(), "operator".to_string());
                headers
            },
            body: Vec::new(),
        };
        let auth = sr_auth_context(&request);
        assert!(auth.can_write);
        assert_eq!(auth.principal.as_deref(), Some("alice"));
    }

    #[test]
    fn workflow_audit_chain_detects_tampering() {
        // REQ-AUDIT-351: audit chain integrity is verifiable and tamper-evident.
        let path = temp_file_path("workflow_audit_chain", "log");
        let first = WorkflowAuditEvent {
            ts_ms: 100,
            tenant: "tenant-1".to_string(),
            principal_hash: "writer-a".to_string(),
            request_id_hash: "req-001".to_string(),
            previous_audit_hash: String::new(),
            audit_hash: String::new(),
            route: "/workflow/metrics".to_string(),
            method: "GET".to_string(),
            operation: "read".to_string(),
            outcome: "success".to_string(),
            scope: "tenant-1".to_string(),
            status: 200,
            anomaly: false,
        };
        let second = WorkflowAuditEvent {
            ts_ms: 101,
            tenant: "tenant-1".to_string(),
            principal_hash: "writer-b".to_string(),
            request_id_hash: "req-002".to_string(),
            previous_audit_hash: String::new(),
            audit_hash: String::new(),
            route: "/workflow/tasks".to_string(),
            method: "POST".to_string(),
            operation: "create".to_string(),
            outcome: "success".to_string(),
            scope: "tenant-1".to_string(),
            status: 201,
            anomaly: false,
        };
        append_workflow_audit_event(&path.to_string_lossy(), 1_048_576, 4, 128, &first);
        append_workflow_audit_event(&path.to_string_lossy(), 1_048_576, 4, 128, &second);

        let lines: Vec<String> = fs::read_to_string(&path)
            .expect("read audit")
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        assert_eq!(lines.len(), 2);
        let parsed_first = parse_workflow_audit_line(&lines[0]).expect("parsed first");
        let parsed_second = parse_workflow_audit_line(&lines[1]).expect("parsed second");
        assert_eq!(parsed_first.previous_audit_hash, "");
        assert_eq!(
            parsed_second.previous_audit_hash, parsed_first.audit_hash,
            "chain should link previous hash"
        );
        assert!(
            verify_workflow_audit_chain(&lines).is_empty(),
            "chain should verify when untouched"
        );

        let mut tampered = lines.clone();
        tampered[1] = tampered[1].replace("tenant-1", "tenant-2");
        let failures = verify_workflow_audit_chain(&tampered);
        assert!(!failures.is_empty());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn route_policy_enforcement_failure_log_includes_required_fields() {
        let actor = WorkflowActorContext {
            principal: Some("alice".to_string()),
            role: Some("writer".to_string()),
            tenant: "tenant-a".to_string(),
        };
        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "corr-123".to_string())]),
            body: Vec::new(),
        };
        let contract =
            workflow_contract_for_request("POST", "/workflow/tasks").expect("route contract");
        let log_line = render_route_policy_enforcement_failure_log(
            &actor,
            &request,
            &contract,
            "mutation_rate_limit",
            "rate limit exceeded",
        );

        assert!(log_line.contains("\"event\":\"workflow_route_policy_enforcement_failure\""));
        assert!(log_line.contains("\"tenant\":\"tenant-a\""));
        assert!(log_line.contains("\"route\":\"/workflow/tasks\""));
        assert!(log_line.contains("\"policy\":\"mutation_rate_limit\""));
        assert!(log_line.contains("\"correlation_id\":\"corr-123\""));
    }

    #[test]
    fn workflow_audit_payload_order_is_deterministic() {
        let lines = vec![
            "{\"ts_ms\":200,\"tenant\":\"tenant-b\"}".to_string(),
            "{\"ts_ms\":100,\"tenant\":\"tenant-a\"}".to_string(),
        ];
        let first = render_workflow_audit_payload(&lines);
        let second = render_workflow_audit_payload(&lines);

        assert_eq!(
            first,
            "[{\"ts_ms\":200,\"tenant\":\"tenant-b\"},{\"ts_ms\":100,\"tenant\":\"tenant-a\"}]"
        );
        assert_eq!(first, second);
    }

    #[test]
    fn workflow_route_error_invariants_are_unique_and_scoped() {
        let mut seen = BTreeMap::new();
        for invariant in WORKFLOW_ROUTE_ERROR_INVARIANTS {
            assert!(!invariant.route_group.is_empty());
            assert!(
                seen.insert(invariant.error_code.to_string(), true)
                    .is_none(),
                "duplicate workflow error-code invariant: {}",
                invariant.error_code
            );
        }
    }

    #[test]
    fn workflow_route_error_invariants_map_task_codes_to_expected_status() {
        let not_found = Error::new(
            "DVF.WORKFLOW.TASK.NOT_FOUND",
            ErrorKind::DecodeError {
                stage: "workflow-task".to_string(),
                detail: "task not found".to_string(),
            },
            "task not found",
        );
        let invalid_transition = Error::new(
            "DVF.WORKFLOW.TASK.INVALID_TRANSITION",
            ErrorKind::DecodeError {
                stage: "workflow-task".to_string(),
                detail: "invalid transition".to_string(),
            },
            "invalid transition",
        );
        assert_eq!(status_for_error(&not_found), (404, "Not Found"));
        assert_eq!(status_for_error(&invalid_transition), (409, "Conflict"));
    }

    #[test]
    fn request_id_uses_x_request_id_header_when_present() {
        let mut headers = BTreeMap::new();
        headers.insert("x-request-id".to_string(), "trace-001".to_string());
        assert_eq!(request_id_from_headers(&headers), "trace-001");
    }

    #[test]
    fn request_id_falls_back_when_header_missing_or_empty() {
        assert!(request_id_from_headers(&BTreeMap::new()).starts_with("request-"));
        let mut headers = BTreeMap::new();
        headers.insert("x-request-id".to_string(), "   ".to_string());
        assert!(request_id_from_headers(&headers).starts_with("request-"));
    }

    #[test]
    fn auth_mode_defaults_to_deny_all() {
        // REQ-AUTH-300: runtime defaults fail closed.
        let mode = AuthMode::DenyAll;
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/healthz".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "tls"));
    }

    #[test]
    fn insecure_transport_rejected_even_when_auth_allows() {
        // REQ-HTTP-303 + REQ-AUTH-300: insecure transport must fail closed by default policy.
        let mode = AuthMode::AllowAll;
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/healthz".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "insecure"));
    }

    #[test]
    fn token_mode_requires_matching_header_on_tls() {
        // REQ-AUTH-300: token mode allows only exact token match.
        let mode = AuthMode::Token("secret-token".to_string());
        let mut headers = BTreeMap::new();
        headers.insert("x-rdvf-token".to_string(), "wrong-token".to_string());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: Vec::new(),
        };
        assert!(!authorize(&request, &mode, "tls"));

        headers.insert("x-rdvf-token".to_string(), "secret-token".to_string());
        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers,
            body: Vec::new(),
        };
        assert!(authorize(&request, &mode, "tls"));
    }

    #[test]
    fn auth_mode_label_never_exposes_secret_values() {
        // REQ-AUTH-300: startup policy logging must not leak token material.
        let mode = AuthMode::Token("super-secret".to_string());
        assert_eq!(auth_mode_label(&mode), "token");
    }

    #[test]
    fn preflight_rejects_directory_path() {
        let path = temp_file_path("workflow_dir", "snapshot");
        fs::create_dir_all(&path).expect("mkdir");
        let err = prepare_persistence_file(&path.to_string_lossy(), 1024, 2, "worklist snapshot")
            .expect_err("expected error");
        assert_eq!(err.kind(), IoErrorKind::InvalidInput);
        let _ = fs::remove_dir_all(&path);
    }

    #[test]
    fn preflight_rotates_oversized_snapshot_and_bounds_backup_count() {
        let path = temp_file_path("workflow_rotate", "snapshot");
        fs::write(&path, vec![7u8; 64]).expect("write snapshot");
        fs::write(path_with_suffix(&path, 1), vec![8u8; 8]).expect("write snapshot.1");
        prepare_persistence_file(&path.to_string_lossy(), 16, 2, "worklist snapshot")
            .expect("preflight");
        assert!(path.exists());
        assert!(path_with_suffix(&path, 1).exists());
        assert!(path_with_suffix(&path, 2).exists());
        assert!(!path_with_suffix(&path, 3).exists());
        cleanup_with_rotations(&path, 3);
    }

    #[test]
    fn workflow_audit_rotation_enforces_retention_cap() {
        let path = temp_file_path("workflow_audit_rotate", "log");
        fs::write(&path, vec![1u8; 64]).expect("write audit log");
        fs::write(path_with_suffix(&path, 1), vec![2u8; 8]).expect("write audit.1");
        fs::write(path_with_suffix(&path, 2), vec![3u8; 8]).expect("write audit.2");
        fs::write(path_with_suffix(&path, 3), vec![4u8; 8]).expect("write audit.3");

        rotate_audit_if_needed(&path.to_string_lossy(), 16, 2).expect("rotate audit");

        assert!(path_with_suffix(&path, 1).exists());
        assert!(path_with_suffix(&path, 2).exists());
        assert!(!path_with_suffix(&path, 3).exists());
        cleanup_with_rotations(&path, 3);
    }

    #[test]
    fn workflow_runtime_persistence_recovers_after_restart() {
        // REQ-WL-303 + REQ-MPPS-354: runtime-level restart must recover persisted workflow state.
        let worklist_path = temp_file_path("workflow_restart_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_restart_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_restart_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_restart_sr", "audit");
        prepare_persistence_file(
            &worklist_path.to_string_lossy(),
            1_048_576,
            2,
            "worklist snapshot",
        )
        .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps snapshot")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr snapshot")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");
        let limits = Limits::default();

        {
            let state = RuntimeState {
                worklist: WorklistStore::open(limits.clone(), &worklist_path)
                    .expect("open worklist"),
                mpps: MppsService::with_persistence(
                    MppsServiceConfig {
                        limits: limits.clone(),
                        audit: None,
                    },
                    &mpps_path,
                )
                .expect("open mpps"),
                sr: SrWorkflowStore::open(
                    limits.clone(),
                    &sr_path.to_string_lossy(),
                    &sr_audit_path.to_string_lossy(),
                )
                .expect("open sr"),
                mpps_idempotency: BTreeMap::new(),
                tasks: BTreeMap::new(),
                task_id_sequence: 1,
                task_idempotency: BTreeMap::new(),
                hl7: Hl7RuntimeState::default(),
            };
            let shared = Arc::new(Mutex::new(state));

            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: b"scheduled_step_id=STEP1&modality=CT&start_date=20260211&start_time=101010&patient_id=P001".to_vec(),
            };
            let worklist_response =
                route_request(&worklist_post, &shared, &limits).expect("worklist upsert");
            match worklist_response {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }

            let mpps_post = HttpRequest {
                method: "POST".to_string(),
                path: "/mpps/updates".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=111111".to_vec(),
            };
            let mpps_response = route_request(&mpps_post, &shared, &limits).expect("mpps upsert");
            match mpps_response {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let restarted = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("reopen worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("reopen mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("reopen sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(restarted));

        let worklist_get = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_get, &shared, &limits).expect("worklist get");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("STEP1"));
            }
        }

        let mpps_get = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let mpps_response = route_request(&mpps_get, &shared, &limits).expect("mpps get");
        match mpps_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_http_flow_create_update_retrieve_is_deterministic() {
        // REQ-SR-300, REQ-HI-165, REQ-HI-170
        let worklist_path = temp_file_path("sr_flow_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_flow_mpps", "snapshot");
        let sr_path = temp_file_path("sr_flow_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_flow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert("x-idempotency-key".to_string(), "create-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=num&concept_code_value=G-D7FE&concept_scheme=SRT&concept_meaning=Length&num_value=12.5&units_code_value=mm&units_scheme=UCUM&units_meaning=millimeter&referenced_sop_instance_uid=9.8.7&known_refs=9.8.7".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut update_headers = BTreeMap::new();
        update_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        update_headers.insert("x-sr-role".to_string(), "writer".to_string());
        update_headers.insert("x-idempotency-key".to_string(), "update-1".to_string());
        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: update_headers.clone(),
            body: b"expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=stable+finding&known_refs=9.8.7&referenced_sop_instance_uid=9.8.7".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("update");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
                assert!(body.contains("\"version\":2"));
            }
        }

        let replay_response = route_request(&update, &shared, &limits).expect("update replay");
        match replay_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"duplicate\""));
                assert!(body.contains("\"idempotency_replay\":true"));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get, &shared, &limits).expect("get");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"version\":2"));
                assert!(body.contains("stable finding"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_route_contract_is_valid_for_create_update_get_and_list_schemas() {
        // REQ-SR-300, REQ-HI-168, REQ-SR-201
        let worklist_path = temp_file_path("sr_contract_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_contract_mpps", "snapshot");
        let sr_path = temp_file_path("sr_contract_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_contract_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-contract-create".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=num&concept_code_value=G-D7FE&concept_scheme=SRT&concept_meaning=Length&num_value=12.5&units_code_value=mm&units_scheme=UCUM&units_meaning=millimeter&known_refs=9.8.7".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create request");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"sop_instance_uid\":\"1.2.3.4.5\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut update_headers = BTreeMap::new();
        update_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        update_headers.insert("x-sr-role".to_string(), "writer".to_string());
        update_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-contract-update".to_string(),
        );
        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: update_headers.clone(),
            body: b"expected_version=1&item_kind=code&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&value_code_value=F-001&value_scheme=DCM&value_meaning=Observation&known_refs=9.8.7".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("update request");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
                assert!(body.contains("\"version\":2"));
            }
        }

        let invalid_updates_method = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = route_request(&invalid_updates_method, &shared, &limits)
            .expect_err("updates path supports POST only");
        assert_eq!(err.code, "DVF.HTTP.DECODE");

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get, &shared, &limits).expect("get request");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"provenance\""));
                assert!(body.contains("\"study_instance_uid\":\"1.2.3\""));
                assert!(body.contains("\"series_instance_uid\":\"1.2.3.4\""));
                assert!(body.contains("\"sop_instance_uid\":\"1.2.3.4.5\""));
                assert!(body.contains("\"version\":2"));
                assert!(body.contains("\"kind\":\"num\""));
                assert!(body.contains("\"kind\":\"code\""));
            }
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list request");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4.5"));
                assert!(body.contains("\"item_count\":2"));
                assert!(body.contains("\"observer\":\"alice\""));
            }
        }

        let invalid_collection_method = HttpRequest {
            method: "PATCH".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = route_request(&invalid_collection_method, &shared, &limits)
            .expect_err("collection path supports GET/HEAD/POST only");
        assert_eq!(err.code, "DVF.HTTP.DECODE");

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    fn workflow_route_template_sample_path(template: &str) -> String {
        let mut path = template.to_string();
        let replacements = [
            ("{sop_instance_uid}", "1.2.3.4.5"),
            ("{task_id}", "TASK-0001"),
            ("{job_id}", "JOB-0001"),
            ("{StudyUID}", "1.2.3"),
            ("{SeriesUID}", "1.2.3.4"),
            ("{InstanceUID}", "1.2.3.4.5.6"),
        ];
        for (template_key, value) in replacements {
            path = path.replace(template_key, value);
        }
        assert!(
            !path.contains('{') && !path.contains('}'),
            "failed to expand route template: {template} => {path}"
        );
        path
    }

    fn workflow_route_template_request_headers(
        contract: &WorkflowRouteContract,
        unique_id: usize,
    ) -> BTreeMap<String, String> {
        let mut headers = BTreeMap::new();
        if contract.requires_writer_role {
            headers.insert(
                "x-sr-principal".to_string(),
                "route-matrix-probe".to_string(),
            );
            headers.insert("x-sr-role".to_string(), "writer".to_string());
        }
        if contract.requires_idempotency_key {
            headers.insert(
                "x-idempotency-key".to_string(),
                format!("route-matrix-probe-{unique_id}"),
            );
        }
        headers
    }

    fn workflow_route_template_invalid_path(
        template: &str,
        placeholder: &str,
        value: &str,
    ) -> String {
        template.replace(placeholder, value)
    }

    #[test]
    fn workflow_route_contract_rows_are_dispatchable_from_request_router() {
        // REQ-HI-275, REQ-HI-326
        let worklist_path = temp_file_path("workflow_route_matrix_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_route_matrix_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_route_matrix_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_route_matrix_sr_audit", "audit");

        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let contracts = workflow_route_contract();
        for (index, contract) in contracts.iter().enumerate() {
            let sample_path = workflow_route_template_sample_path(contract.path_template);
            for method in contract.method.split('|') {
                let headers = workflow_route_template_request_headers(contract, index);
                let request = HttpRequest {
                    method: method.to_string(),
                    path: sample_path.clone(),
                    query: BTreeMap::new(),
                    headers,
                    body: Vec::new(),
                };
                let outcome = route_request(&request, &state, &limits);
                if let Err(err) = outcome {
                    if let dicom_core::ErrorKind::DecodeError { detail, .. } = &err.kind {
                        assert_ne!(
                            detail.as_str(),
                            "unsupported route",
                            "route contract row is not wired to routing: {} {}",
                            method,
                            contract.path_template
                        );
                    }
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_route_contract_path_parameters_are_validated_by_contract() {
        // REQ-HI-327
        let worklist_path = temp_file_path("workflow_route_param_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_route_param_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_route_param_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_route_param_sr_audit", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let invalid_values = [
            ("{sop_instance_uid}", "uid with spaces"),
            ("{task_id}", "task with spaces"),
            ("{job_id}", "job with spaces"),
        ];

        for contract in workflow_route_contract() {
            for (placeholder, value) in invalid_values {
                if !contract.path_template.contains(placeholder) {
                    continue;
                }
                let invalid_path = workflow_route_template_invalid_path(
                    contract.path_template,
                    placeholder,
                    value,
                );
                for method in contract.method.split('|') {
                    let headers = workflow_route_template_request_headers(contract, 100);
                    let request = HttpRequest {
                        method: method.to_string(),
                        path: invalid_path.clone(),
                        query: BTreeMap::new(),
                        headers,
                        body: Vec::new(),
                    };
                    let err = route_request(&request, &state, &limits)
                        .expect_err("invalid path parameter should fail");
                    assert_eq!(err.code, "DVF.HTTP.DECODE");
                    assert!(
                        matches!(err.kind, ErrorKind::DecodeError { .. }),
                        "route validation must fail as decode error for {}/{}",
                        method,
                        contract.path_template
                    );
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_identifier_aliases_and_normalization_are_supported() {
        let worklist_path = temp_file_path("id_alias_worklist", "snapshot");
        let mpps_path = temp_file_path("id_alias_mpps", "snapshot");
        let sr_path = temp_file_path("id_alias_sr", "snapshot");
        let sr_audit_path = temp_file_path("id_alias_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let alias_patient = "  P-ALIAS  ";
        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: format!(
                "scheduled_step_id=STEP-A&modality=CT&start_date=20260211&start_time=101010&patient_id={alias_patient}"
            )
            .into_bytes(),
        };
        let worklist_response =
            route_request(&worklist_post, &shared, &limits).expect("worklist upsert");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("PatientID=%20P-ALIAS%20", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_list =
            route_request(&worklist_query, &shared, &limits).expect("query by patient");
        match worklist_list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("P-ALIAS"));
            }
        }

        let mut sr_headers = BTreeMap::new();
        sr_headers.insert("x-sr-role".to_string(), "writer".to_string());
        sr_headers.insert("x-idempotency-key".to_string(), "id-alias-1".to_string());
        let sr_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers,
            body: b"study_uid=1.2.3&series_uid=1.2.3.4&sop_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=normalized".to_vec(),
        };
        let sr_create_response = route_request(&sr_create, &shared, &limits).expect("sr create");
        match sr_create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let sr_list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map("StudyInstanceUID=%201.2.3%20", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let sr_list_response = route_request(&sr_list, &shared, &limits).expect("sr list");
        match sr_list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4.5"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_http_lifecycle_endpoints_enforce_state_transitions_and_immutable_history() {
        // REQ-SR-300, REQ-SR-315, REQ-HI-170
        let worklist_path = temp_file_path("sr_lifecycle_workflow_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_lifecycle_workflow_mpps", "snapshot");
        let sr_path = temp_file_path("sr_lifecycle_workflow_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_lifecycle_workflow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-lifecycle-create-1".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=baseline&known_refs=9.8.7".to_vec(),
        };
        match route_request(&create, &shared, &limits).expect("sr create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
                assert!(body.contains("\"version\":1"));
            }
        }

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        review_headers.insert("x-sr-role".to_string(), "writer".to_string());
        review_headers.insert("x-idempotency-key".to_string(), "sr-review-1".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/review".to_string(),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("sr review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let mut finalize_headers = BTreeMap::new();
        finalize_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        finalize_headers.insert("x-sr-role".to_string(), "writer".to_string());
        finalize_headers.insert("x-idempotency-key".to_string(), "sr-finalize-1".to_string());
        let finalize = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/finalize".to_string(),
            query: BTreeMap::new(),
            headers: finalize_headers,
            body: Vec::new(),
        };
        match route_request(&finalize, &shared, &limits).expect("sr finalize") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"FINALIZED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let mut commit_headers = BTreeMap::new();
        commit_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        commit_headers.insert("x-sr-role".to_string(), "writer".to_string());
        commit_headers.insert("x-idempotency-key".to_string(), "sr-commit-1".to_string());
        let commit = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/commit".to_string(),
            query: BTreeMap::new(),
            headers: commit_headers,
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("sr commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
                assert!(body.contains("\"version\":1"));
                assert!(body.contains("\"idempotency_replay\":false"));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("sr get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"lifecycle_status\":\"COMMITTED\""));
            }
        }

        let history = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents/1.2.3.4.5/history".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&history, &shared, &limits).expect("sr history") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"action\":\"create\""));
                assert!(body.contains("\"action\":\"review\""));
                assert!(body.contains("\"action\":\"finalize\""));
                assert!(body.contains("\"action\":\"commit\""));
            }
        }

        let mut create_second_headers = BTreeMap::new();
        create_second_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        create_second_headers.insert("x-sr-role".to_string(), "writer".to_string());
        create_second_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-lifecycle-create-2".to_string(),
        );
        let second_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: create_second_headers,
            body: b"study_instance_uid=2.3.4&series_instance_uid=2.3.4.6&sop_instance_uid=2.3.4.6.7&observer=alice&authored_epoch_ms=50&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=draft".to_vec(),
        };
        match route_request(&second_create, &shared, &limits).expect("sr create second") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let mut invalid_headers = BTreeMap::new();
        invalid_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        invalid_headers.insert("x-sr-role".to_string(), "writer".to_string());
        invalid_headers.insert("x-idempotency-key".to_string(), "sr-invalid-1".to_string());
        let invalid_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/2.3.4.6.7/commit".to_string(),
            query: BTreeMap::new(),
            headers: invalid_headers,
            body: Vec::new(),
        };
        let err = match route_request(&invalid_transition, &shared, &limits) {
            Ok(_) => panic!("invalid lifecycle transition must fail"),
            Err(err) => err,
        };
        assert_eq!(err.code, "DVF.WORKFLOW.SR.INVALID_TRANSITION");
        assert_eq!(status_for_error(&err).0, 409);

        let restarted =
            build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        match route_request(&history, &restarted, &limits).expect("history after restart") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"action\":\"commit\""));
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_route_shims_accept_legacy_paths_with_normalized_ids() {
        let worklist_path = temp_file_path("id_compat_worklist", "snapshot");
        let mpps_path = temp_file_path("id_compat_mpps", "snapshot");
        let sr_path = temp_file_path("id_compat_sr", "snapshot");
        let sr_audit_path = temp_file_path("id_compat_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-COMPAT&requested_procedure_id=RP-COMPAT&worker=compat"
                .to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("task create request") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body.find(prefix).expect("task id field") + prefix.len();
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                let raw_id = &remainder[..end];
                assert!(!raw_id.trim().is_empty());
                raw_id.to_string()
            }
            _ => panic!("unexpected create response"),
        };
        assert!(!task_id.is_empty());
        let normalized_task_id = task_id.trim();

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("legacy task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&normalized_task_id));
            }
            _ => panic!("unexpected list response"),
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("legacy task get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&format!("\"task_id\":\"{normalized_task_id}\"")));
            }
            _ => panic!("unexpected get response"),
        }

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "compat-worker".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/tasks/{task_id}/start/"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("legacy task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
            _ => panic!("unexpected start response"),
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/tasks/{task_id}/complete/"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("legacy task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
            _ => panic!("unexpected complete response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_supports_filter_sort_and_pagination() {
        // REQ-HI-201, REQ-WF-300
        let worklist_path = temp_file_path("workflow_list_worklist", "snapshot");
        let mpps_path = temp_file_path("workflow_list_mpps", "snapshot");
        let sr_path = temp_file_path("workflow_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("workflow_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut posts = vec![
            (
                "scheduled_step_id=STEP1&modality=CT&start_date=20260211&start_time=101010&patient_id=P1",
                "accession_number=ACC1",
            ),
            (
                "scheduled_step_id=STEP2&modality=MR&start_date=20260210&start_time=095959&patient_id=P2",
                "accession_number=ACC2",
            ),
            (
                "scheduled_step_id=STEP3&modality=CT&start_date=20260212&start_time=111111&patient_id=P3",
                "accession_number=ACC3",
            ),
        ];
        for post in posts.drain(..) {
            let body = format!("{}&{}", post.0, post.1);
            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: body.into_bytes(),
            };
            match route_request(&worklist_post, &shared, &limits).expect("insert worklist") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=1&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP3\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP1\""));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=2&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP1\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP3\""));
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&sort=scheduled_step_id&order=desc&page=2&page_size=2",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_deterministic_defaults_apply_sort_and_pagination() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_default_sort_pagination_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_default_sort_pagination_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_default_sort_pagination_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_default_sort_pagination_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        for i in (0..51).rev() {
            let scheduled_step_id = format!("PS-{i:03}");
            let start_time = format!("{:06}", 100_000 + i);
            let body = format!(
                "scheduled_step_id={scheduled_step_id}&modality=CT&start_date=20260210&start_time={start_time}&patient_id=PAT-{i}&accession_number=ACC-{i}"
            );
            let worklist_post = HttpRequest {
                method: "POST".to_string(),
                path: "/worklist/items".to_string(),
                query: BTreeMap::new(),
                headers: BTreeMap::new(),
                body: body.into_bytes(),
            };
            match route_request(&worklist_post, &shared, &limits).expect("insert worklist") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("inserted"));
                }
            }
        }

        let worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let worklist_response =
            route_request(&worklist_query, &shared, &limits).expect("query worklist");
        match worklist_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body.matches("\"scheduled_step_id\":\"").count(), 50);
                let first = body
                    .find("\"scheduled_step_id\":\"PS-000\"")
                    .expect("first row");
                let second = body
                    .find("\"scheduled_step_id\":\"PS-001\"")
                    .expect("second row");
                let third = body
                    .find("\"scheduled_step_id\":\"PS-002\"")
                    .expect("third row");
                assert!(first < second);
                assert!(second < third);
                assert!(body.contains("\"scheduled_step_id\":\"PS-049\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS-050\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_language_expression_filters_and_sorts() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let ct_item = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-CT&modality=CT&start_date=20260211&start_time=101010&patient_id=PAT-1&accession_number=ACC-1".to_vec(),
        };
        let mr_item = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=STEP-MR&modality=MR&start_date=20260211&start_time=101020&patient_id=PAT-2&accession_number=ACC-2".to_vec(),
        };
        assert!(matches!(
            route_request(&ct_item, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));
        assert!(matches!(
            route_request(&mr_item, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "query=modality%3DCT%26PatientID%3DPAT-1%26sort%3Dscheduled_step_id%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("query worklist");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"STEP-CT\""));
                assert!(!body.contains("\"scheduled_step_id\":\"STEP-MR\""));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_supports_status_filter_sort_and_pagination() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_list_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_list_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let in_progress = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=101010".to_vec(),
        };
        let in_progress_next = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.5&status=IN+PROGRESS&performed_step_id=PS2&start_date=20260211&start_time=102010".to_vec(),
        };
        let _ = route_request(&in_progress, &shared, &limits).expect("mpps in progress");
        let _ = route_request(&in_progress_next, &shared, &limits).expect("mpps in progress");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map(
                "status=IN+PROGRESS&sort=sop_instance_uid&page_size=1&page=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("mpps list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
                assert!(!body.contains("1.2.3.5"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_language_expression_filters_and_sorts() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let pending = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-IN_PROGRESS&start_date=20260211&start_time=101010".to_vec(),
        };
        let completed = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.5&status=COMPLETED&performed_step_id=PS-COMPLETED&start_date=20260211&start_time=102010&end_date=20260211&end_time=102010".to_vec(),
        };
        let _ = route_request(&pending, &shared, &limits).expect("mpps in progress");
        let _ = route_request(&completed, &shared, &limits).expect("mpps completed");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map(
                "query=status%3DCOMPLETED%26performed_step_id%3DPS-COMPLETED%26sort%3Dsop_instance_uid%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("query mpps");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.5"));
                assert!(!body.contains("1.2.3.4"));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_query_language_rejects_malformed_nested_query_expression() {
        // REQ-WF-305
        let worklist_path = temp_file_path("mpps_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let in_progress = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: parse_query_map("query=status%3DCOMPLETED%26badpair", &limits)
                .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&in_progress, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_status_filter_aliases_and_reproducible_defaults() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_status_filter_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_status_filter_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_status_filter_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_status_filter_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS1&modality=CT&start_date=20260211&start_time=101010&patient_id=P1&accession_number=ACC1".to_vec(),
        };
        assert!(matches!(
            route_request(&worklist_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let worklist_post = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS2&modality=MR&start_date=20260211&start_time=101020&patient_id=P2&accession_number=ACC2".to_vec(),
        };
        assert!(matches!(
            route_request(&worklist_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let mpps_post = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=COMPLETED&performed_step_id=PS1&start_date=20260211&start_time=101010&end_date=20260211&end_time=101100".to_vec(),
        };
        assert!(matches!(
            route_request(&mpps_post, &shared, &limits),
            Ok(WorkflowResponse::Json(200, body)) if body.contains("inserted")
        ));

        let completed_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=CT&status=COMPLETED&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let completed_response =
            route_request(&completed_query, &shared, &limits).expect("query completed");
        match completed_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS2\""));
            }
        }

        let scheduled_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=MR&status=SCHEDULED&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let scheduled_response =
            route_request(&scheduled_query, &shared, &limits).expect("query scheduled");
        match scheduled_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS2\""));
                assert!(!body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let scheduled_alias_response = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map(
                "modality=MR&status=pending&sort=scheduled_step_id&order=asc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let scheduled_alias_response = route_request(&scheduled_alias_response, &shared, &limits)
            .expect("query scheduled alias");
        match scheduled_alias_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scheduled_step_id\":\"PS2\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_rejects_invalid_sort() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_sort_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_sort_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_sort_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_sort_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let invalid_sort = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("sort=urgency&order=asc", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let invalid_sort_err = match route_request(&invalid_sort, &shared, &limits) {
            Ok(_) => panic!("invalid sort must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&invalid_sort_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_rejects_invalid_status_filter() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_status_filter_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_status_filter_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_status_filter_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_status_filter_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let invalid_status = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("status=unknown", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let invalid_status_err = match route_request(&invalid_status, &shared, &limits) {
            Ok(_) => panic!("invalid status filter must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&invalid_status_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn worklist_query_validation_enforces_pagination_limits() {
        // REQ-WF-300
        let worklist_path = temp_file_path("worklist_invalid_pagination_worklist", "snapshot");
        let mpps_path = temp_file_path("worklist_invalid_pagination_mpps", "snapshot");
        let sr_path = temp_file_path("worklist_invalid_pagination_sr", "snapshot");
        let sr_audit_path = temp_file_path("worklist_invalid_pagination_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let page_zero = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page=0", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_zero_err = match route_request(&page_zero, &shared, &limits) {
            Ok(_) => panic!("page=0 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_zero_err).0, 400);

        let page_size_zero = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page_size=0", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_size_zero_err = match route_request(&page_size_zero, &shared, &limits) {
            Ok(_) => panic!("page_size=0 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_size_zero_err).0, 400);

        let page_size_over_limit = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: parse_query_map("page_size=501", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let page_size_over_limit_err = match route_request(&page_size_over_limit, &shared, &limits)
        {
            Ok(_) => panic!("page_size>500 must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&page_size_over_limit_err).0, 413);
        match page_size_over_limit_err.kind {
            ErrorKind::LimitExceeded { limit_name, .. } => {
                assert_eq!(limit_name, "page_size");
            }
            _ => panic!("expected limit_exceeded for page_size=501"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn mpps_status_endpoint_enforces_transition_rules_and_idempotency() {
        // REQ-WF-305 + REQ-WF-306
        let worklist_path = temp_file_path("mpps_status_endpoints_worklist", "snapshot");
        let mpps_path = temp_file_path("mpps_status_endpoints_mpps", "snapshot");
        let sr_path = temp_file_path("mpps_status_endpoints_sr", "snapshot");
        let sr_audit_path = temp_file_path("mpps_status_endpoints_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS1&start_date=20260211&start_time=101010".to_vec(),
        };
        let create_response = route_request(&create, &shared, &limits).expect("create mpps");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let get_status = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_response = route_request(&get_status, &shared, &limits).expect("read mpps status");
        match get_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
                assert!(body.contains("\"performed_step_id\":\"PS1\""));
            }
        }

        let mut headers = BTreeMap::new();
        headers.insert(
            "x-idempotency-key".to_string(),
            "mpps-status-update-1".to_string(),
        );
        let complete = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: b"status=COMPLETED&end_date=20260211&end_time=101212".to_vec(),
        };
        let complete_response = route_request(&complete, &shared, &limits).expect("complete mpps");
        match complete_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }

        let complete_replay = route_request(&complete, &shared, &limits).expect("replay complete");
        match complete_replay {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }

        let terminal_transition_setup = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=2.3.4.5&status=IN+PROGRESS&performed_step_id=PS2&start_date=20260211&start_time=111111".to_vec(),
        };
        let terminal_transition_setup_response =
            route_request(&terminal_transition_setup, &shared, &limits)
                .expect("create terminal transition mpps");
        match terminal_transition_setup_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }
        let terminal_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/2.3.4.5/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=COMPLETED&end_date=20260211&end_time=131313".to_vec(),
        };
        let terminal_transition_response =
            route_request(&terminal_transition, &shared, &limits).expect("terminal transition");
        match terminal_transition_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"updated\""));
            }
        }
        let invalid_terminal_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/2.3.4.5/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=IN+PROGRESS".to_vec(),
        };
        let invalid_terminal_transition =
            match route_request(&invalid_terminal_transition, &shared, &limits) {
                Ok(_) => panic!("terminal status transition must fail"),
                Err(err) => err,
            };
        assert_eq!(invalid_terminal_transition.code, "DVF.INTEGRITY.ERROR");
        assert_eq!(status_for_error(&invalid_terminal_transition).0, 409);

        let invalid_transition = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers,
            body: b"status=DISCONTINUED".to_vec(),
        };
        let invalid = match route_request(&invalid_transition, &shared, &limits) {
            Ok(_) => panic!("invalid transition must fail"),
            Err(err) => err,
        };
        assert_eq!(invalid.code, "DVF.WORKFLOW.MPPS.IDEMPOTENCY_CONFLICT");
        assert_eq!(status_for_error(&invalid).0, 409);

        let get_completed = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let get_completed_response =
            route_request(&get_completed, &shared, &limits).expect("read mpps completed");
        match get_completed_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_tenant_context_blocks_cross_tenant_leakage() {
        // REQ-WF-401, REQ-WF-402
        let worklist_path = temp_file_path("tenant_scope_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_scope_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_scope_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_scope_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut tenant_a_headers = BTreeMap::new();
        tenant_a_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        tenant_a_headers.insert("x-sr-role".to_string(), "writer".to_string());
        tenant_a_headers.insert("x-workflow-tenant".to_string(), "tenant-a".to_string());

        let mut tenant_b_headers = BTreeMap::new();
        tenant_b_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        tenant_b_headers.insert("x-sr-role".to_string(), "writer".to_string());
        tenant_b_headers.insert("x-workflow-tenant-id".to_string(), "tenant-b".to_string());

        let worklist_create = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: b"scheduled_step_id=STEP-TENANT-A&modality=CT&start_date=20260211&start_time=101010&patient_id=PA".to_vec(),
        };
        match route_request(&worklist_create, &shared, &limits).expect("create tenant-a worklist") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let worklist_cross_tenant_upsert = HttpRequest {
            method: "POST".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: b"scheduled_step_id=STEP-TENANT-A&modality=MR&start_date=20260211&start_time=102020&patient_id=PB".to_vec(),
        };
        let denied = match route_request(&worklist_cross_tenant_upsert, &shared, &limits) {
            Ok(_) => panic!("cross-tenant worklist upsert must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied.code, "DVF.WORKFLOW.SR.AUTH_DENIED");

        let tenant_a_worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: Vec::new(),
        };
        match route_request(&tenant_a_worklist_query, &shared, &limits)
            .expect("tenant-a worklist query")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("STEP-TENANT-A"));
            }
        }

        let tenant_b_worklist_query = HttpRequest {
            method: "GET".to_string(),
            path: "/worklist/items".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: Vec::new(),
        };
        match route_request(&tenant_b_worklist_query, &shared, &limits)
            .expect("tenant-b worklist query")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
        }

        let mpps_create = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: tenant_a_headers.clone(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-A&start_date=20260211&start_time=101010".to_vec(),
        };
        match route_request(&mpps_create, &shared, &limits).expect("create tenant-a mpps") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("inserted"));
            }
        }

        let mpps_cross_tenant_update = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: b"sop_instance_uid=1.2.3.4&status=COMPLETED&performed_step_id=PS-B&start_date=20260211&start_time=101020&end_date=20260211&end_time=101121".to_vec(),
        };
        let denied_mpps = match route_request(&mpps_cross_tenant_update, &shared, &limits) {
            Ok(_) => panic!("cross-tenant mpps update must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied_mpps.code, "DVF.WORKFLOW.SR.AUTH_DENIED");

        let mpps_cross_tenant_get = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates/1.2.3.4".to_string(),
            query: BTreeMap::new(),
            headers: tenant_b_headers.clone(),
            body: Vec::new(),
        };
        let denied_mpps_read = match route_request(&mpps_cross_tenant_get, &shared, &limits) {
            Ok(_) => panic!("cross-tenant mpps read must be denied"),
            Err(err) => err,
        };
        assert_eq!(denied_mpps_read.code, "DVF.WORKFLOW.SR.AUTH_DENIED");

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_http_flow_creates_lists_gets_and_controls_lifecycle() {
        // REQ-WF-400: task lifecycle endpoints must support planned procedure execution flow control.
        let worklist_path = temp_file_path("task_flow_worklist", "snapshot");
        let mpps_path = temp_file_path("task_flow_mpps", "snapshot");
        let sr_path = temp_file_path("task_flow_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_flow_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-idempotency-key".to_string(), "task-create-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers.clone(),
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-100&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("task create request") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map("status=SCHEDULED&sort=task_id", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id));
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/workflow/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&get, &shared, &limits).expect("task get") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&format!("\"task_id\":\"{task_id}\"")));
            }
        }

        let mut pause_reject_headers = BTreeMap::new();
        pause_reject_headers.insert("x-task-worker".to_string(), "nurse".to_string());
        let pause = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/pause"),
            query: BTreeMap::new(),
            headers: pause_reject_headers,
            body: Vec::new(),
        };
        let pause_err = match route_request(&pause, &shared, &limits) {
            Ok(_) => panic!("pause before start must fail"),
            Err(err) => err,
        };
        assert_eq!(pause_err.code, "DVF.WORKFLOW.TASK.INVALID_TRANSITION");

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/complete"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        let commit_without_review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let commit_without_review_err =
            match route_request(&commit_without_review, &shared, &limits) {
                Ok(_) => panic!("commit must follow review"),
                Err(err) => err,
            };
        assert_eq!(
            commit_without_review_err.code,
            "DVF.WORKFLOW.TASK.INVALID_TRANSITION"
        );

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-task-worker".to_string(), "quality-reviewer".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/review"),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("task review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
            }
        }

        let commit = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("task commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_http_flow_supports_ups_aliases() {
        // REQ-WF-400: /ups aliases must provide the same task lifecycle behavior as /workflow/tasks.
        let worklist_path = temp_file_path("ups_alias_task_worklist", "snapshot");
        let mpps_path = temp_file_path("ups_alias_task_mpps", "snapshot");
        let sr_path = temp_file_path("ups_alias_task_sr", "snapshot");
        let sr_audit_path = temp_file_path("ups_alias_task_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let mut create_headers = BTreeMap::new();
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "ups-task-create-1".to_string(),
        );
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/ups".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-101&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create, &shared, &limits).expect("ups task create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/ups".to_string(),
            query: parse_query_map("status=SCHEDULED&sort=task_id", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("ups task list") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id));
                assert!(body.contains("\"scheduled_step_id\":\"PS1\""));
            }
        }

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        match route_request(&start, &shared, &limits).expect("ups task start") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let complete = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/complete"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&complete, &shared, &limits).expect("ups task complete") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMPLETED\""));
            }
        }

        let mut review_headers = BTreeMap::new();
        review_headers.insert("x-task-worker".to_string(), "quality-reviewer".to_string());
        let review = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/review"),
            query: BTreeMap::new(),
            headers: review_headers,
            body: Vec::new(),
        };
        match route_request(&review, &shared, &limits).expect("ups task review") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"REVIEWED\""));
            }
        }

        let commit = HttpRequest {
            method: "POST".to_string(),
            path: format!("/ups/{task_id}/commit"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&commit, &shared, &limits).expect("ups task commit") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"COMMITTED\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_query_language_expression_filters_and_sorts() {
        // REQ-WF-400
        let worklist_path = temp_file_path("task_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("task_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("task_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS1&requested_procedure_id=RP-100&worker=planner".to_vec(),
        };
        let task_id1 = match route_request(&create, &shared, &limits).expect("task create") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let prefix = "\"task_id\":\"";
                let start = body
                    .find(prefix)
                    .expect("task id field")
                    .saturating_add(prefix.len());
                let remainder = &body[start..];
                let end = remainder.find('"').expect("task id close");
                remainder[..end].to_string()
            }
            _ => panic!("unexpected task create response"),
        };
        let create_in_progress = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"scheduled_step_id=PS2&requested_procedure_id=RP-101&worker=planner".to_vec(),
        };
        let task_id2 =
            match route_request(&create_in_progress, &shared, &limits).expect("task create") {
                WorkflowResponse::Json(code, body) => {
                    assert_eq!(code, 200);
                    assert!(body.contains("\"outcome\":\"inserted\""));
                    let prefix = "\"task_id\":\"";
                    let start = body
                        .find(prefix)
                        .expect("task id field")
                        .saturating_add(prefix.len());
                    let remainder = &body[start..];
                    let end = remainder.find('"').expect("task id close");
                    remainder[..end].to_string()
                }
                _ => panic!("unexpected task create response"),
            };
        assert_ne!(task_id1, task_id2);

        let mut start_headers = BTreeMap::new();
        start_headers.insert("x-task-worker".to_string(), "operator-a".to_string());
        let start = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id2}/start"),
            query: BTreeMap::new(),
            headers: start_headers,
            body: Vec::new(),
        };
        let _ = route_request(&start, &shared, &limits).expect("task start");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map(
                "query=scheduled_step_id%3DPS2%26status%3DIN+PROGRESS%26sort%3Dtask_id%26order%3Dasc",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        match route_request(&list, &shared, &limits).expect("task list query language") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains(&task_id2));
                assert!(!body.contains(&task_id1));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_task_query_language_rejects_malformed_nested_query_expression() {
        // REQ-WF-400
        let worklist_path = temp_file_path("task_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("task_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("task_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("task_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks".to_string(),
            query: parse_query_map("query=scheduled_step_id%3DPS2%26badpair", &limits)
                .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&list, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_supports_observer_filter_sort_and_pagination() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_list_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_list_mpps", "snapshot");
        let sr_path = temp_file_path("sr_list_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_list_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut alice_headers = BTreeMap::new();
        alice_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        alice_headers.insert("x-sr-role".to_string(), "writer".to_string());
        alice_headers.insert("x-idempotency-key".to_string(), "sr-list-1".to_string());
        let alice_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: alice_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1000&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=alice".to_vec(),
        };
        let _ = route_request(&alice_create, &shared, &limits).expect("create alice");

        let mut bob_headers = BTreeMap::new();
        bob_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        bob_headers.insert("x-sr-role".to_string(), "writer".to_string());
        bob_headers.insert("x-idempotency-key".to_string(), "sr-list-2".to_string());
        let bob_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: bob_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.6&observer=bob&authored_epoch_ms=1001&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob".to_vec(),
        };
        let _ = route_request(&bob_create, &shared, &limits).expect("create bob");

        let mut bob_update_headers = bob_headers;
        bob_update_headers.insert("x-idempotency-key".to_string(), "sr-list-3".to_string());
        let bob_update = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.6/updates".to_string(),
            query: BTreeMap::new(),
            headers: bob_update_headers,
            body: b"expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob-v2".to_vec(),
        };
        let _ = route_request(&bob_update, &shared, &limits).expect("update bob");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map(
                "observer=bob&sort=version&order=desc&page=1&page_size=1",
                &limits.clone(),
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("sr list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"observer\":\"bob\""));
                assert!(body.contains("\"version\":2"));
                assert!(!body.contains("\"observer\":\"alice\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_language_expression_filters_and_sorts() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_query_language_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_query_language_mpps", "snapshot");
        let sr_path = temp_file_path("sr_query_language_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_query_language_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut alice_headers = BTreeMap::new();
        alice_headers.insert("x-sr-principal".to_string(), "alice".to_string());
        alice_headers.insert("x-sr-role".to_string(), "writer".to_string());
        alice_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-language-alice".to_string(),
        );
        let alice_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: alice_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1000&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=alice".to_vec(),
        };
        let _ = route_request(&alice_create, &shared, &limits).expect("create alice");

        let mut bob_headers = BTreeMap::new();
        bob_headers.insert("x-sr-principal".to_string(), "bob".to_string());
        bob_headers.insert("x-sr-role".to_string(), "writer".to_string());
        bob_headers.insert(
            "x-idempotency-key".to_string(),
            "sr-language-bob".to_string(),
        );
        let bob_create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: bob_headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.6&observer=bob&authored_epoch_ms=1001&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=bob".to_vec(),
        };
        let _ = route_request(&bob_create, &shared, &limits).expect("create bob");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map(
                "query=observer%3Dbob%26sort%3Dversion%26order%3Ddesc&page=1&page_size=1",
                &limits,
            )
            .expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("sr list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"observer\":\"bob\""));
                assert!(!body.contains("\"observer\":\"alice\""));
            }
            _ => panic!("unexpected response"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_query_language_rejects_malformed_nested_query_expression() {
        // REQ-SR-332
        let worklist_path = temp_file_path("sr_query_language_invalid_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_query_language_invalid_mpps", "snapshot");
        let sr_path = temp_file_path("sr_query_language_invalid_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_query_language_invalid_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("open worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("open mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("open sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: parse_query_map("query=observer%3Dbob%26badpair", &limits).expect("parse query"),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let err = match route_request(&list, &shared, &limits) {
            Ok(_) => panic!("malformed nested query must fail"),
            Err(err) => err,
        };
        assert_eq!(status_for_error(&err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn sr_write_fails_closed_without_write_role() {
        // REQ-HI-168, REQ-AUTH-300
        let worklist_path = temp_file_path("sr_auth_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_auth_mpps", "snapshot");
        let sr_path = temp_file_path("sr_auth_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_auth_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let state = RuntimeState {
            worklist: WorklistStore::open(limits.clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: limits.clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                limits.clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut headers = BTreeMap::new();
        headers.insert("x-sr-principal".to_string(), "alice".to_string());
        headers.insert("x-idempotency-key".to_string(), "auth-fail-1".to_string());
        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers,
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=42&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=unauthorized".to_vec(),
        };
        match route_request(&create, &shared, &limits) {
            Ok(_) => panic!("must fail"),
            Err(err) => assert_eq!(err.code, "DVF.WORKFLOW.SR.AUTH_DENIED"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn diagnostics_redact_uid_path_and_host_tokens() {
        // REQ-HI-188, REQ-HI-195, REQ-HI-245
        let raw = "uid=1.2.840.10008 path=/var/state/workflow.snapshot peer=workflow.local:8082";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("1.2.840.10008"));
        assert!(!redacted.contains("/var/state/workflow.snapshot"));
        assert!(!redacted.contains("workflow.local:8082"));
        assert!(redacted.contains("[REDACTED_UID]"));
        assert!(redacted.contains("[REDACTED_PATH]"));
        assert!(redacted.contains("[REDACTED_HOST]"));
    }

    #[test]
    fn diagnostics_redact_phi_pii_keyed_and_email_tokens() {
        let raw = "patient_id=PX-7788 patient_name=Bob email=bob@example.org";
        let redacted = redact_diagnostic_message(raw);
        assert!(!redacted.contains("PX-7788"));
        assert!(!redacted.contains("Bob"));
        assert!(!redacted.contains("bob@example.org"));
        assert!(redacted.contains("patient_id=[REDACTED_PII]"));
        assert!(redacted.contains("patient_name=[REDACTED_PII]"));
        assert!(redacted.contains("email=[REDACTED_PII]"));
    }

    #[test]
    fn hl7_adt_creates_task_and_orm_updates_mpps_when_supplied() {
        let worklist_path = temp_file_path("interop_hl7_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_hl7_mpps", "snapshot");
        let sr_path = temp_file_path("interop_hl7_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_hl7_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let adt = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&scheduled_step_id=STEP-101&task_id=TASK-101&requested_procedure_id=REQ-77&status=SCHEDULED&worker=agent"
                .to_vec(),
        };
        let adt_response = route_request(&adt, &shared, &limits).expect("interop adt");
        match adt_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"outcome\":\"inserted\""));
            }
        }

        let orm = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORM&scheduled_step_id=STEP-101&task_id=TASK-101&status=IN_PROGRESS&sop_instance_uid=1.2.3.4&performed_step_id=PS-1&start_date=20260222&start_time=101010&mpps_status=IN_PROGRESS"
                .to_vec(),
        };
        let orm_response = route_request(&orm, &shared, &limits).expect("interop orm");
        match orm_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORM\""));
                assert!(body.contains("\"task_status\":\"IN PROGRESS\""));
            }
        }

        let get_task = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/tasks/TASK-101".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let task_response = route_request(&get_task, &shared, &limits).expect("get task");
        match task_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"task_id\":\"TASK-101\""));
            }
        }

        let list_mpps = HttpRequest {
            method: "GET".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let mpps_response = route_request(&list_mpps, &shared, &limits).expect("mpps list");
        match mpps_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("1.2.3.4"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_oru_updates_sr_and_subscriptions_and_failures() {
        let worklist_path = temp_file_path("interop_oru_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_oru_mpps", "snapshot");
        let sr_path = temp_file_path("interop_oru_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_oru_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        std::env::set_var(
            "DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS",
            "http://connector.example/interop",
        );

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&event_filter=oru&sink_kind=webhook&sink_target=https%3A%2F%2Finterop.example%2Fevents"
                .to_vec(),
        };
        let create_sub_response =
            route_request(&create_sub, &shared, &limits).expect("create subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
            }
        }

        let create_custom_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&event_filter=oru&sink_kind=custom&sink_connector=enterprise_his&sink_target=https%3A%2F%2Fconnector.example%2Forg%2Foru"
                .to_vec(),
        };
        let create_custom_sub_response = route_request(&create_custom_sub, &shared, &limits)
            .expect("create custom subscription");
        match create_custom_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"sink_kind\":\"custom\""));
            }
        }

        let list_sub_before = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_before =
            route_request(&list_sub_before, &shared, &limits).expect("list subscriptions");
        match list_before {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
            }
        }

        let oru = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORU&study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&report_text=report+from+his&authored_epoch_ms=170000&known_refs=1.2.3.4.5"
                .to_vec(),
        };
        let oru_response = route_request(&oru, &shared, &limits).expect("interop oru");
        match oru_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORU\""));
                assert!(body.contains("\"outcome\":\"created\""));
            }
        }

        let list_sub_after =
            route_request(&list_sub_before, &shared, &limits).expect("list subscriptions");
        match list_sub_after {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":1"));
            }
        }

        let bad_adt = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-adt-bad-1".to_string())]),
            body: b"source=his&message_type=ADT&status=SCHEDULED".to_vec(),
        };
        assert!(route_request(&bad_adt, &shared, &limits).is_err());

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
                assert!(body.contains("\"correlation_id\":\"hlt-adt-bad-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
        }

        std::env::remove_var("DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS");
        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_malformed_payload_is_rejected_with_decode_error() {
        let worklist_path = temp_file_path("interop_malformed_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_malformed_mpps", "snapshot");
        let sr_path = temp_file_path("interop_malformed_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_malformed_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let malformed = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-malformed-1".to_string())]),
            body: b"source=his&message_type=ADT&task=TASK-MALFORMED".to_vec(),
        };
        let err = route_request(&malformed, &shared, &limits)
            .expect_err("malformed hl7 payload should fail");
        assert_eq!(err.code, "DVF.HTTP.DECODE");
        assert!(err
            .message
            .contains("missing required identifier: scheduled_step_id"));

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"his\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"scope\":\"ingest\""));
                assert!(body.contains("\"correlation_id\":\"hlt-malformed-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_interop_failure_mapping_preserves_correlation_across_audit_and_dlq() {
        let worklist_path = temp_file_path("interop_e2e_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_e2e_mpps", "snapshot");
        let sr_path = temp_file_path("interop_e2e_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_e2e_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let correlation_id = "e2e-corr-bridge-1";
        let writer_headers = BTreeMap::from([
            ("x-sr-principal".to_string(), "interop-e2e".to_string()),
            ("x-sr-role".to_string(), "writer".to_string()),
            (
                "x-idempotency-key".to_string(),
                "interop-e2e-task-1".to_string(),
            ),
            ("x-request-id".to_string(), correlation_id.to_string()),
        ]);

        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: writer_headers,
            body: b"scheduled_step_id=STEP-E2E&requested_procedure_id=RP-E2E".to_vec(),
        };
        let _ = route_request(&create_task, &shared, &limits).expect("seed workflow task");

        let bad_hl7 = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-request-id".to_string(), correlation_id.to_string()),
            ]),
            body: b"source=his&message_type=ADT&status=SCHEDULED".to_vec(),
        };
        let err = route_request(&bad_hl7, &shared, &limits)
            .expect_err("interop failure should be captured in DLQ");
        assert_eq!(err.code, "DVF.HTTP.DECODE");

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let failures_response =
            route_request(&failures, &shared, &limits).expect("list interop failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scope\":\"ingest\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
                assert!(body.contains("\"correlation_id\":\"e2e-corr-bridge-1\""));
            }
        }

        let audit = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/audit".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-e2e".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let audit_response = route_request(&audit, &shared, &limits).expect("workflow audit");
        match audit_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"route\":\"/interop/hl7\""));
                assert!(body.contains(&hash_text(correlation_id)));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_status_dashboard_reports_connector_and_failure_summary() {
        let worklist_path = temp_file_path("interop_connectors_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_connectors_mpps", "snapshot");
        let sr_path = temp_file_path("interop_connectors_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_connectors_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        {
            let mut state = shared
                .lock()
                .expect("shared state lock for dashboard fixture");
            state.hl7.connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );

            let _ = state.hl7.subscriptions.insert(
                "sub-enterprise".to_string(),
                Hl7Subscription {
                    id: "sub-enterprise".to_string(),
                    source: "his".to_string(),
                    event_filter: vec!["adt".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("enterprise_his".to_string()),
                        target: "https://connectors.enterprise-his.example/interop".to_string(),
                    },
                    delivered_events: 7,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.subscriptions.insert(
                "sub-lab".to_string(),
                Hl7Subscription {
                    id: "sub-lab".to_string(),
                    source: "*".to_string(),
                    event_filter: vec!["oru".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("lab".to_string()),
                        target: "https://connectors.lab.example/interop".to_string(),
                    },
                    delivered_events: 4,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.subscriptions.insert(
                "sub-webhook".to_string(),
                Hl7Subscription {
                    id: "sub-webhook".to_string(),
                    source: "lab".to_string(),
                    event_filter: vec!["orf".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Webhook,
                        target: "https://notify.example/events".to_string(),
                    },
                    delivered_events: 11,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );
            let _ = state.hl7.subscriptions.insert(
                "sub-bus".to_string(),
                Hl7Subscription {
                    id: "sub-bus".to_string(),
                    source: "ris".to_string(),
                    event_filter: vec!["mpps".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::MessageBus,
                        target: "bus://workflow/connectivity".to_string(),
                    },
                    delivered_events: 2,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: now_epoch_millis(),
                },
            );

            state.hl7.failures.push_back(Hl7FailureRecord {
                id: "failure-ingest-1".to_string(),
                source: "his".to_string(),
                message_type: "ADT".to_string(),
                reason: "validation failure".to_string(),
                payload_excerpt: "source=his&message_type=ADT".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "ingest".to_string(),
                subscription_id: String::new(),
                event_id: "evt-ingest-1".to_string(),
                correlation_id: "corr-ingest-1".to_string(),
                sequence: 1,
                attempt: 3,
                max_attempts: 5,
            });
            state.hl7.failures.push_back(Hl7FailureRecord {
                id: "failure-callback-1".to_string(),
                source: "his".to_string(),
                message_type: "ORU".to_string(),
                reason: "callback timeout".to_string(),
                payload_excerpt: "callback timeout".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "callback".to_string(),
                subscription_id: "sub-enterprise".to_string(),
                event_id: "evt-callback-1".to_string(),
                correlation_id: "corr-callback-1".to_string(),
                sequence: 2,
                attempt: 3,
                max_attempts: 5,
            });
        }

        let status = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-test".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&status, &shared, &limits).expect("connector status");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"subscription_count\":4"));
                assert!(body.contains("\"custom_connector_subscriptions\":2"));
                assert!(body.contains("\"webhook_subscriptions\":1"));
                assert!(body.contains("\"message_bus_subscriptions\":1"));
                assert!(body.contains("\"wildcard_subscriptions\":1"));
                assert!(body.contains("\"failures\":{\"total\":2,\"ingest\":1,\"callback\":1"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"delivered_events\":7"));
                assert!(body.contains("\"callback_failures\":1"));
                assert!(body.contains("\"generated_at_ms\":"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_status_payload_orders_aliases_deterministically() {
        let worklist_path = temp_file_path("interop_connectors_order_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_connectors_order_mpps", "snapshot");
        let sr_path = temp_file_path("interop_connectors_order_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_connectors_order_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector ordering fixture");
            state.hl7.connector_registry.insert(
                "zeta".to_string(),
                "https://connectors.example/zeta".to_string(),
            );
            state.hl7.connector_registry.insert(
                "alpha".to_string(),
                "https://connectors.example/alpha".to_string(),
            );
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-order".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };

        let first = route_request(&request, &shared, &limits).expect("connector status first");
        let second = route_request(&request, &shared, &limits).expect("connector status second");

        let (first_body, second_body) = match (first, second) {
            (WorkflowResponse::Json(200, first_body), WorkflowResponse::Json(200, second_body)) => {
                (first_body, second_body)
            }
            _ => panic!("unexpected response type"),
        };

        let first_alpha = first_body.find("\"alias\":\"alpha\"").expect("alpha alias");
        let first_zeta = first_body.find("\"alias\":\"zeta\"").expect("zeta alias");
        let second_alpha = second_body
            .find("\"alias\":\"alpha\"")
            .expect("alpha alias");
        let second_zeta = second_body.find("\"alias\":\"zeta\"").expect("zeta alias");
        assert!(first_alpha < first_zeta);
        assert!(second_alpha < second_zeta);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_features_endpoint_reports_feature_flag_and_rollout() {
        let worklist_path = temp_file_path("interop_features_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_features_mpps", "snapshot");
        let sr_path = temp_file_path("interop_features_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_features_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for features dashboard fixture");
            state.hl7.connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.connector_registry.insert(
                "corp*".to_string(),
                "https://connectors.example/corp/{*}".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("enterprise_his".to_string(), false);
            state
                .hl7
                .connector_rollout_percent
                .insert("enterprise_his".to_string(), 42);
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/features".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-test".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector features");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"feature_enabled\":false"));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"alias\":\"corp*\""));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_rollout_admin_api_updates_with_bounds_validation() {
        let worklist_path = temp_file_path("interop_rollout_admin_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_rollout_admin_mpps", "snapshot");
        let sr_path = temp_file_path("interop_rollout_admin_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_rollout_admin_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for rollout admin fixture");
            state.hl7.connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
        }

        let update = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&rollout_percent=42".to_vec(),
        };
        let update_response = route_request(&update, &shared, &limits).expect("rollout update");
        match update_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"updated\""));
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"percent\":42"));
            }
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("rollout list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"rollout_percent\":42"));
                assert!(body.contains("\"percent\":42"));
            }
        }

        let legacy_update = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&percent=64".to_vec(),
        };
        let legacy_response =
            route_request(&legacy_update, &shared, &limits).expect("legacy percent update");
        match legacy_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":64"));
                assert!(body.contains("\"percent\":64"));
            }
        }

        let invalid = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-rollout-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: b"alias=lab&rollout_percent=101".to_vec(),
        };
        let invalid_err =
            route_request(&invalid, &shared, &limits).expect_err("rollout >100 should fail");
        assert_eq!(status_for_error(&invalid_err).0, 400);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    #[test]
    fn workflow_admin_api_versioning_accepts_v1_alias_and_rejects_unknown_versions() {
        let worklist_path = temp_file_path("admin_api_version_worklist", "snapshot");
        let mpps_path = temp_file_path("admin_api_version_mpps", "snapshot");
        let sr_path = temp_file_path("admin_api_version_sr", "snapshot");
        let sr_audit_path = temp_file_path("admin_api_version_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for admin api versioning fixture");
            let _ = state.hl7.connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connector.example/interop".to_string(),
            );
        }

        let headers = BTreeMap::from([
            ("x-sr-principal".to_string(), "admin-api".to_string()),
            ("x-sr-role".to_string(), "admin".to_string()),
        ]);

        let no_version = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::new(),
            headers: headers.clone(),
            body: b"alias=enterprise_his&rollout_percent=40".to_vec(),
        };
        let default_response =
            route_request(&no_version, &shared, &limits).expect("default admin api version");
        match default_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":40"));
            }
        }

        let legacy_v1_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::from([("api_version".to_string(), "1".to_string())]),
            headers: headers.clone(),
            body: b"alias=enterprise_his&rollout_percent=55".to_vec(),
        };
        let legacy_response =
            route_request(&legacy_v1_alias, &shared, &limits).expect("legacy v1 alias support");
        match legacy_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"rollout_percent\":55"));
            }
        }

        let unsupported = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/connectors/rollout".to_string(),
            query: BTreeMap::from([("api_version".to_string(), "v2".to_string())]),
            headers,
            body: b"alias=enterprise_his&rollout_percent=60".to_vec(),
        };
        let err = route_request(&unsupported, &shared, &limits)
            .expect_err("unknown admin api version should fail closed");
        assert_eq!(err.code, "DVF.HTTP.DECODE");
        assert!(err.message.contains("unsupported admin api_version"));

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_capabilities_endpoint_reports_version_and_message_classes() {
        let worklist_path = temp_file_path("interop_capabilities_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_capabilities_mpps", "snapshot");
        let sr_path = temp_file_path("interop_capabilities_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_capabilities_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for capabilities fixture");
            state.hl7.connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            state.hl7.connector_registry.insert(
                "dimse_bridge".to_string(),
                "dimse://bridge.example/ingest".to_string(),
            );
            state.hl7.connector_plugins.insert(
                "enterprise_his".to_string(),
                ConnectorPluginMetadata {
                    plugin_path: "/opt/connectors/enterprise_his.wasm".to_string(),
                    adapter_version: "2.4.0".to_string(),
                    compatible_min: "2.0.0".to_string(),
                    compatible_max: "3.0.0".to_string(),
                },
            );
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/capabilities".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-capabilities".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector capabilities");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"connector_count\":2"));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"version\":\"2.4.0\""));
                assert!(body.contains("\"adapter_version\":\"2.4.0\""));
                assert!(body
                    .contains("\"supported_message_classes\":[\"ADT\",\"ORM\",\"ORU\",\"SIU\"]"));
                assert!(body.contains("\"alias\":\"dimse_bridge\""));
                assert!(body
                    .contains("\"supported_message_classes\":[\"C-FIND\",\"N-CREATE\",\"N-SET\"]"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_health_endpoint_reports_degraded_and_downstream_timeout_states() {
        let worklist_path = temp_file_path("interop_health_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_health_mpps", "snapshot");
        let sr_path = temp_file_path("interop_health_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_health_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector health fixture");
            state
                .hl7
                .connector_registry
                .insert("lab".to_string(), String::new());
            state.hl7.connector_registry.insert(
                "enterprise_his".to_string(),
                "https://connectors.enterprise-his.example/interop".to_string(),
            );
            let _ = state.hl7.subscriptions.insert(
                "sub-enterprise".to_string(),
                Hl7Subscription {
                    id: "sub-enterprise".to_string(),
                    source: "his".to_string(),
                    event_filter: vec!["oru".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("enterprise_his".to_string()),
                        target: "https://connectors.enterprise-his.example/interop".to_string(),
                    },
                    delivered_events: 0,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: 0,
                },
            );
            state.hl7.failures.push_back(Hl7FailureRecord {
                id: "failure-timeout-1".to_string(),
                source: "his".to_string(),
                message_type: "ORU".to_string(),
                reason: "callback timeout".to_string(),
                payload_excerpt: "timeout".to_string(),
                created_at_ms: now_epoch_millis(),
                scope: "callback".to_string(),
                subscription_id: "sub-enterprise".to_string(),
                event_id: "evt-timeout-1".to_string(),
                correlation_id: "corr-timeout-1".to_string(),
                sequence: 1,
                attempt: 3,
                max_attempts: 3,
            });
        }

        let request = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/connectors/health".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                (
                    "x-sr-principal".to_string(),
                    "interop-health-admin".to_string(),
                ),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let response = route_request(&request, &shared, &limits).expect("connector health");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"alias\":\"lab\""));
                assert!(body.contains("\"status\":\"degraded\""));
                assert!(body.contains("\"alias\":\"enterprise_his\""));
                assert!(body.contains("\"status\":\"downstream_timeout\""));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn fhir_ingest_returns_typed_contract_when_enabled() {
        let _guard = ENV_LOCK.lock().expect("env lock for fhir ingest");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=Observation&source_system=ehr-core&observation_code=LOINC-1234&subject_id=patient-1&observed_at=2026-02-24T10%3A00%3A00Z&content_sha256=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef&dry_run=true"
                .to_vec(),
        };
        let response = handle_fhir_ingest(&request, &Limits::default()).expect("typed fhir ingest");
        match response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 202);
                assert!(body.contains("\"mode\":\"typed\""));
                assert!(body.contains("\"resource_type\":\"Observation\""));
                assert!(body.contains("\"source_system\":\"ehr-core\""));
                assert!(body.contains("\"dry_run\":true"));
                assert!(body.contains("\"content_sha256_present\":true"));
            }
        }

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn fhir_ingest_rejects_unsupported_resource_type() {
        let _guard = ENV_LOCK.lock().expect("env lock for fhir ingest rejection");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=MedicationRequest".to_vec(),
        };
        let err = handle_fhir_ingest(&request, &Limits::default())
            .expect_err("unsupported fhir resource_type should fail");
        assert_eq!(status_for_error(&err).0, 400);
        assert!(err.message.contains("unsupported fhir resource_type"));

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn fhir_ingest_rejects_missing_resource_specific_required_fields() {
        let _guard = ENV_LOCK
            .lock()
            .expect("env lock for fhir required field validation");
        let previous = std::env::var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED").ok();
        std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", "true");

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/fhir".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"tenant=tenant-fhir&resource_type=Observation&source_system=ehr-core".to_vec(),
        };
        let err = handle_fhir_ingest(&request, &Limits::default())
            .expect_err("missing observation required fields must fail");
        assert_eq!(status_for_error(&err).0, 400);
        assert!(err.message.contains("missing required fhir field"));

        if let Some(value) = previous {
            std::env::set_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_FHIR_INGEST_ENABLED");
        }
    }

    #[test]
    fn hl7_connector_delivery_is_skipped_when_feature_disabled() {
        let worklist_path = temp_file_path("interop_feature_disabled_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_feature_disabled_mpps", "snapshot");
        let sr_path = temp_file_path("interop_feature_disabled_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_feature_disabled_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for feature gate disabled fixture");
            state.hl7.connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("lab".to_string(), false);
        }

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&event_filter=all&sink_kind=custom&sink_connector=lab&sink_target=https://callback.example/his".to_vec(),
        };
        let create_response =
            route_request(&create, &shared, &limits).expect("create subscription");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("sub-00001"));
            }
            _ => panic!("unexpected response type"),
        }

        let ingest = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&message_type=ADT&scheduled_step_id=STEP-100&patient_id=P-1&status=SCHEDULED".to_vec(),
        };
        let _ = route_request(&ingest, &shared, &limits).expect("deliverable event");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list subscriptions");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_connector_delivery_is_skipped_when_rollout_zero() {
        let worklist_path = temp_file_path("interop_rollout_zero_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_rollout_zero_mpps", "snapshot");
        let sr_path = temp_file_path("interop_rollout_zero_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_rollout_zero_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for feature gate rollout fixture");
            state.hl7.connector_registry.insert(
                "lab".to_string(),
                "https://connectors.lab.example/interop".to_string(),
            );
            state
                .hl7
                .connector_feature_flags
                .insert("lab".to_string(), true);
            state
                .hl7
                .connector_rollout_percent
                .insert("lab".to_string(), 0);
        }

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&event_filter=all&sink_kind=custom&sink_connector=lab&sink_target=https://callback.example/his".to_vec(),
        };
        let create_response =
            route_request(&create, &shared, &limits).expect("create subscription");
        match create_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("sub-00001"));
            }
            _ => panic!("unexpected response type"),
        }

        let ingest = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: b"source=lab&message_type=ADT&scheduled_step_id=STEP-200&patient_id=P-2&status=SCHEDULED".to_vec(),
        };
        let _ = route_request(&ingest, &shared, &limits).expect("deliverable event");

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "interop-feature".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
            ]),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list subscriptions");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_schema_evolution_aliases_are_accepted() {
        let worklist_path = temp_file_path("interop_alias_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_alias_mpps", "snapshot");
        let sr_path = temp_file_path("interop_alias_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_alias_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let adt_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&task=TASK-ALIAS&ScheduledStepID=STEP-ALIAS&status=IN_PROGRESS"
                .to_vec(),
        };
        let adt_response = route_request(&adt_alias, &shared, &limits).expect("interop adt alias");
        match adt_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"task_id\":\"TASK-ALIAS\""));
                assert!(body.contains("\"message_type\":\"ADT\""));
            }
            _ => panic!("unexpected response type"),
        }

        let oru_alias = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ORU&StudyInstanceUID=1.2.3&SeriesInstanceUID=1.2.3.4&SOPInstanceUID=1.2.3.4.5&text=report+from+legacy"
                .to_vec(),
        };
        let oru_response = route_request(&oru_alias, &shared, &limits).expect("interop oru alias");
        match oru_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ORU\""));
                assert!(body.contains("\"outcome\":\"created\""));
            }
            _ => panic!("unexpected response type"),
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert_eq!(body, "[]");
            }
            _ => panic!("unexpected response type"),
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_replay_payload_is_detected_and_cached() {
        let worklist_path = temp_file_path("interop_hl7_replay_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_hl7_replay_mpps", "snapshot");
        let sr_path = temp_file_path("interop_hl7_replay_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_hl7_replay_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let body = b"source=his&message_type=ADT&message_control_id=MSG-HL7-REPLAY&task_id=TASK-REPLAY&scheduled_step_id=STEP-REPLAY&status=SCHEDULED"
            .to_vec();

        let request = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: body.clone(),
        };

        let first = match route_request(&request, &shared, &limits).expect("interop adt first") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"message_type\":\"ADT\""));
                body
            }
            _ => panic!("unexpected response type"),
        };

        let second = match route_request(&request, &shared, &limits).expect("interop adt replay") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                body
            }
            _ => panic!("unexpected response type"),
        };
        assert_eq!(first, second);

        let mutated = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/hl7".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=his&message_type=ADT&message_control_id=MSG-HL7-REPLAY&task_id=TASK-REPLAY&scheduled_step_id=STEP-REPLAY&status=IN_PROGRESS"
                .to_vec(),
        };
        let err = route_request(&mutated, &shared, &limits).expect_err("replay mismatch must fail");
        assert_eq!(err.code, "DVF.HTTP.DECODE");
        assert!(
            err.message.contains("hl7 replay payload mismatch")
                || err.message.contains("decode error")
                || err.message.contains("DVF.HTTP.DECODE")
        );

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_task_transition_callbacks_are_delivered_and_counted() {
        let worklist_path = temp_file_path("interop_cb_task_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_task_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_task_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_task_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=task&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback.example%2Fevents"
                .to_vec(),
        };
        let create_sub_response = route_request(&create_sub, &shared, &limits)
            .expect("create task callback subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"workflow\""));
                assert!(body.contains("\"event_filter\":[\"task\"]"));
            }
        }

        let mut create_headers = BTreeMap::new();
        create_headers.insert("x-idempotency-key".to_string(), "task-cb-1".to_string());
        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS-CB&requested_procedure_id=RP-CB&worker=planner".to_vec(),
        };
        let task_id = match route_request(&create_task, &shared, &limits)
            .expect("create task for callback")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let marker = "\"task_id\":\"";
                let start = body
                    .find(marker)
                    .expect("task id field")
                    .saturating_add(marker.len());
                let end = body[start..].find('"').expect("task id terminator");
                body[start..start + end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let start_task = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-cb-success-1".to_string())]),
            body: Vec::new(),
        };
        match route_request(&start_task, &shared, &limits).expect("start task") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":1"));
            }
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(!body.contains("\"scope\":\"callback\""));
                assert!(!body.contains("\"attempt\":3"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn hl7_workflow_transition_callbacks_record_dlq_metadata_after_retries() {
        let worklist_path = temp_file_path("interop_cb_dlq_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_dlq_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_dlq_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_dlq_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=task&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-fail.local%2Fevents"
                .to_vec(),
        };
        let create_sub_response = route_request(&create_sub, &shared, &limits)
            .expect("create failing callback subscription");
        match create_sub_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"source\":\"workflow\""));
                assert!(body.contains("\"event_filter\":[\"task\"]"));
            }
        }

        let mut create_headers = BTreeMap::new();
        create_headers.insert(
            "x-idempotency-key".to_string(),
            "task-cb-fail-1".to_string(),
        );
        let create_task = HttpRequest {
            method: "POST".to_string(),
            path: "/workflow/tasks".to_string(),
            query: BTreeMap::new(),
            headers: create_headers,
            body: b"scheduled_step_id=PS-CB-F&requested_procedure_id=RP-CB-F&worker=planner"
                .to_vec(),
        };
        let task_id = match route_request(&create_task, &shared, &limits)
            .expect("create task for callback failure path")
        {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"outcome\":\"inserted\""));
                let marker = "\"task_id\":\"";
                let start = body
                    .find(marker)
                    .expect("task id field")
                    .saturating_add(marker.len());
                let end = body[start..].find('"').expect("task id terminator");
                body[start..start + end].to_string()
            }
        };
        assert!(!task_id.is_empty());

        let start_task = HttpRequest {
            method: "POST".to_string(),
            path: format!("/workflow/tasks/{task_id}/start"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([("x-request-id".to_string(), "hlt-cb-dlq-1".to_string())]),
            body: Vec::new(),
        };
        match route_request(&start_task, &shared, &limits).expect("start task") {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let task_get = HttpRequest {
            method: "GET".to_string(),
            path: format!("/workflow/tasks/{task_id}"),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let task_after =
            route_request(&task_get, &shared, &limits).expect("get task after callback failure");
        match task_after {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"status\":\"IN PROGRESS\""));
            }
        }

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"scope\":\"callback\""));
                assert!(body.contains("\"attempt\":3"));
                assert!(body.contains("\"max_attempts\":3"));
                assert!(body.contains("\"event_id\":\"HL7-000001\""));
                assert!(body.contains("\"subscription_id\":\"sub-00001\""));
                assert!(body.contains("\"correlation_id\":\"hlt-cb-dlq-1\""));
                assert!(body.contains("\"sequence\":1"));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"delivered_events\":0"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn connector_callback_circuit_breaker_opens_after_repeated_failures() {
        let worklist_path = temp_file_path("interop_cb_circuit_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_circuit_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_circuit_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_circuit_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        {
            let mut state = shared
                .lock()
                .expect("shared state lock for connector circuit fixture");
            state.hl7.connector_registry.insert(
                "lab".to_string(),
                "https://callback-fail.local/events".to_string(),
            );
            let _ = state.hl7.subscriptions.insert(
                "sub-00001".to_string(),
                Hl7Subscription {
                    id: "sub-00001".to_string(),
                    source: "workflow".to_string(),
                    event_filter: vec!["task".to_string()],
                    sink: Hl7Sink {
                        kind: Hl7SinkKind::Custom("lab".to_string()),
                        target: "https://callback-fail.local/events".to_string(),
                    },
                    delivered_events: 0,
                    created_at_ms: now_epoch_millis(),
                    last_event_ms: 0,
                },
            );

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-1",
                1,
                "corr-circuit-1",
            );
            assert_eq!(state.hl7.failures.len(), 1);

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-2",
                2,
                "corr-circuit-2",
            );
            assert_eq!(state.hl7.failures.len(), 2);
            let open_until = state
                .hl7
                .connector_circuit_open_until_ms
                .get("lab")
                .copied()
                .expect("circuit should open after repeated callback failures");
            assert!(open_until > now_epoch_millis());

            publish_hl7_event(
                &mut state,
                "workflow",
                "task",
                &BTreeMap::new(),
                "evt-circuit-3",
                3,
                "corr-circuit-3",
            );
            assert_eq!(
                state.hl7.failures.len(),
                2,
                "circuit-open connector should not emit additional callback failures until backoff elapses"
            );
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn workflow_tenant_quota_readonly_api_and_snapshot_export_override_values() {
        let _guard = ENV_LOCK.lock().expect("env lock for tenant quota api");
        let prev_task = std::env::var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A").ok();
        let prev_sub = std::env::var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A").ok();
        std::env::set_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A", "17");
        std::env::set_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A", "5");

        let worklist_path = temp_file_path("tenant_quota_api_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_quota_api_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_quota_api_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_quota_api_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let read_override = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/policy/quotas".to_string(),
            query: BTreeMap::from([("tenant".to_string(), "tenant-a".to_string())]),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "policy-admin".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: Vec::new(),
        };
        let read_response =
            route_request(&read_override, &shared, &limits).expect("tenant quota read api");
        match read_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"tenant\":\"tenant-a\""));
                assert!(body.contains("\"task_quota\":17"));
                assert!(body.contains("\"subscription_quota\":5"));
            }
        }

        let snapshot = HttpRequest {
            method: "GET".to_string(),
            path: "/workflow/policy/quotas/snapshot".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "policy-admin".to_string()),
                ("x-sr-role".to_string(), "admin".to_string()),
            ]),
            body: Vec::new(),
        };
        let snapshot_response =
            route_request(&snapshot, &shared, &limits).expect("tenant quota snapshot api");
        match snapshot_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"generated_at_ms\":"));
                assert!(body.contains("\"tenant_a\":{\"task_quota\":17,\"subscription_quota\":5}"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);

        if let Some(value) = prev_task {
            std::env::set_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A");
        }
        if let Some(value) = prev_sub {
            std::env::set_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A", value);
        } else {
            std::env::remove_var("DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A");
        }
    }

    #[test]
    fn tenant_rate_limit_override_snapshot_round_trip() {
        let snapshot_path = temp_file_path("tenant_rate_limit_overrides", "state");
        let mut overrides = BTreeMap::new();
        let _ = overrides.insert(
            "tenant_a".to_string(),
            TenantRateLimitOverride {
                query_rate_limit: 45,
                mutation_rate_limit: 21,
                upload_cap_bytes: 2 * 1024 * 1024,
            },
        );
        persist_tenant_rate_limit_overrides(&snapshot_path.to_string_lossy(), &overrides);

        let loaded = load_tenant_rate_limit_overrides(&snapshot_path.to_string_lossy());
        let tenant_a = loaded
            .get("tenant_a")
            .expect("tenant_a override should round-trip");
        assert_eq!(tenant_a.query_rate_limit, 45);
        assert_eq!(tenant_a.mutation_rate_limit, 21);
        assert_eq!(tenant_a.upload_cap_bytes, 2 * 1024 * 1024);

        cleanup_with_rotations(&snapshot_path, 0);
    }

    #[test]
    fn tenant_rate_limit_overrides_apply_and_fallback_to_defaults() {
        let worklist_path = temp_file_path("tenant_rate_policy_worklist", "snapshot");
        let mpps_path = temp_file_path("tenant_rate_policy_mpps", "snapshot");
        let sr_path = temp_file_path("tenant_rate_policy_sr", "snapshot");
        let sr_audit_path = temp_file_path("tenant_rate_policy_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);
        let mut overrides = BTreeMap::new();
        let _ = overrides.insert(
            tenant_env_suffix("tenant-a").to_ascii_lowercase(),
            TenantRateLimitOverride {
                query_rate_limit: 61,
                mutation_rate_limit: 17,
                upload_cap_bytes: 1_024,
            },
        );
        set_tenant_rate_limit_override_cache(overrides);

        {
            let state = shared
                .lock()
                .expect("shared state lock for tenant rate policy assertions");
            let tenant_a_policy = tenant_policy(&state, "tenant-a");
            assert_eq!(tenant_a_policy.query_rate_limit, 61);
            assert_eq!(tenant_a_policy.mutation_rate_limit, 17);
            assert_eq!(tenant_a_policy.upload_cap_bytes, 1_024);

            let tenant_b_policy = tenant_policy(&state, "tenant-b");
            assert_eq!(tenant_b_policy.query_rate_limit, DEFAULT_QUERY_RATE_LIMIT);
            assert_eq!(
                tenant_b_policy.mutation_rate_limit,
                DEFAULT_MUTATION_RATE_LIMIT
            );
            assert_eq!(tenant_b_policy.upload_cap_bytes, DEFAULT_UPLOAD_CAP_BYTES);
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
        set_tenant_rate_limit_override_cache(BTreeMap::new());
    }

    #[test]
    fn reconciliation_jobs_enforce_interval_bounds_max_jobs_and_tenant_isolation() {
        let worklist_path = temp_file_path("recon_guardrails_worklist", "snapshot");
        let mpps_path = temp_file_path("recon_guardrails_mpps", "snapshot");
        let sr_path = temp_file_path("recon_guardrails_sr", "snapshot");
        let sr_audit_path = temp_file_path("recon_guardrails_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let invalid_interval = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/reconciliation/jobs".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-admin".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: b"source=his&target_endpoint=https://recon.example/jobs&interval_seconds=10"
                .to_vec(),
        };
        let invalid_interval_err = route_request(&invalid_interval, &shared, &limits)
            .expect_err("interval below floor should fail");
        assert_eq!(status_for_error(&invalid_interval_err).0, 400);

        let create = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/reconciliation/jobs".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-admin".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-a".to_string()),
            ]),
            body: b"source=his&target_endpoint=https://recon.example/jobs&interval_seconds=120"
                .to_vec(),
        };
        let created = route_request(&create, &shared, &limits).expect("create reconciliation job");
        let created_job_id = match created {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"tenant\":\"tenant-a\""));
                let marker = "\"id\":\"";
                let start = body
                    .find(marker)
                    .expect("reconciliation id field")
                    .saturating_add(marker.len());
                let end = body[start..]
                    .find('"')
                    .expect("reconciliation id terminator");
                body[start..start + end].to_string()
            }
        };

        let cross_tenant_run = HttpRequest {
            method: "POST".to_string(),
            path: format!("/interop/reconciliation/jobs/{created_job_id}/run"),
            query: BTreeMap::new(),
            headers: BTreeMap::from([
                ("x-sr-principal".to_string(), "recon-other".to_string()),
                ("x-sr-role".to_string(), "writer".to_string()),
                ("x-workflow-tenant".to_string(), "tenant-b".to_string()),
                (
                    "x-idempotency-key".to_string(),
                    "recon-run-cross-1".to_string(),
                ),
            ]),
            body: Vec::new(),
        };
        let cross_tenant_err = route_request(&cross_tenant_run, &shared, &limits)
            .expect_err("cross-tenant run should fail");
        assert_eq!(cross_tenant_err.code, "DVF.WORKFLOW.SR.AUTH_DENIED");

        {
            let mut state = shared
                .lock()
                .expect("shared state lock for reconciliation cap fixture");
            while state.hl7.reconciliation_jobs.len() < MAX_RECONCILIATION_JOBS {
                state.hl7.reconciliation_seq = state.hl7.reconciliation_seq.saturating_add(1);
                let id = format!("recon-cap-{0:05}", state.hl7.reconciliation_seq);
                let _ = state.hl7.reconciliation_jobs.insert(
                    id.clone(),
                    StudyReconciliationJob {
                        id,
                        tenant: "tenant-a".to_string(),
                        source: "his".to_string(),
                        target_endpoint: "https://recon.example/jobs".to_string(),
                        interval_seconds: 120,
                        runs_enqueued: 0,
                        runs_completed: 0,
                        last_run_at_ms: 0,
                        created_at_ms: now_epoch_millis(),
                        enabled: true,
                    },
                );
            }
        }

        let over_cap = route_request(&create, &shared, &limits)
            .expect_err("reconciliation max jobs guardrail should fail");
        assert_eq!(status_for_error(&over_cap).0, 413);

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }

    #[test]
    fn parse_hl7_event_filter_supports_workflow_transition_aliases() {
        let parsed = parse_hl7_event_filter("workflow,task,mpps,sr")
            .expect("parse workflow transition filters");
        assert_eq!(parsed, vec!["mpps", "sr", "task", "workflow"]);
        let parsed_workflow_only =
            parse_hl7_event_filter("workflow").expect("parse workflow filter");
        assert_eq!(parsed_workflow_only, vec!["workflow"]);
        let err = parse_hl7_event_filter("badfilter")
            .expect_err("unsupported hl7 event filter must fail");
        assert_eq!(err.code, "DVF.WORKFLOW.HTTP.DECODE_ERROR");
    }

    #[test]
    fn hl7_mpps_and_sr_transition_callbacks_use_workflow_filters() {
        let worklist_path = temp_file_path("interop_cb_mpps_sr_worklist", "snapshot");
        let mpps_path = temp_file_path("interop_cb_mpps_sr_mpps", "snapshot");
        let sr_path = temp_file_path("interop_cb_mpps_sr_sr", "snapshot");
        let sr_audit_path = temp_file_path("interop_cb_mpps_sr_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Limits::default();
        let shared = build_sr_workflow_state(&worklist_path, &mpps_path, &sr_path, &sr_audit_path);

        let create_mpps_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=workflow&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-mpps.example%2Fevents"
                .to_vec(),
        };
        let _ = route_request(&create_mpps_sub, &shared, &limits)
            .expect("create mpps/sub transition subscription");

        let create_sr_sub = HttpRequest {
            method: "POST".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"source=workflow&event_filter=workflow&sink_kind=webhook&sink_target=https%3A%2F%2Fcallback-sr.example%2Fevents"
                .to_vec(),
        };
        let _ = route_request(&create_sr_sub, &shared, &limits)
            .expect("create sr transition subscription");

        let create_mpps = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"sop_instance_uid=1.2.3.4&status=IN+PROGRESS&performed_step_id=PS-1&start_date=20260222&start_time=101010".to_vec(),
        };
        let _ = route_request(&create_mpps, &shared, &limits).expect("create mpps");

        let update_mpps = HttpRequest {
            method: "POST".to_string(),
            path: "/mpps/updates/1.2.3.4/status".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: b"status=COMPLETED".to_vec(),
        };
        let _ = route_request(&update_mpps, &shared, &limits).expect("update mpps");

        let mut sr_headers = BTreeMap::new();
        sr_headers.insert("x-sr-role".to_string(), "writer".to_string());
        let create_sr = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers.clone(),
            body: b"study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid=1.2.3.4.5&observer=alice&authored_epoch_ms=1700&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=transition"
                .to_vec(),
        };
        let _ = route_request(&create_sr, &shared, &limits).expect("create sr");
        let review_sr = HttpRequest {
            method: "POST".to_string(),
            path: "/sr/documents/1.2.3.4.5/review".to_string(),
            query: BTreeMap::new(),
            headers: sr_headers,
            body: Vec::new(),
        };
        let _ = route_request(&review_sr, &shared, &limits).expect("review sr");

        let failures = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/hl7/failures".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let failures_response = route_request(&failures, &shared, &limits).expect("list failures");
        match failures_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(!body.contains("\"scope\":\"callback\""));
                assert!(!body.contains("\"attempt\":3"));
            }
        }

        let list_subscriptions = HttpRequest {
            method: "GET".to_string(),
            path: "/interop/subscriptions".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list =
            route_request(&list_subscriptions, &shared, &limits).expect("list subscriptions");
        match list {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                assert!(body.contains("\"event_filter\":[\"workflow\"]"));
                assert!(body.contains("\"delivered_events\":2"));
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }
}

#[cfg(all(test, feature = "workflow-main-tests"))]
mod parallel_tests {
    use super::*;
    use dicom_workflow_server::path_with_suffix;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str, ext: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("rdvf_{name}_{nonce}.{ext}"))
    }

    fn cleanup_with_rotations(path: &Path, max_rotations: usize) {
        let _ = std::fs::remove_file(path);
        for index in 1..=max_rotations + 1 {
            let _ = std::fs::remove_file(path_with_suffix(path, index));
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }

    #[test]
    fn sr_http_parallel_create_update_is_deterministic() {
        // REQ-SR-300, REQ-TEST-742
        let worklist_path = temp_file_path("sr_parallel_worklist", "snapshot");
        let mpps_path = temp_file_path("sr_parallel_mpps", "snapshot");
        let sr_path = temp_file_path("sr_parallel_sr", "snapshot");
        let sr_audit_path = temp_file_path("sr_parallel_sr", "audit");
        prepare_persistence_file(&worklist_path.to_string_lossy(), 1_048_576, 2, "worklist")
            .expect("worklist preflight");
        prepare_persistence_file(&mpps_path.to_string_lossy(), 1_048_576, 2, "mpps")
            .expect("mpps preflight");
        prepare_persistence_file(&sr_path.to_string_lossy(), 1_048_576, 2, "sr")
            .expect("sr preflight");
        prepare_persistence_file(&sr_audit_path.to_string_lossy(), 1_048_576, 2, "sr audit")
            .expect("sr audit preflight");

        let limits = Arc::new(Limits::default());
        let state = RuntimeState {
            worklist: WorklistStore::open((*limits).clone(), &worklist_path).expect("worklist"),
            mpps: MppsService::with_persistence(
                MppsServiceConfig {
                    limits: (*limits).clone(),
                    audit: None,
                },
                &mpps_path,
            )
            .expect("mpps"),
            sr: SrWorkflowStore::open(
                (*limits).clone(),
                &sr_path.to_string_lossy(),
                &sr_audit_path.to_string_lossy(),
            )
            .expect("sr"),
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence: 1,
            task_idempotency: BTreeMap::new(),
            hl7: Hl7RuntimeState::default(),
        };
        let shared = Arc::new(Mutex::new(state));

        let mut handles = Vec::new();
        for index in 0..8u32 {
            let shared = Arc::clone(&shared);
            let limits = Arc::clone(&limits);
            handles.push(thread::spawn(move || {
                let sop_uid = format!("1.2.840.10008.5.1.{}", index + 1);

                let mut create_headers = BTreeMap::new();
                create_headers.insert("x-sr-principal".to_string(), "parallel-user".to_string());
                create_headers.insert("x-sr-role".to_string(), "writer".to_string());
                create_headers.insert(
                    "x-idempotency-key".to_string(),
                    format!("parallel-create-{index}"),
                );
                let create_body = format!(
                    "study_instance_uid=1.2.3&series_instance_uid=1.2.3.4&sop_instance_uid={sop_uid}&observer=parallel-user&authored_epoch_ms={}&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=create-{index}",
                    1000 + index
                );
                let create = HttpRequest {
                    method: "POST".to_string(),
                    path: "/sr/documents".to_string(),
                    query: BTreeMap::new(),
                    headers: create_headers,
                    body: create_body.into_bytes(),
                };
                let create_response = route_request(&create, &shared, &limits).expect("parallel create");
                match create_response {
                    WorkflowResponse::Json(code, body) => {
                        assert_eq!(code, 200);
                        assert!(body.contains("\"version\":1"));
                    }
                }

                let mut update_headers = BTreeMap::new();
                update_headers.insert("x-sr-principal".to_string(), "parallel-user".to_string());
                update_headers.insert("x-sr-role".to_string(), "writer".to_string());
                update_headers.insert(
                    "x-idempotency-key".to_string(),
                    format!("parallel-update-{index}"),
                );
                let update_body =
                    format!("expected_version=1&item_kind=text&concept_code_value=121071&concept_scheme=DCM&concept_meaning=Finding&text_value=update-{index}");
                let update = HttpRequest {
                    method: "POST".to_string(),
                    path: format!("/sr/documents/{sop_uid}/updates"),
                    query: BTreeMap::new(),
                    headers: update_headers,
                    body: update_body.into_bytes(),
                };
                let update_response = route_request(&update, &shared, &limits).expect("parallel update");
                match update_response {
                    WorkflowResponse::Json(code, body) => {
                        assert_eq!(code, 200);
                        assert!(body.contains("\"version\":2"));
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread join");
        }

        let list = HttpRequest {
            method: "GET".to_string(),
            path: "/sr/documents".to_string(),
            query: BTreeMap::new(),
            headers: BTreeMap::new(),
            body: Vec::new(),
        };
        let list_response = route_request(&list, &shared, &limits).expect("list");
        match list_response {
            WorkflowResponse::Json(code, body) => {
                assert_eq!(code, 200);
                for index in 0..8u32 {
                    let sop_uid = format!("1.2.840.10008.5.1.{}", index + 1);
                    assert!(body.contains(&sop_uid));
                }
            }
        }

        cleanup_with_rotations(&worklist_path, 2);
        cleanup_with_rotations(&mpps_path, 2);
        cleanup_with_rotations(&sr_path, 2);
        cleanup_with_rotations(&sr_audit_path, 2);
    }
}
