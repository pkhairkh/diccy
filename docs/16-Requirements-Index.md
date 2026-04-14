# Requirements index

This document defines the requirement identifier (REQ) scheme used across this repository and indexes high-level cross-cutting requirements.

## 1. Identifier scheme (normative)

- **REQ-FAMILY-NNN** where:
  - `FAMILY` is a short area code (e.g., `SCOPE`, `PIX`, `GPU`, `WASM`, `SEC`, `TEST`, `REL`, `TEL`),
  - `NNN` is a zero-padded integer unique within the family.

Requirements:
- **REQ-INDEX-001:** Every **new** normative requirement added to `docs/` **MUST** include a REQ identifier.
- **REQ-INDEX-002:** If a change modifies the semantics of an existing requirement, the change **MUST**:
  - update the requirement text in-place,
  - update verification text,
  - and, when outputs/limits change, bump `envelope_version` rules per `docs/14`.
- **REQ-INDEX-003:** Tests **MUST** reference at least one REQ identifier for the behavior they verify (see `docs/10`).

Verification:
- A CI check **MUST** parse `docs/` for `REQ-` identifiers and produce:
  - a list of MUST-level REQs with their locations,
  - a list of referenced REQs found in tests,
  - a report of REQs without any test references.

## 2. Cross-cutting requirement index

### Scope / claim surface

- `REQ-SCOPE-001` .. `REQ-SCOPE-004` — Intended purpose boundaries for workstation workflow scope, provenance, and claim-surface controls.  
  Source: `docs/01-Vision-and-Scope.md`.

### Glossary governance

- `REQ-TERM-001` .. `REQ-TERM-003` — Glossary authority and term usage requirements.  
  Source: `docs/00-Glossary.md`.

### Pixel pipeline determinism

- `REQ-PIX-201` — CPU pipeline is the source of truth for VOI/modality math.  
- `REQ-GPU-210` — GPU shaders MUST NOT implement VOI/modality transforms; GPU is presentation-only.  
- `REQ-PIX-220` — Non-finite (NaN/Inf) intermediate values MUST fail closed.  
  Source: `docs/05-Pixel-Pipeline.md`.

### WASM numerics

- `REQ-WASM-301` — WASM builds MUST avoid NaN/Inf nondeterminism via rejection/canonicalization policy.  
  Source: `docs/08-WASM-Target.md`.

### Security limits

- `REQ-SEC-401` — Numeric default limits MUST be defined and versioned as part of the envelope.  
- `REQ-SEC-410` — Limits MUST be enforced pre-allocation and fuzz harnesses MUST use conservative limits.  
  Source: `docs/09-Security-Threat-Model.md`, `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

### Networking (DIMSE UL)

- `REQ-NET-300` .. `REQ-NET-399` - UL PDU parsing, association negotiation/state machine sequencing, AE title validation, and networking limits.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`, `docs/13-Error-Model-and-Telemetry.md`.

### DIMSE command handling

- `REQ-DIMSE-300` .. `REQ-DIMSE-399` - DIMSE command parsing for Verification/Storage/Query/Retrieve services, including deterministic pending/final response sequencing.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/13-Error-Model-and-Telemetry.md`.

### DICOMweb

- `REQ-WEB-300` .. `REQ-WEB-399` - DICOMweb endpoint coverage, UID validation, request body handling, and service-runtime auth/audit/storage-query execution semantics.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/13-Error-Model-and-Telemetry.md`.
- `REQ-HTTP-300` .. `REQ-HTTP-349` - HTTP method and URI/query parsing limits for DICOMweb.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`.

### Transfer syntax and codec packs

- `REQ-TS-201` .. `REQ-TS-204` - Transfer syntax tier coverage and codec pack acceptance rules.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`.
- `REQ-TS-205` - Video transfer syntax feature gating.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`.
- `REQ-CODEC-350` .. `REQ-CODEC-399` - Codec pack decoding determinism and boundary checks.  
  Source: `docs/05-Pixel-Pipeline.md`, `docs/09-Security-Threat-Model.md`, `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

### Storage and metadata indexing

- `REQ-STOR-300` .. `REQ-STOR-399` - Deterministic ingestion, hashing, deduplication, and write-ahead log replay semantics.  
  Source: `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`, `docs/13-Error-Model-and-Telemetry.md`.
- `REQ-META-300` .. `REQ-META-349` - Metadata extraction, deterministic ordering, and index limits.  
  Source: `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`.

### Query/Retrieve services

- `REQ-QR-300` .. `REQ-QR-399` - Query key support (UID and supported text filters including Patient ID/Modality/Accession Number/Study Date), deterministic ordering, and query limit enforcement.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`.

### Worklist and MPPS

- `REQ-WL-300` .. `REQ-WL-349` - Modality Worklist required tags, deterministic ordering, persisted query/upsert semantics, and limit enforcement.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`.
- `REQ-MPPS-350` .. `REQ-MPPS-399` - MPPS update validation, deterministic status transitions, persisted service audit semantics, and limit enforcement.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/09-Security-Threat-Model.md`, `docs/13-Error-Model-and-Telemetry.md`.
- `REQ-WF-401` .. `REQ-WF-402` - Workflow tenancy isolation enforces tenant scoping on MWL/MPPS/SR/task queries and mutations, and cross-tenant mutation attempts return `DVF.WORKFLOW.SR.AUTH_DENIED`.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/09-Security-Threat-Model.md`, `docs/12-API-Surface-and-Crate-Boundaries.md`.

### Imaging packs (SOP classes and enhanced multi-frame)

- `REQ-SOP-300` .. `REQ-SOP-349` - Imaging SOP class envelope expansion, pack gating, and manifest validation.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/06-Rendering-and-Interaction.md`, `docs/07-Volume-and-Fusion.md`.
- `REQ-ENH-350` .. `REQ-ENH-399` - Enhanced multi-frame functional group parsing and validation.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`.

### Presentation State (GSPS)

- `REQ-GSPS-300` .. `REQ-GSPS-399` - GSPS parsing, shutter/graphics rules, and deterministic overlay precedence.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/05-Pixel-Pipeline.md`, `docs/06-Rendering-and-Interaction.md`.

### Segmentation and RT

- `REQ-SEG-300` .. `REQ-SEG-349` - Segmentation alignment, single-frame and multi-frame constraints, and overlay determinism.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/07-Volume-and-Fusion.md`.
- `REQ-RT-350` .. `REQ-RT-399` - RT Dose scaling/alignment, structure set overlay validation, and RT Plan reference checks.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/07-Volume-and-Fusion.md`.

### Structured Reporting (SR)

- `REQ-SR-300` .. `REQ-SR-399` - SR parsing, ingestion/create-update handling, and reference validation for measurement extraction.  
  Source: `docs/03-DICOM-Conformance-Envelope.md`, `docs/06-Rendering-and-Interaction.md`.

### Measurements

- `REQ-MEAS-001`, `REQ-MEAS-010` .. `REQ-MEAS-040` — Measurement mode gating and provenance.  
  Source: `docs/06-Rendering-and-Interaction.md`.

### Human interface

- `REQ-HI-100` .. `REQ-HI-439` - Human interface design controls for governance, role/session safety, workflow-state integrity, deterministic viewer behavior, derived-object/reporting UX, fail-closed error handling, privacy/security UX controls, accessibility/localization, lifecycle traceability gates, interoperability/runtime operator controls, protocol-level service semantics, WASM/GPU runtime boundary controls, strict runtime-config contracts, advanced pack semantics (Enhanced/GSPS/SEG/RT/SR), and persisted MWL/MPPS service-state controls.  
  Source: `HI_REQ.md`, `docs/06-Rendering-and-Interaction.md`, `docs/09-Security-Threat-Model.md`, `docs/10-Testing-Corpus-Fuzzing-Evals.md`, `docs/13-Error-Model-and-Telemetry.md`.

### Performance

- `REQ-PERF-701` .. `REQ-PERF-703` — Web performance matrix constraints.  
  Source: `docs/11-Performance-and-Caching.md`.

### Volume

- `REQ-VOL-901` .. `REQ-VOL-929` — Volume assembly, MPR determinism, fusion eligibility, and scalability constraints.  
  Source: `docs/07-Volume-and-Fusion.md`.
### Telemetry redaction

- `REQ-TEL-501` — Telemetry MUST NOT emit raw UIDs/paths/URLs; must emit salted digests for correlation.  
  Source: `docs/13-Error-Model-and-Telemetry.md`.

### Auth and audit

- `REQ-AUTH-300` .. `REQ-AUTH-349` - Authentication/authorization policy hooks and fail-closed behavior.  
  Source: `docs/09-Security-Threat-Model.md`, `docs/13-Error-Model-and-Telemetry.md`.
- `REQ-AUDIT-350` .. `REQ-AUDIT-399` - Audit redaction, retention, and limit enforcement.  
  Source: `docs/09-Security-Threat-Model.md`, `docs/13-Error-Model-and-Telemetry.md`.

### Release artifacts / SBOM

- `REQ-REL-601` — Releases MUST publish SBOM and build+corpus metadata and link security advisories to envelope versions.  
  Source: `docs/14-Release-and-Versioning.md`.

### Traceability

- `REQ-TEST-701` — Tests MUST reference REQs and CI MUST report coverage.  
- `REQ-TEST-761` .. `REQ-TEST-762` — Workflow feature-gated test policy (`workflow-main-tests`) and explicit CI/release evidence expectations.  
  Source: `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

### Wave 1 operationalization

- `REQ-WAVE1-801` — Product profiles (`core`, `workstation`, `server`) MUST declare explicit capability boundaries.  
  Source: `docs/33-Productization-Profiles-and-Playbooks.md`.
- `REQ-WAVE1-802` — WASM backend runtime mode and fallback evidence MUST be exportable from the host diagnostics surface.  
  Source: `docs/08-WASM-Target.md`, `docs/34-Wave1-Test-Plan-and-Evidence.md`.
- `REQ-WAVE1-803` — Raw-mode parser fallback MUST be explicit opt-in and MUST emit structured warnings.  
  Source: `docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/13-Error-Model-and-Telemetry.md`.
- `REQ-WAVE1-804` — Wave release evidence MUST include traceability mapping and verification command outcomes.  
  Source: `docs/14-Release-and-Versioning.md`, `docs/34-Wave1-Test-Plan-and-Evidence.md`.

## 3. Evidence mapping examples

- Interoperability and workflow verification commonly maps `REQ-WF-401`, `REQ-WF-402`, `REQ-AUTH-300`, `REQ-AUDIT-350`, `REQ-HI-335`, and `REQ-HI-340` in release evidence under `reports/interoperability/` and `reports/traceability/`.
- Testing and audit closure commonly maps `REQ-TEST-701`, `REQ-INDEX-003`, `REQ-OPS-001`, and `REQ-OPS-008` in release evidence under `reports/traceability/` and `reports/release/`.
- Frontend governance and role-aware pages map to `REQ-OPS-009`, `REQ-HI-100`, and `REQ-HI-317`.
