# Vision and scope

## Mission (normative)

Status: **Implemented/Deferred by subsystem** (As of 2026-02-21)
Reference: `docs/31-Implementation-Status.md#major-subsystems`


Requirements:

- **REQ-SCOPE-005:** The project **MUST** provide a Rust PACS workstation framework that can ingest, manage, and render DICOM studies with:
  - strict correctness within a declared **conformance/workflow envelope**,
  - deterministic pixel processing,
  - deterministic rendering on native GPU and web CPU/canvas baseline, with explicit WebGPU fallback orchestration and gated production web draw path,
  - security-first handling of untrusted inputs,
  - complete PACS workflow coverage (ingest, storage, query/retrieve, reporting, and operations) in the workstation profile.

See:
- `docs/03-DICOM-Conformance-Envelope.md`
- `docs/05-Pixel-Pipeline.md`
- `docs/09-Security-Threat-Model.md`

Verification:

- Documentation lint confirms this section links to `docs/03-DICOM-Conformance-Envelope.md`, `docs/05-Pixel-Pipeline.md`, and `docs/09-Security-Threat-Model.md`.
- Tier 0 corpus tests confirm deterministic pixel outputs (see `docs/10-Testing-Corpus-Fuzzing-Evals.md`).

## Design principles (normative)

1. **Small core**
   - **REQ-SCOPE-006:** The core **MUST** remain minimal and stable.
   - **REQ-SCOPE-007:** Expanded modality behavior **MUST** live behind explicit features or separate crates.

2. **Fail closed**
   - **REQ-SCOPE-008:** Inputs outside the envelope **MUST** yield a typed error and **MUST NOT** produce undefined outputs.

3. **Deterministic pipelines**
   - **REQ-SCOPE-009:** For fixed inputs and config, the pixel pipeline **MUST** be deterministic (see `docs/05-Pixel-Pipeline.md`).

4. **Security by default**
   - **REQ-SCOPE-010:** Resource limits **MUST** be enabled by default (see `docs/09-Security-Threat-Model.md`).

5. **Portability**
   - **REQ-SCOPE-011:** Core logic **MUST** be portable to `wasm32-unknown-unknown` without platform-specific APIs.
   - **REQ-SCOPE-012:** Platform glue **MUST** be isolated (see `docs/08-WASM-Target.md`).

Verification:

- WASM CI builds pass for portable crates (see `docs/08-WASM-Target.md`).
- Limit enforcement tests pass (see `docs/09-Security-Threat-Model.md`).
- Golden corpus tests confirm deterministic CPU-boundary outputs (see `docs/10-Testing-Corpus-Fuzzing-Evals.md`).

## In scope

Requirements:

- **REQ-SCOPE-013:** Features listed as in-scope **MUST** have a corresponding specification and verification method before being considered supported.

- DICOM Part 10 file parsing and dataset decoding within envelope (`docs/04-DICOM-IO-and-Series-Assembly.md`, `docs/03-DICOM-Conformance-Envelope.md`).
- Image series assembly and geometry validation (`docs/04-DICOM-IO-and-Series-Assembly.md`).
- Pixel decoding and deterministic presentation pipeline (`docs/05-Pixel-Pipeline.md`).
- 2D rendering on native (`viewer-wgpu`) and web (`viewer-wasm` CPU/canvas host path) with deterministic CPU-oracle frames (`docs/06-Rendering-and-Interaction.md`, `docs/08-WASM-Target.md`, `docs/31-Implementation-Status.md`).
- Volume geometry validation, baseline `VolumeGrid` assembly, and baseline MPR execution are in scope and implemented; migration to richer patient-space metadata/requests remains staged (`docs/07-Volume-and-Fusion.md`, `docs/31-Implementation-Status.md`).
- DIMSE and DICOMweb transport, service-class handling, and policy enforcement (`docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`).
- Deterministic storage/index/query, worklist, and MPPS workflow services (`docs/03-DICOM-Conformance-Envelope.md`, `docs/04-DICOM-IO-and-Series-Assembly.md`).
- Clinical annotation/reporting and derived-object workflows (GSPS/SEG/SR/RT) within deterministic and fail-closed constraints (`docs/03-DICOM-Conformance-Envelope.md`, `docs/06-Rendering-and-Interaction.md`, `docs/07-Volume-and-Fusion.md`).
- Testing: golden corpus, fuzzing, deterministic regression (`docs/10-Testing-Corpus-Fuzzing-Evals.md`).

### Verified workstation runtime posture (as of 2026-02-24)

- Workstation interaction starts from a script-hosted WASM front-end by default (`tools/run_viewer_wasm_frontend.sh`).
- `viewer-wasm` and `viewer-wgpu` are integration/render crates in this release posture; runtime startup does not rely on guaranteed `bin/viewer-*` artifacts from the workstation tarball unless a dedicated launch strategy is added.
- `docs/33-Productization-Profiles-and-Playbooks.md`, `docs/12-API-Surface-and-Crate-Boundaries.md`, and `docs/40-Reference-Deployment-Topology.md` are the canonical references for current runtime topology alignment.

Verification:

- Documentation lint confirms each in-scope bullet links to a spec and verification section in `docs/`.

## Out of scope (explicit)

- Autonomous diagnosis or therapeutic decision automation.
- Direct treatment control/delivery (e.g., therapy actuation).
- Enterprise billing/claims processing and payer workflows.
- Unbounded “accept-anything” DICOM parsing outside documented envelope controls.

Out-of-scope features may be implemented experimentally only under the constraints in **REQ-SCOPE-014**.

Requirements:

- **REQ-SCOPE-014:** If an out-of-scope feature is implemented experimentally, it **MUST** be gated behind an unstable feature flag and **MUST NOT** expand the default envelope.

Verification:

- Feature lists and crate docs label experimental features as `unstable-*`, and workstation-profile conformance tests pass with all unstable features disabled.


## Intended purpose guardrails and claim surface control (normative)

This repository specifies a **PACS workstation framework** with explicit clinical workflow coverage. It is intentionally engineered to support broad workflow operations while preventing accidental expansion into unsupported automation or unsafe behavior.

Framework Intended Purpose (normative text):

> DiCCY is a Rust PACS workstation framework that ingests, stores, queries, retrieves, renders, annotates, and reports on DICOM studies for clinical workflow integration. Regulatory authorization and deployment claims are controlled by integrators and deployment programs.


Requirements:

- **REQ-SCOPE-001:** The repository **MUST** maintain a single canonical **Framework Intended Purpose** statement describing PACS workstation and clinical workflow scope boundaries.
- **REQ-SCOPE-017:** The **Framework Intended Purpose** statement **MUST** appear verbatim in both `README.md` and `docs/01-Vision-and-Scope.md`. Any change **MUST** update both locations in the same change set.
- **REQ-SCOPE-002:** Public documentation in this repository **MUST NOT** assert or imply regulatory authorization, clearance, or classification status claims, except in `docs/15-Regulatory-and-Standards-Mapping.md` which is explicitly informative.
- **REQ-SCOPE-003:** Clinical workflow functions (storage/index/query, DIMSE, DICOMweb, worklist, MPPS, reporting, and measurements) **MUST** provide explicit provenance and fail-closed behavior when required inputs are missing or invalid.
- **REQ-SCOPE-004:** Workstation-profile builds **MUST** operate in a **workflow-complete** posture:
  - measurements include calibrated outputs when calibration is valid and pixel-domain fallback when invalid;
  - geometry-dependent operations fail closed when required tags are missing or invalid;
  - no implicit network access outside configured service endpoints and policies.

### Controlled claim vocabulary (normative)

Allowed repository claim verbs for baseline scope (non-authorization posture):

- framework,
- ingest / store / query / retrieve,
- render / annotate / report,
- interoperability / workflow integration,
- deterministic / fail-closed / provenance.

Prohibited claim wording outside `docs/15-Regulatory-and-Standards-Mapping.md`:

- direct diagnosis-use assertions,
- primary-clinical-decision assertions,
- market-authorization assertions (US/EU labels),
- conformity-tier assertions that imply approved product status.

Verification:

- CI runs claim-surface lint (`python3 tools/claim_surface_lint.py`) and fails on prohibited phrases outside `docs/15-Regulatory-and-Standards-Mapping.md`.
- UI/API tests confirm that workflow functions expose provenance, fail-closed behavior, and explicit policy controls.
- Documentation review verifies that the **Framework Intended Purpose** statement matches exactly between `README.md` and this section.
- Build-profile review verifies that repository default builds remain minimal-core while workstation-profile builds satisfy REQ-SCOPE-004.

See also:
- `docs/15-Regulatory-and-Standards-Mapping.md` (informative mapping and primary references)
- `docs/16-Requirements-Index.md` (REQ identifier scheme and index)

## Compatibility envelope strategy

Requirements:

- **REQ-SCOPE-016:** The project **MUST** publish:
  - supported SOP Classes and Transfer Syntaxes in tiers,
  - required tags and interpretation rules,
  - non-supported items and failure behavior.

See `docs/03-DICOM-Conformance-Envelope.md`.

Verification:

- Documentation lint confirms `docs/03-DICOM-Conformance-Envelope.md` includes sections for supported SOP Classes, Transfer Syntaxes, required tags, and explicit non-support items.

## Verification requirements

Requirements:

- **REQ-SCOPE-015:** Each scope claim **MUST** be backed by:
  - at least one corpus-based integration test, or
  - a fuzz target for parsers/decoders, or
  - both.

The verification mapping is defined in `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

Verification:

- Traceability report links scope claims to corpus or fuzz coverage in `docs/10` (REQ-SCOPE-015).
