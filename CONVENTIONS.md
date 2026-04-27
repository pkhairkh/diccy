# diccy Code Conventions

> Established Sprint 13 (S13-T4). All new code must follow these conventions.

## Error Construction

All error types use the `dicom_core::Error` type with this pattern:

```rust
// Primary factory — auto-derives the canonical error code from the kind.
Error::from_kind(ErrorKind::SomeVariant { .. }, "human-readable message")

// Full explicit construction (override code when needed).
Error::new("CUSTOM.CODE", ErrorKind::SomeVariant { .. }, "message")

// Builder chaining for rich errors.
Error::from_kind(kind, msg)
    .with_context("key", "value")
    .with_source(underlying_error)
```

**Methods:**
- `Error::new(code, kind, message)` — explicit code, kind, and message
- `Error::from_kind(kind, message)` — derives code from kind (preferred)
- `.with_context(key, value)` — attach structured context
- `.with_source(error)` — attach causal source error

## Validation Methods

All public validation methods return `Result<()>`:

```rust
// Correct
pub fn validate(&self) -> Result<()> { ... }

// Incorrect — do not use these patterns for validation:
pub fn is_valid(&self) -> bool { ... }
pub fn check(&self) -> bool { ... }
```

Private helper predicates (e.g., `is_valid_zoom`, `is_valid_spacing`) are acceptable
when used for boolean control flow in UI or geometry code.

## Config Types

All `*Config` types must implement `Default`:

```rust
#[derive(Debug, Clone)]
pub struct SomeConfig { /* fields */ }

impl Default for SomeConfig {
    fn default() -> Self { /* sensible defaults */ }
}
```

Config types should provide:
- `::from_env()` — parse from environment variables
- `::default()` — sensible defaults
- `::builder()` — optional builder pattern for complex configs

## Domain Type Construction

Domain types with validation use `::new()` with validation:

```rust
impl SomeDomainType {
    /// Create a new instance, validating invariants.
    pub fn new(field1: &str, field2: u64) -> Result<Self> { ... }
}
```

For performance-critical paths where validation is known to be unnecessary:

```rust
    /// Create without validation. Caller guarantees invariants hold.
    ///
    /// # Safety
    /// The caller must ensure the same invariants that `new()` would check.
    pub fn new_unchecked(field1: &str, field2: u64) -> Self { ... }
```

## Error Kind Pattern

Error construction uses `Error::from_kind(kind, message)` consistently:

```rust
Error::from_kind(
    ErrorKind::DecodeError {
        stage: "my-module".to_string(),
        detail: "specific failure".to_string(),
    },
    "decode failed",
)
```

## Method Naming

| Pattern | Convention | Example |
|---------|-----------|---------|
| Validation | `validate() -> Result<()>` | `config.validate()` |
| Factory | `::new()` with validation | `Limits::new()` |
| Builder | `::builder() -> FooBuilder` | `Limits::builder()` |
| Auth reauth | Short, descriptive | `force_reauth_suspicious()` |

## Tenant Indexing

Tenant index collections use `BTreeSet<String>` for O(log n) lookups:

```rust
// Correct — efficient membership testing.
pub tenant_index: BTreeMap<String, BTreeSet<String>>

// Incorrect — linear scan for membership.
pub tenant_index: BTreeMap<String, Vec<String>>
```

## Arc<dyn Trait> Usage

When using `Arc<dyn Trait>`:

- **Read-only traits:** Document that trait methods take `&self`. Keep `Arc<dyn Trait>`.
- **Mutation needed:** Use `Arc<Mutex<dyn Trait>>` or `Arc<RwLock<dyn Trait>>`.
- **Always** add `Send + Sync` bounds on trait objects.

## Type Naming Conventions

All types follow consistent naming patterns:

| Category | Suffix | Examples |
|----------|--------|---------|
| Configuration | `*Config` | `DicomWebServiceConfig`, `S3Config`, `RdvfConfig`, `ReaderConfig`, `VolumeAssemblyConfig` |
| Builder | `*Builder` | `RdvfConfigBuilder`, `LimitsBuilder` |
| Error | `*Error` | `ViewerError`, `VolumeError`, `ClinicalError`, `FhirAdapterError` |
| Request | `*Request` | `StorageCommitmentRequest`, `BreakGlassRequest` |
| Response/Result | `*Result` | `CalciumScoreResult`, `EjectionFractionResult` |
| Policy | `*Policy` | `RetentionPolicy`, `SessionPolicy`, `WebPolicy` |
| State | `*State` | `DicomWebRouteState`, `FusionOverlayState`, `SessionStatus` |
| Newtype (domain) | Domain name | `Uid`, `AeTitle`, `SopClassUid`, `MeasurementId`, `Timestamp`, `MonotonicTick` |

**Deprecated aliases** are provided for backward compatibility:
- `VolumeAssemblyOptions` → `VolumeAssemblyConfig` (deprecated since 0.14.0)
- `ReaderOptions` → `ReaderConfig` (deprecated since 0.14.0)
- `Config` → `RdvfConfig` (deprecated since 0.14.0)
- `ConfigBuilder` → `RdvfConfigBuilder` (deprecated since 0.14.0)

## Crate Naming Conventions

| Prefix | Purpose | Examples |
|--------|---------|---------|
| `dicom-` | DICOM protocol/domain crates | `dicom-core`, `dicom-io`, `dicom-web`, `dicom-auth` |
| `pack-` | DICOM IOD pack crates | `pack-gsps`, `pack-seg`, `pack-rt`, `pack-shared` |
| `modality-` | Modality-specific packs | `modality-ct`, `modality-pet`, `modality-mg`, `modality-cr` |
| `viewer-` | Viewer stack | `viewer-core`, `viewer-wgpu`, `viewer-wasm` |
| `rdvf` | Public API facade (no prefix) | `rdvf` |

## Route Capability Pattern

DICOMweb route capabilities use the type-safe `DicomWebRoute` enum + `BTreeMap`:

```rust
// Get the capability matrix
let matrix = dicomweb_route_capability_matrix();

// Check if a specific route is enabled
if is_route_enabled(&matrix, DicomWebRoute::WadoInstanceRetrieveGet) {
    // ...
}

// Access route metadata
let path = DicomWebRoute::WadoInstanceRetrieveGet.path();
let method = DicomWebRoute::WadoInstanceRetrieveGet.method();
let state = matrix.get(&DicomWebRoute::WadoInstanceRetrieveGet);
```
