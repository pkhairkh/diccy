# S15-T1: Multi-tenancy in dicom-storage and dicom-index

## Task Summary
Implemented multi-tenancy support across the dicom-storage crate, including tenant identification, policy enforcement, storage namespace isolation, and authentication-based tenant resolution.

## Files Created
- `/home/z/diccy/crates/dicom-storage/src/tenant.rs` — Core multi-tenancy module
- `/home/z/diccy/crates/dicom-storage/tests/multi_tenant.rs` — 55 integration tests

## Files Modified
- `/home/z/diccy/crates/dicom-storage/src/lib.rs` — Added `mod tenant`, re-exports, tenant fields on `Storage`, tenant methods, tenant-aware blob operations on `FileBlobStore`
- `/home/z/diccy/crates/dicom-storage/src/s3_backend.rs` — Added `object_key_for_tenant` on `S3Config`, tenant-scoped CRUD methods on `S3Backend`
- `/home/z/diccy/crates/dicom-storage/Cargo.toml` — Added `serde_json` dependency and dev-dependency

## Implementation Details

### TenantId (validated newtype)
- Wraps `String` with validation: non-empty, 1-128 chars, alphanumeric + hyphens/underscores, must start/end with alphanumeric
- Provides `as_str()`, `blob_prefix()` (returns `tenant-{id}/`), `Display`, serde roundtrip

### TenantPolicy
- Per-tenant configuration: `storage_quota_bytes`, `retention_policy`, `feature_flags`, `max_studies`, `rate_limit_per_minute`
- Builder pattern for construction
- Enforcement methods: `check_storage_quota()`, `check_study_limit()`, `require_feature()`

### TenantNamespace
- Encapsulates tenant ID + blob prefix
- Methods: `prefix_key()`, `strip_prefix()`, `owns_key()`

### TenantResolver trait + implementations
- `JwtTenantResolver` — extracts tenant from JWT claim by key name
- `ApiKeyTenantResolver` — maps API keys to tenants
- `CompositeTenantResolver` — chains multiple resolvers with fallback

### TenantBlobStore
- Wraps `Arc<dyn BlobStore>` with automatic key prefixing
- All CRUD operations scoped to tenant namespace
- `cross_tenant_get()` always returns error (fail-closed)

### TenantEnforcer
- Tracks per-tenant byte usage and study counts
- `check_storage_allowed()` validates quota, study limits, feature flags
- `enforce_scope()` static method for fail-closed cross-tenant access denial

### TenantStorageView
- Combines `TenantBlobStore` + `TenantEnforcer` for a complete tenant-scoped storage interface

### Storage struct updates
- Added `current_tenant: Option<TenantId>` and `tenant_policies: HashMap<TenantId, TenantPolicy>`
- Methods: `with_tenant()`, `current_tenant()`, `register_tenant_policy()`, `current_tenant_policy()`, `tenant_policy()`, `store_for_tenant()`, `check_tenant_access()`, `ingest_bytes_tenant_aware()`
- Backward compatible: when no tenant configured, system works as before

### S3Backend tenant methods
- `put_tenant_object()`, `get_tenant_object()`, `delete_tenant_object()`, `list_tenant_objects()`
- `S3Config::object_key_for_tenant()` for tenant-scoped S3 key computation

### FileBlobStore tenant methods
- `put_tenant()`, `get_tenant()`, `delete_tenant()`, `list_tenant()`, `tenant_key_path()`

## Test Results
All 81 tests pass (15 unit + 3 hi-storage + 8 inline + 55 multi_tenant):
- `cargo test -p dicom-storage` — 81/81 passed
- `cargo test -p dicom-storage -- multi_tenant` — 55/55 passed

## Design Decisions
1. **Fail-closed**: All tenant checks deny access when tenant cannot be resolved
2. **Backward compatible**: No tenant configured = single-tenant mode (existing tests unchanged)
3. **Arc-based sharing**: `TenantBlobStore` uses `Arc<dyn BlobStore>` so multiple tenants can share one backend
4. **Namespace isolation via prefixing**: `tenant-{id}/` prefix on all blob keys provides natural S3/filesystem/in-memory isolation
