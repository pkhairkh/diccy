//! S3-Compatible Object Storage Backend.
//!
//! Contains S3 configuration, simulated backend, and multipart upload types.

use dicom_core::{Error, ErrorKind, Result};
use secrecy::{ExposeSecret, SecretString};
use std::collections::BTreeMap;

/// S3 backend configuration for object storage.
///
/// Secret fields (`access_key_id`, `secret_access_key`) are stored as
/// `SecretString` from the `secrecy` crate to prevent accidental leakage
/// via Debug, serialization, or log output.
#[derive(Clone)]
pub struct S3Config {
    /// S3 endpoint URL (e.g., "https://s3.amazonaws.com" or MinIO endpoint).
    pub endpoint: String,
    /// Bucket name for DICOM storage.
    pub bucket: String,
    /// AWS region.
    pub region: String,
    /// Access key ID (protected by `SecretString`).
    access_key_id: SecretString,
    /// Secret access key (protected by `SecretString`).
    secret_access_key: SecretString,
    /// Whether to use path-style addressing (required for MinIO).
    pub path_style: bool,
    /// Multipart upload threshold in bytes (default: 100 MB).
    pub multipart_threshold_bytes: u64,
    /// Multipart upload part size in bytes (default: 10 MB).
    pub multipart_part_size_bytes: u64,
}

impl PartialEq for S3Config {
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
            && self.bucket == other.bucket
            && self.region == other.region
            && self.access_key_id.expose_secret() == other.access_key_id.expose_secret()
            && self.secret_access_key.expose_secret() == other.secret_access_key.expose_secret()
            && self.path_style == other.path_style
            && self.multipart_threshold_bytes == other.multipart_threshold_bytes
            && self.multipart_part_size_bytes == other.multipart_part_size_bytes
    }
}

impl std::fmt::Debug for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("access_key_id", &"[REDACTED]")
            .field("secret_access_key", &"[REDACTED]")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .finish()
    }
}

impl Default for S3Config {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            bucket: String::new(),
            region: "us-east-1".to_string(),
            access_key_id: SecretString::new(String::new().into()),
            secret_access_key: SecretString::new(String::new().into()),
            path_style: false,
            multipart_threshold_bytes: 100 * 1024 * 1024,
            multipart_part_size_bytes: 10 * 1024 * 1024,
        }
    }
}

impl S3Config {
    /// Create a new S3 configuration.
    pub fn new(endpoint: &str, bucket: &str, region: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            bucket: bucket.to_string(),
            region: region.to_string(),
            ..Self::default()
        }
    }

    /// Return the access key ID (exposed from SecretString; use with caution).
    pub fn access_key_id(&self) -> &str {
        self.access_key_id.expose_secret()
    }

    /// Return a redacted representation of the access key ID.
    /// Shows at most the first 4 characters followed by `***`.
    pub fn access_key_id_redacted(&self) -> String {
        redact_secret(self.access_key_id.expose_secret())
    }

    /// Return the secret access key (exposed from SecretString; use with caution).
    pub fn secret_access_key(&self) -> &str {
        self.secret_access_key.expose_secret()
    }

    /// Return a redacted representation of the secret access key.
    /// Always returns `[REDACTED]` regardless of value.
    pub fn secret_access_key_redacted(&self) -> &'static str {
        "[REDACTED]"
    }

    /// Set the access key ID (wrapped in SecretString).
    pub fn set_access_key_id(&mut self, value: impl Into<String>) {
        self.access_key_id = SecretString::new(value.into().into());
    }

    /// Set the secret access key (wrapped in SecretString).
    pub fn set_secret_access_key(&mut self, value: impl Into<String>) {
        self.secret_access_key = SecretString::new(value.into().into());
    }

    /// Validate the S3 configuration.
    pub fn validate(&self) -> Result<()> {
        if self.endpoint.is_empty() {
            return Err(storage_ext_error("S3 endpoint must not be empty"));
        }
        if self.bucket.is_empty() {
            return Err(storage_ext_error("S3 bucket must not be empty"));
        }
        Ok(())
    }

    /// Compute the S3 object key for a given canonical hash.
    pub fn object_key(&self, hash: &str) -> String {
        // Use a two-level prefix for better S3 performance
        format!("{}/{}/{}", &hash[0..2], &hash[2..4], hash)
    }

    /// Compute the S3 object key for a given canonical hash, scoped to a tenant.
    ///
    /// The tenant prefix is prepended, ensuring tenant isolation in S3.
    /// For example, tenant `acme` with hash `abcdef...` produces:
    /// `tenant-acme/ab/cd/abcdef...`
    pub fn object_key_for_tenant(&self, tenant_id: &super::tenant::TenantId, hash: &str) -> String {
        format!("{}{}", tenant_id.blob_prefix(), self.object_key(hash))
    }
}

fn redact_secret(value: &str) -> String {
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &value[..4])
    }
}

/// Outcome of an S3 multipart upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUploadResult {
    /// S3 object key.
    pub object_key: String,
    /// Total bytes uploaded.
    pub total_bytes: u64,
    /// Number of parts uploaded.
    pub parts_count: usize,
    /// Upload ID from S3.
    pub upload_id: String,
}

/// Simulated S3 backend for testing (no actual network calls).
///
/// **STUB:** This implementation is not production-ready. It uses in-memory
/// storage instead of real S3 network calls. Using this in production will
/// result in data loss on restart and no actual cloud persistence.
/// Set the `DICCY_ALLOW_STUBS` environment variable to acknowledge this.
#[derive(Debug, Clone, PartialEq)]
pub struct S3Backend {
    /// Configuration.
    config: S3Config,
    /// Simulated stored objects (object_key -> data).
    stored_objects: BTreeMap<String, Vec<u8>>,
    /// Lifecycle policy for tiered storage.
    lifecycle_policy: super::vna::LifecyclePolicy,
}

impl S3Backend {
    /// Create a new S3 backend with the given configuration.
    pub fn new(config: S3Config) -> Self {
        Self {
            config,
            stored_objects: BTreeMap::new(),
            lifecycle_policy: super::vna::LifecyclePolicy::default(),
        }
    }

    /// Store data in the S3 backend (simulated).
    ///
    /// # Panics
    /// Panics if the `DICCY_ALLOW_STUBS` environment variable is not set,
    /// since this stub should not be used in production.
    pub fn put_object(&mut self, key: &str, data: Vec<u8>) -> Result<()> {
        assert_stub_allowed();
        self.stored_objects.insert(key.to_string(), data);
        Ok(())
    }

    /// Retrieve data from the S3 backend (simulated).
    pub fn get_object(&self, key: &str) -> Option<&[u8]> {
        self.stored_objects.get(key).map(|v| v.as_slice())
    }

    /// Delete an object from the S3 backend (simulated).
    pub fn delete_object(&mut self, key: &str) -> bool {
        self.stored_objects.remove(key).is_some()
    }

    /// Check if an object exists.
    pub fn object_exists(&self, key: &str) -> bool {
        self.stored_objects.contains_key(key)
    }

    /// Return the number of stored objects.
    pub fn object_count(&self) -> usize {
        self.stored_objects.len()
    }

    /// Return total bytes stored.
    pub fn total_bytes(&self) -> u64 {
        self.stored_objects.values().map(|v| v.len() as u64).sum()
    }

    /// Simulate multipart upload for large objects.
    pub fn multipart_upload(&mut self, key: &str, data: Vec<u8>) -> Result<MultipartUploadResult> {
        let total_bytes = data.len() as u64;
        let parts_count = if total_bytes > self.config.multipart_threshold_bytes {
            ((total_bytes + self.config.multipart_part_size_bytes - 1)
                / self.config.multipart_part_size_bytes) as usize
        } else {
            1
        };

        self.stored_objects.insert(key.to_string(), data);

        Ok(MultipartUploadResult {
            object_key: key.to_string(),
            total_bytes,
            parts_count,
            upload_id: format!("upload-{}", self.stored_objects.len()),
        })
    }

    /// Return the S3 configuration.
    pub fn config(&self) -> &S3Config {
        &self.config
    }

    /// Apply lifecycle policy and return the number of objects transitioned.
    pub fn apply_lifecycle(&mut self) -> usize {
        self.lifecycle_policy.apply(&mut self.stored_objects)
    }

    /// Set the lifecycle policy.
    pub fn set_lifecycle_policy(&mut self, policy: super::vna::LifecyclePolicy) {
        self.lifecycle_policy = policy;
    }

    // ===================================================================
    // Multi-tenant methods
    // ===================================================================

    /// List all object keys that belong to a specific tenant.
    ///
    /// Returns only keys that start with the tenant's prefix, with the
    /// tenant prefix stripped from the returned keys.
    pub fn list_tenant_objects(
        &self,
        tenant_id: &super::tenant::TenantId,
    ) -> Vec<String> {
        let prefix = tenant_id.blob_prefix();
        self.stored_objects
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .filter_map(|k| k.strip_prefix(&prefix).map(|s| s.to_string()))
            .collect()
    }

    /// Store data under a tenant-scoped key.
    ///
    /// Automatically prefixes the key with the tenant's namespace.
    pub fn put_tenant_object(
        &mut self,
        tenant_id: &super::tenant::TenantId,
        key: &str,
        data: Vec<u8>,
    ) -> Result<()> {
        let full_key = format!("{}{}", tenant_id.blob_prefix(), key);
        self.put_object(&full_key, data)
    }

    /// Retrieve data from a tenant-scoped key.
    ///
    /// Automatically prefixes the key with the tenant's namespace.
    pub fn get_tenant_object(
        &self,
        tenant_id: &super::tenant::TenantId,
        key: &str,
    ) -> Option<&[u8]> {
        let full_key = format!("{}{}", tenant_id.blob_prefix(), key);
        self.get_object(&full_key)
    }

    /// Delete an object from a tenant-scoped key.
    ///
    /// Automatically prefixes the key with the tenant's namespace.
    pub fn delete_tenant_object(
        &mut self,
        tenant_id: &super::tenant::TenantId,
        key: &str,
    ) -> bool {
        let full_key = format!("{}{}", tenant_id.blob_prefix(), key);
        self.delete_object(&full_key)
    }
}

/// Assert that stub implementations are explicitly allowed via the `DICCY_ALLOW_STUBS`
/// environment variable. Panics if the variable is not set, preventing accidental
/// use of stub code in production deployments.
fn assert_stub_allowed() {
    if std::env::var("DICCY_ALLOW_STUBS").is_err() {
        panic!(
            "STUB: S3Backend is a simulated implementation not suitable for production. \
             Set the DICCY_ALLOW_STUBS environment variable to explicitly opt in."
        );
    }
}

fn storage_ext_error(detail: impl Into<String>) -> Box<Error> {
    Error::from_kind(
        ErrorKind::DecodeError {
            stage: "dicom-storage-ext".to_string(),
            detail: detail.into(),
        },
        "storage extension error",
    )
    .into()
}
