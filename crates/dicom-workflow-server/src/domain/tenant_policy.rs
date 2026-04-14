//! Tenant-policy bounded-context constants.

/// Tenant header precedence list for actor context extraction.
pub(crate) const TENANT_HEADER_CANDIDATES: [&str; 4] = [
    "x-tenant",
    "x-workflow-tenant",
    "x-tenant-id",
    "x-workflow-tenant-id",
];

/// Workflow metrics route.
pub(crate) const WORKFLOW_METRICS_PATH: &str = "/workflow/metrics";
/// Workflow audit route.
pub(crate) const WORKFLOW_AUDIT_PATH: &str = "/workflow/audit";
/// Tenant quota read-only API route.
pub(crate) const WORKFLOW_TENANT_QUOTAS_PATH: &str = "/workflow/policy/quotas";
/// Tenant quota snapshot export API route.
pub(crate) const WORKFLOW_TENANT_QUOTAS_SNAPSHOT_PATH: &str = "/workflow/policy/quotas/snapshot";
/// Env prefix for tenant task quota overrides.
pub(crate) const TENANT_TASK_QUOTA_ENV_PREFIX: &str = "DICOM_WORKFLOW_TENANT_TASK_QUOTA_";
/// Env prefix for tenant subscription quota overrides.
pub(crate) const TENANT_SUBSCRIPTION_QUOTA_ENV_PREFIX: &str =
    "DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_";
/// Env prefix for tenant query-rate-limit overrides.
pub(crate) const TENANT_QUERY_RATE_LIMIT_ENV_PREFIX: &str =
    "DICOM_WORKFLOW_TENANT_QUERY_RATE_LIMIT_";
/// Env prefix for tenant mutation-rate-limit overrides.
pub(crate) const TENANT_MUTATION_RATE_LIMIT_ENV_PREFIX: &str =
    "DICOM_WORKFLOW_TENANT_MUTATION_RATE_LIMIT_";
/// Env prefix for tenant upload-cap overrides.
pub(crate) const TENANT_UPLOAD_CAP_BYTES_ENV_PREFIX: &str =
    "DICOM_WORKFLOW_TENANT_UPLOAD_CAP_BYTES_";
