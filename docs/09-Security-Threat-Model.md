# Security threat model

## Summary

Normative:
- **REQ-SEC-405:** Security controls in this document **MUST** be implemented and **MUST NOT** be disabled by default.

Verification:
- Configuration tests **MUST** confirm security controls are enabled by default and require explicit opt-in to relax limits.

The framework ingests **untrusted DICOM bytes** and executes complex parsing and decompression logic. The primary security objective is to prevent:

- memory safety violations,
- decompressor exploitation,
- denial of service via resource exhaustion,
- unintended data exfiltration (especially in web contexts),
- integrity issues (non-deterministic or ambiguous outputs).

---

## 1. Assets and security objectives

### Assets

Requirements:
- **REQ-SEC-406:** Controls and limits **MUST** be designed to protect these assets even under malicious input:
  - Host process integrity (native and browser).
  - Host system resources (CPU, memory, GPU memory).
  - Confidentiality of medical data (PHI/PII) when present.
  - Deterministic correctness of pixel outputs and measurements.

Verification:
- Security review **MUST** map each asset to at least one control and one verification method in this document.

### Objectives (normative)

Requirements:
- **REQ-SEC-407:** The framework **MUST** be memory-safe for all in-envelope and out-of-envelope inputs (Rust safety + hardened boundaries).
- **REQ-SEC-408:** The framework **MUST** fail closed on malformed or out-of-envelope inputs.
- **REQ-SEC-409:** The framework **MUST** enforce explicit resource limits per **REQ-SEC-401** through **REQ-SEC-404**.
- **REQ-SEC-411:** The framework **MUST** avoid implicit network access and **MUST NOT** emit PHI/PII in logs or telemetry by default (see `docs/13-Error-Model-and-Telemetry.md`).
- **REQ-SEC-414:** Inputs (DICOM files, archives, byte buffers) **MUST** be treated as hostile.

Verification:
- Conformance tests **MUST** assert fail-closed behavior for out-of-envelope inputs.
- Unit and integration tests **MUST** assert limit enforcement and structured error returns.
- Telemetry tests **MUST** assert redaction and opt-in behavior (see `docs/13`).

---

## 2. Trust boundaries and attack surfaces

Requirements:
- **REQ-SEC-412:** Implementations **MUST** respect trust boundaries; cross-boundary data flows **MUST** validate sizes and sanitize metadata before use.
- **REQ-SEC-413:** Each attack surface listed here **MUST** have at least one corresponding mitigation and a verification method (tests or fuzzing).

Verification:
- Code review **MUST** confirm boundary validation prior to allocation and execution.
- Fuzz targets and tests **MUST** be listed per surface in `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

### Trust boundaries

- Input boundary: file/bytes into `dicom-io` / `dicom-core`.
- Codec boundary: compressed bitstreams into decoders (pure Rust or optional FFI).
- GPU boundary: uploading textures and running shaders on driver-controlled execution.

### Attack surfaces

- DICOM parser (VR/VL, sequences, nested items).
- Transfer syntax decoders (RLE/JPEG, optional JPEG-LS/J2K).
- Non-DICOM raster decoders (BMP/PNG/JPEG/TIFF) when `raster-io` is enabled.
- Archive handling (if zip supported).
- UI file selection and path handling (native).
- WASM host integration (JS <-> WASM boundary).

---

## 3. Threats and mitigations

### T1: Malformed DICOM leading to panics or OOM

Threat:
- Crafted elements with huge lengths, deep nesting, or invalid offsets.

Controls (normative):
- **REQ-SEC-420:** Parsers **MUST** enforce:
  - max total input bytes,
  - max element VL,
  - max nesting depth for sequences/items,
  - max number of elements per dataset.
- **REQ-SEC-421:** Parsers **MUST** validate:
  - sequence/item delimiters,
  - VR/VL consistency (where possible),
  - string parsing boundaries.
- **REQ-SEC-422:** Panics **MUST** be treated as security bugs; fuzzing **MUST** target panic elimination.

Verification:
- Unit tests **MUST** cover limit enforcement and delimiter validation error paths.
- Parser fuzz targets **MUST** run without panics and regression tests **MUST** capture any prior crashes.

### T2: Decompressor exploits (codec vulnerabilities)

Threat:
- Malformed compressed streams triggering out-of-bounds, excessive allocations, or FFI bugs.

Controls (normative):
- **REQ-SEC-423:** Decoders **MUST** enforce:
  - maximum decompressed pixel count,
  - maximum decompressed byte size,
  - maximum frames per instance,
  - maximum segments/fragment counts.
- **REQ-SEC-424:** Optional FFI codecs (if enabled) **MUST** be isolated:
  - in dedicated crates,
  - behind explicit feature flags,
  - with validated input sizes and strict error translation.
- **REQ-SEC-425:** Codec boundaries **MUST** be fuzzed (see `docs/10`).

Verification:
- Unit tests **MUST** validate decoder limit enforcement and error translation.
- Codec fuzz targets **MUST** exist for each enabled codec boundary.

### T3: Path traversal and unsafe filesystem interactions (native)

Threat:
- Inputs referencing external files, archives with `../`, symlink tricks.

Controls (normative):
- **REQ-SEC-426:** The core **MUST NOT** follow external references by default.
- **REQ-SEC-427:** Archive extraction (if any) **MUST** sanitize paths:
  - reject absolute paths,
  - reject `..`,
  - ignore directory components unless explicitly safe.
- **REQ-SEC-428:** Directory scanning **MUST** treat symlinks conservatively (default: do not follow).

Verification:
- Unit tests **MUST** assert path sanitization behavior and symlink handling defaults.

### T4: Zip bombs / decompression bombs

Threat:
- Small compressed inputs expanding to huge outputs.

Controls (normative):
- **REQ-SEC-429:** Archive processing **MUST** enforce:
  - max compressed bytes,
  - max decompressed bytes,
  - max entries,
  - max total decompressed bytes.
- **REQ-SEC-430:** Pixel decoders **MUST** enforce decompressed pixel limits independent of archive limits.

Verification:
- Unit tests **MUST** assert archive and pixel decompression limits trigger `LimitExceeded`.

### T5: Resource exhaustion (CPU/GPU/time)

Threat:
- Large images, many frames, expensive resampling, repeated interactions.

Controls (normative):
- **REQ-SEC-431:** The system **MUST** implement:
  - bounded caches (LRU) with explicit memory budgets,
  - bounded decode concurrency (native),
  - timeboxing / cooperative cancellation hooks for long-running operations (at least at the task level),
  - bounded GPU texture allocations and eviction (see `docs/11-Performance-and-Caching.md`).
- **REQ-SEC-432:** The UI **SHOULD** prevent unbounded background decoding without user intent.

Verification:
- Unit tests **MUST** confirm cache budgeting and eviction policies.
- Integration tests **MUST** exercise bounded concurrency and cancellation paths.

### T6: Web security (WASM)

Threat:
- Network exfiltration, XSS via metadata rendering, browser sandbox bypass assumptions.

Controls (normative):
- **REQ-SEC-433:** WASM builds **MUST**:
  - perform no network requests by default,
  - treat metadata strings as untrusted and escape in any HTML contexts (prefer text nodes),
  - avoid exposing raw dataset bytes to JS unless explicitly requested by host.
- **REQ-SEC-434:** If network loading is enabled:
  - it **MUST** be opt-in and documented,
  - origin restrictions and CORS constraints **MUST** be explicit.
- **REQ-SEC-438:** Raw-mode parser fallback **MUST** be explicit opt-in and **MUST NOT** bypass configured parse/size limits.
- **REQ-SEC-439:** Raw-mode and charset warnings **MUST** remain telemetry-safe (no raw patient identifiers in warning payloads).
- **REQ-SEC-440:** WebGPU shader provisioning in WASM host integrations **MUST** be CSP-compatible without runtime `eval` or remote shader fetch requirements.

Verification:
- WASM integration tests **MUST** assert no network usage without explicit opt-in.
- UI tests **MUST** validate HTML escaping for metadata rendering paths.
- IO tests **MUST** assert raw-mode behavior is opt-in and warning ordering is deterministic.

Implementation note (RC-2026.02.14):
- Operational batch artifacts that render untrusted metadata (`manifest.csv`, optional `index.html`) apply deterministic escaping/encoding controls and write `manifest.integrity.json` with non-secret version metadata and `sha256` integrity hashes (see `REQ-HI-279`, `REQ-HI-314`, `REQ-HI-359`).

### T7: Supply chain risks

Threat:
- Malicious dependencies, non-reproducible builds.

Controls (normative):
- **REQ-SEC-435:** Builds **SHOULD** be reproducible to the extent feasible.
- **REQ-SEC-436:** Dependency updates **MUST** be reviewed for:
  - unsafe code usage,
  - codec dependencies,
  - changes in transitive dependencies.
- **REQ-SEC-437:** The project **MUST** document any `unsafe` usage and justify it (target: zero `unsafe` in core).

Verification:
- CI **SHOULD** include a reproducibility smoke check when feasible.
- Review checklists **MUST** include dependency safety review and `unsafe` audits.

### T8: Unauthorized access and audit log exposure

Threat:
- Unauthorized access to network services or missing audit trails.

Controls (normative):
- **REQ-AUTH-300:** Authorization decisions **MUST** be fail-closed (default deny unless explicitly allowed).
- **REQ-AUTH-300:** Workflow and SR mutation endpoints must allow only `writer`, `admin`, or `operator` roles. `x-sr-role` and `x-auth-role` are both accepted for role selection.
- **REQ-AUDIT-350:** Audit events **MUST** redact sensitive identifiers using the redaction policy in `docs/13-Error-Model-and-Telemetry.md`.
- **REQ-AUDIT-351:** Audit logs **MUST** enforce retention and rotation via explicit limits on event count and event size.

Verification:
- Unit tests **MUST** validate authorization denial handling and audit redaction/retention limits.

### T9: FHIR ingest and connector callback spoofing/replay

Threat:
- Malicious callback senders forge webhook events, replay stale payloads, or inject malformed FHIR ingest parameters to force unsafe processing paths.

Controls (operational):
- FHIR ingest processing keeps fail-closed typed validation at runtime (`resource_type`, tenant scope, and required payload fields).
- Webhook signing controls only allow explicitly supported strategies (`none`, `hmac-sha256`); invalid strategy values and missing shared secrets are rejected.
- Connector callback failure capture stores bounded, redacted excerpts and deterministic correlation data so incident triage does not expose raw patient identifiers.

Verification:
- Regression tests cover webhook auth strategy misconfiguration (`missing secret`, `invalid strategy`) and fail-closed outcomes.
- Runtime route-policy tests confirm deny-list precedence still applies under role/tenant-context requests.
- Release evidence bundles include callback failure and security gate artifacts for the active `release_id`.

---

## 3.1 Runtime default security profile (release baseline)

Release-profile runtime defaults are intentionally fail-closed:

- `dicom-web-server` defaults to `DICOM_WEB_TLS_POLICY=require_tls` and `DICOM_WEB_AUTH_MODE=deny_all`; insecure transport declarations do not bypass the TLS policy gate.
- `dicom-workflow-server` defaults to `DICOM_WORKFLOW_AUTH_MODE=deny_all` and rejects requests unless `DICOM_WORKFLOW_TRANSPORT_SECURITY=tls` is explicitly declared.
- `dicom-dimse-service::DimseServerConfig::default()` requires TLS (`TlsPolicy::RequireTls`), starts with insecure transport state, and denies authorization by default (`DimseAuthConfig::deny_all()`), requiring explicit operator override for non-production harnesses.
- **REQ-WF-401:** Workflow operations MUST enforce tenant context isolation across MWL/MPPS/SR/task workflows. The server MUST resolve tenant context from `x-tenant`, `x-workflow-tenant`, `x-tenant-id`, or `x-workflow-tenant-id` (default `tenant-default`) and return `DVF.WORKFLOW.SR.AUTH_DENIED` on tenant mismatch.

Verification:
- Runtime tests assert TLS-rejection and auth-denial behavior across DICOMweb, DIMSE, and workflow runtime entrypoints.

---

## 3.2 Persistence retention and rotation profile (release baseline)

Release-profile persistence paths are bounded to prevent uncontrolled disk growth:

- `dicom-web-server` enforces WAL retention/rotation using:
  - `DICOM_WEB_STORAGE_WAL_MAX_BYTES` (default `134217728`)
  - `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES` (default `3`)
- `dicom-workflow-server` enforces snapshot retention/rotation using:
  - `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES` (default `33554432`)
  - `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES` (default `2`)
- Startup preflight checks:
  - persistence target must be a writable file path,
  - parent directories are created deterministically,
  - invalid directory-as-file targets fail fast.

Verification:
- Runtime tests assert preflight path validation, bounded rotation count, and restart continuity after persisted writes.

---

## 4. Default limits (normative)

Limits are part of the **security and correctness envelope**. Defaults are published to make behavior reproducible across builds and to ensure pre-allocation bounding for hostile inputs.

Requirements:

- **REQ-SEC-401:** The project **MUST** define explicit numeric default values for each limit listed below.
- **REQ-SEC-402:** Defaults **MUST** be enforced by default and **MUST NOT** be disabled. Integrators may override defaults only by providing an explicit `Limits` configuration.
- **REQ-SEC-403:** Changing any default limit value **MUST** increment `envelope_version` **minor** at minimum (see `docs/14-Release-and-Versioning.md`).
- **REQ-SEC-404:** All limits **MUST** be enforced before allocations based on declared sizes (rows/cols/frames/VLs) and before decompression expands buffers.

### 4.1 Default limit values

All byte counts use **binary prefixes** (MiB/GiB).

| Limit | Default | Applies to | Failure behavior |
|---|---:|---|---|
| `max_input_bytes` | 512 MiB | single input buffer/file | `LimitExceeded(max_input_bytes)` |
| `max_dataset_elements` | 250,000 | number of decoded elements | `LimitExceeded(max_dataset_elements)` |
| `max_workflow_pair_count` | 256 | workflow query/form key-value pairs | `LimitExceeded(max_workflow_pair_count)` |
| `max_sequence_depth` | 64 | nested sequence/item depth | `LimitExceeded(max_sequence_depth)` |
| `max_string_bytes` | 1 MiB | single text element value | `LimitExceeded(max_string_bytes)` |
| `max_element_vl_bytes` | 64 MiB | single element VL for bulk data | `LimitExceeded(max_element_vl_bytes)` |
| `max_frames_per_instance` | 4,096 | Number of Frames | `LimitExceeded(max_frames_per_instance)` |
| `max_pixels_per_frame` | 16,777,216 (4096x4096) | rowsxcols | `LimitExceeded(max_pixels_per_frame)` |
| `max_decompressed_bytes` | 1 GiB | post-decode pixel buffer(s) per instance | `LimitExceeded(max_decompressed_bytes)` |
| `max_gpu_texture_bytes` | 512 MiB | total cached GPU textures | `LimitExceeded(max_gpu_texture_bytes)` |
| `max_cache_bytes` | 1 GiB | total CPU caches (headers+pixels+volumes) | `LimitExceeded(max_cache_bytes)` |
| `max_pdu_bytes` | 1 MiB | DIMSE UL PDU value length | `LimitExceeded(max_pdu_bytes)` |
| `max_pdv_bytes` | 256 KiB | DIMSE PDV value length | `LimitExceeded(max_pdv_bytes)` |
| `max_presentation_contexts` | 128 | association presentation context count | `LimitExceeded(max_presentation_contexts)` |
| `max_command_bytes` | 64 KiB | DIMSE command set byte length | `LimitExceeded(max_command_bytes)` |
| `max_connections` | 64 | concurrent DIMSE associations | `LimitExceeded(max_connections)` |

Notes (informative):
- The defaults target common CT/MR use cases while keeping decompression bombs bounded. Integrators handling very large series **MAY** raise limits, but must then update their own risk analysis and performance budgets.
- Networking limits are enforced by `dicom-net::NetworkLimits` and `dicom-dimse::DimseLimits`.
- `max_connections` is enforced by `dicom-dimse-service` before accepting associations.
- `max_input_bytes` also bounds DIMSE C-STORE data set accumulation.
- When DIMSE query/retrieve features are enabled, identifier data sets should be bounded by `max_input_bytes` before accumulation.
- DICOMweb request bodies are bounded by `max_input_bytes`; request URIs, headers, and query keys/values are bounded by `max_string_bytes`, and query parameter count is bounded by `max_dataset_elements`.
- Workflow endpoint query/body maps are additionally bounded by `max_workflow_pair_count`, and parser overrun reports use `LimitExceeded(max_workflow_pair_count)`.

### 4.2 Audit log limits

Audit retention limits are enforced by `dicom-audit` and are independent of `Limits`.

| Limit | Default | Applies to | Failure behavior |
|---|---:|---|---|
| `max_audit_events` | 8,192 | retained audit records | oldest records dropped deterministically |
| `max_audit_event_bytes` | 2,048 | encoded size per event | `LimitExceeded(max_audit_event_bytes)` |

Notes (informative):
- Audit logging can be disabled only via explicit configuration (e.g., `AuditConfig::disabled()`).

### 4.3 Runtime policy and security-boundary schema

This section defines the operational controls that directly affect security boundaries and abuse resistance.

| Scope | Environment variable | Type | Default | Constraint | Enforcement impact |
|---|---|---|---|---|---|
| Rate limiting | `DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS` | milliseconds | `15000` | `> 0` | throttling window applied to `query` and `mutation` routes |
| Rate limiting | `DICOM_WORKFLOW_QUERY_RATE_LIMIT` | integer | `300` | `>= 0` | `LimitExceeded(workflow_rate_limit)` for read paths when over quota |
| Rate limiting | `DICOM_WORKFLOW_MUTATION_RATE_LIMIT` | integer | `120` | `>= 0` | `LimitExceeded(workflow_rate_limit)` for write paths when over quota |
| Storage durability | `DICOM_WEB_STORAGE_WAL_MAX_BYTES` | bytes | `134217728` | `>= 0` | storage WAL preflight ensures bounded disk growth before listen |
| Storage durability | `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES` | count | `3` | `>= 1` | storage rotation count validated before startup |
| Storage durability | `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES` | bytes | `33554432` | `>= 0` | workflow snapshot preflight rotates prior to first request |
| Storage durability | `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES` | count | `2` | `>= 1` | startup fails if rotation policy is invalid |
| Storage durability | `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES` | count | `3` | `>= 1` | startup preflight validates WAL policy consistency |
| Storage durability | `DICOM_WORKFLOW_AUDIT_MAX_BYTES` | bytes | `8388608` | `>= 0` | workflow audit preflight validates persistence targets before serving |
| Storage durability | `DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES` | count | `4` | `>= 1` | startup fails for non-positive rotation policy |
| Storage durability | `DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT` | count | `256` | `>= 0` | export command paths cap event rows at configured limit |

Notes:
- Any unparseable value for integer controls above is startup-invalid (`invalid input`) and must be corrected before the process can bind.
- DIMSE transport policy, auth, and size controls are covered by the separate `docs/33-Productization-Profiles-and-Playbooks.md` and `docs/12-API-Surface-and-Crate-Boundaries.md` threat contracts.
- `DICOM_WORKFLOW_DENYLIST_PATHS`, `DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_AUTH_TOKEN`, `DICOM_WORKFLOW_TRANSPORT_SECURITY`, and `DICOM_WEB_*` auth/tls toggles are security boundary controls and must remain fail-closed by default.

### 4.3.1 Explicit security-boundary schema

Security controls use strict parser contracts. Any control failing parse must fail before service startup.

| Scope | Variable | Parser | Constraint | Default | Enforcement outcome |
|---|---|---|---|---|---|
| Rate limiting | `DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS` | `u64` | `> 0` | `15000` | closes request window and applies `workflow_rate_limit`; startup-failed on invalid parse |
| Rate limiting | `DICOM_WORKFLOW_QUERY_RATE_LIMIT` | `u64` | `>= 0` | `300` | `LimitExceeded(workflow_rate_limit)` on read over quota |
| Rate limiting | `DICOM_WORKFLOW_MUTATION_RATE_LIMIT` | `u64` | `>= 0` | `120` | `LimitExceeded(workflow_rate_limit)` on write over quota |
| Data-plane input | `DICOM_WEB_STORAGE_WAL_MAX_BYTES` | `u64` | `>= 0` | `134217728` | WAL preflight validation before bind |
| Data-plane input | `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES` | `usize` | `>= 1` | `3` | bounded WAL rotation policy |
| Data-plane input | `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES` | `u64` | `>= 0` | `33554432` | snapshot preflight validation before serving |
| Data-plane input | `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES` | `usize` | `>= 1` | `2` | snapshot preflight validation before serving |
| Audit and retention | `DICOM_WORKFLOW_AUDIT_MAX_BYTES` | `u64` | `>= 0` | `8388608` | workflow audit preflight and bounded retention |
| Audit and retention | `DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES` | `usize` | `>= 1` | `4` | startup fails when non-positive |
| Audit and retention | `DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT` | `usize` | `>= 0` | `256` | export endpoint caps returned rows |
| Routing control | `DICOM_WORKFLOW_DENYLIST_PATHS` | string list | delimiters `[,; \\t\\n ]` | service defaults | deny-list match is any-template OR; defaults apply only when unset/empty |
| Security boundary | `DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_TRANSPORT_SECURITY`, `DICOM_WORKFLOW_TLS_CERT_PATH`, `DICOM_WORKFLOW_TLS_KEY_PATH` | enum/string | constrained enum values and file existence | deny-by-default | startup-failed on invalid policy-material pair |
| Security boundary | `DICOM_WEB_TLS_POLICY`, `DICOM_WEB_AUTH_MODE`, `DICOM_WEB_TRANSPORT_SECURITY` | enum | constrained enum values | secure-by-default `require_tls` / `deny_all` | startup must not silently relax policy |
| Security boundary | `DICOM_DIMSE_TLS_POLICY`, `DICOM_DIMSE_TRANSPORT_SECURITY`, `DICOM_DIMSE_AUTH_MODE`, `DICOM_DIMSE_TLS_*` | enum/string | constrained enum values and file existence | secure/deny-by-default | startup-failed on invalid policy-material pair |

Implementation note (informative):
- Env-schema drift for controls listed above is part of the runtime contract and must be mirrored in docs and release notes before rollout.

#### DICOMweb transport controls (normative)

- **REQ-HTTP-303:** DICOMweb integrations **MUST** enforce an explicit TLS policy at the request boundary; insecure transports **MUST** fail closed when TLS is required.
- **REQ-HTTP-304:** DICOMweb integrations **MUST** enforce request throttling decisions before routing; rejected requests **MUST** return `LimitExceeded` with policy-provided limit metadata.

Implementation note (informative):
- `dicom-web::DicomWebServiceConfig::default()` is secure-by-default (`TlsPolicy::RequireTls`, `WebAuthConfig::deny_all`); permissive modes require explicit operator opt-in.

#### DIMSE transport controls (normative)

- **REQ-NET-308:** DIMSE integrations **MUST** enforce an explicit TLS policy at the association boundary; insecure transports **MUST** fail closed when TLS is required.
- **REQ-NET-309:** DIMSE integrations **MUST** enforce association throttling via `max_connections`; rejected associations **MUST** return `LimitExceeded(max_connections)`.

#### Storage and index limits (normative)

- **REQ-STOR-303:** Storage ingestion **MUST** enforce `max_input_bytes` per input and `max_cache_bytes` across stored bytes before accepting data.
- **REQ-META-302:** Metadata indexing **MUST** enforce `max_dataset_elements` as a bound on total indexed instances.

#### Query/Retrieve limits (normative)

- **REQ-QR-302:** Query key values **MUST** be bounded by `max_string_bytes`, and query result counts **MUST** be bounded by `max_dataset_elements`.
- **REQ-HTTP-301:** Workflow endpoint query and form parsing **MUST** also enforce `max_workflow_pair_count` before allocation and return `LimitExceeded(max_workflow_pair_count)` when exceeded.

#### Worklist and MPPS limits (normative)

- **REQ-WL-302:** MWL string values **MUST** be bounded by `max_string_bytes`, and MWL response item counts **MUST** be bounded by `max_dataset_elements`.
- **REQ-MPPS-353:** MPPS ingestion **MUST** enforce `max_dataset_elements` as a bound on total tracked MPPS instances.

Verification:
- Unit tests **MUST** verify each limit returns `LimitExceeded` with the correct `limit_name`.
- Release notes **MUST** record any default limit changes alongside `envelope_version` bumps.

### 4.2 Fuzzing limits (normative)

- **REQ-SEC-410:** Fuzz harnesses **MUST** set limits lower than production defaults to maximize throughput, and any limit violation **MUST** return structured errors, not panics.

Verification:
- Fuzz harness configs **MUST** include explicit limit values lower than defaults.

---

## 5. Sandboxing and execution environment assumptions

Assumption: Developer/CI execution occurs in a sandbox with network disabled by default.

Justification: reduces exposure to supply chain and accidental data exfiltration.

Required workflow controls:
- **REQ-SEC-438:** CI **MUST NOT** download datasets by default (see `docs/10-Testing-Corpus-Fuzzing-Evals.md`).
- **REQ-SEC-439:** When network is enabled for explicit tasks:
  - commands **MUST** be allowlisted,
  - fetched artifacts **MUST** be pinned by hash,
  - results **MUST** be recorded in manifests.

Verification:
- CI configuration **MUST** show dataset downloads disabled by default.
- Any network-enabled workflow **MUST** include hash manifests and command allowlists in review.

---

## 6. Verification requirements

Requirements:
- **REQ-SEC-440:** Parser fuzz targets **MUST** exist.
- **REQ-SEC-441:** Codec boundary fuzz targets **MUST** exist for each enabled codec.
- **REQ-SEC-442:** All fuzz crashers **MUST** become regression tests or corpus seeds.
- **REQ-SEC-443:** `cargo clippy` with warnings-as-errors **MUST** pass under the safe command set.

Verification:
- CI **MUST** run fuzz smoke tests (or document manual gate) and enforce clippy in the safe command set.

---

## 7. RC-2026.02.11 security closure baseline (release-specific)

This section records the Sprint 13 release-specific security evidence bundle and control mappings.

Evidence bundle:
- Fuzz campaign report: `reports/security/fuzz-campaign-report-RC-2026.02.11.md`
- Fuzz crash inventory: `reports/security/crash-inventory-RC-2026.02.11.md`
- Authz/audit regression report: `reports/security/authz-audit-regression-report-RC-2026.02.11.md`
- Dependency/SBOM review report: `reports/security/dependency-vulnerability-review-RC-2026.02.11.md`
- Penetration checklist and findings: `reports/security/penetration-checklist-RC-2026.02.11.md`, `reports/security/penetration-findings-RC-2026.02.11.md`
- Security remediation ledger: `reports/security/security-remediation-ledger-RC-2026.02.11.md`
- Security gate decision: `reports/security/security-gate-decision-RC-2026.02.11.md`

Control mapping snapshot:
- Parser/network/pixel hostile-input coverage: REQ-SEC-440, REQ-SEC-441, REQ-SEC-442
- Runtime auth/TLS fail-closed controls: REQ-AUTH-300, REQ-NET-308, REQ-HTTP-303
- Audit redaction/retention controls: REQ-AUDIT-350, REQ-AUDIT-351
- Dependency source/checksum review controls: REQ-SEC-436, REQ-REL-601

Release posture outcome:
- Sprint 13 security closure is release-gated by signed evidence artifacts listed above and the signed gate decision.
