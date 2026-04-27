//! Runtime configuration types, environment parsing, tenant policies, and auth helpers.

use super::*;

pub const TAG_SPS_SEQUENCE: Tag = Tag(0x0040, 0x0100);
pub const TAG_SPS_ID: Tag = Tag(0x0040, 0x0009);
pub const TAG_SPS_START_DATE: Tag = Tag(0x0040, 0x0002);
pub const TAG_SPS_START_TIME: Tag = Tag(0x0040, 0x0003);
pub const TAG_SCHEDULED_STATION_AE_TITLE: Tag = Tag(0x0040, 0x0001);
pub const TAG_MODALITY: Tag = Tag(0x0008, 0x0060);
pub const TAG_REQUESTED_PROCEDURE_ID: Tag = Tag(0x0040, 0x1001);
pub const TAG_PATIENT_ID: Tag = Tag(0x0010, 0x0020);
pub const TAG_ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);

pub const TAG_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
pub const TAG_STATUS: Tag = Tag(0x0040, 0x0252);
pub const TAG_PERFORMED_STEP_ID: Tag = Tag(0x0040, 0x0253);
pub const TAG_START_DATE: Tag = Tag(0x0040, 0x0244);
pub const TAG_START_TIME: Tag = Tag(0x0040, 0x0245);
pub const TAG_END_DATE: Tag = Tag(0x0040, 0x0250);
pub const TAG_END_TIME: Tag = Tag(0x0040, 0x0251);
pub const DEFAULT_WORKFLOW_PAGE_SIZE: usize = 50;
pub const MAX_WORKFLOW_PAGE_SIZE: usize = 500;
pub const DEFAULT_UPLOAD_CAP_BYTES: u64 = 8 * 1024 * 1024;
pub const DEFAULT_QUERY_RATE_LIMIT: u64 = 300;
pub const DEFAULT_MUTATION_RATE_LIMIT: u64 = 120;
pub const DEFAULT_AUDIT_MAX_BYTES: u64 = 8 * 1024 * 1024;
pub const DEFAULT_AUDIT_MAX_ROTATED_FILES: usize = 4;
pub const DEFAULT_AUDIT_EXPORT_LIMIT: usize = 256;
pub const DEFAULT_AUDIT_PATH: &str = "./state/workflow/workflow.audit.log";
pub const DEFAULT_RATE_LIMIT_WINDOW_MS: u64 = 15_000;
pub const DEFAULT_ANOMALY_ALERT_THRESHOLD: u64 = 6;
pub const DEFAULT_WORKFLOW_MAX_PAIR_COUNT: usize = 256;
pub const DEFAULT_WORKFLOW_DENYLIST_PATHS: &[&str] = &[
    "/workflow/tasks/{task_id}/cancel",
    "/workflow/tasks/{task_id}/commit",
    "/mpps/updates/{sop_instance_uid}/status",
];
pub const WORKITEM_COLLECTION_PATH: &str = "/workflow/workitems";
pub const WORKITEM_ITEM_PREFIX: &str = "/workflow/workitems/";
pub const INTEROP_IAN_PATH: &str = "/interop/ian";
pub const INTEROP_STORAGE_COMMITMENT_STATUS_PATH: &str = "/interop/storage-commitment/status";
pub const INTEROP_STORAGE_COMMITMENT_STATUS_PREFIX: &str = "/interop/storage-commitment/status/";
pub const INTEROP_HL7_UPS_CORRELATION_PATH: &str = "/interop/hl7/ups-correlation";
pub const MAX_MPPS_IDEMPOTENCY_ENTRIES: usize = 128;
pub const MAX_TASK_IDEMPOTENCY_ENTRIES: usize = 128;
pub const MAX_HL7_FAILURES: usize = 256;
pub const MAX_HL7_FAILURE_EXCERPT_BYTES: usize = 180;
pub const MAX_HL7_SUBSCRIPTIONS: usize = 64;
pub const MAX_HL7_IDEMPOTENCY_ENTRIES: usize = 128;
pub const MAX_HL7_CONNECTOR_REGISTRY: usize = 64;
pub const DEFAULT_TENANT_TASK_QUOTA: usize = 512;
pub const DEFAULT_TENANT_SUBSCRIPTION_QUOTA: usize = 64;
pub const DEFAULT_HL7_CONNECTOR_ROLLOUT_PERCENT: u64 = 100;
pub const HL7_CONNECTOR_FEATURE_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_";
pub const HL7_CONNECTOR_ROLLOUT_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_";
pub const HL7_CONNECTOR_PLUGIN_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_";
pub const HL7_CONNECTOR_VERSION_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_";
pub const HL7_CONNECTOR_COMPAT_MIN_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_";
pub const HL7_CONNECTOR_COMPAT_MAX_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_";
pub const DEFAULT_HL7_FILE_DROP_POLL_INTERVAL_MS: u64 = 3000;
pub const HL7_MLLP_START_BYTE: u8 = 0x0b;
pub const HL7_MLLP_END_BYTES: [u8; 2] = [0x1c, 0x0d];
pub const MAX_WORKFLOW_CALLBACK_ATTEMPTS: u32 = 3;
pub const CALLBACK_CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 2;
pub const CALLBACK_CIRCUIT_BREAKER_BASE_BACKOFF_MS: u64 = 15_000;
pub const CALLBACK_CIRCUIT_BREAKER_MAX_BACKOFF_MS: u64 = 300_000;
pub const DEFAULT_HL7_CALLBACK_IDEMPOTENCY_TTL_MS: u64 = 30 * 60 * 1000;
pub const HL7_CALLBACK_MAX_ATTEMPTS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS";
pub const HL7_CALLBACK_CB_FAILURE_THRESHOLD_ENV: &str =
    "DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD";
pub const HL7_CALLBACK_CB_BASE_BACKOFF_MS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS";
pub const HL7_CALLBACK_CB_MAX_BACKOFF_MS_ENV: &str = "DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS";
pub const MAX_RECONCILIATION_JOBS: usize = 128;
pub const MAX_RECONCILIATION_JOBS_PER_TENANT: usize = 32;
pub const RECONCILIATION_INTERVAL_FLOOR_SECONDS: u64 = 60;
pub const RECONCILIATION_INTERVAL_CEILING_SECONDS: u64 = 86_400;
pub const RECONCILIATION_RUN_IDEMPOTENCY_PREFIX: &str = "reconciliation-run:";
pub const WORKFLOW_ADMIN_API_VERSION: &str = "v1";
pub const WEBHOOK_AUTH_STRATEGY_ENV: &str = "DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY";
pub const WEBHOOK_AUTH_SECRET_ENV: &str = "DICOM_WORKFLOW_WEBHOOK_AUTH_SHARED_SECRET";
pub const WORKFLOW_SERVICE_NAME: &str = "dicom-workflow-server";
pub const WORKFLOW_SECRET_AUTH_TOKEN_FILE_HINTS: &[&str] = &["auth_token", "workflow_auth_token"];
pub const WORKFLOW_SECRET_TLS_CERT_FILE_HINTS: &[&str] =
    &["tls_cert", "tls_cert.pem", "workflow_tls_cert"];
pub const WORKFLOW_SECRET_TLS_KEY_FILE_HINTS: &[&str] = &["tls_key", "tls_key.pem", "workflow_tls_key"];
pub const HL7_CONNECTOR_ENV_PREFIX: &str = "DICOM_WORKFLOW_HL7_CONNECTOR_";
pub const TENANT_ID_DEFAULT: &str = "tenant-default";
pub static REQUEST_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Policy and identity context derived from request headers.
#[derive(Debug, Clone)]
pub struct WorkflowActorContext {
    pub principal: Option<String>,
    pub role: Option<String>,
    pub tenant: String,
}

impl WorkflowActorContext {
    pub fn can_write(&self) -> bool {
        self.role.as_deref().is_some_and(workflow_role_is_writer)
    }
}

/// Fixed-window request throttle for a policy scope.
#[derive(Debug, Clone, Copy)]
pub struct RequestWindow {
    pub window_start_ms: u64,
    pub count: u64,
}

impl RequestWindow {
    pub fn empty() -> Self {
        Self {
            window_start_ms: 0,
            count: 0,
        }
    }
}

/// Tenant policy object for baseline rate limits and quotas.
#[derive(Debug, Clone, Copy)]
pub struct TenantWorkflowPolicy {
    pub query_rate_limit: u64,
    pub mutation_rate_limit: u64,
    pub upload_cap_bytes: u64,
    pub task_quota: usize,
    pub subscription_quota: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantQuotaOverride {
    pub task_quota: usize,
    pub subscription_quota: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantRateLimitOverride {
    pub query_rate_limit: u64,
    pub mutation_rate_limit: u64,
    pub upload_cap_bytes: u64,
}

/// Lightweight mutable per-tenant/route anomaly and operations metrics.
#[derive(Debug, Clone, Copy, Default)]
pub struct TenantOperationMetrics {
    pub read_operations: u64,
    pub mutation_operations: u64,
    pub query_operations: u64,
    pub denied_operations: u64,
    pub anomaly_operations: u64,
}

/// Runtime operational audit event written to append-only file.
#[derive(Debug, Clone)]
pub struct WorkflowAuditEvent {
    pub ts_ms: u64,
    pub tenant: String,
    pub principal_hash: String,
    pub request_id_hash: String,
    pub previous_audit_hash: String,
    pub audit_hash: String,
    pub route: String,
    pub method: String,
    pub operation: String,
    pub outcome: String,
    pub scope: String,
    pub status: u16,
    pub anomaly: bool,
}

pub static TENANT_RATE_LIMIT_OVERRIDE_CACHE: OnceLock<
    Mutex<BTreeMap<String, TenantRateLimitOverride>>,
> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hl7ConnectorFeatureFlag {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone)]
pub struct ConnectorPluginMetadata {
    pub plugin_path: String,
    pub adapter_version: String,
    pub compatible_min: String,
    pub compatible_max: String,
}

#[derive(Debug, Clone, Copy)]
pub enum NormalizedConnectorAdapterKind {
    Dimse,
    HisRis,
    Generic,
}

impl NormalizedConnectorAdapterKind {
    pub fn as_label(self) -> &'static str {
        match self {
            Self::Dimse => "dimse",
            Self::HisRis => "his_ris",
            Self::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProfile {
    Azure,
    AwsHealthImaging,
    Orthanc,
    Dcm4chee,
    Generic,
}

impl ProviderProfile {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "azure" => Some(Self::Azure),
            "aws_health_imaging" | "aws" | "ahi" => Some(Self::AwsHealthImaging),
            "orthanc" => Some(Self::Orthanc),
            "dcm4chee" => Some(Self::Dcm4chee),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Azure => "azure",
            Self::AwsHealthImaging => "aws_health_imaging",
            Self::Orthanc => "orthanc",
            Self::Dcm4chee => "dcm4chee",
            Self::Generic => "generic",
        }
    }

    pub fn capabilities(self) -> ProviderCapabilityMap {
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
pub struct ProviderCapabilityMap {
    pub retrieve: bool,
    pub search: bool,
    pub store: bool,
    pub delete: bool,
    pub workitem: bool,
}

#[derive(Debug, Clone)]
pub struct NormalizedConnectorAdapter {
    pub alias: String,
    pub target: String,
    pub adapter_kind: NormalizedConnectorAdapterKind,
    pub plugin: Option<ConnectorPluginMetadata>,
}

impl Hl7ConnectorFeatureFlag {
    pub fn parse(raw: &str, alias: &str) -> std::io::Result<Self> {
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

    pub fn as_bool(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// HL7 sink adapter for outbound interoperability notifications.
#[derive(Debug, Clone)]
pub struct Hl7Sink {
    /// Transport kind.
    pub kind: Hl7SinkKind,
    /// Target URI/topic.
    pub target: String,
}

#[derive(Debug, Clone)]
pub enum Hl7SinkKind {
    Webhook,
    MessageBus,
    Custom(String),
}

impl Hl7SinkKind {
    pub fn as_label(&self) -> &'static str {
        match self {
            Hl7SinkKind::Webhook => "webhook",
            Hl7SinkKind::MessageBus => "message_bus",
            Hl7SinkKind::Custom(_) => "custom",
        }
    }
}

/// Deterministic interop subscription state.
#[derive(Debug, Clone)]
pub struct Hl7Subscription {
    /// Stable subscription identifier.
    pub id: String,
    /// Adapter source name.
    pub source: String,
    /// Event filters (e.g., `adt`, `orm`, `oru`, `task`, `mpps`, or `sr`).
    /// (S13-T6) Uses `BTreeSet<String>` for O(log n) membership testing.
    pub event_filter: BTreeSet<String>,
    /// Selected sink.
    pub sink: Hl7Sink,
    /// Total events delivered to this subscription endpoint.
    pub delivered_events: u64,
    /// Creation timestamp.
    pub created_at_ms: u64,
    /// Last event timestamp.
    pub last_event_ms: u64,
}

/// Supported HL7 message class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hl7MessageClass {
    Adt,
    Orm,
    Oru,
    Siu,
}

impl Hl7MessageClass {
    pub fn as_label(&self) -> &'static str {
        match self {
            Hl7MessageClass::Adt => "ADT",
            Hl7MessageClass::Orm => "ORM",
            Hl7MessageClass::Oru => "ORU",
            Hl7MessageClass::Siu => "SIU",
        }
    }
}

pub fn parse_hl7_message_class(raw: &str) -> Option<Hl7MessageClass> {
    match raw.trim().to_ascii_uppercase().as_str() {
        "ADT" | "A01" | "A04" => Some(Hl7MessageClass::Adt),
        "ORM" | "ORM_O01" | "ORM-O01" => Some(Hl7MessageClass::Orm),
        "ORU" | "ORU_R01" | "ORU-R01" => Some(Hl7MessageClass::Oru),
        "SIU" | "SIU_S12" | "SIU-S12" | "S12" => Some(Hl7MessageClass::Siu),
        _ => None,
    }
}

pub fn hl7_field<'a>(field: Option<&'a &'a str>) -> Option<&'a str> {
    field
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
}

/// Failure quarantine entry for malformed HL7 payloads.
#[derive(Debug, Clone)]
pub struct Hl7FailureRecord {
    /// Deterministic record identifier.
    pub id: String,
    /// Source adapter (provider).
    pub source: String,
    /// Parsed HL7 message type, or `invalid`.
    pub message_type: String,
    /// Human-readable failure reason.
    pub reason: String,
    /// Optional payload excerpt.
    pub payload_excerpt: String,
    /// Event timestamp.
    pub created_at_ms: u64,
    /// Failure scope (e.g., `ingest` or `callback`).
    pub scope: String,
    /// Callback subscription identifier when scope is `callback`.
    pub subscription_id: String,
    /// Event identifier used for failure correlation.
    pub event_id: String,
    /// Correlation identifier for the inbound interface request.
    pub correlation_id: String,
    /// Monotonic sequence number of the inbound interface event.
    pub sequence: u64,
    /// Last attempt number attempted.
    pub attempt: u32,
    /// Maximum attempts allowed before dead-lettering.
    pub max_attempts: u32,
}

#[derive(Debug, Clone)]
pub struct StudyReconciliationJob {
    pub id: String,
    pub tenant: String,
    pub source: String,
    pub target_endpoint: String,
    pub interval_seconds: u64,
    pub runs_enqueued: u64,
    pub runs_completed: u64,
    pub last_run_at_ms: u64,
    pub created_at_ms: u64,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct IanEventRecord {
    pub event_id: String,
    pub sop_instance_uid: String,
    pub outcome: CompletionOutcome,
    pub ingested_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct StorageCommitmentStatusRecord {
    pub transaction_uid: String,
    pub outcome: CompletionOutcome,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Hl7CallbackPolicy {
    pub max_attempts: u32,
    pub circuit_failure_threshold: u32,
    pub circuit_base_backoff_ms: u64,
    pub circuit_max_backoff_ms: u64,
}

/// Runtime state for HL7 transport and notification hooks.
#[derive(Debug, Default)]
pub struct Hl7RuntimeState {
    pub subscriptions: BTreeMap<String, Hl7Subscription>,
    pub failures: VecDeque<Hl7FailureRecord>,
    pub ups: UpsCommandAdapter,
    pub completion: CompletionWorkflowAdapter,
    pub ian_events: BTreeMap<String, IanEventRecord>,
    pub storage_commitment_status: BTreeMap<String, StorageCommitmentStatusRecord>,
    pub hl7_ups_correlation: BTreeMap<String, String>,
    pub subscription_seq: u64,
    pub failure_seq: u64,
    pub event_seq: u64,
    pub replay_cache: BTreeMap<String, CachedMppsRequest>,
    pub connector_registry: BTreeMap<String, String>,
    pub connector_feature_flags: BTreeMap<String, Hl7ConnectorFeatureFlag>,
    pub connector_rollout_percent: BTreeMap<String, u64>,
    pub connector_plugins: BTreeMap<String, ConnectorPluginMetadata>,
    pub callback_delivery_idempotency: BTreeMap<String, u64>,
    pub callback_idempotency_ttl_ms: u64,
    pub callback_idempotency_path: String,
    pub callback_max_attempts: u32,
    pub callback_circuit_breaker_failure_threshold: u32,
    pub callback_circuit_breaker_base_backoff_ms: u64,
    pub callback_circuit_breaker_max_backoff_ms: u64,
    pub connector_callback_failure_streak: BTreeMap<String, u32>,
    pub connector_circuit_open_until_ms: BTreeMap<String, u64>,
    pub reconciliation_jobs: BTreeMap<String, StudyReconciliationJob>,
    pub reconciliation_seq: u64,
}

/// Workflow runtime auth mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowAuthMode {
    DenyAll,
    AllowAll,
    Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookAuthStrategy {
    None,
    HmacSha256,
}

impl WorkflowAuthMode {
    pub fn parse(raw: Option<&str>) -> std::io::Result<Self> {
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

// ===========================================================================
// S10-T8: Decomposed sub-structs with own Mutex/RwLock
// ===========================================================================

/// HL7-related runtime state (S10-T8: owns its own Mutex).
#[derive(Debug)]
pub struct Hl7State {
    inner: Mutex<Hl7RuntimeState>,
}

impl Hl7State {
    /// Create from an existing Hl7RuntimeState.
    pub fn new(inner: Hl7RuntimeState) -> Self { Self { inner: Mutex::new(inner) } }
    /// Lock for exclusive access.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Hl7RuntimeState> { self.inner.lock().unwrap() }
}

impl Default for Hl7State {
    fn default() -> Self { Self { inner: Mutex::new(Hl7RuntimeState::default()) } }
}

/// Tenant management state (S10-T8: owns its own Mutex).
pub struct TenantState {
    inner: Mutex<TenantStateData>,
}

/// Inner tenant data with pub fields for handler access after locking.
///
/// (S13-T6) Tenant index fields use `BTreeSet<String>` for O(log n) membership
/// testing instead of `Vec<String>` which required O(n) linear scans.
pub struct TenantStateData {
    /// Per-tenant worklist index.
    pub tenant_worklist: BTreeMap<String, BTreeSet<String>>,
    /// Per-tenant MPPS index.
    pub tenant_mpps: BTreeMap<String, BTreeSet<String>>,
    /// Per-tenant SR index.
    pub tenant_sr: BTreeMap<String, BTreeSet<String>>,
    /// Per-tenant task index.
    pub tenant_tasks: BTreeMap<String, BTreeSet<String>>,
    /// Per-tenant operation metrics.
    pub metrics: BTreeMap<String, TenantOperationMetrics>,
}

impl TenantState {
    /// Create empty.
    pub fn new() -> Self {
        Self { inner: Mutex::new(TenantStateData {
            tenant_worklist: BTreeMap::new(),
            tenant_mpps: BTreeMap::new(),
            tenant_sr: BTreeMap::new(),
            tenant_tasks: BTreeMap::new(),
            metrics: BTreeMap::new(),
        }) }
    }
    /// Lock for exclusive access.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, TenantStateData> { self.inner.lock().unwrap() }
}

/// Worker pool state (S10-T8: owns its own Mutex).
pub struct WorkerState {
    inner: Mutex<WorkerStateData>,
}

/// Inner worker data.
pub struct WorkerStateData {
    /// MPPS idempotency cache.
    pub mpps_idempotency: BTreeMap<String, CachedMppsRequest>,
    /// Procedure tasks indexed by task ID.
    pub tasks: BTreeMap<String, ProcedureTask>,
    /// Task ID sequence counter.
    pub task_id_sequence: u64,
    /// Task idempotency cache.
    pub task_idempotency: BTreeMap<String, CachedMppsRequest>,
}

impl WorkerState {
    /// Create with initial state.
    pub fn new(task_id_sequence: u64, task_idempotency: BTreeMap<String, CachedMppsRequest>) -> Self {
        Self { inner: Mutex::new(WorkerStateData {
            mpps_idempotency: BTreeMap::new(),
            tasks: BTreeMap::new(),
            task_id_sequence,
            task_idempotency,
        }) }
    }
    /// Lock for exclusive access.
    pub fn lock(&self) -> std::sync::MutexGuard<'_, WorkerStateData> { self.inner.lock().unwrap() }
}

/// Health check and audit state (S10-T8: owns its own RwLock).
pub struct HealthState {
    inner: std::sync::RwLock<HealthStateData>,
}

/// Inner health data.
pub struct HealthStateData {
    /// Audit file path.
    pub audit_path: String,
    /// Audit rate window in milliseconds.
    pub audit_rate_window_ms: u64,
    /// Query rate limit.
    pub query_rate_limit: u64,
    /// Mutation rate limit.
    pub mutation_rate_limit: u64,
    /// Upload cap in bytes.
    pub upload_cap_bytes: u64,
    /// Audit max bytes before rotation.
    pub audit_max_bytes: u64,
    /// Audit max rotated files.
    pub audit_max_rotated_files: usize,
    /// Rate windows per tenant.
    pub rate_windows: BTreeMap<String, RequestWindow>,
    /// Anomaly alert threshold.
    pub anomaly_alert_threshold: u64,
    /// Audit export limit.
    pub audit_export_limit: usize,
    /// Denylist routes.
    pub denylist_routes: Vec<String>,
}

impl HealthState {
    /// Create with the given configuration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        audit_path: String,
        audit_rate_window_ms: u64,
        query_rate_limit: u64,
        mutation_rate_limit: u64,
        upload_cap_bytes: u64,
        audit_max_bytes: u64,
        audit_max_rotated_files: usize,
        anomaly_alert_threshold: u64,
        audit_export_limit: usize,
        denylist_routes: Vec<String>,
    ) -> Self {
        Self { inner: std::sync::RwLock::new(HealthStateData {
            audit_path, audit_rate_window_ms, query_rate_limit, mutation_rate_limit,
            upload_cap_bytes, audit_max_bytes, audit_max_rotated_files,
            rate_windows: BTreeMap::new(), anomaly_alert_threshold, audit_export_limit, denylist_routes,
        }) }
    }
    /// Lock for read access.
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, HealthStateData> { self.inner.read().unwrap() }
    /// Lock for write access.
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, HealthStateData> { self.inner.write().unwrap() }
}

/// Shared workflow runtime state, decomposed into fine-grained sub-structs
/// each owning its own Mutex/RwLock for reduced lock contention (S10-T8).
pub struct RuntimeState {
    /// Worklist store.
    pub worklist: WorklistStore,
    /// MPPS service.
    pub mpps: MppsService,
    /// SR workflow store.
    pub sr: SrWorkflowStore,
    /// HL7-related state.
    pub hl7: Hl7State,
    /// Tenant management state.
    pub tenant: TenantState,
    /// Worker pool state.
    pub worker: WorkerState,
    /// Health check and audit state.
    pub health: HealthState,
}

#[derive(Debug, Clone)]
pub struct Hl7TransportConfig {
    pub mllp_enabled: bool,
    pub mllp_bind: Option<String>,
    pub file_drop_dir: Option<PathBuf>,
    pub file_drop_done_dir: Option<PathBuf>,
    pub file_drop_error_dir: Option<PathBuf>,
    pub file_drop_poll_interval_ms: u64,
}

impl Default for Hl7TransportConfig {
    /// Default HL7 transport: MLLP disabled, file-drop disabled (S13-T5).
    fn default() -> Self {
        Self {
            mllp_enabled: false,
            mllp_bind: None,
            file_drop_dir: None,
            file_drop_done_dir: None,
            file_drop_error_dir: None,
            file_drop_poll_interval_ms: DEFAULT_HL7_FILE_DROP_POLL_INTERVAL_MS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedMppsRequest {
    pub signature: String,
    pub response: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskStatus {
    Scheduled,
    InProgress,
    OnHold,
    Completed,
    Reviewed,
    Committed,
    Discontinued,
}

#[derive(Debug, Clone)]
pub struct ProcedureTask {
    pub task_id: String,
    pub scheduled_step_id: String,
    pub tenant: String,
    pub requested_procedure_id: Option<String>,
    pub status: TaskStatus,
    pub worker: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

/// Parsed HTTP request.
pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_string()
}

pub fn workflow_actor_context(headers: &BTreeMap<String, String>) -> WorkflowActorContext {
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

pub fn workflow_event_source(actor: &WorkflowActorContext) -> String {
    actor
        .principal
        .clone()
        .unwrap_or_else(|| "workflow".to_string())
}

pub fn tenant_env_suffix(raw_tenant: &str) -> String {
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

pub fn tenant_quota_override_for_tenant(tenant: &str) -> Option<TenantQuotaOverride> {
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

pub fn collect_tenant_quota_overrides_from_env() -> BTreeMap<String, TenantQuotaOverride> {
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

pub fn tenant_rate_limit_override_cache() -> &'static Mutex<BTreeMap<String, TenantRateLimitOverride>> {
    TENANT_RATE_LIMIT_OVERRIDE_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn set_tenant_rate_limit_override_cache(overrides: BTreeMap<String, TenantRateLimitOverride>) {
    if let Ok(mut cache) = tenant_rate_limit_override_cache().lock() {
        *cache = overrides;
    }
}

pub fn collect_tenant_rate_limit_overrides_from_env(
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

pub fn tenant_rate_limit_override_for_tenant(tenant: &str) -> Option<TenantRateLimitOverride> {
    let tenant_key = tenant_env_suffix(tenant).to_ascii_lowercase();
    let cache = tenant_rate_limit_override_cache().lock().ok()?;
    cache.get(&tenant_key).copied()
}

pub fn tenant_rate_limit_overrides_snapshot() -> BTreeMap<String, TenantRateLimitOverride> {
    tenant_rate_limit_override_cache()
        .lock()
        .map(|cache| cache.clone())
        .unwrap_or_default()
}

pub fn tenant_policy(state: &RuntimeState, tenant: &str) -> TenantWorkflowPolicy {
    let mut policy = TenantWorkflowPolicy {
        query_rate_limit: state.health.read().query_rate_limit,
        mutation_rate_limit: state.health.read().mutation_rate_limit,
        upload_cap_bytes: state.health.read().upload_cap_bytes,
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


pub fn auth_denied_error(detail: &str) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-workflow-server-auth".to_string(),
            detail: detail.to_string(),
        },
        "auth_denied",
    )
    .into()
}

#[allow(missing_docs)]
pub fn not_found_error(detail: &str, code: &str) -> Box<Error> {
    Error::from_kind(
        ErrorKind::NotFound {
            detail: detail.to_string(),
        },
        code,
    )
    .into()
}

pub fn route_path_normalize(raw_path: &str) -> Result<String, Box<Error>> {
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

pub fn workflow_role_is_writer(role: &str) -> bool {
    matches!(role, "writer" | "admin" | "operator")
}

/// Insert an id into a per-tenant index set (S13-T6: BTreeSet for O(log n)).
pub fn route_id_to_tenant_index(
    tenant_indexes: &mut BTreeMap<String, BTreeSet<String>>,
    tenant: &str,
    id: &str,
) {
    let bucket = tenant_indexes.entry(tenant.to_string()).or_default();
    bucket.insert(id.to_string());
}

/// Check whether a per-tenant index set contains an id (S13-T6: O(log n)).
pub fn tenant_indexes_contains(
    tenant_indexes: &BTreeMap<String, BTreeSet<String>>,
    tenant: &str,
    id: &str,
) -> bool {
    tenant_indexes
        .get(tenant)
        .is_some_and(|entries| entries.contains(id))
}

/// Find which tenant owns a given id (S13-T6: O(log n) per tenant bucket).
pub fn tenant_of_id(tenant_indexes: &BTreeMap<String, BTreeSet<String>>, id: &str) -> Option<String> {
    for (tenant, entries) in tenant_indexes {
        if entries.contains(id) {
            return Some(tenant.clone());
        }
    }
    None
}

/// Check whether an actor's tenant owns a given resource (S13-T6: O(log n)).
pub fn actor_has_resource_access(
    actor: &WorkflowActorContext,
    indexes: &BTreeMap<String, BTreeSet<String>>,
    resource_id: &str,
) -> bool {
    tenant_indexes_contains(indexes, &actor.tenant, resource_id)
}

pub fn touch_tenant_index(state: &mut RuntimeState, tenant: &str) {
    let _ = state.tenant.lock().tenant_worklist.entry(tenant.to_string()).or_default();
    let _ = state.tenant.lock().tenant_mpps.entry(tenant.to_string()).or_default();
    let _ = state.tenant.lock().tenant_sr.entry(tenant.to_string()).or_default();
    let _ = state.tenant.lock().tenant_tasks.entry(tenant.to_string()).or_default();
}

pub fn ensure_tenant_indexes_initialized(state: &mut RuntimeState) {
    if !state.tenant.lock().tenant_worklist.is_empty()
        && !state.tenant.lock().tenant_mpps.is_empty()
        && !state.tenant.lock().tenant_sr.is_empty()
        && !state.tenant.lock().tenant_tasks.is_empty()
    {
        return;
    }

    state.tenant.lock().tenant_worklist.clear();
    state.tenant.lock().tenant_mpps.clear();
    state.tenant.lock().tenant_sr.clear();
    state.tenant.lock().tenant_tasks.clear();

    let default_tenant = TENANT_ID_DEFAULT.to_string();
    touch_tenant_index(state, &default_tenant);

    for task in state.worker.lock().tasks.values() {
        let tenant = if task.tenant.is_empty() {
            default_tenant.clone()
        } else {
            task.tenant.clone()
        };
        route_id_to_tenant_index(&mut state.tenant.lock().tenant_tasks, &tenant, &task.task_id);
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
                        .and_then(|element| match element.value() {
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
        route_id_to_tenant_index(&mut state.tenant.lock().tenant_worklist, &default_tenant, &step_id);
    }

    let mpps_ids = state.mpps.all_updates();
    for update in mpps_ids {
        route_id_to_tenant_index(
            &mut state.tenant.lock().tenant_mpps,
            &default_tenant,
            &update.sop_instance_uid,
        );
    }

    for document in state.sr.documents() {
        route_id_to_tenant_index(
            &mut state.tenant.lock().tenant_sr,
            &default_tenant,
            &document.provenance.sop_instance_uid,
        );
    }
}

pub fn secret_dir_root(raw_secret_dir: Option<&str>) -> std::io::Result<Option<PathBuf>> {
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

pub fn resolve_secret_path(secret_dir: Option<&Path>, raw_path: &str) -> std::io::Result<PathBuf> {
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

pub fn load_secret_value(
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

pub fn secret_path_with_default(
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

pub fn hash_text(value: &str) -> String {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    pub const FNV_PRIME: u64 = 1_099_511_628_211;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

pub fn parse_denylist_routes(raw_routes: Option<String>) -> Vec<String> {
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

pub fn environment_parse_error(var_name: &str, parsed_value: &str, detail: &str) -> IoError {
    IoError::new(
        IoErrorKind::InvalidInput,
        format!(
            "service={WORKFLOW_SERVICE_NAME} var={var_name} parsed_value={parsed_value} detail={detail}",
        ),
    )
}

pub fn parse_log_level(name: &str) -> std::io::Result<WorkflowLogLevel> {
    match parse_optional_string(WORKFLOW_SERVICE_NAME, name)? {
        Some(raw) => WorkflowLogLevel::parse(&raw).ok_or_else(|| {
            environment_parse_error(name, raw.trim(), "must be off/error/warn/info/debug/trace")
        }),
        None => Ok(WorkflowLogLevel::Info),
    }
}

pub fn lookup_identifier_from_map(values: &BTreeMap<String, String>, keys: &[&str]) -> Option<String> {
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

pub fn required_identifier_from_map(
    values: &BTreeMap<String, String>,
    keys: &[&str],
    context: &str,
) -> Result<String, Box<Error>> {
    lookup_identifier_from_map(values, keys)
        .ok_or_else(|| decode_error(format!("missing required identifier: {context}")))
}

pub fn normalize_connector_adapter_kind(alias: &str, target: &str) -> NormalizedConnectorAdapterKind {
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

pub fn normalized_connector_descriptors(state: &RuntimeState) -> Vec<NormalizedConnectorAdapter> {
    let mut out = Vec::new();
    for (alias, target) in &state.hl7.lock().connector_registry {
        let plugin = state.hl7.lock().connector_plugins.get(alias).cloned();
        out.push(NormalizedConnectorAdapter {
            alias: alias.clone(),
            target: target.clone(),
            adapter_kind: normalize_connector_adapter_kind(alias, target),
            plugin,
        });
    }
    out
}

pub fn parse_hl7_connector_plugin_path(var_name: &str, raw: &str) -> std::io::Result<String> {
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

pub fn parse_semver_triplet(raw: &str) -> std::io::Result<(u64, u64, u64)> {
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

pub fn validate_hl7_connector_plugin_compatibility(
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

pub fn parse_hl7_callback_policy_from_env() -> std::io::Result<Hl7CallbackPolicy> {
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

pub fn parse_hl7_connector_plugins() -> std::io::Result<BTreeMap<String, ConnectorPluginMetadata>> {
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

pub fn parse_hl7_connector_registry() -> BTreeMap<String, String> {
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

pub fn parse_hl7_connector_feature_flags() -> std::io::Result<BTreeMap<String, Hl7ConnectorFeatureFlag>>
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

pub fn parse_hl7_connector_rollout_percents() -> std::io::Result<BTreeMap<String, u64>> {
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

pub fn parse_hl7_transport_dir(var_name: &str, raw: &str) -> std::io::Result<PathBuf> {
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

pub fn parse_hl7_mllp_bind(raw: &str) -> std::io::Result<String> {
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

pub fn parse_hl7_transport_config() -> std::io::Result<Hl7TransportConfig> {
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

pub fn normalize_connector_alias(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase();
    if let Some(prefix) = normalized.strip_suffix("_star") {
        format!("{}*", prefix.replace('_', ""))
    } else {
        normalized.replace('_', "")
    }
}

pub fn resolve_hl7_connector_alias(
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

pub fn resolve_hl7_connector_setting<T: Copy>(
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

pub fn resolve_hl7_connector_rollout_percent(
    rollout_percents: &BTreeMap<String, u64>,
    alias: &str,
) -> u64 {
    resolve_hl7_connector_setting(
        rollout_percents,
        alias,
        DEFAULT_HL7_CONNECTOR_ROLLOUT_PERCENT,
    )
}

pub fn hl7_connector_rollout_bucket(event_id: &str, alias: &str) -> u64 {
    let raw = hash_text(&format!("{alias}|{event_id}"));
    u64::from_str_radix(&raw[..16], 16).unwrap_or_default() % 100
}

pub fn hl7_connector_rollout_allows(event_id: &str, alias: &str, rollout_percent: u64) -> bool {
    if rollout_percent >= 100 {
        return true;
    }
    if rollout_percent == 0 {
        return false;
    }
    hl7_connector_rollout_bucket(event_id, alias) < rollout_percent
}

pub fn normalize_route_identifier(raw: &str) -> Result<String, Box<Error>> {
    let normalized = normalize_identifier(raw);
    if normalized.is_empty() || normalized.contains('/') {
        return Err(decode_error("invalid identifier in route"));
    }
    if normalized.contains(' ') {
        return Err(decode_error("identifier contains embedded whitespace"));
    }
    Ok(normalized)
}

pub fn resolve_hl7_connector_feature_flag(
    feature_flags: &BTreeMap<String, Hl7ConnectorFeatureFlag>,
    alias: &str,
) -> bool {
    resolve_hl7_connector_setting(feature_flags, alias, Hl7ConnectorFeatureFlag::Enabled).as_bool()
}

pub fn json_escape(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

