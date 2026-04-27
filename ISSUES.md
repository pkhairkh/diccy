# ISSUES.md — Architecture & Code Design Audit

> Comprehensive evaluation of the DiCCY codebase with emphasis on
> Domain-Driven Design (DDD) principles, encapsulation, cohesion,
> coupling, and structural health. Based on a full source-code audit
> of all 48 workspace crates (102,346 lines of Rust).
>
> **50 issues identified** across Critical (3), High (18), Medium (23), and Low (6) severity levels.
> Issues #1–#37 from original code audit; #38–#42 from competitive gap analysis;
> #43–#50 from DiCCY Competitive Analysis paper (April 2026).

## Sprint Resolution Summary

Sprints 9–13 addressed these issues systematically. The table below shows
the resolution status as of the completion of Sprint 13.

| Sprint | Focus | Issues Addressed |
|---|---|---|
| S9 (P0) | Critical architecture: bounded contexts, encapsulation, integer safety, rendering decoupling | #1 (partial), #5, #6, #22 |
| S10 (P1) | Newtypes, DRY elimination, dependency injection, credential security, pack consolidation, validated constructors, error semantics | #1 (phase 1), #3, #4, #7, #8, #9, #16, #18, #19, #26 |
| S11 (P2) | File decomposition, rich domain model, error unification, BTreeMap Dataset, boolean traps, shared types, audit hash | #2, #6, #12, #13, #17, #23, #35, #37 |
| S12 (P3-A) | Workspace deps, stub safety, test infra, feature flags, newtypes, monotonic tick | #8, #10, #11, #14, #15, #17 |
| S13 (P3-B) | Pack trait, FromDataset/OverlayRenderable, named constants, naming consistency, BTreeSet tenants, Arc fixes, route capability | #20, #21, #24, #27, #28, #30, #31, #32, #34, #36 |
| S14 (Competitive) | WebGL2 fallback, RBAC authorization, OAuth2/OpenID Connect, pixel codec trait API, plugin architecture | #38, #39, #40, #41 |
| S15 (Competitive) | Multi-tenancy, bidirectional HL7 workflow, FHIR publication, regulatory certification, audit hardening | #42 |
| S16 (Competitive) | JS/WASM embedding SDK, CORS/multi-origin, multi-monitor display, PWA/offline, OpenAPI spec, real-PACS tests, benchmarks, community SDK | #43, #44, #45, #46, #47, #48, #49, #50 |

### Partially Resolved

- **#1 (Encapsulation):** Core types (`Element`, `Error`, `Limits`, `Capabilities`, `S3Config`, `FusionOverlayState`, `RtDoseOverlayState`, `SessionStatus`) now have private fields with validated constructors and getters. Secondary types in `viewer-core`, `dicom-auth`, and `dicom-dimse-service` still expose some public fields.
- **#2 (Anemic Domain Model):** Key types now have behavioral methods (`Element::as_uid()`, `Dataset::insert_validated()`, `MeasurementRecord::soft_delete()`), but many secondary types remain data-only.
- **#10 (Stubs):** Marked with doc annotations and runtime assertions. Production implementations still needed for S3Backend, OnnxRuntime, and XrRenderer.

### Open

- **#33 / #34 (Inline Tests):** Resolved — all ~1190 `#[test]` functions extracted to `tests/` directories; production source has zero inline tests (S13-T8 complete).
- **#25 (Type Aliases):** Partially resolved (SessionId, UserId, Tick now newtypes in dicom-collab); other type aliases may remain.
- **#1 (Encapsulation):** Further improved — private fields and TAG constants made public with proper accessors for integration test visibility across multiple crates.
- **#38 (WebGL Fallback):** Resolved — WebGL2 fallback renderer implemented in `viewer-wasm` with WebGPU → WebGL2 → CPU cascade (S14-T1 complete). 32 tests passing.
- **#39 (RBAC/OAuth2):** Resolved — RBAC authorization with 5 roles/6 permissions + OAuth2/OpenID Connect with JWT validation implemented in `dicom-auth` (S14-T2, S14-T3 complete). 95 tests passing.
- **#40 (Pixel Codec Trait API):** Resolved — `PixelCodec` trait, `CodecRegistry`, and 5 codec implementations (Raw, RLE, JPEG, JPEG-LS, JPEG 2000) in `dicom-pixel` (S14-T4 complete). 61+ tests passing.
- **#41 (Plugin Architecture):** Resolved — `dicom-plugin` crate with `DiccyPlugin` trait, 4 extension points, sandboxing, WASM runtime, and manifest parsing (S14-T5 complete). 55 tests passing.
- **#42 (Regulatory Certification):** Resolved — `dicom-regulatory` crate with IEC 62304 SRS/SDD/STP/RMF documentation bundle + audit trail hardening with ATNA export and tamper-evident logging (S15-T4, S15-T5 complete). 65 tests passing.
- **#43 (JS/WASM Embedding SDK):** Resolved — `dicom-viewer-sdk` crate with `DicomViewer` class, event-driven API, React/Vue/Svelte wrappers (S16-T1 complete). 32 tests passing.
- **#44 (CORS / Multi-Origin):** Resolved — `CorsConfig` with origin whitelist, preflight handling, per-origin RBAC scoping in `dicom-web` (S16-T2 complete). 20 tests passing.
- **#45 (Multi-Monitor Display):** Resolved — `viewer-core::multi_display` with `DiagnosticLayoutEngine`, MQSA-compliant mammography layout, cross-monitor sync (S16-T3 complete). 19 tests passing.
- **#46 (PWA / Offline Mode):** Resolved — `dicom-pwa` crate with cache strategies, offline sync queue, quota monitoring, Service Worker and manifest generation (S16-T4 complete). 28 tests passing.
- **#47 (OpenAPI Spec):** Resolved — `dicom-openapi` crate generating OpenAPI 3.1 spec for all DICOMweb endpoints with Bearer/OAuth2 security schemes (S16-T5 complete). 10 tests passing.
- **#48 (Real-PACS Integration Tests):** Resolved — `pacs-integration` test crate with Docker Compose orchestration, Orthanc/dcm4chee/HAPI FHIR interop tests (S16-T6 complete). 21 tests passing.
- **#49 (Performance Benchmarks):** Resolved — `diccy-bench` crate with criterion-based benchmarks, competitor comparison, regression detection (S16-T7 complete). 8 tests passing.
- **#50 (Community SDK / Docs):** Resolved — `dicom-sdk-docs` crate with trait guides, example extensions, plugin manifest schema, stability policy (S16-T8 complete). 29 tests passing.

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

`dicom-core` defines 16 feature flags that propagate through the crate tree. The `diccy` facade crate adds 16 more. The total feature matrix has 2^32 possible combinations, and the vast majority have never been tested. Some combinations are invalid (e.g., `codec-j2k` without the `j2k` external dependency).

### Evidence

| Crate | Feature Flags | Combinations |
|---|---|---|
| `dicom-core` | 16 features | 65,536 |
| `diccy` (facade) | 16 features | 65,536 |
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

## 18. Pack Crate Code Duplication — 6× Duplicated `read_str`, `missing_required_tag`, Sequence Helpers

**Severity: HIGH**

### Problem

The `pack-*` crates (pack-enhanced, pack-gsps, pack-seg, pack-rt, pack-sr) each re-implement the same DICOM parsing helpers with subtle behavioral differences, creating a fragile web of near-identical code.

### Duplication Map

| Helper | Duplicated In | Key Difference |
|---|---|---|
| `read_str()` | pack-shared, pack-enhanced, pack-gsps, pack-seg, pack-rt, pack-sr | pack-shared handles `Value::Bytes`; pack-sr returns `&str` not `Option<&str>` |
| `missing_required_tag()` | pack-enhanced, pack-gsps, pack-seg, pack-rt, pack-sr, modality-pet | modality-pet adds `.with_context` |
| `invalid_tag_value()` | pack-enhanced, pack-gsps, pack-seg, pack-rt, pack-sr | Identical |
| `sequence_items()` / `first_sequence_item()` / `read_sequence()` | pack-enhanced, pack-seg, pack-rt, pack-sr | 4–5× duplication |
| `read_u16()` / `read_bytes()` | pack-seg, pack-rt | Near-identical |
| `parse_manifest_uids()` test helper | pack-enhanced, pack-seg, pack-rt, pack-sr, pack-us, pack-nm, pack-xa, modality-mg | **8× duplicated** — fragile TOML-parsing hack |

### Critical Footgun

`pack-sr::read_str()` returns `Result<&str>` (errors on missing tag) while every other pack's `read_str()` returns `Result<Option<&str>>` (graceful on missing tag). This API inconsistency will cause unexpected panics when migrating code between packs.

### Recommendation

1. Expand `pack-shared` into a comprehensive parsing helper crate with all these functions.
2. Fix `pack-sr::read_str` to return `Result<Option<&str>>` for consistency.
3. Replace all local copies with imports from `pack-shared`.
4. Create a shared test utility crate with `parse_manifest_uids()`.

---

## 19. Identical Types Duplicated Across `pack-us`, `pack-nm`, `pack-xa`

**Severity: HIGH**

### Problem

Three crates define character-for-character identical types that cannot be used interchangeably, forcing consumers to convert between them.

| Type | pack-us | pack-nm | pack-xa |
|---|---|---|---|
| `CalibrationSource` | 3 variants | 3 variants (identical) | 3 variants (identical) |
| `MeasurementWarning` | 7 variants | 9 variants (superset) | 9 variants (identical to NM) |
| `extract_measurement_context()` | 86–133 lines | 88–146 lines (NM adds frame time) | 90–148 lines (identical to NM) |
| `FRAME_TIME_EPS = 1e-6` | Not present | Defined | Defined (duplicated) |

These are **different Rust types** — `pack_us::CalibrationSource` ≠ `pack_nm::CalibrationSource` — despite being semantically identical. This forces any consumer that works with multiple modalities to write conversion code.

### Recommendation

1. Create a `pack-calibration-shared` crate with `CalibrationSource`, `MeasurementWarning` (NM/XA superset), and `extract_measurement_context()`.
2. Re-export from `pack-us`, `pack-nm`, `pack-xa` for backward compatibility.

---

## 20. Missing `Pack` Trait — 8 Identical Marker Type Boilerplates

**Severity: MEDIUM**

### Problem

Eight structurally identical marker types all implement `enabled()` + `ensure_supported()` with identical boilerplate:

- `EnhancedPack`, `GspsPack`, `SegPack`, `RtPack`, `SrPack`, `UsPack`, `NmPack`, `XaPack`

Each has:
```rust
pub struct XPack;
impl XPack {
    pub fn enabled() -> bool { cfg!(feature = "x") }
    pub fn ensure_supported(uid: &str) -> Result<()> { ... }
}
```

### Recommendation

Define a shared trait:
```rust
trait Pack: Sized {
    const FEATURE: &'static str;
    const SOP_CLASS_UIDS: &[&str];
    fn enabled() -> bool;
    fn ensure_supported(uid: &str) -> Result<()>;
}
```

---

## 21. Missing `FromDataset` and `OverlayRenderable` Traits

**Severity: MEDIUM**

### Problem

Multiple domain types implement the same patterns but share no interface:

| Pattern | Implementors |
|---|---|
| `from_dataset(dataset: &Dataset, limits: &Limits) -> Result<Self>` | `RtDoseGrid`, `RtStructureSet`, `RtPlanSummary`, `Segmentation`, `PresentationState` |
| `overlay_on(frame: &mut DisplayFrame)` | `Segmentation`, `RtDoseGrid`, `RtStructureSet`, `PresentationState` |

Without shared traits, generic code cannot be written over these types.

### Recommendation

1. Define `trait FromDataset: Sized { fn from_dataset(dataset: &Dataset, limits: &Limits) -> Result<Self>; }`
2. Define `trait OverlayRenderable { fn overlay_on(&self, frame: &mut DisplayFrame); }`

---

## 22. Rendering Code Leaked Into Domain/Parsing Crates

**Severity: HIGH**

### Problem

Domain crates (pack-gsps, pack-seg, pack-rt) depend on `dicom_pixel::DisplayFrame` — a rendering surface type — for overlay rendering. This makes the domain layer depend on the rendering layer, inverting the dependency direction.

| Crate | Rendering Dependency | Should Be |
|---|---|---|
| `pack-gsps` | `dicom_pixel::{DisplayFrame, DisplayTransform, PixelFormat}` | Return domain types; let the rendering layer handle pixel operations |
| `pack-seg` | `dicom_pixel::{DisplayFrame, PixelFormat}` | Same |
| `pack-rt` | `dicom_pixel::{DisplayFrame, PixelFormat}` | Same |
| `modality-pet` | `viewer_core::VolumeGrid` | Domain crate depends on UI/runtime crate |

### Additional Evidence

- Bresenham line/circle algorithms are implemented directly in `pack-gsps` and `pack-rt` (2× duplicated), mixed with DICOM parsing logic.
- `dicom-mesh` contains no rendering dependency — it returns pure domain types (`TriangleMesh`). This is the correct pattern.

### Recommendation

1. Move `DisplayFrame`-dependent code from pack-gsps/pack-seg/pack-rt into a separate rendering layer.
2. Return domain types (contour points, overlay descriptors) from pack crates.
3. Move Bresenham algorithms to a rendering/painting utility crate.
4. Remove `viewer_core` dependency from `modality-pet` — use a trait abstraction instead.

---

## 23. Boolean Trap — Raw `bool` Fields Instead of Enums

**Severity: MEDIUM**

### Problem

Multiple types use raw `bool` fields or `Option<bool>` where semantically meaningful enums would prevent invalid combinations and improve readability.

| Crate | Type | Field | Should Be |
|---|---|---|---|
| `pack-gsps` | `PresentationState` | `flip_x: Option<bool>`, `flip_y: Option<bool>` | `enum Flip { None, Horizontal, Vertical, Both }` |
| `pack-gsps` | `ViewportState` | `rotation_quadrants: Option<i32>` | `enum Rotation { Q0, Q90, Q180, Q270 }` |
| `modality-ct` | `SliceSpacing` | `unknown: bool` + `non_uniform: bool` | `enum Spacing { Unknown, Uniform(f64), NonUniform(f64) }` |
| `modality-ct` | `Measurement` | `calibrated: bool` | Part of `MeasurementUnit` variant |
| `modality-mg` | `TomoNavigation` | `cine_active: bool` + `cine_direction: i32` | `enum CineState { Stopped, Playing(Forward), Playing(Backward) }` |
| `modality-mg` | `MqsaDisplayControls` | `gsdf_calibrated: bool` | Part of calibration state enum |
| `modality-mg` | `DualMonitorHangingProtocol` | `show_priors: bool` | `enum PriorDisplay { Hidden, Visible }` |
| `pack-rt` | `RtContour` | `closed: bool` | `enum ContourType { ClosedPlanar, OpenPlanar }` |
| `dicom-mesh` | `TriangleMesh` normals field | `normals: Option<Vec<[f64; 3]>>` | Could be `enum Normals { Computed(Vec<...>), NotComputed }` |
| `viewer-core` | `RtssOverlayState` | `clipping_enabled: bool` | `enum ClippingMode { Enabled, Disabled }` |

### Impact

- `SliceSpacing { unknown: false, non_uniform: false, spacing_mm: -1.0 }` — negative spacing with contradictory flags is constructable.
- `PresentationState { flip_x: Some(true), flip_y: Some(true) }` — two separate bools where `Flip::Both` is clearer.
- `MqsaDisplayControls { mqsa_compliant: true, gsdf_calibrated: false }` — claiming compliance without calibration is a regulatory risk.

### Recommendation

Replace each `bool` pair or `Option<bool>` with a proper enum that encodes the valid states.

---

## 24. Magic Numbers Without Named Constants

**Severity: MEDIUM**

### Problem

Critical clinical and rendering values are hardcoded as magic numbers without documentation or named constants.

| Crate | Value | Context | Risk |
|---|---|---|---|
| `pack-gsps` | `0xFF` | GRAPHIC_LUMA — should be `const` in shared theme | Hard to change globally |
| `pack-gsps` / `pack-rt` | `[0xFF, 0x00, 0x00]` | DEFAULT_CONTOUR_COLOR — duplicated | Color inconsistency between GSPS and RT |
| `pack-gsps` | `180` | ELLIPSE_SEGMENTS — undocumented | Why 180? |
| `pack-rt` | `200` | STRUCTURE_ALPHA — undocumented | Clinical significance unknown |
| `pack-seg` | `73, 151, 199, 200, 30` | `segment_color` hash — trivial hash with poor distribution | Colors may collide for different segment indices |
| `modality-pet` | `8.0` | SUV normalization denominator | Critical clinical value — must be documented and configurable |
| `modality-pet` | `512*512*2048`, `512*1024*1024` | Default limits | Should be named constants |
| `pack-nm` / `pack-xa` / `pack-rt` | `1e-6` | FRAME_TIME_EPS / GEOM_EPS — same value, different names | Confusing |
| `pack-gsps` | `1e-6`, `1e-3`, `1e-3` | Three different epsilons | Should be in shared geometry config |
| `dicom-workflow-server` | `14_695_981_039_346_656_037` | FNV offset basis for `hash_text()` | Should use a proper hash crate |

### Recommendation

1. Create a `dicom-const` module or crate with all named constants.
2. Extract shared epsilon values into a geometry configuration type.
3. Replace the hand-rolled FNV hash with `fnv` or `ahash` crate.

---

## 25. Integer Overflow and Bounds-Checking Gaps

**Severity: HIGH**

### Problem

Several locations compute buffer sizes from untrusted DICOM data without overflow checks, creating potential panics or memory corruption on maliciously crafted files.

| Crate | Location | Risk |
|---|---|---|
| `pack-rt` | `rows*cols*frames` for dose grid | Could overflow `usize` for large dimensions |
| `pack-seg` | `expected_pixels = rows*cols*frames` | Same overflow risk |
| `pack-gsps` | `(y * width + x)` pixel index | Could overflow `u32` for very large frames |
| `modality-pet` | `Vec::with_capacity(dims[0]*dims[1]*dims[2])` | Could panic on OOM or overflow |
| `pack-sr` | `authored_epoch_ms.min(i32::MAX as u64) as i32` | **Silent data loss** — u64 truncated to i32 |
| `pack-sr` | `version.min(i32::MAX as u64) as i32` | **Silent data loss** — same truncation |
| `modality-pet` | `pet_voxel[0].round() as usize` | No bounds checking — could panic |

### Impact

- A crafted DICOM file with `rows=65535, cols=65535, frames=65535` would cause `rows*cols*frames` to overflow on 32-bit targets or OOM on 64-bit.
- The `pack-sr` u64→i32 truncation silently loses the upper 32 bits of timestamps and version numbers — data corruption that won't be detected until much later.

### Recommendation

1. Use `checked_mul()` / `saturating_mul()` for all dimension calculations.
2. Replace `as i32` casts with `try_into()` or explicit range checks.
3. Add bounds validation before all array indexing with computed indices.

---

## 26. Missing Validation on Construction

**Severity: HIGH**

### Problem

Many types that have natural invariants provide no validation at construction, relying on callers to remember to call `validate()` separately — or having no `validate()` at all.

| Crate | Type | Missing Validation |
|---|---|---|
| `pack-enhanced` | `EnhancedRescale` | `slope: f64::NAN` is accepted |
| `pack-enhanced` | `EnhancedFrameGeometry` | No IOP orthonormality check |
| `pack-gsps` | `ViewportState.with_rotation()` | No check that `rotation_quadrants` is 0–3 |
| `pack-gsps` | `ViewportState.with_zoom()` | No check that zoom > 0 |
| `pack-gsps` | `encode_viewport_as_gsps()` | Returns `Dataset` not `Result<Dataset>` — silently encodes invalid data |
| `modality-mg` | `TomoNavigation::new()` | No validation that `slice_thickness_mm > 0` |
| `modality-mg` | `MqsaDisplayControls::new()` | No check `max_luminance > min_luminance` |
| `modality-pet` | `SuvScaleInput` | `scale_factor: 0.0` passes through `apply_suv()` |
| `pack-rt` | `RtDoseGrid` | `dose_grid_scaling` can be any f64 externally |
| `dicom-mesh` | `TriangleMesh` | No check that triangle indices are within vertex bounds |
| `dicom-xr` | `HeadPose` | `validate()` exists but is not called in constructor |

### Recommendation

1. Add validated constructors (`::new()` with checks) for all types with invariants.
2. Make `encode_viewport_as_gsps()` return `Result<Dataset>`.
3. Call `validate()` inside `HeadPose::new()` — don't make it opt-in.

---

## 27. Naming Inconsistencies Across the Codebase

**Severity: MEDIUM**

### Problem

The same concepts are named differently across crates, violating the DDD principle of Ubiquitous Language.

| Concept | Crate A | Crate B | Inconsistency |
|---|---|---|---|
| SOP class check | `Pack::ensure_supported(uid)` | `modality-ct::ensure_ct_supported()` | Pack crates take UID param; modality crates don't |
| Pixel spacing | `(f64, f64)` tuple | `PixelSpacing` not defined | No shared type for a ubiquitous concept |
| Image orientation | `[f64; 6]` raw array | No named type | Used in pack-enhanced, pack-rt, modality-ct |
| Measurement unit | `MeasurementUnit::Millimeter` | `MeasurementMode::PhysicalMillimeters` | Two names for same concept in modality-ct |
| UID representation | `SegReference.frame_of_reference_uid: &'a str` | `RtReferenceGeometry.frame_of_reference_uid: String` | Inconsistent `&str` vs `String` |
| SOP class constant | `SOP_CLASS_ENHANCED_CT` in `pack-enhanced` | `SOP_CLASS_ENHANCED_CT` in `modality-ct` | **Duplicated constant** with same value |
| `read_str` return type | `Result<Option<&str>>` (5 crates) | `Result<&str>` (pack-sr) | Different error semantics |
| Pack marker naming | `RtPack`, `CrPack`, `MgPack`, `PetPack` | Some 2-letter, some 3-letter abbreviations | Inconsistent abbreviation style |

### Recommendation

1. Define `PixelSpacing`, `ImageOrientationPatient`, `ImagePositionPatient` shared types.
2. Unify `read_str` return types to `Result<Option<&str>>` everywhere.
3. Remove duplicated SOP class constants — define once in `dicom-core` or `pack-shared`.
4. Standardize pack/modality marker naming.

---

## 28. Cross-Crate Pattern Inconsistencies

**Severity: MEDIUM**

### Problem

Structurally similar crates follow different patterns, making the codebase unpredictable.

| Inconsistency | Crates Affected |
|---|---|
| `pack-us`/`pack-nm` use `pack-shared`; `pack-enhanced`/`pack-gsps`/`pack-seg`/`pack-rt`/`pack-sr` don't | Inconsistent use of shared crate |
| `pack-enhanced` defines TAG constants as `pub`; other packs use `const` (private) | API surface inconsistency |
| `pack-gsps` has encoding (writeback) support; `pack-seg` has encoder; other packs don't | Inconsistent feature coverage |
| `modality-mg` derives `Serialize, Deserialize`; no other modality crate does | Serialization inconsistency |
| `modality-pet` has `bin/fusion_benchmark.rs`; no other modality crate has benchmarks | Infrastructure inconsistency |
| `modality-pet` uses `viewer_core::VolumeGrid`; other modality crates are pure DICOM | **Architecture inconsistency** — a parsing crate depending on UI crate |
| `modality-ct` works with already-parsed geometry inputs (no `Dataset` parsing); other crates parse from `Dataset` | Pattern inconsistency |
| `dicom-web` uses `Arc<dyn Authorizer>`; `dicom-dimse-service` uses concrete `AllowAll`/`DenyAll` | DI inconsistency |

### Recommendation

1. All `pack-*` crates should depend on and use `pack-shared`.
2. All modality crates should follow the same dependency pattern (no `viewer_core` dependency).
3. Standardize on trait-based dependency injection for cross-cutting concerns.

---

## 29. Collaboration Crate Uses `DecodeError` for Non-Decode Failures

**Severity: MEDIUM**

### Problem

`dicom-collab` uses `ErrorKind::DecodeError { stage: "dicom-collab", detail }` for collaboration errors like "user already in session" or "operation from unknown user." These are not decode failures — they are domain validation errors.

Similarly, `dicom-workflow-server` uses `ErrorKind::DecodeError { stage: "dicom-workflow-server-auth" }` for authorization denials, and `dicom-telerad` uses it for bandwidth adaptation decisions.

### Affected Crates

| Crate | Error Code | Actual Meaning |
|---|---|---|
| `dicom-collab` | `ErrorKind::DecodeError` | Session validation, user management |
| `dicom-telerad` | `ErrorKind::DecodeError` | Network adaptation, streaming errors |
| `dicom-workflow-server` | `ErrorKind::DecodeError` | Authorization denial, route validation |
| `dicom-ups` | `ErrorKind::DecodeError` | UPS state transition validation |
| `dicom-web` | `ErrorKind::DecodeError` via `stage == "dicom-auth"` check | Authorization denial detected by string matching on stage |

### Critical Hack in `dicom-web`

```rust
// dicom-web/src/lib.rs line ~829
ErrorKind::DecodeError { stage, detail: _ } if stage == "dicom-auth" => (403, "Forbidden"),
```

The code **string-matches on the `stage` field** of `DecodeError` to determine if an error is an authorization failure. This is a fragile workaround for the missing `AuthorizationDenied` error kind.

### Recommendation

1. Add `ErrorKind::AuthorizationDenied`, `ErrorKind::SessionError`, `ErrorKind::ConcurrencyConflict`, and `ErrorKind::NetworkAdaptation` variants to `dicom-core`.
2. Remove the `stage == "dicom-auth"` string-match hack in `dicom-web`.
3. Update all affected crates to use semantically correct error kinds.

---

## 30. `dicom-workflow-server` Runtime State Is a 25-Field God Struct

**Severity: HIGH**

### Problem

The `RuntimeState` struct in `dicom-workflow-server/main.rs` contains 25 fields spanning at least 6 different domains:

```rust
struct RuntimeState {
    worklist: WorklistStore,
    mpps: MppsService,
    sr: SrWorkflowStore,
    mpps_idempotency: BTreeMap<String, CachedMppsRequest>,
    tasks: BTreeMap<String, ProcedureTask>,
    task_id_sequence: u64,
    task_idempotency: BTreeMap<String, CachedMppsRequest>,
    hl7: Hl7RuntimeState,              // 18 more fields inside
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
```

And `Hl7RuntimeState` itself has 18 more fields. Every handler function takes `&mut RuntimeState`, meaning every function has access to every piece of state — no separation of concerns, no capability-based access.

### Impact

- Any handler can accidentally mutate any state field.
- No compile-time enforcement of access control.
- Impossible to reason about which fields a function modifies.
- Thread safety is impossible — the entire struct is passed as `&mut`.

### Recommendation

1. Decompose `RuntimeState` into domain-specific sub-states: `WorklistState`, `MppsState`, `TaskState`, `Hl7State`, `TenantState`, `AuditState`, `RateLimitState`.
2. Pass only the required sub-state to each handler.
3. Consider using interior mutability (`RefCell`, `Mutex`) for concurrent access.

---

## 31. Tenant Indexing Uses `Vec<String>` Linear Scans Instead of Sets

**Severity: MEDIUM**

### Problem

The `dicom-workflow-server` tenant indexing uses `BTreeMap<String, Vec<String>>` where the `Vec<String>` is searched with `.iter().any()` — an O(n) linear scan per lookup.

```rust
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
```

`tenant_of_id()` is O(n*m) — iterates all tenants and all entries per tenant. For a system with 100 tenants and 10,000 tasks, this is 1,000,000 comparisons per lookup.

### Recommendation

1. Replace `Vec<String>` with `BTreeSet<String>` or `HashSet<String>`.
2. Add a reverse index: `BTreeMap<String, String>` mapping ID → tenant for O(log n) reverse lookups.

---

## 32. `Arc<dyn Trait>` Without Interior Mutability — Impostor Pattern

**Severity: MEDIUM**

### Problem

Several crates use `Arc<dyn SomeTrait + Send + Sync>` for dependency injection, but the traits only have `&self` methods (no `&mut self`). This means:

1. The trait cannot mutate internal state.
2. If the implementation needs mutation, it must use `Mutex` or `RefCell` internally, adding hidden synchronization overhead.
3. The `Arc` is often unnecessary — `&dyn SomeTrait` or a generic parameter would suffice.

### Evidence

| Crate | Trait | Used As |
|---|---|---|
| `dicom-web` | `Arc<dyn Authorizer + Send + Sync>` | `WebAuthConfig.authorizer` |
| `dicom-web` | `Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>` | `WebAuthConfig.audit` |
| `dicom-collab` | `Arc<dyn Fn(AuditEvent) -> Result<()> + Send + Sync>` | `CollabSession.audit` |
| `dicom-inference` | `Arc<dyn InferenceRuntime>` in some tests | `InferenceRuntime` has `&mut self` methods |

The `Authorizer` trait has `fn authorize(&self, ...)` — no mutation needed, so `Arc` is reasonable. But `InferenceRuntime` has `fn load_model(&mut self, ...)` — `Arc<dyn InferenceRuntime>` cannot call this because `Arc` only gives `&self`.

### Recommendation

1. For read-only traits like `Authorizer`, consider a generic parameter `<A: Authorizer>` instead of `Arc<dyn Authorizer>`.
2. For traits with `&mut self` methods, use `Arc<Mutex<dyn InferenceRuntime>>` or pass ownership.

---

## 33. Test Data Inline in Production Source Files

**Severity: MEDIUM**

### Problem

Several crates include massive test modules at the bottom of their `lib.rs` files, inflating compilation times even when not running tests.

| Crate | Test Lines (approx) | Percentage of File |
|---|---|---|
| `dicom-workflow-server/main.rs` | ~10,000 | 60% |
| `dicom-dimse-service/lib.rs` | ~1,200 | 27% |
| `pack-gsps/lib.rs` | ~600 | 25% |
| `pack-seg/lib.rs` | ~500 | 24% |
| `pack-rt/lib.rs` | ~500 | 25% |
| `pack-enhanced/lib.rs` | ~400 | 20% |

While `#[cfg(test)]` prevents test code from being compiled in release builds, it still slows down `cargo check` and IDE analysis because the compiler must parse and type-check the entire file.

### Recommendation

1. Move test modules into `tests/` directories as integration tests.
2. Keep only unit tests (testing private functions) in `lib.rs`.
3. Share test infrastructure via a `dicom-test-fixtures` dev-dependency.

---

## 34. `dicom-web` 34-Element Route Capability Array Is Brittle

**Severity: LOW**

### Problem

`dicomweb_route_capability_matrix()` returns a `[DicomWebRouteCapability; 34]` — a fixed-size array of exactly 34 elements. Adding a new DICOMweb route requires:
1. Finding the function.
2. Changing the array size `34` → `35`.
3. Adding the new element in the correct position.
4. Updating all callers that destructure the array.

This is error-prone and the fixed size serves no performance benefit.

### Recommendation

Return `Vec<DicomWebRouteCapability>` or use a `const` slice `&'static [DicomWebRouteCapability]`.

---

## 35. `viewer-core` Duplicates Types That Exist in `dicom-core`

**Severity: MEDIUM**

### Problem

`viewer-core` is intentionally independent of `dicom-core` (for GPU-agnostic, WASM-compatible design). However, this means several concepts are duplicated:

| Concept | `viewer-core` | `dicom-core` |
|---|---|---|
| UIDs | `StudySeriesContext.study_uid: String` | `Tag(0x0020, 0x000D)` + `Value::Uid` |
| Pixel spacing | `MeasurementCalibration.pixel_spacing: (f64, f64)` | `Tag(0x0028, 0x0030)` parsing |
| Measurement identifiers | `Measurement.id: String` | `MeasurementRecord.id: String` |
| Error handling | `ClinicalError`, `MprError`, `VolumeError`, `GsdfError`, `HangingProtocolError` | `Error` with `ErrorKind` |

The bridge crates (`modality-pet`, `viewer-wgpu`, `diccy` facade) that depend on both must manually translate between these parallel type universes.

### Impact

- Changes to one representation must be mirrored in the other.
- Bridge code is tedious and error-prone.
- Tests must validate both representations.

### Recommendation

1. Define a `dicom-types` crate with shared value objects (UID, PixelSpacing, Point3D, etc.) that both `dicom-core` and `viewer-core` can depend on.
2. Keep domain-specific logic separate, but share the primitive vocabulary.
3. Alternatively, make `viewer-core` depend on a minimal `dicom-values` subset.

---

## 36. Unnecessary Heap Allocations in Hot Paths

**Severity: MEDIUM**

### Problem

Several frequently-called functions perform unnecessary heap allocations that could be avoided.

| Crate | Location | Issue |
|---|---|---|
| `pack-enhanced` / `pack-gsps` | `raw.split('\\').collect::<Vec<_>>()` | Allocates Vec just to check length — use `split('\\').count()` or iterator |
| `pack-sr` | `document.items.clone()` then re-sort | Items already sorted by `build()` — unnecessary clone + sort |
| `pack-sr` | `coded_concepts::*()` functions | Each allocates 3 `String`s per call — should return `const` values or `LazyLock` |
| `pack-gsps` / `pack-rt` | `apply_rect()` / `apply_circle()` | Iterates all pixels including those inside the rect — could skip |
| `modality-mg` | `MammographyCadeHook::visible_findings()` | Allocates new `Vec` + collects on every call — should cache or return iterator |
| `viewer-core` | `MeasurementStore::active_measurements()` | Sorts and allocates Vec on every call — should maintain sorted order |
| `dicom-collab` | `CollabSession::merge_from()` | `entry.operation.clone()` for every merged operation — could borrow |

### Recommendation

1. Replace `.collect::<Vec<_>>()` + length check with `.count()`.
2. Cache sorted measurement lists instead of re-sorting on every access.
3. Use `Cow<'static, str>` for coded concept values.
4. Return iterators instead of allocating Vecs where possible.

---

## 37. `dicom-workflow-server` Hand-Rolled FNV Hash for Audit Integrity

**Severity: MEDIUM**

### Problem

The workflow server implements its own FNV-1a hash function for audit chain integrity:

```rust
fn hash_text(value: &str) -> String {
    let mut hash: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}
```

FNV-1a is not cryptographically secure. If audit integrity is a regulatory requirement (which it likely is for a medical device), a non-cryptographic hash is insufficient — an attacker could forge audit entries that produce the same hash.

### Recommendation

1. Use SHA-256 (already a dependency via `sha2` in other crates) for audit chain hashing.
2. Remove the hand-rolled FNV implementation.
3. Add the `sha2` crate as a dependency.

---

## Summary — Priority-Ordered Remediation Plan

| Priority | Issue | Effort | Impact |
|---|---|---|---|
| **P0** | Issue 5: Split bounded contexts (storage, auth, dimse-service, workflow-server, web) | 2–3 weeks | Enables independent evolution and deployment |
| **P0** | Issue 1: Make fields private + validated constructors | 2–3 weeks | Restores invariant safety |
| **P0** | Issue 25: Fix integer overflow and bounds-checking gaps | 1 week | Prevents panics and memory corruption on malformed input |
| **P0** | Issue 22: Decouple rendering from domain crates (pack-gsps/se/rt) | 1–2 weeks | Restores proper dependency direction |
| **P1** | Issue 4: Add domain newtypes (Uid, AeTitle, SopClassUid, etc.) | 1–2 weeks | Catches bugs at compile time |
| **P1** | Issue 3: Extract shared utilities into `dicom-util` | 1 week | Eliminates 60+ duplicated functions |
| **P1** | Issue 7: Add dependency injection (Transport, BlobStore traits) | 1–2 weeks | Enables testing and deployment flexibility |
| **P1** | Issue 16: Fix credential handling (SecretString, redacted Debug) | 3–5 days | Prevents credential leaks |
| **P1** | Issue 18: Consolidate pack crate parsing helpers into `pack-shared` | 1 week | Eliminates 6× duplicated helpers |
| **P1** | Issue 19: Create `pack-calibration-shared` for US/NM/XA duplicates | 3–5 days | Eliminates identical types across 3 crates |
| **P1** | Issue 26: Add validated constructors for types with invariants | 1–2 weeks | Prevents invalid state construction |
| **P1** | Issue 30: Decompose RuntimeState god struct | 1 week | Enables safe concurrent access |
| **P1** | Issue 29: Add proper ErrorKind variants (AuthorizationDenied, SessionError, etc.) | 3–5 days | Eliminates DecodeError misuse and string-match hacks |
| **P2** | Issue 6: Split oversized files | 1 week | Improves maintainability and compilation |
| **P2** | Issue 2: Enrich domain types with behavior | 2–3 weeks | Reduces scattered business logic |
| **P2** | Issue 9 / 29: Add `ErrorKind::AuthorizationDenied` + other variants | 3–5 days | Fixes semantic error misuse across 5+ crates |
| **P2** | Issue 12: Unify error types | 1–2 weeks | Simplifies cross-crate composition |
| **P2** | Issue 13: Replace Vec with BTreeMap in Dataset | 3–5 days | Improves performance |
| **P2** | Issue 23: Replace boolean traps with enums | 1 week | Prevents invalid state combinations |
| **P2** | Issue 35: Create `dicom-types` shared value objects | 1 week | Eliminates viewer-core/dicom-core duplication |
| **P2** | Issue 37: Replace FNV hash with SHA-256 for audit integrity | 2–3 days | Prevents audit chain forgery |
| **P3** | Issue 8: Add `[workspace.dependencies]` | 1 day | Prevents version skew |
| **P3** | Issue 10: Mark stubs and add runtime assertions | 2–3 days | Prevents accidental production use |
| **P3** | Issue 14: Create shared test infrastructure | 1 week | Reduces test boilerplate |
| **P3** | Issue 15: Rationalize feature flags | 3–5 days | Reduces untested combinations |
| **P3** | Issue 11: Replace type aliases with newtypes | 1 day | Improves type safety |
| **P3** | Issue 17: Enforce monotonic tick at type level | 2–3 days | Prevents non-monotonic ordering bugs |
| **P3** | Issue 20: Create `Pack` trait for marker types | 1 day | Reduces 8× boilerplate |
| **P3** | Issue 21: Create `FromDataset` and `OverlayRenderable` traits | 1–2 days | Enables generic code |
| **P3** | Issue 24: Extract magic numbers into named constants | 2–3 days | Improves readability and maintainability |
| **P3** | Issue 27: Unify naming inconsistencies | 3–5 days | Establishes ubiquitous language |
| **P3** | Issue 28: Standardize cross-crate patterns | 1 week | Makes codebase predictable |
| **P3** | Issue 31: Replace Vec<String> tenant indexes with Sets | 1–2 days | Improves O(n) → O(log n) lookups |
| **P3** | Issue 32: Fix `Arc<dyn Trait>` misuse | 2–3 days | Removes hidden synchronization overhead |
| **P3** | Issue 33: Move inline tests to `tests/` directories | 3–5 days | Reduces compilation time |
| **P3** | Issue 34: Replace fixed-size route capability array | 1 day | Eliminates brittle constant |
| **P3** | Issue 36: Eliminate unnecessary heap allocations | 1 week | Improves hot-path performance |

---

## Methodology

This audit was performed by:
1. Reading the root `Cargo.toml` workspace configuration.
2. Reading all 48 crate `Cargo.toml` files to map dependencies.
3. Reading the source code of all 48 crates (all `.rs` files, 102,346 lines total).
4. Analyzing each type for DDD alignment, encapsulation, and design quality.
5. Cross-referencing with the TASKS.md sprint roadmap for completeness claims.
6. Identifying patterns that repeat across crates (DRY violations, structural patterns).
7. Deep-diving into pack-*, modality-*, viewer-*, and server crates for additional findings.
8. Evaluating dependency direction, rendering coupling, and cross-crate consistency.

**Audit date**: 2026-04-26 (updated with extended findings)
**Codebase version**: Commit `3ff7177` (Sprint 5 complete)
**Total crates**: 48
**Total lines**: 102,346
**Total issues identified**: 37


## 38. No WebGL Fallback in WASM Viewer — Browser Compatibility Gap

**Severity: HIGH**

### Problem

The WASM viewer in `viewer-wasm` only supports the WebGPU rendering path. Safari (as of 2026) and many enterprise-managed browsers do not support WebGPU, making the viewer completely non-functional on those platforms. The competitive analysis identified this as the most impactful gap compared to OHIF and DWV, both of which support WebGL-based rendering.

### Evidence

- `viewer-wasm` unconditionally attempts WebGPU initialization
- `BackendRuntimeState` has no `WebGL2` variant
- No fallback shader code exists for WebGL2 context
- The CPU fallback exists for server-side rendering but produces no interactive viewer

### Impact

- Viewer is unusable on Safari (desktop and iOS), Firefox pre-WebGPU, and enterprise-managed Chrome versions
- Hospital IT policies often lock browsers to specific versions; lack of WebGL fallback blocks clinical deployment
- This is the #1 competitive gap identified in the DiCCY vs. open-source PACS analysis (April 2026)

### Recommendation

1. Add runtime GPU capability detection: `navigator.gpu` → WebGPU; else → WebGL2; else → CPU
2. Port MPR, MIP, and volume rendering shaders to WebGL2 (use 2D texture arrays for 3D data)
3. Guarantee deterministic pixel output on both WebGPU and WebGL2 paths
4. Add `RendererBackend` enum to `BackendRuntimeState`

---

## 39. No RBAC / OAuth2 Authorization — Enterprise Deployment Blocker

**Severity: HIGH**

### Problem

DiCCY's `dicom-auth` crate only provides `AllowAll` and `DenyAll` authorizer implementations. There is no role-based access control (RBAC), no OAuth2/OpenID Connect integration, and no study-level access filtering. Competitors such as dcm4chee offer full RBAC with multi-tenancy, and OHIF supports OpenID Connect authentication. Without these, DiCCY cannot be deployed in multi-user clinical environments.

### Evidence

- `AllowAll` always returns `Ok(())` — any user has full access
- `DenyAll` always returns `Err()` — no user has any access
- No `Role` or `Permission` types exist in `dicom-auth`
- `SessionStatus` tracks `failed_attempts` but no lockout is enforced
- No JWT token validation or OAuth2 flow exists

### Impact

- Cannot deploy in hospitals where radiologists, technologists, and administrators require different access levels
- Cannot integrate with enterprise identity providers (Keycloak, Active Directory, Auth0)
- Audit logs cannot attribute actions to specific roles (critical for HIPAA/MDR compliance)
- Cloud PACS deployments require tenant-scoped access control

### Recommendation

1. Define `Role` and `Permission` enums with configurable role-to-permission mapping
2. Implement `RbacAuthorizer` that enforces role-based permissions on all API endpoints
3. Add OAuth2/OpenID Connect authentication via JWT token validation
4. Add study-level access control: filter query results by patient/study assignment
5. Integration with `AuditLog` for all permission check outcomes (grant + deny)

---

## 40. No Public Pixel Codec Trait API — Blocks Third-Party Codec Integration

**Severity: MEDIUM**

### Problem

`dicom-pixel` hardcodes codec support internally. There is no public trait interface that allows third-party codec implementations to be registered at compile time or runtime. The competitive analysis identified this as an opportunity: no open-source PACS competitor currently offers a modular codec interface, making this a potential differentiator.

### Evidence

- Codec selection is done via `match` on transfer syntax UID strings inside `dicom-pixel`
- Adding a new codec (e.g., HTJ2K) requires modifying `dicom-pixel` source code
- No `PixelCodec` trait or `CodecRegistry` exists
- The planned codec trait API was mentioned in the competitive analysis as "open-source pixel codec infrastructure"

### Impact

- Cannot support emerging compression standards (HTJ2K, JPEG XL) without core modifications
- Third-party or proprietary codecs cannot be integrated without forking
- Missed opportunity to be the first open-source PACS with a modular codec architecture

### Recommendation

1. Define `PixelCodec` trait with `encode()`, `decode()`, `capabilities()`, `supported_transfer_syntaxes()`
2. Create `CodecRegistry` for runtime codec registration and lookup
3. Implement trait for existing codecs: Raw, JPEG-LS, JPEG 2000
4. Document the trait API and provide a "how to add a codec" guide

---

## 41. No Runtime Plugin Architecture — Extensions Require Core Recompilation

**Severity: MEDIUM**

### Problem

DiCCY's extension model relies entirely on Cargo features and crate composition. Adding new functionality (e.g., AI tools, custom measurement plugins) requires modifying the workspace and recompiling. Competitors such as OHIF (JavaScript extension system) and Weasis (Java plugin API) allow third-party extensions without core modifications. The competitive analysis identified this as a strategic gap for ecosystem growth.

### Evidence

- All functionality is compiled in via `Cargo.toml` feature flags
- No dynamic library loading exists in the codebase
- No plugin discovery or registration mechanism
- The `libloading` crate is not a dependency anywhere in the workspace

### Impact

- Third parties cannot develop plugins without forking the repository
- Clinical sites cannot customize workflows without core changes
- Limits ecosystem growth compared to OHIF's thriving extension marketplace

### Recommendation

1. Define `DiccyPlugin` trait with extension points: `ViewerTool`, `ImageProcessor`, `WorkflowHook`, `StorageBackend`
2. Implement plugin discovery via directory scanning + dynamic library loading (`libloading`)
3. Consider WASM-based plugins for safe sandboxed execution
4. Implement plugin capability declaration to restrict access to declared extension points

---

## 42. No Formal Regulatory Certification Pathway

**Severity: MEDIUM**

### Problem

DiCCY is explicitly research-only software. No FDA 510(k) submission, CE-IVDR classification, or IEC 62304 software lifecycle documentation has been prepared. The competitive analysis notes that OHIF has FDA-compatible derivatives, and all clinical PACS systems require regulatory clearance. While DiCCY's fail-closed design and deterministic rendering are architecturally prepared for certification, the formal process has not been initiated.

### Evidence

- `README.md` and `docs/` contain no regulatory classification statements
- No Software Requirements Specification (SRS) tied to `manifest.toml` REQ identifiers
- No Software Design Description (SDD) mapping crate architecture to requirements
- No Risk Management File (RMF) despite `ISSUES.md` severity analysis providing the raw material
- The `docs/03` regulatory conformance envelope exists but is not structured for IEC 62304 compliance

### Impact

- Cannot be used for clinical diagnosis in any regulated market
- Cannot be sold or distributed as a medical device component
- Missed positioning as a "high-reliability PACS for regulated environments" — the competitive analysis identified this as DiCCY's strongest strategic niche

### Recommendation

1. Compile IEC 62304 documentation bundle: SRS, SDD, STP, RMF
2. Map existing `manifest.toml` REQ identifiers to SRS requirements
3. Document deterministic rendering guarantees for regulatory validation
4. Engage regulatory consultant for FDA 510(k) pre-submission meeting
5. Target Class II medical device classification (diagnostic workstation)

---

# PART 2 — Competitive Analysis Issues

> Issues #43–#50 derived from the DiCCY Competitive Analysis paper (April 2026),
> which compared DiCCY against OHIF, Weasis, Orthanc, ClearCanvas, Conquest,
> dcm4chee, dicom-rs, DWV, and Papaya across feature coverage, interoperability,
> enterprise readiness, and market positioning.

---

## 43. No JS/WASM Embedding SDK for Third-Party Web Apps

**Severity: HIGH**

### Problem

DiCCY's WASM viewer boundary (`viewer-wasm`) exposes low-level rendering functions but provides no high-level JavaScript/TypeScript SDK that third-party web applications can use to embed the viewer. Every competitor with a web viewer (OHIF, Weasis, DWV) provides a JavaScript integration layer. OHIF in particular ships a full React-based SDK with component wrappers, event listeners, and configuration APIs. Without an embedding SDK, DiCCY cannot be integrated into EMR systems, teleradiology portals, or any third-party web application without writing custom WASM bridge code.

### Evidence

- `viewer-wasm` exposes `gpu_volume_render_json()` and `upload_volume_grid()` but no `loadStudy()`, `addEventListener()`, or configuration API
- No npm package exists for DiCCY viewer integration
- No React/Vue/Svelte component wrappers
- OHIF provides `@ohif/viewer` npm package with full React SDK and iframe-less embedding
- DWV provides `dwv` npm package with mobile-responsive viewer and RESTful data connector

### Impact

- Third-party web applications cannot embed DiCCY without writing custom WASM bridge code
- No path to EMR/PACS portal integration without significant custom development
- Competitive evaluations reject DiCCY because integration effort is too high compared to OHIF/DWV
- Missing a key differentiator: DiCCY could offer iframe-less embedding (unlike OHIF) with native WASM performance

### Recommendation

1. Create `crates/dicom-viewer-sdk` that generates a TypeScript SDK via `wasm-pack build --target web`
2. Expose a high-level `DicomViewer` class with `loadStudy()`, `setWindowLevel()`, `addMeasurementListener()` API
3. Generate React/Vue/Svelte component wrappers from the SDK
4. Publish as npm package `@diccy/viewer-sdk`
5. Document iframe-less embedding pattern as competitive differentiator vs. OHIF

---

## 44. No CORS / Multi-Origin Support in DICOMweb Server

**Severity: HIGH**

### Problem

`dicom-web-server` does not emit CORS headers (Access-Control-Allow-Origin, Access-Control-Allow-Methods, Access-Control-Allow-Headers). This means that any web-based viewer (OHIF, Weasis, DWV) hosted on a different origin cannot directly communicate with DiCCY's DICOMweb endpoints. A reverse proxy or server-side modification is required for every cross-origin deployment, adding operational complexity and blocking ad-hoc evaluations.

### Evidence

- No CORS middleware in `dicom-web-server` or `dicom-web` crate
- No `Access-Control-Allow-*` header emission in any HTTP response
- No OPTIONS pre-flight request handling
- Orthanc provides configurable CORS settings via its configuration file
- dcm4chee deploys behind WildFly which handles CORS at the servlet container level

### Impact

- OHIF/Weasis cannot connect to DiCCY without a reverse proxy (Nginx/Caddy)
- Multi-site deployments where viewer and server are on different domains are blocked
- Competitive evaluations fail at the first step: "connect viewer to server"
- Security review teams see no CORS policy as a missing security control

### Recommendation

1. Add configurable CORS middleware to `dicom-web-server` with origin whitelist
2. Support wildcard patterns for enterprise deployments (e.g., `*.hospital.org`)
3. Handle pre-flight OPTIONS requests for DICOMweb endpoints
4. Integrate with S14-T2 RBAC for per-origin permission scoping
5. Add CORS configuration to Helm chart values

---

## 45. No Multi-Monitor Diagnostic Display Layout

**Severity: MEDIUM**

### Problem

DiCCY has no multi-monitor display layout engine. Diagnostic radiology reading rooms use dual or quad monitor setups where each display shows a different series or prior study. Weasis provides multi-monitor support with independent viewport control per display. Sectra PACS is specifically marketed on its dual-monitor mammography reading capability. Without this, DiCCY cannot serve diagnostic reading room workflows.

### Evidence

- No `multi_display` module or crate exists
- No `DiagnosticLayout` engine for multi-monitor viewport assignment
- `ComparisonSyncState` exists for side-by-side comparison but only within a single viewport
- No MQSA-compliant mammography dual-monitor layout (CC on left monitor, MLO on right, priors below)
- Weasis supports multi-monitor layout via its "Multi-display" mode
- Sectra PACS targets diagnostic reading rooms with 2–4 medical displays

### Impact

- DiCCY cannot be used in diagnostic reading rooms with multi-monitor setups
- Mammography reading is blocked without MQSA-compliant dual-monitor layout
- Radiologists evaluating DiCCY reject it because single-monitor reading is not clinically acceptable
- Competitive gap vs. Weasis/Sectra in the diagnostic workstation market

### Recommendation

1. Create `viewer-core::multi_display` module with `DiagnosticLayout` engine
2. Implement preset layouts: 1-up, 2-up (dual), 4-up (quad), 1+2 (primary + two priors)
3. Per-monitor viewport with independent window/level, zoom, pan
4. Synchronized scrolling across monitors (same series, different slices)
5. MQSA-compliant mammography dual-monitor layout integration

---

## 46. No PWA / Offline Mode for WASM Viewer

**Severity: MEDIUM**

### Problem

The WASM viewer has no Service Worker, no Web App Manifest, and no offline cache strategy. Enterprise browser deployments in teleradiology scenarios require offline-capable viewers that can function without continuous connectivity. The existing `TeleradGateway` provides offline mode on the native side, but the WASM viewer has no corresponding offline capability.

### Evidence

- No Service Worker registration in WASM viewer
- No Web App Manifest for PWA installability
- No Cache API integration for study metadata or pixel data
- No background sync queue for offline annotations
- `TeleradGateway` has offline mode with sync on reconnect (native only)
- OHIF supports offline caching via its data source abstraction layer
- Weasis has a desktop mode that works offline by design

### Impact

- WASM viewer is completely non-functional without network connectivity
- Teleradiology scenarios with intermittent connectivity (ambulance, remote clinics) are not supported
- Enterprise browser deployments cannot use DiCCY as a reliable offline tool
- PWA installability (home screen icon, standalone mode) is not available

### Recommendation

1. Add Service Worker with Cache API for offline study access
2. Create Web App Manifest for PWA installability
3. Implement cache strategies: metadata on first load, pixel data on demand with LRU eviction
4. Add background sync queue for measurements/annotations created offline
5. Integrate with `TeleradGateway` offline mode for seamless online/offline transition

---

## 47. No OpenAPI/Swagger Specification for DICOMweb API

**Severity: MEDIUM**

### Problem

DiCCY provides no published OpenAPI/Swagger specification for its DICOMweb API. Orthanc publishes a comprehensive REST API specification. OHIF documents its integration endpoints. Without a spec, integrators must read source code to understand endpoints, request/response formats, and authentication mechanisms, significantly raising the integration barrier.

### Evidence

- No OpenAPI 3.x spec file in the repository
- No auto-generation from `dicom-web` route handlers
- No Swagger UI or API documentation endpoint
- Orthanc provides `/explorer.html` interactive API documentation
- dcm4chee publishes REST API documentation
- OHIF documents its data source configuration API

### Impact

- Integrators must read Rust source code to understand the DICOMweb API
- No interactive API exploration for evaluation or debugging
- Automated integration tooling (code generators, test suites) cannot be built against DiCCY
- Competitive evaluations see lack of API documentation as a maturity signal

### Recommendation

1. Auto-generate OpenAPI 3.1 spec from `dicom-web` route handlers and types
2. Serve Swagger UI at `/api/docs` endpoint
3. Include authentication schemes (Bearer, OAuth2 from S14-T3) in spec
4. Add TypeSpec / schema definitions for all request/response bodies
5. Validate spec with `swagger-cli` in CI

---

## 48. No Integration Test Suite Against Real PACS Endpoints

**Severity: MEDIUM**

### Problem

All current tests use mock data and in-memory simulations. There are no integration tests that verify DiCCY's interoperability against real PACS servers (Orthanc, dcm4chee). Orthanc and dcm4chee both maintain comprehensive interop test suites that validate DIMSE and DICOMweb compatibility against each other. Without real-PACS tests, interoperability regressions are discovered in production rather than in CI.

### Evidence

- No Docker Compose configuration for spinning up reference PACS containers
- No test scenarios for DIMSE C-STORE/C-FIND against real Orthanc
- No DICOMweb STOW/WADO round-trip tests against dcm4chee
- No FHIR mapping integration tests against HAPI FHIR server
- Orthanc maintains integration tests against dcm4chee and other PACS
- dcm4chee has interop tests against Orthanc

### Impact

- Interoperability regressions are discovered in production, not in CI
- DIMSE protocol changes may break compatibility with real PACS without detection
- DICOMweb endpoint changes may violate the standard without detection
- Competitive evaluations that run interop tests may expose untested failure modes

### Recommendation

1. Create `tests/pacs-integration/` with Docker Compose orchestration
2. Add Orthanc container as reference PACS for DIMSE interop tests
3. Add dcm4chee container for DICOMweb round-trip tests
4. Add HAPI FHIR container for FHIR mapping integration tests
5. Run nightly CI pipeline against real PACS endpoints

---

## 49. No Published Performance Benchmarks vs. Competitors

**Severity: LOW**

### Problem

DiCCY has no published performance benchmarks comparing its rendering throughput, study loading times, or memory footprint against OHIF, Orthanc, or Weasis. Competitive evaluations and procurement decisions are heavily influenced by benchmark data. All major competitors publish or provide benchmark tools.

### Evidence

- No `crates/diccy-bench` benchmark suite exists
- No criterion-based benchmarks for study loading, rendering, or memory
- No comparison harness against competitor viewers
- No published performance data in documentation
- OHIF provides benchmark data for Cornerstone3D rendering pipeline
- Orthanc provides benchmark plugins for loading and query performance

### Impact

- Competitive evaluations cannot compare DiCCY performance objectively
- No regression detection when performance degrades
- Procurement decisions default to competitors with published data
- Marketing cannot claim performance advantages without evidence

### Recommendation

1. Create `crates/diccy-bench` with criterion-based benchmarks
2. Benchmark categories: study loading, rendering FPS, memory footprint, WASM cold start
3. Add comparison harness against OHIF + Orthanc + Weasis (Docker containers)
4. Publish results to `docs/benchmarks/` with regression detection
5. Fail CI if performance degrades more than 10% from baseline

---

## 50. No Community SDK or Extension Developer Documentation

**Severity: LOW**

### Problem

DiCCY has no public extension developer documentation, no community SDK, and no API stability guarantees. OHIF has an active extension ecosystem with a published guide for writing OHIF extensions. Orthanc has a plugin SDK with C/Python bindings and a registry. Without these, DiCCY cannot build a third-party developer community, which limits ecosystem growth and adoption.

### Evidence

- No `docs/sdk-guide/` directory exists
- No public documentation for `Pack`, `FromDataset`, or `OverlayRenderable` traits
- No plugin manifest schema (`plugin.toml`)
- No semver policy or deprecation schedule published
- No example extensions (custom codec, modality pack, overlay renderer)
- OHIF has `docs/latest/extensions/` with step-by-step extension development guide
- Orthanc has `OrthancPluginSDK` with C API and Python bindings

### Impact

- Third-party developers cannot build extensions without reading source code
- No plugin ecosystem can form around DiCCY
- API changes may break third-party code without warning
- Competitive evaluations see lack of SDK as ecosystem immaturity signal

### Recommendation

1. Create `docs/sdk-guide/` with extension development tutorials
2. Document `Pack`, `FromDataset`, `OverlayRenderable` traits for third-party authors
3. Define `plugin.toml` manifest schema for declaring extensions
4. Publish semver policy, deprecation schedule, and migration guides
5. Provide example extensions: custom transfer syntax codec, modality-specific pack, custom overlay renderer
