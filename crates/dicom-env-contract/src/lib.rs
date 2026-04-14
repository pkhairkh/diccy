#![deny(missing_docs)]

//! Shared runtime environment contract helpers for service startup validation.

use std::{collections::BTreeSet, env, io};

/// Canonical env var that selects the active conformance envelope version.
pub const DICOM_ENVELOPE_VERSION: &str = "DICOM_ENVELOPE_VERSION";

/// Supported envelope versions for strict validation at runtime.
pub const SUPPORTED_DICOM_ENVELOPE_VERSIONS: &[&str] = &["1.1"];

/// Default envelope version used when `DICOM_ENVELOPE_VERSION` is unset.
pub const DEFAULT_DICOM_ENVELOPE_VERSION: &str = "1.1";

/// Canonical environment prefix for `dicom-web-server` keys.
pub const DICOM_WEB_ENV_PREFIX: &str = "DICOM_WEB_";

/// Canonical environment prefix for `dicom-workflow-server` keys.
pub const DICOM_WORKFLOW_ENV_PREFIX: &str = "DICOM_WORKFLOW_";

/// Canonical environment prefix for `dicom-dimse-service` keys.
pub const DICOM_DIMSE_ENV_PREFIX: &str = "DICOM_DIMSE_";

/// Compile-time typed wrapper for `dicom-web-server` environment keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebEnvVar(&'static str);

impl WebEnvVar {
    /// Return the canonical environment key name.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Compile-time typed wrapper for `dicom-workflow-server` environment keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowEnvVar(&'static str);

impl WorkflowEnvVar {
    /// Return the canonical environment key name.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Compile-time typed wrapper for `dicom-workflow-server` dynamic key prefixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkflowEnvPrefix(&'static str);

impl WorkflowEnvPrefix {
    /// Return the canonical environment key prefix.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Compile-time typed wrapper for `dicom-dimse-service` environment keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimseEnvVar(&'static str);

impl DimseEnvVar {
    /// Return the canonical environment key name.
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

const WEB_ENV_VARS_PROD: &[WebEnvVar] = &[
    WebEnvVar("DICOM_ENVELOPE_VERSION"),
    WebEnvVar("DICOM_WEB_ACCEPT_QUEUE_DEPTH"),
    WebEnvVar("DICOM_WEB_AUTH_MODE"),
    WebEnvVar("DICOM_WEB_BIND"),
    WebEnvVar("DICOM_WEB_ENABLE_QIDO"),
    WebEnvVar("DICOM_WEB_ENABLE_STOW"),
    WebEnvVar("DICOM_WEB_ENABLE_WADO"),
    WebEnvVar("DICOM_WEB_LOG_LEVEL"),
    WebEnvVar("DICOM_WEB_QIDO_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_QIDO_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STOW_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STOW_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL_MAX_BYTES"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES"),
    WebEnvVar("DICOM_WEB_TELEMETRY_ENABLED"),
    WebEnvVar("DICOM_WEB_TELEMETRY_SAFE_SUBSET"),
    WebEnvVar("DICOM_WEB_TLS_POLICY"),
    WebEnvVar("DICOM_WEB_TRANSPORT_SECURITY"),
    WebEnvVar("DICOM_WEB_WADO_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_WADO_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_WORKERS"),
];

const WEB_ENV_VARS_TEST: &[WebEnvVar] = &[
    WebEnvVar("DICOM_ENVELOPE_VERSION"),
    WebEnvVar("DICOM_WEB_ACCEPT_QUEUE_DEPTH"),
    WebEnvVar("DICOM_WEB_AUTH_MODE"),
    WebEnvVar("DICOM_WEB_BIND"),
    WebEnvVar("DICOM_WEB_ENABLE_QIDO"),
    WebEnvVar("DICOM_WEB_ENABLE_STOW"),
    WebEnvVar("DICOM_WEB_ENABLE_WADO"),
    WebEnvVar("DICOM_WEB_LOG_LEVEL"),
    WebEnvVar("DICOM_WEB_QIDO_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_QIDO_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STOW_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STOW_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL_MAX_BYTES"),
    WebEnvVar("DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES"),
    WebEnvVar("DICOM_WEB_TEST_STORAGE_BYTES"),
    WebEnvVar("DICOM_WEB_TEST_WORKERS"),
    WebEnvVar("DICOM_WEB_TELEMETRY_ENABLED"),
    WebEnvVar("DICOM_WEB_TELEMETRY_SAFE_SUBSET"),
    WebEnvVar("DICOM_WEB_TLS_POLICY"),
    WebEnvVar("DICOM_WEB_TRANSPORT_SECURITY"),
    WebEnvVar("DICOM_WEB_WADO_P95_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_WADO_P99_LATENCY_MS"),
    WebEnvVar("DICOM_WEB_WORKERS"),
];

const WEB_ALLOWED_EXACT_PROD: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_WEB_ACCEPT_QUEUE_DEPTH",
    "DICOM_WEB_AUTH_MODE",
    "DICOM_WEB_BIND",
    "DICOM_WEB_ENABLE_QIDO",
    "DICOM_WEB_ENABLE_STOW",
    "DICOM_WEB_ENABLE_WADO",
    "DICOM_WEB_LOG_LEVEL",
    "DICOM_WEB_QIDO_P95_LATENCY_MS",
    "DICOM_WEB_QIDO_P99_LATENCY_MS",
    "DICOM_WEB_STOW_P95_LATENCY_MS",
    "DICOM_WEB_STOW_P99_LATENCY_MS",
    "DICOM_WEB_STORAGE_WAL",
    "DICOM_WEB_STORAGE_WAL_MAX_BYTES",
    "DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES",
    "DICOM_WEB_TELEMETRY_ENABLED",
    "DICOM_WEB_TELEMETRY_SAFE_SUBSET",
    "DICOM_WEB_TLS_POLICY",
    "DICOM_WEB_TRANSPORT_SECURITY",
    "DICOM_WEB_WADO_P95_LATENCY_MS",
    "DICOM_WEB_WADO_P99_LATENCY_MS",
    "DICOM_WEB_WORKERS",
];

const WEB_ALLOWED_EXACT_TEST: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_WEB_ACCEPT_QUEUE_DEPTH",
    "DICOM_WEB_AUTH_MODE",
    "DICOM_WEB_BIND",
    "DICOM_WEB_ENABLE_QIDO",
    "DICOM_WEB_ENABLE_STOW",
    "DICOM_WEB_ENABLE_WADO",
    "DICOM_WEB_LOG_LEVEL",
    "DICOM_WEB_QIDO_P95_LATENCY_MS",
    "DICOM_WEB_QIDO_P99_LATENCY_MS",
    "DICOM_WEB_STOW_P95_LATENCY_MS",
    "DICOM_WEB_STOW_P99_LATENCY_MS",
    "DICOM_WEB_STORAGE_WAL",
    "DICOM_WEB_STORAGE_WAL_MAX_BYTES",
    "DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES",
    "DICOM_WEB_TEST_STORAGE_BYTES",
    "DICOM_WEB_TEST_WORKERS",
    "DICOM_WEB_TELEMETRY_ENABLED",
    "DICOM_WEB_TELEMETRY_SAFE_SUBSET",
    "DICOM_WEB_TLS_POLICY",
    "DICOM_WEB_TRANSPORT_SECURITY",
    "DICOM_WEB_WADO_P95_LATENCY_MS",
    "DICOM_WEB_WADO_P99_LATENCY_MS",
    "DICOM_WEB_WORKERS",
];

const WORKFLOW_ENV_VARS_PROD: &[WorkflowEnvVar] = &[
    WorkflowEnvVar("DICOM_ENVELOPE_VERSION"),
    WorkflowEnvVar("DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_MAX_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUTH_MODE"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUTH_TOKEN_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_BIND"),
    WorkflowEnvVar("DICOM_WORKFLOW_DENYLIST_PATHS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_MLLP_BIND"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_MLLP_ENABLED"),
    WorkflowEnvVar("DICOM_WORKFLOW_FHIR_INGEST_ENABLED"),
    WorkflowEnvVar("DICOM_WORKFLOW_LOG_LEVEL"),
    WorkflowEnvVar("DICOM_WORKFLOW_MPPS_STATE_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_MUTATION_RATE_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_QUERY_RATE_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_SECRET_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES"),
    WorkflowEnvVar("DICOM_WORKFLOW_SR_AUDIT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_SR_STATE_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TLS_CERT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TLS_KEY_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TRANSPORT_SECURITY"),
    WorkflowEnvVar("DICOM_WORKFLOW_UPLOAD_CAP_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_WORKLIST_STATE_PATH"),
];

const WORKFLOW_ENV_VARS_TEST: &[WorkflowEnvVar] = &[
    WorkflowEnvVar("DICOM_ENVELOPE_VERSION"),
    WorkflowEnvVar("DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_MAX_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUDIT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUTH_MODE"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUTH_TOKEN"),
    WorkflowEnvVar("DICOM_WORKFLOW_AUTH_TOKEN_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_BIND"),
    WorkflowEnvVar("DICOM_WORKFLOW_DENYLIST_PATHS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_MLLP_BIND"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_MLLP_ENABLED"),
    WorkflowEnvVar("DICOM_WORKFLOW_FHIR_INGEST_ENABLED"),
    WorkflowEnvVar("DICOM_WORKFLOW_LOG_LEVEL"),
    WorkflowEnvVar("DICOM_WORKFLOW_MPPS_STATE_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_MUTATION_RATE_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_QUERY_RATE_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS"),
    WorkflowEnvVar("DICOM_WORKFLOW_SECRET_DIR"),
    WorkflowEnvVar("DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES"),
    WorkflowEnvVar("DICOM_WORKFLOW_SR_AUDIT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_SR_STATE_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TEST_RATE_LIMIT"),
    WorkflowEnvVar("DICOM_WORKFLOW_TEST_ROTATIONS"),
    WorkflowEnvVar("DICOM_WORKFLOW_TLS_CERT_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TLS_KEY_PATH"),
    WorkflowEnvVar("DICOM_WORKFLOW_TRANSPORT_SECURITY"),
    WorkflowEnvVar("DICOM_WORKFLOW_UPLOAD_CAP_BYTES"),
    WorkflowEnvVar("DICOM_WORKFLOW_WORKLIST_STATE_PATH"),
];

const WORKFLOW_ALLOWED_EXACT_PROD: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD",
    "DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT",
    "DICOM_WORKFLOW_AUDIT_MAX_BYTES",
    "DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES",
    "DICOM_WORKFLOW_AUDIT_PATH",
    "DICOM_WORKFLOW_AUTH_MODE",
    "DICOM_WORKFLOW_AUTH_TOKEN_PATH",
    "DICOM_WORKFLOW_BIND",
    "DICOM_WORKFLOW_DENYLIST_PATHS",
    "DICOM_WORKFLOW_HL7_FILE_DROP_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS",
    "DICOM_WORKFLOW_HL7_MLLP_BIND",
    "DICOM_WORKFLOW_HL7_MLLP_ENABLED",
    "DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
    "DICOM_WORKFLOW_LOG_LEVEL",
    "DICOM_WORKFLOW_MPPS_STATE_PATH",
    "DICOM_WORKFLOW_MUTATION_RATE_LIMIT",
    "DICOM_WORKFLOW_QUERY_RATE_LIMIT",
    "DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS",
    "DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS",
    "DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD",
    "DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS",
    "DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS",
    "DICOM_WORKFLOW_SECRET_DIR",
    "DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES",
    "DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES",
    "DICOM_WORKFLOW_SR_AUDIT_PATH",
    "DICOM_WORKFLOW_SR_STATE_PATH",
    "DICOM_WORKFLOW_TLS_CERT_PATH",
    "DICOM_WORKFLOW_TLS_KEY_PATH",
    "DICOM_WORKFLOW_TRANSPORT_SECURITY",
    "DICOM_WORKFLOW_UPLOAD_CAP_BYTES",
    "DICOM_WORKFLOW_WORKLIST_STATE_PATH",
];

const WORKFLOW_ALLOWED_EXACT_TEST: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD",
    "DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT",
    "DICOM_WORKFLOW_AUDIT_MAX_BYTES",
    "DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES",
    "DICOM_WORKFLOW_AUDIT_PATH",
    "DICOM_WORKFLOW_AUTH_MODE",
    "DICOM_WORKFLOW_AUTH_TOKEN",
    "DICOM_WORKFLOW_AUTH_TOKEN_PATH",
    "DICOM_WORKFLOW_BIND",
    "DICOM_WORKFLOW_DENYLIST_PATHS",
    "DICOM_WORKFLOW_HL7_FILE_DROP_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR",
    "DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS",
    "DICOM_WORKFLOW_HL7_MLLP_BIND",
    "DICOM_WORKFLOW_HL7_MLLP_ENABLED",
    "DICOM_WORKFLOW_FHIR_INGEST_ENABLED",
    "DICOM_WORKFLOW_LOG_LEVEL",
    "DICOM_WORKFLOW_MPPS_STATE_PATH",
    "DICOM_WORKFLOW_MUTATION_RATE_LIMIT",
    "DICOM_WORKFLOW_QUERY_RATE_LIMIT",
    "DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS",
    "DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS",
    "DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD",
    "DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS",
    "DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS",
    "DICOM_WORKFLOW_SECRET_DIR",
    "DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES",
    "DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES",
    "DICOM_WORKFLOW_SR_AUDIT_PATH",
    "DICOM_WORKFLOW_SR_STATE_PATH",
    "DICOM_WORKFLOW_TEST_RATE_LIMIT",
    "DICOM_WORKFLOW_TEST_ROTATIONS",
    "DICOM_WORKFLOW_TLS_CERT_PATH",
    "DICOM_WORKFLOW_TLS_KEY_PATH",
    "DICOM_WORKFLOW_TRANSPORT_SECURITY",
    "DICOM_WORKFLOW_UPLOAD_CAP_BYTES",
    "DICOM_WORKFLOW_WORKLIST_STATE_PATH",
];

const WORKFLOW_ENV_PREFIX_VARS: &[WorkflowEnvPrefix] = &[
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_TENANT_QUERY_RATE_LIMIT_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_TENANT_MUTATION_RATE_LIMIT_"),
    WorkflowEnvPrefix("DICOM_WORKFLOW_TENANT_UPLOAD_CAP_BYTES_"),
];

const WORKFLOW_ALLOWED_PREFIXES: &[&str] = &[
    "DICOM_WORKFLOW_HL7_CONNECTOR_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_",
    "DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_",
    "DICOM_WORKFLOW_TENANT_QUERY_RATE_LIMIT_",
    "DICOM_WORKFLOW_TENANT_MUTATION_RATE_LIMIT_",
    "DICOM_WORKFLOW_TENANT_UPLOAD_CAP_BYTES_",
];

const DIMSE_ENV_VARS_PROD: &[DimseEnvVar] = &[
    DimseEnvVar("DICOM_ENVELOPE_VERSION"),
    DimseEnvVar("DICOM_DIMSE_ALLOWED_HOSTS"),
    DimseEnvVar("DICOM_DIMSE_AUTH_MODE"),
    DimseEnvVar("DICOM_DIMSE_BIND"),
    DimseEnvVar("DICOM_DIMSE_CALLED_AE"),
    DimseEnvVar("DICOM_DIMSE_HEALTH_BIND"),
    DimseEnvVar("DICOM_DIMSE_LOG_LEVEL"),
    DimseEnvVar("DICOM_DIMSE_MAX_CACHE_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_COMMAND_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_CONNECTIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_DATASET_ELEMENTS"),
    DimseEnvVar("DICOM_DIMSE_MAX_DECOMPRESSED_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_ELEMENT_VL_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE"),
    DimseEnvVar("DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_INPUT_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_PDU_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_PDV_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_PIXELS_PER_FRAME"),
    DimseEnvVar("DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS"),
    DimseEnvVar("DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT"),
    DimseEnvVar("DICOM_DIMSE_MAX_SEQUENCE_DEPTH"),
    DimseEnvVar("DICOM_DIMSE_MAX_STRING_BYTES"),
    DimseEnvVar("DICOM_DIMSE_READ_TIMEOUT_SECS"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_ECHO_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_FIND_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_GET_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_MOVE_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_STORE_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL_MAX_BYTES"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES"),
    DimseEnvVar("DICOM_DIMSE_TELEMETRY_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_TELEMETRY_SAFE_SUBSET"),
    DimseEnvVar("DICOM_DIMSE_TLS_CA_BUNDLE_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_CERT_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS"),
    DimseEnvVar("DICOM_DIMSE_TLS_KEY_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_POLICY"),
    DimseEnvVar("DICOM_DIMSE_TRANSPORT_SECURITY"),
    DimseEnvVar("DICOM_DIMSE_WRITE_TIMEOUT_SECS"),
];

const DIMSE_ENV_VARS_TEST: &[DimseEnvVar] = &[
    DimseEnvVar("DICOM_ENVELOPE_VERSION"),
    DimseEnvVar("DICOM_DIMSE_ALLOWED_HOSTS"),
    DimseEnvVar("DICOM_DIMSE_AUTH_MODE"),
    DimseEnvVar("DICOM_DIMSE_BIND"),
    DimseEnvVar("DICOM_DIMSE_CALLED_AE"),
    DimseEnvVar("DICOM_DIMSE_HEALTH_BIND"),
    DimseEnvVar("DICOM_DIMSE_LOG_LEVEL"),
    DimseEnvVar("DICOM_DIMSE_MAX_CACHE_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_COMMAND_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_CONNECTIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_DATASET_ELEMENTS"),
    DimseEnvVar("DICOM_DIMSE_MAX_DECOMPRESSED_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_ELEMENT_VL_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE"),
    DimseEnvVar("DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_INPUT_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS"),
    DimseEnvVar("DICOM_DIMSE_MAX_PDU_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_PDV_BYTES"),
    DimseEnvVar("DICOM_DIMSE_MAX_PIXELS_PER_FRAME"),
    DimseEnvVar("DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS"),
    DimseEnvVar("DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT"),
    DimseEnvVar("DICOM_DIMSE_MAX_SEQUENCE_DEPTH"),
    DimseEnvVar("DICOM_DIMSE_MAX_STRING_BYTES"),
    DimseEnvVar("DICOM_DIMSE_READ_TIMEOUT_SECS"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_ECHO_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_FIND_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_GET_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_MOVE_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_ROLE_C_STORE_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL_MAX_BYTES"),
    DimseEnvVar("DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES"),
    DimseEnvVar("DICOM_DIMSE_TEST_MAX_BYTES"),
    DimseEnvVar("DICOM_DIMSE_TEST_OPTIONAL_SECONDS"),
    DimseEnvVar("DICOM_DIMSE_TELEMETRY_ENABLED"),
    DimseEnvVar("DICOM_DIMSE_TELEMETRY_SAFE_SUBSET"),
    DimseEnvVar("DICOM_DIMSE_TLS_CA_BUNDLE_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_CERT_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS"),
    DimseEnvVar("DICOM_DIMSE_TLS_KEY_PATH"),
    DimseEnvVar("DICOM_DIMSE_TLS_POLICY"),
    DimseEnvVar("DICOM_DIMSE_TRANSPORT_SECURITY"),
    DimseEnvVar("DICOM_DIMSE_WRITE_TIMEOUT_SECS"),
];

const DIMSE_ALLOWED_EXACT_PROD: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_DIMSE_ALLOWED_HOSTS",
    "DICOM_DIMSE_AUTH_MODE",
    "DICOM_DIMSE_BIND",
    "DICOM_DIMSE_CALLED_AE",
    "DICOM_DIMSE_HEALTH_BIND",
    "DICOM_DIMSE_LOG_LEVEL",
    "DICOM_DIMSE_MAX_CACHE_BYTES",
    "DICOM_DIMSE_MAX_COMMAND_BYTES",
    "DICOM_DIMSE_MAX_CONNECTIONS",
    "DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_DATASET_ELEMENTS",
    "DICOM_DIMSE_MAX_DECOMPRESSED_BYTES",
    "DICOM_DIMSE_MAX_ELEMENT_VL_BYTES",
    "DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE",
    "DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES",
    "DICOM_DIMSE_MAX_INPUT_BYTES",
    "DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS",
    "DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS",
    "DICOM_DIMSE_MAX_PDU_BYTES",
    "DICOM_DIMSE_MAX_PDV_BYTES",
    "DICOM_DIMSE_MAX_PIXELS_PER_FRAME",
    "DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS",
    "DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT",
    "DICOM_DIMSE_MAX_SEQUENCE_DEPTH",
    "DICOM_DIMSE_MAX_STRING_BYTES",
    "DICOM_DIMSE_READ_TIMEOUT_SECS",
    "DICOM_DIMSE_ROLE_C_ECHO_ENABLED",
    "DICOM_DIMSE_ROLE_C_FIND_ENABLED",
    "DICOM_DIMSE_ROLE_C_GET_ENABLED",
    "DICOM_DIMSE_ROLE_C_MOVE_ENABLED",
    "DICOM_DIMSE_ROLE_C_STORE_ENABLED",
    "DICOM_DIMSE_STORAGE_WAL",
    "DICOM_DIMSE_STORAGE_WAL_MAX_BYTES",
    "DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES",
    "DICOM_DIMSE_TELEMETRY_ENABLED",
    "DICOM_DIMSE_TELEMETRY_SAFE_SUBSET",
    "DICOM_DIMSE_TLS_CA_BUNDLE_PATH",
    "DICOM_DIMSE_TLS_CERT_PATH",
    "DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS",
    "DICOM_DIMSE_TLS_KEY_PATH",
    "DICOM_DIMSE_TLS_POLICY",
    "DICOM_DIMSE_TRANSPORT_SECURITY",
    "DICOM_DIMSE_WRITE_TIMEOUT_SECS",
];

const DIMSE_ALLOWED_EXACT_TEST: &[&str] = &[
    "DICOM_ENVELOPE_VERSION",
    "DICOM_DIMSE_ALLOWED_HOSTS",
    "DICOM_DIMSE_AUTH_MODE",
    "DICOM_DIMSE_BIND",
    "DICOM_DIMSE_CALLED_AE",
    "DICOM_DIMSE_HEALTH_BIND",
    "DICOM_DIMSE_LOG_LEVEL",
    "DICOM_DIMSE_MAX_CACHE_BYTES",
    "DICOM_DIMSE_MAX_COMMAND_BYTES",
    "DICOM_DIMSE_MAX_CONNECTIONS",
    "DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES",
    "DICOM_DIMSE_MAX_DATASET_ELEMENTS",
    "DICOM_DIMSE_MAX_DECOMPRESSED_BYTES",
    "DICOM_DIMSE_MAX_ELEMENT_VL_BYTES",
    "DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE",
    "DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES",
    "DICOM_DIMSE_MAX_INPUT_BYTES",
    "DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS",
    "DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS",
    "DICOM_DIMSE_MAX_PDU_BYTES",
    "DICOM_DIMSE_MAX_PDV_BYTES",
    "DICOM_DIMSE_MAX_PIXELS_PER_FRAME",
    "DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS",
    "DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT",
    "DICOM_DIMSE_MAX_SEQUENCE_DEPTH",
    "DICOM_DIMSE_MAX_STRING_BYTES",
    "DICOM_DIMSE_READ_TIMEOUT_SECS",
    "DICOM_DIMSE_ROLE_C_ECHO_ENABLED",
    "DICOM_DIMSE_ROLE_C_FIND_ENABLED",
    "DICOM_DIMSE_ROLE_C_GET_ENABLED",
    "DICOM_DIMSE_ROLE_C_MOVE_ENABLED",
    "DICOM_DIMSE_ROLE_C_STORE_ENABLED",
    "DICOM_DIMSE_STORAGE_WAL",
    "DICOM_DIMSE_STORAGE_WAL_MAX_BYTES",
    "DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES",
    "DICOM_DIMSE_TEST_MAX_BYTES",
    "DICOM_DIMSE_TEST_OPTIONAL_SECONDS",
    "DICOM_DIMSE_TELEMETRY_ENABLED",
    "DICOM_DIMSE_TELEMETRY_SAFE_SUBSET",
    "DICOM_DIMSE_TLS_CA_BUNDLE_PATH",
    "DICOM_DIMSE_TLS_CERT_PATH",
    "DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS",
    "DICOM_DIMSE_TLS_KEY_PATH",
    "DICOM_DIMSE_TLS_POLICY",
    "DICOM_DIMSE_TRANSPORT_SECURITY",
    "DICOM_DIMSE_WRITE_TIMEOUT_SECS",
];

/// Return typed wrappers for all `dicom-web-server` environment keys.
pub fn dicom_web_env_vars(include_test_vars: bool) -> &'static [WebEnvVar] {
    if include_test_vars {
        WEB_ENV_VARS_TEST
    } else {
        WEB_ENV_VARS_PROD
    }
}

/// Build the canonical runtime contract for `dicom-web-server`.
pub fn dicom_web_env_contract(include_test_vars: bool) -> EnvContract<'static> {
    let allowed_exact = if include_test_vars {
        WEB_ALLOWED_EXACT_TEST
    } else {
        WEB_ALLOWED_EXACT_PROD
    };
    EnvContract::new("dicom-web-server", DICOM_WEB_ENV_PREFIX, allowed_exact, &[])
}

/// Return typed wrappers for all `dicom-workflow-server` exact environment keys.
pub fn dicom_workflow_env_vars(include_test_vars: bool) -> &'static [WorkflowEnvVar] {
    if include_test_vars {
        WORKFLOW_ENV_VARS_TEST
    } else {
        WORKFLOW_ENV_VARS_PROD
    }
}

/// Return typed wrappers for dynamic `dicom-workflow-server` key prefixes.
pub fn dicom_workflow_env_prefixes() -> &'static [WorkflowEnvPrefix] {
    WORKFLOW_ENV_PREFIX_VARS
}

/// Build the canonical runtime contract for `dicom-workflow-server`.
pub fn dicom_workflow_env_contract(include_test_vars: bool) -> EnvContract<'static> {
    let allowed_exact = if include_test_vars {
        WORKFLOW_ALLOWED_EXACT_TEST
    } else {
        WORKFLOW_ALLOWED_EXACT_PROD
    };
    EnvContract::new(
        "dicom-workflow-server",
        DICOM_WORKFLOW_ENV_PREFIX,
        allowed_exact,
        WORKFLOW_ALLOWED_PREFIXES,
    )
}

/// Return typed wrappers for all `dicom-dimse-service` environment keys.
pub fn dicom_dimse_env_vars(include_test_vars: bool) -> &'static [DimseEnvVar] {
    if include_test_vars {
        DIMSE_ENV_VARS_TEST
    } else {
        DIMSE_ENV_VARS_PROD
    }
}

/// Build the canonical runtime contract for `dicom-dimse-service`.
pub fn dicom_dimse_env_contract(include_test_vars: bool) -> EnvContract<'static> {
    let allowed_exact = if include_test_vars {
        DIMSE_ALLOWED_EXACT_TEST
    } else {
        DIMSE_ALLOWED_EXACT_PROD
    };
    EnvContract::new(
        "dicom-dimse-service",
        DICOM_DIMSE_ENV_PREFIX,
        allowed_exact,
        &[],
    )
}

/// Contract describing supported environment variables for a runtime service.
#[derive(Debug, Clone, Copy)]
pub struct EnvContract<'a> {
    /// Human-readable service label used in validation messages.
    pub service: &'static str,
    /// Scope prefix used to identify service-specific environment variables.
    pub env_prefix: &'a str,
    /// Exact environment variables that this service parses.
    pub allowed_exact: &'a [&'a str],
    /// Prefixes for supported dynamic variables (for example connector or route families).
    pub allowed_prefixes: &'a [&'a str],
}

/// Snapshot metadata for supported service env variables.
#[derive(Debug, Clone)]
pub struct EnvContractSnapshot {
    /// Service identifier for the snapshot owner.
    pub service: &'static str,
    /// Prefix used by this service env namespace.
    pub env_prefix: String,
    /// Normalized allowed exact env vars.
    pub allowed_exact: Vec<String>,
    /// Allowed env prefixes for dynamic var families.
    pub allowed_prefixes: Vec<String>,
    /// Runtime env vars with values observed for this contract.
    pub runtime_env: Vec<EnvContractSnapshotEntry>,
    /// Unknown set env vars currently present in environment.
    pub unknown: Vec<String>,
}

/// Single named env value in a snapshot.
#[derive(Debug, Clone)]
pub struct EnvContractSnapshotEntry {
    /// Variable name.
    pub name: String,
    /// Variable value if present.
    pub value: Option<String>,
}

impl<'a> EnvContract<'a> {
    /// Creates a new contract for a service.
    pub const fn new(
        service: &'static str,
        env_prefix: &'a str,
        allowed_exact: &'a [&'a str],
        allowed_prefixes: &'a [&'a str],
    ) -> Self {
        Self {
            service,
            env_prefix,
            allowed_exact,
            allowed_prefixes,
        }
    }

    /// Returns `true` when `name` is covered by the contract.
    pub fn is_allowed(&self, name: &str) -> bool {
        self.allowed_exact.contains(&name)
            || self
                .allowed_prefixes
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }

    /// Scans environment variables and returns unknown service-scoped names.
    pub fn unknown_env_vars(&self) -> Vec<String> {
        let mut unknown: Vec<String> = env::vars()
            .filter_map(|(name, _)| {
                if !name.starts_with(self.env_prefix) || self.is_allowed(&name) {
                    None
                } else {
                    Some(name)
                }
            })
            .collect();
        unknown.sort();
        unknown
    }

    /// Validates all service-scoped variables against the contract.
    ///
    /// Returns `Err(io::ErrorKind::InvalidInput)` when unknown variables are found.
    pub fn validate(&self) -> io::Result<()> {
        let unknown = self.unknown_env_vars();
        if unknown.is_empty() {
            return Ok(());
        }

        let unknown_variables = unknown.join(", ");
        let known_exact = if self.allowed_exact.is_empty() {
            "<none>".to_string()
        } else {
            self.allowed_exact.join(", ")
        };
        let known_prefixes = if self.allowed_prefixes.is_empty() {
            "<none>".to_string()
        } else {
            self.allowed_prefixes.join(", ")
        };
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "env-contract violation for {}: unrecognized {} environment variables: {unknown_variables}. \
Allowed exact keys: {known_exact}. Allowed dynamic prefixes: {known_prefixes}. \
Remove unknown variables or extend the contract intentionally.",
                self.service, self.env_prefix
            ),
        ))
    }

    /// Collect a deterministic snapshot of service env contracts and observed values.
    pub fn snapshot(&self) -> EnvContractSnapshot {
        let mut variable_names = BTreeSet::new();
        for exact in self.allowed_exact {
            variable_names.insert((*exact).to_string());
        }
        for (name, _) in env::vars() {
            if self.is_allowed(&name) {
                variable_names.insert(name);
            }
        }

        let runtime_env = variable_names
            .into_iter()
            .map(|name| EnvContractSnapshotEntry {
                value: env::var(&name).ok(),
                name,
            })
            .collect::<Vec<EnvContractSnapshotEntry>>();

        EnvContractSnapshot {
            service: self.service,
            env_prefix: self.env_prefix.to_string(),
            allowed_exact: self
                .allowed_exact
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            allowed_prefixes: self
                .allowed_prefixes
                .iter()
                .map(|prefix| (*prefix).to_string())
                .collect(),
            runtime_env,
            unknown: self.unknown_env_vars(),
        }
    }

    /// Render a deterministic JSON-like snapshot suitable for support diagnostics.
    pub fn snapshot_json(&self, envelope_version: &str) -> String {
        let snapshot = self.snapshot();
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"service\": ");
        out.push_str(&format!("\"{}\",\n", snapshot.service));
        out.push_str("  \"env_prefix\": ");
        out.push_str(&format!("\"{}\",\n", snapshot.env_prefix));
        out.push_str("  \"effective_envelope_version\": ");
        out.push_str(&format!("\"{envelope_version}\",\n"));
        out.push_str("  \"supported_envelope_versions\": [\"1.1\"],\n");

        out.push_str("  \"allowed_exact\": [\n");
        for (index, name) in snapshot.allowed_exact.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!("    \"{}\"\n", json_escape(name)));
        }
        out.push_str("  ],\n");

        out.push_str("  \"allowed_prefixes\": [\n");
        for (index, prefix) in snapshot.allowed_prefixes.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!("    \"{}\"\n", json_escape(prefix)));
        }
        out.push_str("  ],\n");

        out.push_str("  \"runtime_env\": [\n");
        for (index, entry) in snapshot.runtime_env.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            match &entry.value {
                Some(value) => {
                    out.push_str(&format!(
                        "    {{\\\"name\\\": \"{}\\\", \\\"value\\\": \"{}\\\", \\\"status\\\": \\\"set\\\"}}\n",
                        json_escape(&entry.name),
                        json_escape(value)
                    ));
                }
                None => {
                    out.push_str(&format!(
                        "    {{\\\"name\\\": \"{}\\\", \\\"status\\\": \"unset\\\"}}\n",
                        json_escape(&entry.name)
                    ));
                }
            }
        }
        out.push_str("  ],\n");

        out.push_str("  \"unknown_env_vars\": [\n");
        for (index, variable) in snapshot.unknown.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            out.push_str(&format!("    \"{}\"\n", json_escape(variable)));
        }
        out.push_str("  ]\n");
        out.push('}');
        out
    }
}

/// Optional numeric bounds for environment values.
#[derive(Debug, Clone, Copy)]
pub struct NumericBounds {
    /// Minimum allowed value (inclusive) when present.
    pub min: Option<u64>,
    /// Maximum allowed value (inclusive) when present.
    pub max: Option<u64>,
}

impl NumericBounds {
    /// Creates unconstrained bounds.
    pub const fn unbounded() -> Self {
        Self {
            min: None,
            max: None,
        }
    }

    /// Creates constraints for a bounded inclusive range.
    pub const fn with_range(min: u64, max: u64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
        }
    }

    /// Creates constraints that require values above or equal to `min`.
    pub const fn at_least(min: u64) -> Self {
        Self {
            min: Some(min),
            max: None,
        }
    }

    /// Creates constraints that require values below or equal to `max`.
    pub const fn at_most(max: u64) -> Self {
        Self {
            min: None,
            max: Some(max),
        }
    }

    fn to_description(self) -> String {
        let min = self.min.map(|value| format!("minimum {value}"));
        let max = self.max.map(|value| format!("maximum {value}"));
        match (min, max) {
            (None, None) => "unbounded integer".to_string(),
            (Some(min), None) => min,
            (None, Some(max)) => max,
            (Some(min), Some(max)) => format!("{min}, {max}"),
        }
    }

    fn validate(self, service: &'static str, name: &str, value: u64) -> io::Result<()> {
        if let Some(min) = self.min {
            if value < min {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "{service}: {name} must be >= {min} ({})",
                        self.to_description()
                    ),
                ));
            }
        }
        if let Some(max) = self.max {
            if value > max {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "{service}: {name} must be <= {max} ({})",
                        self.to_description()
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn parse_env_var(service: &'static str, name: &str) -> io::Result<Option<String>> {
    env::var(name).map(Some).or_else(|error| {
        if error == env::VarError::NotPresent {
            Ok(None)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{service}: unable to read {name}"),
            ))
        }
    })
}

/// Parse an optional string value and fail on unreadable environment lookup.
pub fn parse_optional_string(service: &'static str, name: &str) -> io::Result<Option<String>> {
    parse_env_var(service, name)
}

/// Parse a required string value that may be empty.
pub fn parse_string(service: &'static str, name: &str, default: &str) -> io::Result<String> {
    parse_env_var(service, name).map(|value| value.unwrap_or_else(|| default.to_string()))
}

/// Parse a required non-empty string.
pub fn parse_string_non_empty(
    service: &'static str,
    name: &str,
    default: &str,
) -> io::Result<String> {
    let value = parse_string(service, name, default)?;
    if value.trim().is_empty() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{service}: {name} must be a non-empty string"),
        ))
    } else {
        Ok(value)
    }
}

/// Parse an optional boolean value.
pub fn parse_bool(service: &'static str, name: &str, default: bool) -> io::Result<bool> {
    let value = parse_env_var(service, name)?;
    let Some(raw) = value else {
        return Ok(default);
    };

    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{service}: {name} must be true/false"),
        )),
    }
}

fn parse_u64_value(service: &'static str, name: &str, raw: String) -> io::Result<u64> {
    let parsed = raw.parse::<u64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{service}: {name} must be a non-negative integer"),
        )
    })?;
    Ok(parsed)
}

/// Validate envelope version from `DICOM_ENVELOPE_VERSION` and reject unsupported versions.
pub fn validate_envelope_version(
    service: &'static str,
    name: &str,
    supported_versions: &'static [&'static str],
    default_version: &str,
) -> io::Result<String> {
    let parsed = parse_string_non_empty(service, name, default_version)?;
    if !supported_versions
        .iter()
        .any(|candidate| *candidate == parsed)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{service}: unsupported {name}={parsed}; supported values are {supported_versions:?}",
            ),
        ));
    }
    Ok(parsed)
}

fn json_escape(raw: &str) -> String {
    let mut output = String::new();
    for raw_char in raw.chars() {
        match raw_char {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c => output.push(c),
        }
    }
    output
}

/// Parse an optional integer and validate using an optional bounded range.
pub fn parse_optional_u64(
    service: &'static str,
    name: &str,
    bounds: NumericBounds,
) -> io::Result<Option<u64>> {
    let value = parse_env_var(service, name)?;
    let Some(raw) = value else {
        return Ok(None);
    };
    let parsed = parse_u64_value(service, name, raw)?;
    bounds.validate(service, name, parsed)?;
    Ok(Some(parsed))
}

/// Parse a non-negative integer value with optional bounds and fallback.
pub fn parse_u64(
    service: &'static str,
    name: &str,
    default: u64,
    bounds: NumericBounds,
) -> io::Result<u64> {
    let value = parse_env_var(service, name)?;
    bounds.validate(service, name, default)?;
    let Some(raw) = value else {
        return Ok(default);
    };
    let parsed = parse_u64_value(service, name, raw)?;
    bounds.validate(service, name, parsed)?;
    Ok(parsed)
}

/// Parse a non-negative integer value with optional bounds and fallback.
pub fn parse_usize(
    service: &'static str,
    name: &str,
    default: usize,
    bounds: NumericBounds,
) -> io::Result<usize> {
    let value = parse_env_var(service, name)?;
    bounds.validate(service, name, default as u64)?;
    let Some(raw) = value else {
        return Ok(default);
    };
    let parsed = parse_u64_value(service, name, raw)?;
    bounds.validate(service, name, parsed)?;
    usize::try_from(parsed).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{service}: {name} must fit into usize"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

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

    #[test]
    fn parse_u64_respects_default_when_unset() {
        with_env_var("DICOM_TEST_ALPHA", None, || {
            let value = parse_u64(
                "dicom-test",
                "DICOM_TEST_ALPHA",
                100,
                NumericBounds::unbounded(),
            )
            .expect("default");
            assert_eq!(value, 100);
        });
    }

    #[test]
    fn parse_u64_rejects_out_of_range() {
        with_env_var("DICOM_TEST_ALPHA", Some("200"), || {
            let err = parse_u64(
                "dicom-test",
                "DICOM_TEST_ALPHA",
                100,
                NumericBounds::at_most(128),
            )
            .expect_err("range violation");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert!(err.to_string().contains("must be <= 128"));
        });
    }

    #[test]
    fn parse_usize_rejects_invalid_default() {
        let bounds = NumericBounds::with_range(10, 20);
        let err = parse_usize("dicom-test", "DICOM_TEST_BETA", 4, bounds).expect_err("bad default");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("DICOM_TEST_BETA"));
    }

    #[test]
    fn parse_bool_defaults_when_unset() {
        with_env_var("DICOM_TEST_BOOL", None, || {
            assert!(parse_bool("dicom-test", "DICOM_TEST_BOOL", true).expect("default"));
        });
    }

    #[test]
    fn validate_envelope_version_uses_default_when_unset() {
        with_env_var(DICOM_ENVELOPE_VERSION, None, || {
            let version = validate_envelope_version(
                "dicom-test",
                DICOM_ENVELOPE_VERSION,
                SUPPORTED_DICOM_ENVELOPE_VERSIONS,
                DEFAULT_DICOM_ENVELOPE_VERSION,
            )
            .expect("default envelope version");
            assert_eq!(version, DEFAULT_DICOM_ENVELOPE_VERSION);
        });
    }

    #[test]
    fn validate_envelope_version_rejects_unknown_value() {
        with_env_var(DICOM_ENVELOPE_VERSION, Some("9.9"), || {
            let err = validate_envelope_version(
                "dicom-test",
                DICOM_ENVELOPE_VERSION,
                SUPPORTED_DICOM_ENVELOPE_VERSIONS,
                DEFAULT_DICOM_ENVELOPE_VERSION,
            )
            .expect_err("unknown envelope must fail closed");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
            assert!(err.to_string().contains("unsupported"));
        });
    }

    #[test]
    fn production_contract_excludes_web_test_only_env_vars() {
        let prod = dicom_web_env_contract(false);
        assert!(!prod.allowed_exact.contains(&"DICOM_WEB_TEST_STORAGE_BYTES"));
        assert!(!prod.allowed_exact.contains(&"DICOM_WEB_TEST_WORKERS"));

        let test = dicom_web_env_contract(true);
        assert!(test.allowed_exact.contains(&"DICOM_WEB_TEST_STORAGE_BYTES"));
        assert!(test.allowed_exact.contains(&"DICOM_WEB_TEST_WORKERS"));
    }

    #[test]
    fn production_contract_excludes_workflow_test_only_env_vars() {
        let prod = dicom_workflow_env_contract(false);
        assert!(!prod
            .allowed_exact
            .contains(&"DICOM_WORKFLOW_TEST_RATE_LIMIT"));
        assert!(!prod
            .allowed_exact
            .contains(&"DICOM_WORKFLOW_TEST_ROTATIONS"));

        let test = dicom_workflow_env_contract(true);
        assert!(test
            .allowed_exact
            .contains(&"DICOM_WORKFLOW_TEST_RATE_LIMIT"));
        assert!(test
            .allowed_exact
            .contains(&"DICOM_WORKFLOW_TEST_ROTATIONS"));
    }

    #[test]
    fn production_contract_excludes_dimse_test_only_env_vars() {
        let prod = dicom_dimse_env_contract(false);
        assert!(!prod.allowed_exact.contains(&"DICOM_DIMSE_TEST_MAX_BYTES"));
        assert!(!prod
            .allowed_exact
            .contains(&"DICOM_DIMSE_TEST_OPTIONAL_SECONDS"));

        let test = dicom_dimse_env_contract(true);
        assert!(test.allowed_exact.contains(&"DICOM_DIMSE_TEST_MAX_BYTES"));
        assert!(test
            .allowed_exact
            .contains(&"DICOM_DIMSE_TEST_OPTIONAL_SECONDS"));
    }
}
