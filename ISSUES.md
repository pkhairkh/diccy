# ISSUES.md — Architecture & Code Design Audit

> Comprehensive evaluation of the diccy codebase with emphasis on
> Domain-Driven Design (DDD) principles, encapsulation, cohesion,
> coupling, and structural health. Based on a full source-code audit
> of all 48 workspace crates (102,346 lines of Rust).

---

## Severity Key

| Level | Meaning |
|---|---|
| **Critical** | Architectural flaw that will block production use or make safe evolution impossible |
| **High** | Systemic design problem that causes ongoing maintenance burden or risk |
| **Medium** | Localized design smell that should be fixed in a refactoring pass |
| **Low** | Minor style or consistency issue |

---

## 1. Pervasive Encapsulation Violation — Every Struct Has `pub` Fields

**Severity: CRITICAL**

### Problem

Nearly every struct in the codebase exposes all fields as `pub`, allowing any consumer to construct instances in invalid states or mutate invariants without going through behavioral methods. This is the single most damaging pattern for long-term codebase health.

### Evidence (representative samples)

| Crate | Type | Pub Fields | Invalid State Possible |
|---|---|---|---|
| `dicom-core` | `Element` | `tag`, `vr`, `value` | `Vr::Ui` + `Value::I32(42)` — VR/value mismatch |
| `dicom-core` | `Limits` | All 10 fields | `max_input_bytes = 0` — denial of service |
| `dicom-core` | `Capabilities` | All 16 fields | `codec_j2k = true` without the feature flag compiled |
| `dicom-core` | `Error` | `code`, `kind`, `message`, `context`, `source` | Code not matching `ErrorKind` |
| `dicom-auth` | `SessionStatus` | `locked`, `failed_attempts`, `last_activity_epoch_secs` | Bypass state machine by setting `locked = false` directly |
| `dicom-auth` | `SessionPolicy` | `inactivity_timeout_secs` | `0` timeout — session never expires |
| `dicom-net` | `PresentationContext` | `id` | `id = 0` (must be odd per DICOM spec) |
| `dicom-net` | `AssociationReject` | `result`, `source`, `reason` | Raw `u8` — any value accepted |
| `dicom-storage` | `S3Config` | `access_key_id`, `secret_access_key` | **Secrets exposed as plain `String`**, debug-printable and serializable |
| `dicom-storage` | `LifecyclePolicy` | All 5 fields | `ia_transition_days > glacier_transition_days` — invalid tiering |
| `dicom-storage` | `RetentionPolicy` | All 5 fields | `min_retention_days > max_retention_days` — logical contradiction |
| `dicom-dimse-service` | `DimseServerConfig` | ~15 fields | Inconsistent TLS + transport policy |
| `viewer-core` | `MeasurementRecord` | All 10 fields | `deleted = true` but still in active list; `revision` set to any value |
| `viewer-core` | `ViewerModel` | All 14 fields | `measurement_counter` can be decremented; `measurements` directly mutated |
| `viewer-core` | `FusionOverlayState` | `visible`, `blend`, `registration` | `blend = 5.0` — outside `[0,1]` invariant |
| `viewer-core` | `RtDoseOverlayState` | `visible`, `window_min`, `window_max` | `window_min >= window_max` — no validation at construction |
| `dicom-xr` | `HeadPose` | `position`, `orientation`, `timestamp_s` | Non-unit quaternion accepted despite `validate()` existing |
| `dicom-mesh` | `TriangleMesh` | `vertices`, `triangles`, `normals` | Indices referencing out-of-bounds vertices |

### Impact

- **Impossible to reason about invariants**: Any function receiving one of these types cannot trust its state. Defensive validation must be repeated at every call site.
- **Refactoring is unsafe**: Making a field private breaks all direct-access consumers. The codebase is locked into its current structure.
- **Concurrency is impossible**: With public mutable fields, no `&self` method can guarantee the object won't be mutated concurrently.

### Recommendation

1. Make all fields `pub(crate)` or private by default.
2. Provide validated constructors (`::new()`, `::builder()`) that enforce invariants at construction.
3. Add getter methods for read access.
4. Add setter methods that validate transitions.
5. Start with the most critical types: `Element`, `Limits`, `Error`, `SessionStatus`, `DimseServerConfig`, `S3Config`.

---

## 2. Anemic Domain Model — Types Are Data Holders Without Behavior

**Severity: HIGH**

### Problem

The vast majority of types across all 48 crates are pure data holders. Business logic lives in free functions or "service/engine" types that operate on the data externally, rather than being methods on the domain entities themselves. This is the textbook definition of the Anemic Domain Model anti-pattern.

### Evidence

| Crate | Type | Has Behavior? | Notes |
|---|---|---|---|
| `dicom-core` | `Element` | No | No method enforces VR/value consistency |
| `dicom-core` | `Dataset` | Minimal | `get()`, `insert()` exist but `insert()` has no validation |
| `dicom-core` | `Limits` | No | No `validate()`, no `new()` with constraints |
| `dicom-audit` | `AuditEvent`, `AuditRecord`, `AuditField` | No | Pure data; `AuditLog` has the behavior |
| `dicom-auth` | `AuthSubject`, `AuthResource`, `AuthRequest` | No | Data holders consumed by `Authorizer` trait |
| `dicom-net` | `AssociationRequest`, `AssociationAccept`, `Pdv` | No | All protocol PDUs are data-only |
| `dicom-query` | `Query`, `QueryMatch`, `QueryKey` | No | Free function `query()` operates on them |
| `dicom-storage` | `WalEntry`, `StorageCommitmentRequest`, `VnaStudyRecord` | No | Pure data; engines have the logic |
| `dicom-fhir` | `FhirPatient`, `FhirImagingStudy`, `FhirObservation` | No | All FHIR resources are data-only |
| `dicom-hl7` | `AdtMessage`, `OrmMessage`, `OruMessage` | No | All HL7 messages are data-only |
| `dicom-inference` | `ModelManifest`, `InferenceOutput` | Minimal | `validate()` exists but fields are still `pub` |
| `dicom-collab` | `CollabSession`, `UserPresence`, `ViewportSyncState` | Minimal | Session has some methods, but most types are data |
| `dicom-cardio` | `CalcifiedLesion`, `VesselCenterline`, `EjectionFractionResult` | No | Pure data |
| `dicom-xr` | `HeadPose`, `XrViewportState`, `ArOverlayState` | Minimal | `validate()` exists but fields are `pub` |
| `dicom-mesh` | `TriangleMesh` | Some | `compute_normals()`, `validate()` exist — good start |
| `viewer-core` | `MeasurementRecord`, `SegmentationRecord`, `Annotation3d` | No | Stores have the behavior, records are anemic |

### Notable Exceptions (doing it right)

- **`MeasurementStore`** — Has `create_measurement()`, `update_measurement()`, `delete_measurement()`, `undo()`, `redo()`, `snapshot()`, `export_bundle()`. This is a proper aggregate root.
- **`SegmentationStore`** — Has `import_seg()`, `create_labelmap()`, `update_style()`, `set_locked()`, `remove()`. Also a proper aggregate root.
- **`VolumeWorkflowState`** — Has `set_mpr_enabled()`, `set_mip_mode()`, `set_volume_3d_enabled()` with capability gating. Good state encapsulation.
- **`ViewerModel`** — Has `apply_event()` state machine, `step_frame()`, `set_mpr_crosshair_voxel()`. Proper behavioral model.
- **`Index`** (dicom-index) — Private fields, validated construction, behavioral methods. The best-encapsulated type in the codebase.

### Impact

- Business rules are scattered across free functions, making them hard to find and easy to duplicate.
- No type can protect its own invariants (see Issue 1).
- Testing requires assembling valid object graphs manually rather than using factory methods.

### Recommendation

1. Move validation and business logic into domain types as methods.
2. Make `Element` enforce VR/value consistency on construction.
3. Make `Limits` validate itself in `::new()`.
4. Make protocol PDU types (`AssociationRequest`, `DimseMessage`) validate DICOM constraints in constructors.
5. Follow the `MeasurementStore` / `Index` pattern for new types.

---

## 3. Massive DRY Violations — Duplicated Utility Functions

**Severity: HIGH**

### Problem

Core utility functions are copy-pasted across 20+ crates with minor variations. The same `decode_error()`, `enforce_limit()`, `limit_exceeded()`, `missing_required_tag()`, and `require_uid()` functions appear in nearly every crate independently.

### Duplication Map

| Function | Duplicated In | Count |
|---|---|---|
| `decode_error()` | dicom-core, dicom-io, dicom-net, dicom-dimse, dicom-dimse-service, dicom-query, dicom-worklist, dicom-mpps, dicom-ups, dicom-web, dicom-pixel, dicom-visualizer, dicom-workflow-server | **13** |
| `enforce_limit()` | dicom-core, dicom-io, dicom-net, dicom-dimse, dicom-dimse-service, dicom-web, dicom-pixel | **7** |
| `limit_exceeded()` | dicom-core, dicom-io, dicom-net, dicom-dimse-service, dicom-web, dicom-workflow-server | **6** |
| `missing_required_tag()` | dicom-io, dicom-query, dicom-index, dicom-storage, dicom-worklist, dicom-mpps, dicom-pixel, dicom-visualizer, pack-rt, pack-seg, modality-pet | **11** |
| `require_uid()` | dicom-index, dicom-query, dicom-mpps, dicom-visualizer | **4** |
| `parse_uid()` | dicom-io, dicom-net, dicom-dimse | **3** |

### Impact

- Bug fixes must be applied in 6–13 places. Any missed copy creates divergent behavior.
- Each copy has a slightly different signature (`stage` parameter, `limits` parameter), making unification harder over time.
- The `decode_error()` in `dicom-auth` uses `ErrorKind::DecodeError` for authorization denials — a semantic misuse caused by copying from a parsing crate.

### Recommendation

1. Create a `dicom-util` crate (or extend `dicom-core`) that re-exports these shared helpers.
2. `decode_error()` should be a method on `ErrorKind` or a factory on `Error`, not a free function duplicated everywhere.
3. `missing_required_tag()` and `require_uid()` belong in `dicom-core` since they operate on `Dataset` and `Tag`.
4. `parse_uid()` should be on the proposed `Uid` newtype (see Issue 5).
5. Replace all local copies with imports from the shared location.

---

## 4. Missing Domain Newtypes — Primitive Obsession

**Severity: HIGH**

### Problem

Critical DICOM domain concepts are represented as raw primitives (`String`, `u8`, `u16`, `u64`) throughout the codebase, losing type safety and validation at the boundary. Two functions that both take `String` but mean different things (UID vs. AE Title vs. SOP Class UID) can be called interchangeably.

### Missing Newtypes

| Primitive | Should Be | Used In | Risk |
|---|---|---|---|
| `String` for DICOM UIDs | `Uid` newtype with validation | **Every crate** (42 of 48) | Invalid UIDs silently accepted; `validate_uid_strict()` called inconsistently |
| `String` for AE Titles | `AeTitle` newtype (16-byte max, ASCII) | dicom-net, dicom-dimse, dicom-dimse-service | Non-ASCII or oversized AE titles silently accepted |
| `String` for SOP Class UIDs | `SopClassUid` newtype with known-UID registry | dicom-core, dicom-io, dicom-dimse, pack-\* | Typos in UID strings not caught at compile time |
| `String` for Transfer Syntax UIDs | `TransferSyntaxUid` newtype | dicom-io, dicom-net | Unknown transfer syntaxes silently accepted |
| `u16` for DIMSE status | `DimseStatus` enum with named variants | dicom-dimse, dicom-dimse-service | Unknown status codes silently accepted |
| `u16` for DIMSE priority | `Priority` enum (`Low=0x0002, Medium=0x0000, High=0x0001`) | dicom-dimse | Invalid priority values possible |
| `u8` for Association reject reason | `RejectReason` enum | dicom-net | Raw `u8` has no semantic meaning |
| `u8` for Presentation Context result | `AcceptResult` enum | dicom-net | `0x00` = accepted, `0x01`/`0x02` = abstract/transfer syntax rejected |
| `String` for S3 credentials | `SecretString` (zeroize on drop) | dicom-storage | Credentials visible in debug output, heap dumps, and serialized JSON |
| `u64` for timeouts | `NonZeroU64` or `Duration` | dicom-auth, dicom-net, dicom-dimse-service | Zero timeout = infinite wait or instant expiry |
| `u64` for epoch timestamps | `Timestamp` newtype | dicom-auth, dicom-audit | No timezone handling, no validation |
| `String` for measurement IDs | `MeasurementId` newtype | viewer-core | Any string accepted; format not enforced |

### Impact

- Compile-time type safety is lost. Swapping a UID and an AE Title compiles fine.
- Validation is done inconsistently — some paths validate, some don't.
- `S3Config::secret_access_key: String` derives both `Debug` and `Serialize`, meaning secrets can leak via `{:?}` or JSON serialization.
- The `Uid` newtype is the most impactful single fix — it affects 42 crates.

### Recommendation

1. **Highest priority**: Add `Uid` newtype to `dicom-core` with `validate_uid_strict()` baked into `::new()`.
2. Add `AeTitle`, `SopClassUid`, `TransferSyntaxUid` newtypes to `dicom-core`.
3. Add `DimseStatus` and `Priority` enums to `dicom-dimse`.
4. Change `S3Config` secrets to `SecretString` and redact `Debug` impl.
5. Use `NonZeroU64` for all timeout/duration fields.

---

## 5. Bounded Context Violations — Crates Mixing Unrelated Domains

**Severity: CRITICAL**

### Problem

Several crates contain multiple distinct bounded contexts (in the DDD sense) that should be separated. A bounded context defines a boundary within which a particular domain model applies. When contexts are mixed, concepts from one domain leak into another, creating coupling that makes independent evolution impossible.

### Violations

#### 5a. `dicom-storage` — 4 Bounded Contexts in 1 Crate

| Context | Types | Should Be |
|---|---|---|
| WAL-based ingestion | `WalEntry`, `Storage`, `deduplicate()` | Core storage crate |
| Storage Commitment | `StorageCommitmentRequest`, `StorageCommitmentEventJob`, `StorageCommitmentPolicy` | `dicom-storage-commitment` crate |
| S3 object backend | `S3Config`, `S3Backend`, `MultipartUploadResult` | `dicom-storage-s3` crate or infrastructure adapter |
| VNA lifecycle | `VnaEngine`, `VnaStudyRecord`, `RetentionPolicy`, `LifecyclePolicy` | `dicom-vna` crate |

These four contexts have different change rates, different regulatory concerns, and different deployment patterns. The S3 backend is an infrastructure concern that should be a pluggable adapter behind a trait. VNA retention policies are an enterprise governance domain that doesn't belong in a core storage crate.

#### 5b. `dicom-auth` — 6+ Bounded Contexts in `human_interface.rs` (1,023 lines)

| Context | Types | Lines |
|---|---|---|
| Session management | `SessionPolicy`, `SessionStatus`, `force_reauthentication_for_suspicious_session()` | ~150 |
| Configuration change control | `ConfigChangeJournalEntry`, `ConfigChangeJournal` | ~150 |
| Break-glass escalation | `BreakGlassPolicy`, `BreakGlassRequest`, `BreakGlassOutcome` | ~100 |
| UI claim-surface governance | `UiClaimSurface`, `ClaimedRect`, `ClaimConflictResolution` | ~150 |
| Interface change control | `InterfaceChangeRecord`, `RequirementRevision`, `ScreenshotExportPolicy` | ~200 |
| Data export policy | `ClipboardPolicy`, `RemovableMediaPolicy`, `WorkspacePrivacyMode` | ~200 |

This file is a grab bag of security, UI governance, and regulatory concerns. These have no business being in the same module, let alone the same file.

#### 5c. `dicom-dimse-service` — 7 Bounded Contexts in 1 Crate

| Context | Types | Should Be |
|---|---|---|
| DIMSE protocol (C-ECHO/C-STORE) | `DimseService` trait, `StorageBackedDimseService` | Core service crate |
| C-FIND/C-MOVE/C-GET | Feature-gated handlers | Separate module or crate |
| Storage Commitment lifecycle | `StorageCommitmentNActionRequest`, `StorageCommitmentLifecycleEvent` | `dicom-storage-commitment` crate |
| IAN notifications | `IanNotification` | Separate module |
| TLS configuration | `DimseTlsMaterialConfig` | Infrastructure adapter |
| Rate limiting | `DimseOperationSizeConfig`, `DimseOperationResourceLimits` | Middleware crate |
| Network I/O | `DimseServer` (TCP listener, thread::spawn) | Transport abstraction layer |

#### 5d. `dicom-workflow-server` — 16,561-line `main.rs`

The single `main.rs` file contains at least:

| Context | Estimated Lines |
|---|---|
| Runtime configuration and env parsing | ~800 |
| HL7 transport and subscription management | ~1,500 |
| Tenant management (policies, quotas, rate limits) | ~1,200 |
| Study reconciliation and IAN events | ~800 |
| SR workflow engine | ~1,000 |
| Storage commitment tracking | ~600 |
| Connector plugin system | ~1,000 |
| Health checks and monitoring | ~400 |
| Worker pool and thread management | ~600 |
| Test infrastructure (10,000+ lines of tests inline) | ~10,000 |

This is a monolith within a microservice. It should be decomposed into at least 5–6 separate modules, with `main.rs` being a thin orchestrator.

#### 5e. `dicom-web` — 5,317-line `lib.rs`

This crate combines:
- QIDO-RS query engine
- WADO-RS retrieval engine
- STOW-RS storage engine
- WADO-URI legacy support
- Authentication/authorization middleware
- HTTP routing and content negotiation

These should be separate modules at minimum, with a thin facade re-exporting the public API.

### Recommendation

1. Split `dicom-storage` into `dicom-storage` (core WAL), `dicom-storage-commitment`, `dicom-storage-s3`, and `dicom-vna`.
2. Split `dicom-auth/human_interface.rs` into separate modules: `session`, `config_control`, `break_glass`, `claim_surface`, `interface_control`, `export_policy`.
3. Decompose `dicom-dimse-service` into protocol, commitment, and transport layers.
4. Break `dicom-workflow-server/main.rs` into at least `config`, `hl7`, `tenant`, `reconciliation`, `commitment`, `health`, `workers` modules.
5. Split `dicom-web/lib.rs` into `qido`, `wado`, `stow`, `auth_middleware`, `router` modules.

---

## 6. God Objects and Oversized Files

**Severity: HIGH**

### Problem

Several files have grown to sizes that make them unmaintainable. These files contain too many concerns, too many types, and too many functions to reason about effectively.

### Worst Offenders

| File | Lines | Types Defined | Primary Concern |
|---|---|---|---|
| `dicom-workflow-server/src/main.rs` | **16,561** | 30+ structs/enums | Everything |
| `dicom-web/src/lib.rs` | **5,317** | 40+ structs/enums | All DICOMweb endpoints |
| `dicom-dimse-service/src/lib.rs` | **4,463** | 20+ structs/enums | DIMSE protocol + storage + TLS |
| `dicom-pixel/src/lib.rs` | **3,072** | 15+ structs/enums | All codec support |
| `viewer-core/src/clinical.rs` | **2,656** | 25+ structs/enums | All clinical workflow types |
| `dicom-io/src/lib.rs` | **2,525** | 10+ structs/enums | All transfer syntax handling |
| `dicom-web-server/src/main.rs` | **2,254** | 15+ structs/enums | HTTP server + routing |
| `pack-gsps/src/lib.rs` | **2,367** | 10+ structs/enums | GSPS encoding |
| `pack-seg/src/lib.rs` | **2,093** | 8+ structs/enums | SEG encoding |
| `viewer-wgpu/src/volume_renderer.rs` | **2,035** | 10+ structs/enums | All GPU rendering |

### Impact

- Compilation times for these files are disproportionately long.
- Changes to any part of the file require recompiling the entire module.
- Code review is impractical — no one can effectively review a 16,000-line diff.
- Merge conflicts are frequent when multiple developers work on the same module.

### Recommendation

1. **Immediate**: Split files over 3,000 lines into submodules.
2. Target 500–800 lines per file as a maximum.
3. Use `mod.rs` to re-export public API so consumers don't see the decomposition.

---

## 7. No Dependency Injection — Hard-Coded Infrastructure

**Severity: HIGH**

### Problem

Infrastructure concerns (TCP sockets, file I/O, storage backends) are hard-coded into domain types, making unit testing impossible without real resources and preventing deployment flexibility.

### Evidence

| Crate | Type | Hard-Coded Dependency |
|---|---|---|
| `dicom-dimse-service` | `DimseServer` | `TcpListener`, `TcpStream`, `thread::spawn` — no transport trait |
| `dicom-dimse-service` | `DimseClient` | `TcpStream` — no mock transport possible |
| `dicom-dimse-service` | `StorageBackedDimseService` | Directly instantiates `Storage` — no trait-based injection |
| `dicom-storage` | `S3Backend` | In-memory simulation only — no actual S3 integration exists |
| `dicom-web-server` | HTTP server | Hand-rolled HTTP handling — no framework abstraction |
| `dicom-workflow-server` | Main loop | Direct TCP + thread management — no runtime abstraction |

### Impact

- **Testing requires real network/file resources**: Cannot write pure unit tests for DIMSE protocol handling.
- **Cannot swap implementations**: S3 backend is simulated, making cloud deployment impossible without rewriting.
- **Cannot test error paths**: Network failures, timeouts, and connection drops cannot be simulated.

### Recommendation

1. Define a `Transport` trait in `dicom-net` with `connect()`, `accept()`, `send()`, `recv()` methods.
2. Provide `TcpTransport` (production) and `MockTransport` (testing) implementations.
3. Define a `BlobStore` trait in `dicom-storage` with `put()`, `get()`, `delete()`, `list()` methods.
4. Provide `FileBlobStore`, `S3BlobStore`, and `InMemoryBlobStore` implementations.
5. Use trait-based dependency injection in `DimseServer`, `DimseClient`, and `StorageBackedDimseService`.

---

## 8. No `[workspace.dependencies]` — Version Skew Risk

**Severity: MEDIUM**

### Problem

The root `Cargo.toml` has no `[workspace.dependencies]` table. Every crate declares its own external dependency versions independently. This means:

- `serde` could be `1.0.195` in one crate and `1.0.210` in another, pulling two versions into the build.
- `sha2` could be `0.10.7` in one crate and `0.10.8` in another.
- No single source of truth for compatible dependency versions.

### Current Affected Dependencies

| Dependency | Used In | Version Pattern |
|---|---|---|
| `serde` | 13+ crates | All `1.0` but no pin |
| `serde_json` | 13+ crates | All `1.0` but no pin |
| `sha2` | 3 crates | All `0.10` but no pin |
| `image` | 2 crates | `0.25` but no pin |
| `wgpu` | 1 crate | `0.19` |
| `jpeg-decoder` | 1 crate | `0.3` |

### Recommendation

1. Add `[workspace.dependencies]` to root `Cargo.toml` with pinned versions of all shared dependencies.
2. Change all crate `Cargo.toml` files to use `workspace = true` for shared deps.
3. Run `cargo update` to lock all transitive dependencies.

---

## 9. Semantic Error Misuse — `DecodeError` for Authorization Failures

**Severity: MEDIUM**

### Problem

`dicom-auth` uses `ErrorKind::DecodeError { stage: "dicom-auth" }` for authorization denial errors. `DecodeError` is semantically reserved for byte-level parsing failures, not access control decisions. This will confuse error handlers, log aggregators, and monitoring systems that categorize errors by kind.

### Evidence

- `auth_denied()` in `dicom-auth` returns `ErrorKind::DecodeError` — there is no `ErrorKind::AuthorizationDenied` variant in `dicom-core`.
- The `ErrorKind` enum in `dicom-core` has variants for `DVF.DICOM.*`, `DVF.GEOM.*`, `DVF.PIXEL.*`, and `DVF.SECURITY.*`, but no `DVF.AUTH.*` category.

### Recommendation

1. Add `ErrorKind::AuthorizationDenied` variant to `dicom-core::ErrorKind`.
2. Add `ErrorKind::PolicyViolation` for non-authorization policy failures.
3. Fix `auth_denied()` in `dicom-auth` to use the new variant.
4. Audit all other crates for similar semantic mismatches.

---

## 10. Stub and Incomplete Implementations

**Severity: MEDIUM**

### Problem

Several features are declared as "implemented" in TASKS.md (marked `[x]`) but are actually stubs, no-ops, or simulations that would not work in production.

| Crate | Feature | Status | Detail |
|---|---|---|---|
| `dicom-storage` | `S3Backend` | Simulation only | In-memory HashMap; no actual S3 API calls |
| `dicom-storage` | `LifecyclePolicy::apply()` | No-op | Returns `0`, comment says "In a real implementation..." |
| `dicom-io` | MPEG2/H264/HEVC transfer syntaxes | Recognized but not decoded | Falls back to `ExplicitVrLittleEndian`; no warning emitted |
| `dicom-inference` | `OnnxRuntime` | Stub trait | `InferenceRuntime` trait exists but `OnnxRuntime::load_model()` and `run_inference()` return test data, not real inference |
| `dicom-xr` | `XrRenderer` | Stub | No actual OpenXR/WebXR integration; `render_frame()` returns synthetic pixel data |
| `dicom-xr` | `ArOverlayEngine` | Stub | No actual AR framework integration; `render_overlay()` returns synthetic pixel data |
| `dicom-core` | `no_std` support | Not implemented | Comment says "not yet available (REQ-API-203)" |
| `dicom-io` | `capture_debug_offsets()` | Partial | Only handles Explicit VR Little Endian; wrong offsets for Implicit VR |
| `dicom-auth` | `Authorizer` implementations | Only `AllowAll` and `DenyAll` | No real policy-based authorizer exists |

### Impact

- TASKS.md is misleading — features are marked complete but are not production-ready.
- Security-critical stubs (`AllowAll`, `DenyAll`) could be accidentally used in production.
- S3 "backend" is not a backend — deploying to AWS would require rewriting from scratch.

### Recommendation

1. Add a `STUB` annotation to all struct/impl blocks that are not production-ready.
2. Add runtime assertions in stub implementations: `panic!("STUB: S3Backend is a simulation — use a real backend in production")`.
3. Update TASKS.md with a "Production Readiness" column.
4. Create a tracking issue for each stub with estimated implementation effort.

---

## 11. Type Alias Proliferation Instead of Newtypes

**Severity: LOW**

### Problem

Several crates use type aliases instead of newtypes, losing type safety and the ability to add validation or behavior.

| Crate | Alias | Should Be |
|---|---|---|
| `dicom-collab` | `pub type SessionId = String;` | `SessionId` newtype |
| `dicom-collab` | `pub type UserId = String;` | `UserId` newtype |
| `dicom-collab` | `pub type Tick = u64;` | `Tick` newtype (prevent mixing with other counters) |

### Recommendation

Use newtypes (`struct SessionId(String);`) instead of type aliases for domain identifiers.

---

## 12. Inconsistent Error Handling Patterns

**Severity: MEDIUM**

### Problem

Error handling varies across crates, making it hard to write code that composes operations from multiple crates.

| Pattern | Crate(s) | Detail |
|---|---|---|
| `Result<T> = std::result::Result<T, Box<Error>>` | `dicom-core`, most crates | Heap-allocated error on every error path |
| `Result<T, ClinicalError>` | `viewer-core` | Custom enum error — no `Box<Error>` |
| `Result<T, MprError>` | `viewer-core::mpr` | Another custom enum error |
| `Result<T, VolumeError>` | `viewer-core::volume` | Another custom enum error |
| `Result<T, GsdfError>` | `viewer-core::gsdf` | Another custom enum error |
| `Result<T, HangingProtocolError>` | `viewer-core::hanging_protocol` | Another custom enum error |

### Impact

- Cannot easily compose operations across `viewer-core` and `dicom-core` because their error types are incompatible.
- `Box<Error>` loses type information — callers must match on `ErrorKind` but `ErrorKind` doesn't cover all domains (see Issue 9).
- `viewer-core` has 5 different error types in the same crate — there's no unified error type.

### Recommendation

1. Define a `DiccyError` enum in `dicom-core` that covers all error domains, or use `thiserror` for derive-based error types.
2. Unify `viewer-core` errors into a single `ViewerError` enum with variants for clinical, MPR, volume, GSDF, and protocol errors.
3. Provide `From<ViewerError>` for `Box<Error>` for cross-crate compatibility.

---

## 13. Dataset Linear Search — O(n) Lookups on Every Tag Access

**Severity: MEDIUM**

### Problem

`Dataset` stores elements as `Vec<Element>` and implements `get()` via `.find()` — an O(n) linear scan. For datasets with thousands of elements (common in DICOM), this is a performance problem. Every call to `require_uid()`, `missing_required_tag()`, or any tag lookup triggers a full linear scan.

### Evidence

```rust
// dicom-core/src/lib.rs
pub fn get(&self, tag: Tag) -> Option<&Element> {
    self.elements.iter().find(|e| e.tag == tag)
}
```

In a typical CT dataset with ~200 elements, called 30–50 times during parsing, this results in 6,000–10,000 comparisons per instance. For a study with 500 instances, that's 3–5 million comparisons.

### Recommendation

1. Replace `Vec<Element>` with `BTreeMap<Tag, Element>` for O(log n) lookups.
2. Maintain insertion-order iteration via a separate `Vec<Tag>` index.
3. Benchmark before/after with realistic DICOM datasets.

---

## 14. No Shared Test Infrastructure

**Severity: LOW**

### Problem

Each crate has its own `tests/` directory with hand-rolled test infrastructure. There is no shared test fixture library, no shared DICOM test data, and no shared assertion helpers.

### Evidence

- Test DICOM datasets are constructed inline in each test file.
- `sha2` is imported as a dev-dependency in 3 crates for the same purpose (hash verification).
- Test assertion patterns (`assert_eq!(result.unwrap_err().kind, ErrorKind::...)`) are repeated across all crates.

### Recommendation

1. Create a `dicom-test-util` crate with:
   - Factory functions for common DICOM test datasets.
   - Assertion macros for error kind matching.
   - Shared test fixture data (minimal DICOM Part 10 files).
2. Use this crate as a dev-dependency across the workspace.

---

## 15. Feature Flag Combinatorial Explosion

**Severity: MEDIUM**

### Problem

`dicom-core` defines 16 feature flags that propagate through the crate tree. `rdvf` adds 16 more. The total feature matrix has 2^32 possible combinations, and the vast majority have never been tested. Some combinations are invalid (e.g., `codec-j2k` without the `j2k` external dependency).

### Evidence

| Crate | Feature Flags | Combinations |
|---|---|---|
| `dicom-core` | 16 features | 65,536 |
| `rdvf` | 16 features | 65,536 |
| Combined | 32 features | 4,294,967,296 |

In practice, only a handful of combinations are ever tested (typically `--all-features` or the default).

### Recommendation

1. Create feature groups (e.g., `codecs = ["codec-jpegls", "codec-j2k"]`, `modalities = ["modality-ct", "modality-pet", ...]`).
2. Add CI matrix testing for the top 5–10 most common feature combinations.
3. Document which combinations are supported in a `FEATURES.md` file.

---

## 16. Security Issues

**Severity: HIGH**

### Problem

Several security concerns exist in the codebase:

| Issue | Location | Detail |
|---|---|---|
| **Credentials in plain text** | `dicom-storage::S3Config` | `access_key_id: String`, `secret_access_key: String` — debug-printable, serializable to JSON |
| **Debug/Serialize on secrets** | `dicom-storage::S3Config` derives both `Debug` and `Serialize` | Secrets will appear in logs and API responses |
| **No zeroization** | All credential types | No `Zeroize` trait; secrets remain in memory until GC |
| **AllowAll authorizer** | `dicom-auth::AllowAll` | Implements `Authorizer` by always returning `Ok(())` — could be accidentally used in production |
| **No rate limiting on auth** | `dicom-auth` | `SessionStatus::failed_attempts` is tracked but no lockout is enforced |
| **Redaction utilities in binary** | `dicom-dimse-service/bin` | `redact_token()`, `looks_like_uid()`, `looks_like_email()` are privacy-critical but buried in a binary |

### Recommendation

1. Replace `String` credentials with `SecretString` from the `secrecy` crate.
2. Custom `Debug` impl that redacts secrets: `S3Config { access_key_id: [REDACTED], ... }`.
3. Remove `Serialize` from `S3Config` or implement custom serializer that skips secrets.
4. Add `#[deprecated = "AllowAll must not be used in production"]` to `AllowAll`.
5. Extract redaction utilities into `dicom-util` or `dicom-audit`.

---

## 17. Monotonic Tick Pattern Not Enforced at Type Level

**Severity: LOW**

### Problem

The codebase uses a `tick: u64` field for deterministic ordering across `MeasurementStore`, `SegmentationStore`, `Annotation3dStore`, and `ViewerModel`. The tick is incremented via `saturating_add(1)`, but nothing prevents:
- Setting `tick` directly (it's `pub` in some types).
- Creating records with non-monotonic ticks.
- Two stores having the same tick value (no global clock).

### Recommendation

1. Create a `MonotonicTick` newtype that can only be incremented, never decremented.
2. Make `tick` private with `tick() -> u64` getter and `next_tick() -> MonotonicTick` increment method.
3. Consider a global `TickProvider` trait for cross-store coordination.

---

## Summary — Priority-Ordered Remediation Plan

| Priority | Issue | Effort | Impact |
|---|---|---|---|
| **P0** | Issue 5: Split bounded contexts (storage, auth, dimse-service, workflow-server, web) | 2–3 weeks | Enables independent evolution and deployment |
| **P0** | Issue 1: Make fields private + validated constructors | 2–3 weeks | Restores invariant safety |
| **P1** | Issue 4: Add domain newtypes (Uid, AeTitle, SopClassUid, etc.) | 1–2 weeks | Catches bugs at compile time |
| **P1** | Issue 3: Extract shared utilities into `dicom-util` | 1 week | Eliminates 60+ duplicated functions |
| **P1** | Issue 7: Add dependency injection (Transport, BlobStore traits) | 1–2 weeks | Enables testing and deployment flexibility |
| **P1** | Issue 16: Fix credential handling (SecretString, redacted Debug) | 3–5 days | Prevents credential leaks |
| **P2** | Issue 6: Split oversized files | 1 week | Improves maintainability and compilation |
| **P2** | Issue 2: Enrich domain types with behavior | 2–3 weeks | Reduces scattered business logic |
| **P2** | Issue 9: Add `ErrorKind::AuthorizationDenied` | 1 day | Fixes semantic error misuse |
| **P2** | Issue 12: Unify error types | 1–2 weeks | Simplifies cross-crate composition |
| **P2** | Issue 13: Replace Vec with BTreeMap in Dataset | 3–5 days | Improves performance |
| **P3** | Issue 8: Add `[workspace.dependencies]` | 1 day | Prevents version skew |
| **P3** | Issue 10: Mark stubs and add runtime assertions | 2–3 days | Prevents accidental production use |
| **P3** | Issue 14: Create shared test infrastructure | 1 week | Reduces test boilerplate |
| **P3** | Issue 15: Rationalize feature flags | 3–5 days | Reduces untested combinations |
| **P3** | Issue 11: Replace type aliases with newtypes | 1 day | Improves type safety |
| **P3** | Issue 17: Enforce monotonic tick at type level | 2–3 days | Prevents non-monotonic ordering bugs |

---

## Methodology

This audit was performed by:
1. Reading the root `Cargo.toml` workspace configuration.
2. Reading all 48 crate `Cargo.toml` files to map dependencies.
3. Reading the source code of all 48 crates (all `.rs` files, 102,346 lines total).
4. Analyzing each type for DDD alignment, encapsulation, and design quality.
5. Cross-referencing with the TASKS.md sprint roadmap for completeness claims.
6. Identifying patterns that repeat across crates (DRY violations, structural patterns).

**Audit date**: 2026-04-26
**Codebase version**: Commit `3ff7177` (Sprint 5 complete)
**Total crates**: 48
**Total lines**: 102,346
