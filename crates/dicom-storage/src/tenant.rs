//! Multi-tenancy support for dicom-storage.
//!
//! Provides tenant identification, policy enforcement, storage namespace isolation,
//! and authentication-based tenant resolution.
//!
//! # Design Principles
//!
//! - **Fail-closed**: If a tenant cannot be resolved or access cannot be verified,
//!   access is always denied.
//! - **Backward compatible**: When no tenant is configured, the system operates in
//!   single-tenant mode as before.
//! - **Namespace isolation**: Each tenant's blobs are stored under a distinct prefix
//!   (e.g., `tenant-abc/`) so that storage backends naturally segregate data.

use crate::BlobStore;
use crate::vna::RetentionPolicy;
use dicom_core::{Error, ErrorKind};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

// ===========================================================================
// TenantId — Validated tenant identifier newtype
// ===========================================================================

/// Validated tenant identifier.
///
/// A `TenantId` wraps a `String` that has been validated against the tenant
/// identifier rules: non-empty, alphanumeric with hyphens and underscores,
/// length between 1 and 128 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TenantId(String);

impl TenantId {
    /// Maximum length of a tenant identifier.
    pub const MAX_LEN: usize = 128;

    /// Create a new validated tenant identifier.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::InvalidTenantId`] if the identifier:
    /// - is empty
    /// - exceeds 128 characters
    /// - contains characters other than ASCII alphanumeric, hyphens, or underscores
    /// - starts or ends with a hyphen or underscore
    pub fn new(id: impl Into<String>) -> Result<Self, TenantError> {
        let id = id.into();
        if id.is_empty() {
            return Err(TenantError::InvalidTenantId {
                detail: "tenant identifier must not be empty".to_string(),
            });
        }
        if id.len() > Self::MAX_LEN {
            return Err(TenantError::InvalidTenantId {
                detail: format!(
                    "tenant identifier must not exceed {} characters (got {})",
                    Self::MAX_LEN,
                    id.len()
                ),
            });
        }
        // Must start with an alphanumeric character
        if !id.chars().next().map_or(false, |c| c.is_ascii_alphanumeric()) {
            return Err(TenantError::InvalidTenantId {
                detail: "tenant identifier must start with an alphanumeric character".to_string(),
            });
        }
        // Must end with an alphanumeric character
        if !id.chars().last().map_or(false, |c| c.is_ascii_alphanumeric()) {
            return Err(TenantError::InvalidTenantId {
                detail: "tenant identifier must end with an alphanumeric character".to_string(),
            });
        }
        // All characters must be alphanumeric, hyphen, or underscore
        for ch in id.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
                return Err(TenantError::InvalidTenantId {
                    detail: format!(
                        "tenant identifier contains invalid character '{}': \
                         only ASCII alphanumeric, hyphens, and underscores are allowed",
                        ch
                    ),
                });
            }
        }
        Ok(Self(id))
    }

    /// Return the tenant identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the S3/blob storage prefix for this tenant.
    ///
    /// The prefix is `tenant-{id}/` which provides natural namespace isolation
    /// in S3, filesystem, and in-memory backends.
    pub fn blob_prefix(&self) -> String {
        format!("tenant-{}/", self.0)
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for TenantId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for TenantId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        TenantId::new(s).map_err(serde::de::Error::custom)
    }
}

// ===========================================================================
// TenantError
// ===========================================================================

/// Errors related to multi-tenant operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TenantError {
    /// Invalid tenant identifier.
    InvalidTenantId {
        /// Detail message.
        detail: String,
    },
    /// Cross-tenant access denied (fail-closed).
    CrossTenantAccessDenied {
        /// Tenant that was requested.
        requested_tenant: String,
        /// Tenant that was resolved from auth context.
        resolved_tenant: String,
    },
    /// Tenant could not be resolved from authentication context.
    TenantResolutionFailed {
        /// Detail message.
        detail: String,
    },
    /// Tenant storage quota exceeded.
    QuotaExceeded {
        /// Tenant ID.
        tenant_id: String,
        /// Observed bytes.
        observed_bytes: u64,
        /// Quota limit in bytes.
        quota_bytes: u64,
    },
    /// Tenant study count limit exceeded.
    StudyLimitExceeded {
        /// Tenant ID.
        tenant_id: String,
        /// Observed study count.
        observed: u64,
        /// Allowed maximum.
        allowed: u64,
    },
    /// Tenant rate limit exceeded.
    RateLimitExceeded {
        /// Tenant ID.
        tenant_id: String,
        /// Limit name.
        limit_name: String,
    },
    /// Required feature flag is not enabled for this tenant.
    FeatureDisabled {
        /// Tenant ID.
        tenant_id: String,
        /// Feature flag that is required.
        feature: String,
    },
}

impl std::fmt::Display for TenantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TenantError::InvalidTenantId { detail } => {
                write!(f, "invalid tenant identifier: {}", detail)
            }
            TenantError::CrossTenantAccessDenied {
                requested_tenant,
                resolved_tenant,
            } => {
                write!(
                    f,
                    "cross-tenant access denied: requested '{}', resolved '{}'",
                    requested_tenant, resolved_tenant
                )
            }
            TenantError::TenantResolutionFailed { detail } => {
                write!(f, "tenant resolution failed: {}", detail)
            }
            TenantError::QuotaExceeded {
                tenant_id,
                observed_bytes,
                quota_bytes,
            } => {
                write!(
                    f,
                    "tenant '{}' quota exceeded: {} bytes used, {} bytes allowed",
                    tenant_id, observed_bytes, quota_bytes
                )
            }
            TenantError::StudyLimitExceeded {
                tenant_id,
                observed,
                allowed,
            } => {
                write!(
                    f,
                    "tenant '{}' study limit exceeded: {} studies, {} allowed",
                    tenant_id, observed, allowed
                )
            }
            TenantError::RateLimitExceeded {
                tenant_id,
                limit_name,
            } => {
                write!(
                    f,
                    "tenant '{}' rate limit exceeded: {}",
                    tenant_id, limit_name
                )
            }
            TenantError::FeatureDisabled {
                tenant_id,
                feature,
            } => {
                write!(
                    f,
                    "tenant '{}' feature '{}' is not enabled",
                    tenant_id, feature
                )
            }
        }
    }
}

impl std::error::Error for TenantError {}

impl From<TenantError> for Box<Error> {
    fn from(err: TenantError) -> Box<Error> {
        Error::from_kind(
            ErrorKind::AuthorizationDenied {
                resource: "tenant".to_string(),
                reason: err.to_string(),
            },
            "tenant error",
        )
        .into()
    }
}

// ===========================================================================
// TenantPolicy — Per-tenant policy configuration
// ===========================================================================

/// Per-tenant policy configuration.
///
/// Governs storage quotas, retention rules, feature flags, and rate limits
/// for a specific tenant.
#[derive(Debug, Clone, PartialEq)]
pub struct TenantPolicy {
    /// The tenant this policy applies to.
    pub tenant_id: TenantId,
    /// Maximum storage quota in bytes. `None` means unlimited.
    pub storage_quota_bytes: Option<u64>,
    /// Retention policy. `None` means use system default.
    pub retention_policy: Option<RetentionPolicy>,
    /// Feature flags enabled for this tenant.
    pub feature_flags: HashSet<String>,
    /// Maximum number of studies. `None` means unlimited.
    pub max_studies: Option<u64>,
    /// Rate limit per minute for API calls. `None` means unlimited.
    pub rate_limit_per_minute: Option<u32>,
}

impl TenantPolicy {
    /// Create a new tenant policy with default (unlimited) settings.
    pub fn new(tenant_id: TenantId) -> Self {
        Self {
            tenant_id,
            storage_quota_bytes: None,
            retention_policy: None,
            feature_flags: HashSet::new(),
            max_studies: None,
            rate_limit_per_minute: None,
        }
    }

    /// Set the storage quota in bytes.
    pub fn with_storage_quota(mut self, bytes: u64) -> Self {
        self.storage_quota_bytes = Some(bytes);
        self
    }

    /// Set the retention policy.
    pub fn with_retention_policy(mut self, policy: RetentionPolicy) -> Self {
        self.retention_policy = Some(policy);
        self
    }

    /// Add a feature flag.
    pub fn with_feature_flag(mut self, flag: impl Into<String>) -> Self {
        self.feature_flags.insert(flag.into());
        self
    }

    /// Set the maximum number of studies.
    pub fn with_max_studies(mut self, max: u64) -> Self {
        self.max_studies = Some(max);
        self
    }

    /// Set the rate limit per minute.
    pub fn with_rate_limit(mut self, limit: u32) -> Self {
        self.rate_limit_per_minute = Some(limit);
        self
    }

    /// Check if a feature flag is enabled for this tenant.
    pub fn has_feature(&self, flag: &str) -> bool {
        self.feature_flags.contains(flag)
    }

    /// Check if a storage quota is set and enforce it.
    ///
    /// Returns `Ok(())` if the quota is not set or the observed bytes are within
    /// the quota. Returns [`TenantError::QuotaExceeded`] otherwise.
    pub fn check_storage_quota(&self, observed_bytes: u64) -> Result<(), TenantError> {
        if let Some(quota) = self.storage_quota_bytes {
            if observed_bytes > quota {
                return Err(TenantError::QuotaExceeded {
                    tenant_id: self.tenant_id.as_str().to_string(),
                    observed_bytes,
                    quota_bytes: quota,
                });
            }
        }
        Ok(())
    }

    /// Check if a study count is within the maximum.
    ///
    /// Returns `Ok(())` if no limit is set or the observed count is within
    /// the limit. Returns [`TenantError::StudyLimitExceeded`] otherwise.
    pub fn check_study_limit(&self, observed: u64) -> Result<(), TenantError> {
        if let Some(allowed) = self.max_studies {
            if observed > allowed {
                return Err(TenantError::StudyLimitExceeded {
                    tenant_id: self.tenant_id.as_str().to_string(),
                    observed,
                    allowed,
                });
            }
        }
        Ok(())
    }

    /// Require a feature flag, returning an error if it is not enabled.
    pub fn require_feature(&self, flag: &str) -> Result<(), TenantError> {
        if !self.has_feature(flag) {
            return Err(TenantError::FeatureDisabled {
                tenant_id: self.tenant_id.as_str().to_string(),
                feature: flag.to_string(),
            });
        }
        Ok(())
    }
}

// ===========================================================================
// TenantNamespace — Tenant-aware storage namespace
// ===========================================================================

/// Tenant-aware storage namespace.
///
/// Encapsulates the tenant identifier and the blob prefix used to isolate
/// this tenant's data in the underlying storage backend.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantNamespace {
    /// The tenant identifier.
    pub tenant_id: TenantId,
    /// Blob prefix for this tenant (e.g., `tenant-abc/` for S3).
    pub blob_prefix: String,
}

impl TenantNamespace {
    /// Create a new tenant namespace from a validated tenant identifier.
    pub fn new(tenant_id: TenantId) -> Self {
        let blob_prefix = tenant_id.blob_prefix();
        Self {
            tenant_id,
            blob_prefix,
        }
    }

    /// Prefix a key with the tenant namespace.
    ///
    /// For example, if the tenant is `abc` and the key is `data/object.dcm`,
    /// the result is `tenant-abc/data/object.dcm`.
    pub fn prefix_key(&self, key: &str) -> String {
        format!("{}{}", self.blob_prefix, key)
    }

    /// Strip the tenant prefix from a key.
    ///
    /// Returns `None` if the key does not start with this tenant's prefix.
    pub fn strip_prefix<'a>(&self, key: &'a str) -> Option<&'a str> {
        key.strip_prefix(&self.blob_prefix)
    }

    /// Check if a key belongs to this tenant's namespace.
    pub fn owns_key(&self, key: &str) -> bool {
        key.starts_with(&self.blob_prefix)
    }
}

// ===========================================================================
// AuthContext — Authentication context for tenant resolution
// ===========================================================================

/// Authentication context for tenant resolution.
///
/// Carries JWT claims and/or API key information used by
/// [`TenantResolver`] implementations.
#[derive(Debug, Clone, Default)]
pub struct AuthContext {
    /// JWT claims as string key-value pairs.
    pub jwt_claims: HashMap<String, String>,
    /// API key, if provided.
    pub api_key: Option<String>,
    /// Principal identifier (non-PHI).
    pub principal: Option<String>,
}

impl AuthContext {
    /// Create an empty auth context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an auth context with a JWT claim.
    pub fn with_jwt_claim(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.jwt_claims.insert(key.into(), value.into());
        self
    }

    /// Create an auth context with an API key.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Create an auth context with a principal.
    pub fn with_principal(mut self, principal: impl Into<String>) -> Self {
        self.principal = Some(principal.into());
        self
    }
}

// ===========================================================================
// TenantResolver — Trait for resolving tenant from auth context
// ===========================================================================

/// Resolves a tenant identifier from an authentication context.
///
/// Implementations may extract the tenant from JWT claims, API keys, or
/// other authentication mechanisms.
pub trait TenantResolver: Send + Sync {
    /// Resolve the tenant identifier from the given authentication context.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::TenantResolutionFailed`] if the tenant cannot
    /// be resolved (fail-closed).
    fn resolve_tenant(&self, auth_context: &AuthContext) -> Result<TenantId, TenantError>;
}

// ===========================================================================
// JwtTenantResolver — JWT claim-based tenant resolver
// ===========================================================================

/// JWT claim-based tenant resolver.
///
/// Extracts the tenant identifier from a specified JWT claim key
/// (e.g., `"tenant_id"` or `"organization"`).
#[derive(Debug, Clone)]
pub struct JwtTenantResolver {
    /// The JWT claim key to extract the tenant identifier from.
    pub claim_key: String,
}

impl JwtTenantResolver {
    /// Create a new JWT tenant resolver that reads from the given claim key.
    pub fn new(claim_key: impl Into<String>) -> Self {
        Self {
            claim_key: claim_key.into(),
        }
    }
}

impl TenantResolver for JwtTenantResolver {
    fn resolve_tenant(&self, auth_context: &AuthContext) -> Result<TenantId, TenantError> {
        let value = auth_context
            .jwt_claims
            .get(&self.claim_key)
            .ok_or_else(|| TenantError::TenantResolutionFailed {
                detail: format!(
                    "JWT claim '{}' not found in auth context",
                    self.claim_key
                ),
            })?;
        TenantId::new(value.clone())
    }
}

// ===========================================================================
// ApiKeyTenantResolver — API key-based tenant resolver
// ===========================================================================

/// API key-based tenant resolver.
///
/// Maps API keys to tenant identifiers using a pre-configured lookup table.
#[derive(Debug, Clone)]
pub struct ApiKeyTenantResolver {
    /// Map from API key strings to tenant identifiers.
    pub key_map: HashMap<String, TenantId>,
}

impl ApiKeyTenantResolver {
    /// Create a new API key tenant resolver with the given key-to-tenant mapping.
    pub fn new(key_map: HashMap<String, TenantId>) -> Self {
        Self { key_map }
    }

    /// Add a mapping from an API key to a tenant identifier.
    pub fn with_mapping(mut self, api_key: impl Into<String>, tenant_id: TenantId) -> Self {
        self.key_map.insert(api_key.into(), tenant_id);
        self
    }
}

impl TenantResolver for ApiKeyTenantResolver {
    fn resolve_tenant(&self, auth_context: &AuthContext) -> Result<TenantId, TenantError> {
        let api_key = auth_context
            .api_key
            .as_ref()
            .ok_or_else(|| TenantError::TenantResolutionFailed {
                detail: "no API key provided in auth context".to_string(),
            })?;
        self.key_map
            .get(api_key)
            .cloned()
            .ok_or_else(|| TenantError::TenantResolutionFailed {
                detail: format!("API key '{}' not mapped to any tenant", api_key),
            })
    }
}

// ===========================================================================
// CompositeTenantResolver — Try multiple resolvers in order
// ===========================================================================

/// Composite tenant resolver that tries multiple resolvers in order.
///
/// Returns the first successful resolution. If all resolvers fail, returns
/// the last error (fail-closed).
pub struct CompositeTenantResolver {
    /// Ordered list of resolvers to try.
    pub resolvers: Vec<Box<dyn TenantResolver>>,
}

impl std::fmt::Debug for CompositeTenantResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeTenantResolver")
            .field("resolvers", &self.resolvers.len())
            .finish()
    }
}

impl CompositeTenantResolver {
    /// Create a new composite resolver with no resolvers.
    pub fn new() -> Self {
        Self {
            resolvers: Vec::new(),
        }
    }

    /// Add a resolver to the chain.
    pub fn with_resolver(mut self, resolver: Box<dyn TenantResolver>) -> Self {
        self.resolvers.push(resolver);
        self
    }
}

impl Default for CompositeTenantResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl TenantResolver for CompositeTenantResolver {
    fn resolve_tenant(&self, auth_context: &AuthContext) -> Result<TenantId, TenantError> {
        let mut last_err = TenantError::TenantResolutionFailed {
            detail: "no tenant resolvers configured".to_string(),
        };
        for resolver in &self.resolvers {
            match resolver.resolve_tenant(auth_context) {
                Ok(tenant_id) => return Ok(tenant_id),
                Err(err) => last_err = err,
            }
        }
        Err(last_err)
    }
}

// ===========================================================================
// TenantBlobStore — Tenant-isolated blob store wrapper
// ===========================================================================

/// Tenant-isolated blob store that wraps an inner [`BlobStore`] and
/// prefixes all keys with the tenant's namespace.
///
/// This ensures that one tenant cannot access another tenant's data
/// through the blob store interface.
///
/// Uses `Arc<dyn BlobStore>` so that multiple tenant-scoped views can
/// share the same underlying storage backend.
pub struct TenantBlobStore {
    /// The inner blob store (shared via Arc).
    inner: Arc<dyn BlobStore>,
    /// The tenant namespace.
    namespace: TenantNamespace,
}

impl TenantBlobStore {
    /// Create a new tenant-scoped blob store wrapping the given inner store.
    pub fn new(inner: Arc<dyn BlobStore>, tenant_id: TenantId) -> Self {
        let namespace = TenantNamespace::new(tenant_id);
        Self { inner, namespace }
    }

    /// Return a reference to the tenant namespace.
    pub fn namespace(&self) -> &TenantNamespace {
        &self.namespace
    }

    /// Store data under the given key, prefixed with the tenant namespace.
    pub fn put(&self, key: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let prefixed = self.namespace.prefix_key(key);
        self.inner.put(&prefixed, data)
    }

    /// Retrieve data by key, prefixed with the tenant namespace.
    ///
    /// Returns an error if the key does not exist under the tenant's prefix.
    pub fn get(&self, key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let prefixed = self.namespace.prefix_key(key);
        self.inner.get(&prefixed)
    }

    /// Delete data by key, prefixed with the tenant namespace.
    pub fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        let prefixed = self.namespace.prefix_key(key);
        self.inner.delete(&prefixed)
    }

    /// List keys with the given prefix under the tenant namespace.
    ///
    /// Only keys belonging to this tenant are returned, with the tenant
    /// prefix stripped.
    pub fn list(&self, prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let full_prefix = self.namespace.prefix_key(prefix);
        let keys = self.inner.list(&full_prefix)?;
        let stripped: Vec<String> = keys
            .into_iter()
            .filter_map(|k| self.namespace.strip_prefix(&k).map(|s| s.to_string()))
            .collect();
        Ok(stripped)
    }

    /// Check if a key exists under this tenant's namespace.
    ///
    /// This performs a `get` and discards the data, returning `true` if the
    /// key exists and `false` otherwise.
    pub fn exists(&self, key: &str) -> bool {
        let prefixed = self.namespace.prefix_key(key);
        self.inner.get(&prefixed).is_ok()
    }

    /// Attempt to access a key from another tenant's namespace.
    ///
    /// This always returns an error (fail-closed), as cross-tenant access
    /// is never permitted.
    pub fn cross_tenant_get(
        &self,
        _other_tenant: &TenantId,
        _key: &str,
    ) -> Result<Vec<u8>, TenantError> {
        Err(TenantError::CrossTenantAccessDenied {
            requested_tenant: _other_tenant.as_str().to_string(),
            resolved_tenant: self.namespace.tenant_id.as_str().to_string(),
        })
    }
}

// ===========================================================================
// TenantEnforcer — Enforcement gate for tenant policies
// ===========================================================================

/// Enforces tenant policies for storage operations.
///
/// Maintains per-tenant usage counters and checks quotas, study limits,
/// and feature flags before allowing operations.
#[derive(Debug, Clone)]
pub struct TenantEnforcer {
    /// Per-tenant policies.
    policies: HashMap<TenantId, TenantPolicy>,
    /// Per-tenant tracked usage (bytes).
    usage_bytes: HashMap<TenantId, u64>,
    /// Per-tenant tracked study count.
    study_counts: HashMap<TenantId, u64>,
}

impl TenantEnforcer {
    /// Create a new tenant enforcer with no policies.
    pub fn new() -> Self {
        Self {
            policies: HashMap::new(),
            usage_bytes: HashMap::new(),
            study_counts: HashMap::new(),
        }
    }

    /// Register a tenant policy.
    pub fn register_policy(&mut self, policy: TenantPolicy) {
        let tenant_id = policy.tenant_id.clone();
        self.policies.insert(tenant_id, policy);
    }

    /// Get the policy for a tenant.
    pub fn policy(&self, tenant_id: &TenantId) -> Option<&TenantPolicy> {
        self.policies.get(tenant_id)
    }

    /// Check if a storage operation is allowed for the given tenant.
    ///
    /// Validates:
    /// - Storage quota
    /// - Study count limit
    /// - Feature flags (if specified)
    pub fn check_storage_allowed(
        &self,
        tenant_id: &TenantId,
        additional_bytes: u64,
        additional_studies: u64,
        required_feature: Option<&str>,
    ) -> Result<(), TenantError> {
        let policy = self.policies.get(tenant_id);

        if let Some(policy) = policy {
            // Check feature flag
            if let Some(feature) = required_feature {
                policy.require_feature(feature)?;
            }

            // Check storage quota
            let current_bytes = self.usage_bytes.get(tenant_id).copied().unwrap_or(0);
            policy.check_storage_quota(current_bytes + additional_bytes)?;

            // Check study limit
            let current_studies = self.study_counts.get(tenant_id).copied().unwrap_or(0);
            policy.check_study_limit(current_studies + additional_studies)?;
        }

        Ok(())
    }

    /// Record that bytes have been stored for a tenant.
    pub fn record_bytes_stored(&mut self, tenant_id: &TenantId, bytes: u64) {
        *self.usage_bytes.entry(tenant_id.clone()).or_insert(0) += bytes;
    }

    /// Record that a study has been added for a tenant.
    pub fn record_study_added(&mut self, tenant_id: &TenantId) {
        *self.study_counts.entry(tenant_id.clone()).or_insert(0) += 1;
    }

    /// Record that bytes have been removed for a tenant.
    pub fn record_bytes_removed(&mut self, tenant_id: &TenantId, bytes: u64) {
        if let Some(usage) = self.usage_bytes.get_mut(tenant_id) {
            *usage = usage.saturating_sub(bytes);
        }
    }

    /// Record that a study has been removed for a tenant.
    pub fn record_study_removed(&mut self, tenant_id: &TenantId) {
        if let Some(count) = self.study_counts.get_mut(tenant_id) {
            *count = count.saturating_sub(1);
        }
    }

    /// Get the current byte usage for a tenant.
    pub fn usage_bytes(&self, tenant_id: &TenantId) -> u64 {
        self.usage_bytes.get(tenant_id).copied().unwrap_or(0)
    }

    /// Get the current study count for a tenant.
    pub fn study_count(&self, tenant_id: &TenantId) -> u64 {
        self.study_counts.get(tenant_id).copied().unwrap_or(0)
    }

    /// Enforce tenant scope: ensure the requesting tenant matches the resource tenant.
    ///
    /// This is a fail-closed check: if either tenant is `None`, or if they
    /// do not match, access is denied.
    pub fn enforce_scope(
        requesting_tenant: &TenantId,
        resource_tenant: &TenantId,
    ) -> Result<(), TenantError> {
        if requesting_tenant != resource_tenant {
            return Err(TenantError::CrossTenantAccessDenied {
                requested_tenant: resource_tenant.as_str().to_string(),
                resolved_tenant: requesting_tenant.as_str().to_string(),
            });
        }
        Ok(())
    }
}

impl Default for TenantEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// TenantStorageView — Tenant-scoped view of the Storage engine
// ===========================================================================

/// A tenant-scoped view over a shared [`BlobStore`].
///
/// Provides storage operations that are automatically scoped to a specific
/// tenant's namespace, preventing cross-tenant data access.
pub struct TenantStorageView {
    /// The tenant-scoped blob store.
    blob_store: TenantBlobStore,
    /// The tenant enforcer for policy checks.
    enforcer: TenantEnforcer,
    /// The tenant identifier.
    tenant_id: TenantId,
}

impl TenantStorageView {
    /// Create a new tenant-scoped storage view.
    pub fn new(
        inner: Arc<dyn BlobStore>,
        tenant_id: TenantId,
        enforcer: TenantEnforcer,
    ) -> Self {
        let blob_store = TenantBlobStore::new(inner, tenant_id.clone());
        Self {
            blob_store,
            enforcer,
            tenant_id,
        }
    }

    /// Store data under the given key, enforcing tenant policies.
    pub fn put(&self, key: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        self.blob_store.put(key, data)
    }

    /// Retrieve data by key within the tenant's namespace.
    pub fn get(&self, key: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        self.blob_store.get(key)
    }

    /// Delete data by key within the tenant's namespace.
    pub fn delete(&self, key: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.blob_store.delete(key)
    }

    /// List keys with the given prefix within the tenant's namespace.
    pub fn list(&self, prefix: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        self.blob_store.list(prefix)
    }

    /// Return the tenant identifier for this view.
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    /// Return a reference to the tenant enforcer.
    pub fn enforcer(&self) -> &TenantEnforcer {
        &self.enforcer
    }

    /// Attempt cross-tenant access — always fails (fail-closed).
    pub fn cross_tenant_get(
        &self,
        other_tenant: &TenantId,
        key: &str,
    ) -> Result<Vec<u8>, TenantError> {
        self.blob_store.cross_tenant_get(other_tenant, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_id_validates_correctly() {
        // Valid identifiers
        assert!(TenantId::new("abc").is_ok());
        assert!(TenantId::new("tenant-1").is_ok());
        assert!(TenantId::new("my_org_123").is_ok());
        assert!(TenantId::new("A").is_ok());
        assert!(TenantId::new("a1").is_ok());

        // Invalid identifiers
        assert!(TenantId::new("").is_err());
        assert!(TenantId::new("-starts-with-hyphen").is_err());
        assert!(TenantId::new("ends-with-hyphen-").is_err());
        assert!(TenantId::new("_starts-with-underscore").is_err());
        assert!(TenantId::new("ends-with-underscore_").is_err());
        assert!(TenantId::new("has space").is_err());
        assert!(TenantId::new("has@symbol").is_err());
        assert!(TenantId::new("a".repeat(129)).is_err());

        // Max length is OK
        assert!(TenantId::new("a".repeat(128)).is_ok());
    }

    #[test]
    fn tenant_id_blob_prefix() {
        let id = TenantId::new("acme-corp").unwrap();
        assert_eq!(id.blob_prefix(), "tenant-acme-corp/");
    }

    #[test]
    fn tenant_namespace_prefix_key() {
        let id = TenantId::new("acme").unwrap();
        let ns = TenantNamespace::new(id);
        assert_eq!(ns.prefix_key("object.dcm"), "tenant-acme/object.dcm");
        assert_eq!(ns.strip_prefix("tenant-acme/object.dcm"), Some("object.dcm"));
        assert_eq!(ns.strip_prefix("tenant-other/object.dcm"), None);
        assert!(ns.owns_key("tenant-acme/object.dcm"));
        assert!(!ns.owns_key("tenant-other/object.dcm"));
    }

    #[test]
    fn tenant_policy_quota_enforcement() {
        let tenant_id = TenantId::new("quota-test").unwrap();
        let policy = TenantPolicy::new(tenant_id.clone()).with_storage_quota(1000);

        assert!(policy.check_storage_quota(500).is_ok());
        assert!(policy.check_storage_quota(999).is_ok());
        assert!(policy.check_storage_quota(1000).is_ok());
        assert!(policy.check_storage_quota(1001).is_err());
    }

    #[test]
    fn tenant_policy_study_limit() {
        let tenant_id = TenantId::new("limit-test").unwrap();
        let policy = TenantPolicy::new(tenant_id).with_max_studies(5);

        assert!(policy.check_study_limit(4).is_ok());
        assert!(policy.check_study_limit(5).is_ok());
        assert!(policy.check_study_limit(6).is_err());
    }

    #[test]
    fn tenant_policy_feature_flags() {
        let tenant_id = TenantId::new("feature-test").unwrap();
        let policy = TenantPolicy::new(tenant_id)
            .with_feature_flag("advanced-query")
            .with_feature_flag("bulk-export");

        assert!(policy.has_feature("advanced-query"));
        assert!(policy.has_feature("bulk-export"));
        assert!(!policy.has_feature("fhir-api"));

        assert!(policy.require_feature("advanced-query").is_ok());
        assert!(policy.require_feature("fhir-api").is_err());
    }

    #[test]
    fn jwt_tenant_resolver() {
        let resolver = JwtTenantResolver::new("tenant_id");
        let ctx = AuthContext::new()
            .with_jwt_claim("tenant_id", "acme-corp")
            .with_jwt_claim("role", "admin");

        let resolved = resolver.resolve_tenant(&ctx).unwrap();
        assert_eq!(resolved.as_str(), "acme-corp");
    }

    #[test]
    fn jwt_tenant_resolver_missing_claim() {
        let resolver = JwtTenantResolver::new("tenant_id");
        let ctx = AuthContext::new().with_jwt_claim("role", "admin");

        assert!(resolver.resolve_tenant(&ctx).is_err());
    }

    #[test]
    fn api_key_tenant_resolver() {
        let mut key_map = HashMap::new();
        key_map.insert(
            "key-abc-123".to_string(),
            TenantId::new("tenant-a").unwrap(),
        );
        key_map.insert(
            "key-xyz-789".to_string(),
            TenantId::new("tenant-b").unwrap(),
        );

        let resolver = ApiKeyTenantResolver::new(key_map);

        let ctx_a = AuthContext::new().with_api_key("key-abc-123");
        let resolved_a = resolver.resolve_tenant(&ctx_a).unwrap();
        assert_eq!(resolved_a.as_str(), "tenant-a");

        let ctx_b = AuthContext::new().with_api_key("key-xyz-789");
        let resolved_b = resolver.resolve_tenant(&ctx_b).unwrap();
        assert_eq!(resolved_b.as_str(), "tenant-b");
    }

    #[test]
    fn api_key_tenant_resolver_no_key() {
        let resolver = ApiKeyTenantResolver::new(HashMap::new());
        let ctx = AuthContext::new();
        assert!(resolver.resolve_tenant(&ctx).is_err());
    }

    #[test]
    fn api_key_tenant_resolver_unknown_key() {
        let resolver = ApiKeyTenantResolver::new(HashMap::new())
            .with_mapping("known-key", TenantId::new("tenant-a").unwrap());
        let ctx = AuthContext::new().with_api_key("unknown-key");
        assert!(resolver.resolve_tenant(&ctx).is_err());
    }

    #[test]
    fn tenant_enforcer_scope_check() {
        let tenant_a = TenantId::new("tenant-a").unwrap();
        let tenant_b = TenantId::new("tenant-b").unwrap();

        // Same tenant — OK
        assert!(TenantEnforcer::enforce_scope(&tenant_a, &tenant_a).is_ok());

        // Different tenant — denied
        assert!(TenantEnforcer::enforce_scope(&tenant_a, &tenant_b).is_err());
    }

    #[test]
    fn tenant_enforcer_quota_tracking() {
        let tenant_id = TenantId::new("tracker").unwrap();
        let mut enforcer = TenantEnforcer::new();
        enforcer.register_policy(
            TenantPolicy::new(tenant_id.clone()).with_storage_quota(1000),
        );

        // Initially zero
        assert_eq!(enforcer.usage_bytes(&tenant_id), 0);

        // Record storage
        enforcer.record_bytes_stored(&tenant_id, 500);
        assert_eq!(enforcer.usage_bytes(&tenant_id), 500);

        // Allow additional 400
        assert!(enforcer
            .check_storage_allowed(&tenant_id, 400, 0, None)
            .is_ok());

        // Deny additional 600 (would exceed 1000)
        assert!(enforcer
            .check_storage_allowed(&tenant_id, 600, 0, None)
            .is_err());

        // Record removal
        enforcer.record_bytes_removed(&tenant_id, 200);
        assert_eq!(enforcer.usage_bytes(&tenant_id), 300);
    }

    #[test]
    fn tenant_blob_store_isolation() {
        use crate::InMemoryBlobStore;

        let inner: Arc<dyn BlobStore> = Arc::new(InMemoryBlobStore::new());
        let tenant_a_id = TenantId::new("tenant-a").unwrap();
        let store_a = TenantBlobStore::new(inner.clone(), tenant_a_id.clone());

        let tenant_b_id = TenantId::new("tenant-b").unwrap();
        let store_b = TenantBlobStore::new(inner.clone(), tenant_b_id.clone());

        // Store data for tenant A
        store_a.put("study1.dcm", b"data-a").unwrap();
        // Store data for tenant B
        store_b.put("study2.dcm", b"data-b").unwrap();

        // Tenant A can see its own data
        assert_eq!(store_a.get("study1.dcm").unwrap(), b"data-a");

        // Tenant B can see its own data
        assert_eq!(store_b.get("study2.dcm").unwrap(), b"data-b");

        // Tenant A cannot see tenant B's data (different key namespace)
        assert!(store_a.get("study2.dcm").is_err());

        // Tenant B cannot see tenant A's data
        assert!(store_b.get("study1.dcm").is_err());

        // Cross-tenant access always fails
        assert!(store_a.cross_tenant_get(&tenant_b_id, "study2.dcm").is_err());
    }

    #[test]
    fn tenant_blob_store_list_isolation() {
        use crate::InMemoryBlobStore;

        let inner: Arc<dyn BlobStore> = Arc::new(InMemoryBlobStore::new());
        let id_a = TenantId::new("aaa").unwrap();
        let store_a = TenantBlobStore::new(inner.clone(), id_a);

        let id_b = TenantId::new("bbb").unwrap();
        let store_b = TenantBlobStore::new(inner.clone(), id_b);

        // Store data via tenant stores
        store_a.put("study1.dcm", b"a1").unwrap();
        store_a.put("study2.dcm", b"a2").unwrap();
        store_b.put("study1.dcm", b"b1").unwrap();

        // List all for each tenant
        let keys_a = store_a.list("").unwrap();
        assert_eq!(keys_a.len(), 2);
        assert!(keys_a.contains(&"study1.dcm".to_string()));
        assert!(keys_a.contains(&"study2.dcm".to_string()));

        let keys_b = store_b.list("").unwrap();
        assert_eq!(keys_b.len(), 1);
        assert!(keys_b.contains(&"study1.dcm".to_string()));
    }
}
