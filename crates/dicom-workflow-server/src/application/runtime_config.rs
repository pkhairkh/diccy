use super::*;

#[derive(Clone)]
#[allow(missing_docs)]
pub struct WorkflowRuntimeConfig {
    pub bind: String,
    pub worklist_state_path: String,
    pub mpps_state_path: String,
    pub sr_state_path: String,
    pub sr_audit_path: String,
    pub workflow_audit_path: String,
    pub snapshot_max_bytes: u64,
    pub snapshot_max_rotated_files: usize,
    pub audit_max_bytes: u64,
    pub audit_max_rotated_files: usize,
    pub auth_mode_raw: Option<String>,
    pub auth_token: Option<String>,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub transport_security_raw: Option<String>,
    pub query_rate_limit: u64,
    pub mutation_rate_limit: u64,
    pub upload_cap_bytes: u64,
    pub anomaly_alert_threshold: u64,
    pub audit_export_limit: usize,
    pub audit_rate_window_ms: u64,
    pub denylist_routes: Vec<String>,
    pub hl7_transport: Hl7TransportConfig,
    pub hl7_connector_registry: BTreeMap<String, String>,
    pub hl7_connector_feature_flags: BTreeMap<String, Hl7ConnectorFeatureFlag>,
    pub hl7_connector_rollout_percents: BTreeMap<String, u64>,
    pub hl7_connector_plugins: BTreeMap<String, ConnectorPluginMetadata>,
    pub auth_mode: WorkflowAuthMode,
    pub transport_security: &'static str,
}

#[allow(missing_docs)]
impl WorkflowRuntimeConfig {
    pub fn from_env() -> std::io::Result<Self> {
        // Defaults and bounds in this parser are the runtime source for docs/09 and docs/14.
        let bind = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_BIND",
            "127.0.0.1:8082",
        )?;
        let worklist_state_path = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_WORKLIST_STATE_PATH",
            "./state/workflow/worklist.snapshot",
        )?;
        let mpps_state_path = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_MPPS_STATE_PATH",
            "./state/workflow/mpps.snapshot",
        )?;
        let sr_state_path = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_SR_STATE_PATH",
            "./state/workflow/sr.snapshot",
        )?;
        let sr_audit_path = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_SR_AUDIT_PATH",
            "./state/workflow/sr.audit.log",
        )?;
        let workflow_audit_path = parse_string_non_empty(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_AUDIT_PATH",
            DEFAULT_AUDIT_PATH,
        )?;
        let snapshot_max_bytes = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES",
            32 * 1024 * 1024,
            NumericBounds::at_least(1),
        )?;
        let snapshot_max_rotated_files = parse_usize(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES",
            2,
            NumericBounds::at_least(1),
        )?;
        let audit_max_bytes = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_AUDIT_MAX_BYTES",
            DEFAULT_AUDIT_MAX_BYTES,
            NumericBounds::at_least(1),
        )?;
        let audit_max_rotated_files = parse_usize(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES",
            DEFAULT_AUDIT_MAX_ROTATED_FILES,
            NumericBounds::at_least(1),
        )?;
        let auth_mode_raw =
            parse_optional_string(WORKFLOW_SERVICE_NAME, "DICOM_WORKFLOW_AUTH_MODE")?;
        let secret_dir = parse_optional_string(WORKFLOW_SERVICE_NAME, "DICOM_WORKFLOW_SECRET_DIR")?
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let auth_token = load_secret_value(
            "DICOM_WORKFLOW_AUTH_TOKEN",
            "DICOM_WORKFLOW_AUTH_TOKEN_PATH",
            secret_dir.as_deref(),
            WORKFLOW_SECRET_AUTH_TOKEN_FILE_HINTS,
        )?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
        let tls_cert_path = secret_path_with_default(
            "DICOM_WORKFLOW_TLS_CERT_PATH",
            WORKFLOW_SECRET_TLS_CERT_FILE_HINTS,
            secret_dir.as_deref(),
        )?
        .filter(|value| !value.trim().is_empty());
        let tls_key_path = secret_path_with_default(
            "DICOM_WORKFLOW_TLS_KEY_PATH",
            WORKFLOW_SECRET_TLS_KEY_FILE_HINTS,
            secret_dir.as_deref(),
        )?
        .filter(|value| !value.trim().is_empty());
        let transport_security_raw =
            parse_optional_string(WORKFLOW_SERVICE_NAME, "DICOM_WORKFLOW_TRANSPORT_SECURITY")?;
        if let Some(raw) = transport_security_raw.as_deref() {
            if raw != "tls" && raw != "insecure" {
                return Err(environment_parse_error(
                    "DICOM_WORKFLOW_TRANSPORT_SECURITY",
                    raw,
                    "supported values are tls | insecure",
                ));
            }
        }
        let transport_security = transport_security_from_env(transport_security_raw.as_deref());
        let auth_mode = auth_mode_from_env(auth_token.as_deref(), auth_mode_raw.as_deref())?;
        let query_rate_limit = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_QUERY_RATE_LIMIT",
            DEFAULT_QUERY_RATE_LIMIT,
            NumericBounds::at_least(1),
        )?;
        let mutation_rate_limit = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_MUTATION_RATE_LIMIT",
            DEFAULT_MUTATION_RATE_LIMIT,
            NumericBounds::at_least(1),
        )?;
        let upload_cap_bytes = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_UPLOAD_CAP_BYTES",
            DEFAULT_UPLOAD_CAP_BYTES,
            NumericBounds::at_least(1),
        )?;
        let anomaly_alert_threshold = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD",
            DEFAULT_ANOMALY_ALERT_THRESHOLD,
            NumericBounds::at_least(1),
        )?;
        let audit_export_limit = parse_usize(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT",
            DEFAULT_AUDIT_EXPORT_LIMIT,
            NumericBounds::at_least(1),
        )?;
        let audit_rate_window_ms = parse_u64(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS",
            DEFAULT_RATE_LIMIT_WINDOW_MS,
            NumericBounds::at_least(1),
        )?;
        let denylist_routes = parse_denylist_routes(parse_optional_string(
            WORKFLOW_SERVICE_NAME,
            "DICOM_WORKFLOW_DENYLIST_PATHS",
        )?);
        let hl7_transport = parse_hl7_transport_config()?;
        let hl7_connector_registry = parse_hl7_connector_registry();
        let hl7_connector_feature_flags = parse_hl7_connector_feature_flags()?;
        let hl7_connector_rollout_percents = parse_hl7_connector_rollout_percents()?;
        let hl7_connector_plugins = parse_hl7_connector_plugins()?;

        Ok(Self {
            bind,
            worklist_state_path,
            mpps_state_path,
            sr_state_path,
            sr_audit_path,
            workflow_audit_path,
            snapshot_max_bytes,
            snapshot_max_rotated_files,
            audit_max_bytes,
            audit_max_rotated_files,
            auth_mode_raw,
            auth_token,
            tls_cert_path,
            tls_key_path,
            transport_security_raw,
            query_rate_limit,
            mutation_rate_limit,
            upload_cap_bytes,
            anomaly_alert_threshold,
            audit_export_limit,
            audit_rate_window_ms,
            denylist_routes,
            hl7_transport,
            hl7_connector_registry,
            hl7_connector_feature_flags,
            hl7_connector_rollout_percents,
            hl7_connector_plugins,
            auth_mode,
            transport_security,
        })
    }
}
