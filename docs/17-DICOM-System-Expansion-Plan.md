# DICOM system expansion plan (informative)

This document is an **informative** implementation plan for expanding RDVF from the workstation baseline to an **enterprise-complete DICOM system**. It does not add new normative requirements; normative changes will be captured in the relevant `docs/` specs with REQ identifiers.

## Definition of "true functional DICOM system"

The target system supports end-to-end clinical data flow with:
- DICOM networking (DIMSE) and DICOMweb endpoints.
- Storage, indexing, and query/retrieve.
- Worklist and procedure status (MWL/MPPS).
- Broad SOP Class and Transfer Syntax coverage with explicit conformance envelope updates.
- Deterministic viewer integration and audit-safe, fail-closed behavior.

## Guardrails that remain non-negotiable

- Determinism and fail-closed behavior remain required for all parsing and rendering.
- Default limits remain enforced and versioned; expansions only happen via explicit features/packs.
- All inputs remain untrusted; security model and fuzzing expand with new surfaces.
- Claim surface remains free of autonomous clinical decision/treatment claims and regulatory authorization claims by default.

## Workstreams

### 1) Networking (DIMSE + DICOMweb)

Scope:
- DIMSE SCU/SCP for C-ECHO, C-STORE, C-FIND, C-MOVE, C-GET.
- DICOMweb QIDO-RS, WADO-RS, STOW-RS.
- TLS, AE Title validation, PDU limits, and throttling.

Implementation pattern:
- New crate(s): `dicom-net`, `dicom-dimse`, `dicom-web`.
- Message parsing and PDV handling are fail-closed and limit-bounded.
- Feature gating for each service class.

Verification:
- Unit tests for PDU parsing, association negotiation, and limit enforcement.
- Integration tests for DIMSE state machines (synthetic data only).
- Fuzz targets for PDU parsing and DICOMweb request decoding.

Docs to update:
- `docs/03`, `docs/04`, `docs/09`, `docs/10`, `docs/12`, `docs/13`.

### 2) Storage and indexing

Scope:
- Instance storage (filesystem or object store abstraction).
- Metadata index for Study/Series/Instance queries.
- De-duplication and deterministic ingestion ordering.

Implementation pattern:
- New crate `dicom-storage` with pluggable backends.
- Canonical hash of raw DICOM bytes for dedup.
- Write-ahead logging for safe ingestion and replay.

Verification:
- Unit tests for index integrity and deterministic ordering.
- Integration tests for query consistency under concurrent ingests.
- Fuzz targets for metadata indexing (no PHI).

Docs to update:
- `docs/04`, `docs/09`, `docs/10`, `docs/12`, `docs/13`.

### 3) Query/Retrieve services

Scope:
- DIMSE C-FIND/C-MOVE/C-GET.
- DICOMweb QIDO-RS/WADO-RS.

Implementation pattern:
- Query translation between DIMSE datasets and index storage.
- Deterministic response ordering for stable results.

Verification:
- Integration tests for query matching and response ordering.
- Negative tests for unsupported query keys (fail closed).

Current baseline status (informative):
- `dicom-dimse-service` includes a storage-backed DIMSE service implementation that persists C-STORE inputs and resolves C-FIND/C-MOVE/C-GET identifiers against persisted datasets.
- `dicom-web` runtime executes QIDO/WADO/STOW over the same storage/query boundaries with deterministic ordering and fail-closed key validation.

Docs to update:
- `docs/04`, `docs/09`, `docs/10`, `docs/12`, `docs/13`.

### 4) Worklist and procedure status

Scope:
- Modality Worklist (MWL).
- Modality Performed Procedure Step (MPPS).

Implementation pattern:
- Separate service crate with strict dataset validation.
- Feature gating for MWL/MPPS.

Verification:
- Unit tests for required tag validation and error mapping.
- Integration tests for MWL query/response and MPPS updates.

Current baseline status (informative):
- `crates/dicom-worklist` now includes a deterministic persisted `WorklistStore` with upsert/query filters and optional audit callback wiring.
- `crates/dicom-mpps` now includes a persisted `MppsService` wrapper over `MppsStore` with transition enforcement and optional audit callback wiring.

Docs to update:
- `docs/03`, `docs/04`, `docs/09`, `docs/10`, `docs/12`, `docs/13`.

### 5) SOP Class and Transfer Syntax expansion

Scope (SOP classes):
- Expand baseline imaging coverage (CT, MR, SC, CR, DX, MG, US, NM, XA/XRF).
- Expand derived objects (SEG, SR, GSPS, RT Dose/Structure/Plan).
- Add video SOP classes when video transfer syntaxes are supported.

Scope (transfer syntaxes):
- JPEG Lossless (Process 14/14SV1), JPEG Extended.
- JPEG-LS and JPEG 2000 already gated; extend to Part 2 where needed.
- MPEG-2/4, H.264/AVC, H.265/HEVC video syntaxes (feature-gated).

Implementation pattern:
- One pack per SOP family, mirroring existing pack design.
- One codec feature per transfer syntax family, with boundary fuzzing.

Verification:
- Golden corpus for new pixel outputs (synthetic by default).
- Differential tests for codecs where reference decoders exist.

Docs to update:
- `docs/03`, `docs/05`, `docs/10`, `docs/12`, `docs/14`, `docs/16`.

### 6) Auth, audit, and access control

Scope:
- Authn/authz for network endpoints.
- Audit log events for ingestion, query, retrieve, and delete.
- Configurable retention policies.

Implementation pattern:
- Authorization policy layer in networking crates.
- Audit logs with redaction consistent with telemetry rules.

Verification:
- Unit tests for policy enforcement.
- Security tests for unauthenticated and unauthorized access paths.

Docs to update:
- `docs/09`, `docs/13`.

## Milestones (ordered)

### Milestone A: Foundations

Deliverables:
- New networking/storage crate skeletons with feature flags.
- Updated envelope draft in `docs/03` with explicit non-support list for services.
- Expanded requirements index plan in `docs/16`.

Done when:
- `cargo build`, `cargo clippy`, `cargo test` pass.
- New crates compile with no runtime functionality beyond stubs.

### Milestone B: Storage + C-STORE + STOW

Deliverables:
- File/object store backend with deterministic indexing.
- DIMSE C-STORE SCP and DICOMweb STOW-RS.
- End-to-end ingest from DIMSE and DICOMweb into storage.

Done when:
- Integration tests ingest and retrieve a synthetic dataset.
- Limits enforced before allocation and on decompressed pixel data.

### Milestone C: Query/Retrieve

Deliverables:
- DIMSE C-FIND/C-MOVE/C-GET.
- DICOMweb QIDO-RS/WADO-RS.

Done when:
- Query results are deterministic and stable across runs.
- Negative tests assert fail-closed behavior for unsupported keys.

### Milestone D: Worklist + MPPS

Deliverables:
- MWL query service with strict tag validation.
- MPPS update ingestion and persistence.

Done when:
- Worklist queries return deterministic, validated responses.
- MPPS updates are recorded with audit logs.

### Milestone E: SOP/TS expansion

Deliverables:
- Pack implementations and manifests for missing SOP families.
- Codec packs and transfer syntax coverage with corpus + fuzz.

Done when:
- Envelope and corpus updates are complete for each new SOP/TS.
- Fuzz target inventory includes each new boundary.

## Risks and mitigations

- Increased attack surface from networking and web endpoints.
  Mitigation: strict limits, TLS-only modes, fuzzing on all parsers.
- Codec complexity and non-determinism across platforms.
  Mitigation: CPU oracle outputs, fixed-point fallback, and golden corpus.
- PHI exposure in logs and debug traces.
  Mitigation: centralized redaction policy and audit controls.

## Next edits required (when executing)

- Update `docs/03` (envelope), `docs/04` (IO/series), `docs/05` (pixel semantics),
  `docs/09` (threat model), `docs/10` (corpus/fuzz), `docs/12` (API surface),
  `docs/13` (error model), `docs/14` (envelope versioning), and `docs/16` (REQ index).
- Add new tasklists under `tasks/` for each workstream to keep diffs scoped.
