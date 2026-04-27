# Testing, corpus, fuzzing, evals

## Purpose

Normative: This testing/corpus/fuzzing model defines the verification bar (see **REQ-TEST-704** through **REQ-TEST-760**).

This document defines the quality gates that enforce:

- correctness within the conformance envelope,
- deterministic pixel outputs,
- security robustness against malformed inputs,
- reproducible evaluation of agent-driven changes.

---

## 1. Test taxonomy

### Unit tests (MUST)

Requirements:
- **REQ-TEST-704:** Unit tests **MUST** run without external datasets and **MUST** be sufficient to validate core math and parsing primitives.

Verification:
- CI **MUST** run unit test suites without external dataset access.

### Integration tests (MUST)

Requirements:
- **REQ-TEST-705:** Integration tests **MUST** validate end-to-end behavior for Tier 0 and **MUST** pin deterministic outputs via hashes.

Verification:
- Integration test suites **MUST** include at least one Tier 0 end-to-end path with a stable hash.

### Feature-gated workflow runtime tests (policy)

Requirements:
- **REQ-TEST-761:** The `workflow-main-tests` feature-gated suite **MUST** remain runnable via `cargo test -p dicom-workflow-server --features workflow-main-tests` and **MUST** stay deterministic (no network or external dataset dependency).
- **REQ-TEST-762:** The repository **MUST** treat `workflow-main-tests` as a non-default suite: it is **NOT** required in the default pull-request CI lane today, and release evidence **MUST** either include a dedicated CI job run or a maintainer-executed local run record with command output.

Verification:
- CI/release checklists **MUST** explicitly state whether `workflow-main-tests` was exercised by job or by maintainer run.
- When executed, `workflow-main-tests` **MUST** pass with no skipped cases caused by missing external resources.

### Golden corpus tests (MUST)

Requirements:
- **REQ-TEST-706:** For each corpus sample, tests **MUST** decode and run the pixel pipeline with a fixed config, hash the output bytes, and compare to the expected hash.
- **REQ-TEST-707:** The hash algorithm **MUST** be stable and fast (e.g., BLAKE3 or SHA-256), and the hash input **MUST** be the CPU-boundary output (e.g., `Luma8` bytes), not a GPU screenshot.

Verification:
- Golden corpus tests **MUST** compare expected hashes for CPU-boundary outputs.

---

## 2. Golden corpus strategy

### Corpus sources

Requirements:
- **REQ-TEST-710:** A synthetic corpus **MUST** be generated in-tree by deterministic generators and **MUST** cover edge cases such as:
  - unusual bit depths,
  - MONOCHROME1 inversion,
  - modality calibration via Rescale Slope/Intercept,
  - Pixel Padding Value,
  - YBR color conversion constraints (`YBR_FULL`, `YBR_FULL_422`),
  - RLE edge conditions,
  - broken/hostile VR/VL sequences.
- **REQ-TEST-711:** External de-identified public corpus data **SHOULD** not be stored in the repository and **SHOULD** be referenced by hash manifests and user-managed download instructions.

Note: The `codec-j2k` pack uses a small, non-PHI JPEG 2000 sample in `corpus/j2k_file1.jp2` to keep codec regression tests offline; its hash is pinned in the corpus manifest.

Verification:
- Corpus generator tests **MUST** be deterministic and regenerate identical outputs.
- Native repeat-run determinism tests **MUST** execute the same fixture set multiple times and assert stable hashes for each fixture.

### Clinical pack coverage (optional)

Requirements:
- **REQ-TEST-742:** When a clinical completeness pack is enabled, the corpus **MUST** include at least one sample per supported SOP Class and at least one negative (out-of-envelope) sample that asserts fail-closed behavior. For derived-object packs without pixel output (e.g., GSPS, SR, RT Plan/Structure), the sample **MAY** be an in-tree synthetic fixture validated by deterministic unit tests.
- **REQ-TEST-743:** Presentation state packs **MUST** include golden render tests that cover GSPS shutters, graphic annotations, and overlay ordering determinism.
- **REQ-TEST-744:** Segmentation packs **MUST** include alignment tests against referenced source images and deterministic color/alpha outputs.
- **REQ-TEST-745:** RT packs **MUST** include dose grid scaling/alignment tests plus RT Structure Set/Plan reference tests, with expected output hashes for fused overlays where applicable.
- **REQ-TEST-746:** SR packs **MUST** include deterministic parsing tests and safe text rendering tests (HTML-escaped where applicable).

Verification:
- Pack-specific test suites **MUST** assert deterministic outputs and fail-closed behavior for the enabled pack features.

### Policy: no downloading in CI (normative)

Requirements:
- **REQ-TEST-712:** CI **MUST NOT** download external datasets by default.
- **REQ-TEST-713:** If a project enables dataset downloads:
  - it **MUST** be explicitly configured and opt-in,
  - downloads **MUST** be pinned by hashes,
  - licensing terms **MUST** be documented.

Verification:
- CI configuration **MUST** show dataset downloads disabled by default.
- Any opt-in download job **MUST** include hash manifest validation and licensing references.

---

## 3. Corpus manifest format

The project defines a manifest format (JSON or TOML) that includes:

- `id` (stable identifier)
- `sha256` (or stronger) of the file bytes
- `sop_class_uid`
- `transfer_syntax_uid`
- `expected_outputs`:
  - one or more output hashes keyed by:
    - pipeline config variant,
    - frame index (for multi-frame),
    - output format.

Normative requirements:
- **REQ-TEST-720:** The manifest **MUST** include the fields listed above with stable identifiers.
- **REQ-TEST-721:** The manifest **MUST** be the only reference used by tests to validate external samples.
- **REQ-TEST-722:** Tests **MUST** refuse to run against a file whose hash does not match the manifest.

Verification:
- Manifest parsing tests **MUST** fail on missing required fields or hash mismatches.
- Release pipelines **MUST** generate a release-scoped determinism manifest in `reports/analytical/` from `corpus/manifest.toml` using a deterministic generator (`tools/generate_determinism_manifest.py`) and verify it is reproducible.

---

## 4. Fuzzing plan

### Targets (MUST)

Requirements:
- **REQ-TEST-730:** Each fuzz target **MUST** be reproducible locally and **MUST** treat panics as failures.
- **REQ-TEST-731:** At minimum, fuzz targets **MUST** cover:
  - `dicom-core` dataset decoding (VR/VL, sequences, length handling),
  - `dicom-io` P10 parsing (meta header + transfer syntax switching), if applicable,
  - `dicom-net` UL PDU parsing, association item decoding, and association state machine sequencing when networking is enabled,
  - `dicom-dimse` command set parsing for supported DIMSE services when networking is enabled,
  - `dicom-web` DICOMweb request parsing boundary when DICOMweb is enabled,
  - `dicom-pixel`:
    - RLE decoder boundary,
    - JPEG baseline decoder boundary (if enabled),
    - pixel pipeline stage composition with random metadata.
- **REQ-TEST-747:** When a clinical completeness pack is enabled, a fuzz target **MUST** cover its primary parser boundary (e.g., SEG, SR, RT) and treat panics as failures.

Fuzz target inventory is maintained in `fuzz/INVENTORY.md` and **MUST** list each target and its crate boundary.

Current DIMSE targets (informative):
- `dimse_pdu` exercises UL PDU parsing and association state sequencing (`crates/dicom-net`).
- `dimse_command` exercises DIMSE command set parsing (`crates/dicom-dimse`).

Current storage/index targets (informative):
- `storage_index_metadata` exercises metadata extraction (`crates/dicom-index`) using datasets parsed via `crates/dicom-io`.
- `crates/dicom-storage` unit coverage includes concurrent ingest + WAL replay consistency checks.

Current query coverage (informative):
- `crates/dicom-query` unit tests cover supported UID and text-key filters (Patient ID, Modality, Accession Number, Study Date) with deterministic result ordering.
- `crates/dicom-dimse-service` all-features tests cover DIMSE identifier-to-query mapping for supported key sets and fail-closed unsupported key handling.

Current worklist/MPPS coverage (informative):
- Worklist and MPPS behaviors are validated via unit tests in `crates/dicom-worklist` and `crates/dicom-mpps` that cover persisted workflow state (`REQ-WL-303`, `REQ-MPPS-354`) in addition to baseline validation/transition requirements (`REQ-WL-*`, `REQ-MPPS-*`).

Current DICOMweb runtime coverage (informative):
- `crates/dicom-web` tests include QIDO/WADO/STOW parsing and service-runtime execution, including `/series`, `/instances`, and `/studies/{StudyUID}/instances` QIDO routing, WADO study/series multipart retrieval coverage, plus auth allow/deny audit assertions (`REQ-WEB-300`, `REQ-WEB-304`, `REQ-WEB-305`).

Current viewer runtime coverage (informative):
- `crates/viewer-core` unit/integration tests cover deterministic viewport transforms, explicit tool-mode transitions, calibrated distance outputs with provenance (`0028,0030`), pixel fallback when calibration is missing/invalid, two-stage angle capture in degrees, and fail-closed zero-length angle behavior (`REQ-UI-003`, `REQ-UI-010`, `REQ-UI-011`, `REQ-MEAS-010`, `REQ-MEAS-020`, `REQ-MEAS-030`, `REQ-MEAS-050`, `REQ-MEAS-060`, `REQ-MEAS-061`, `REQ-MEAS-070`).
- `crates/viewer-wgpu` unit/integration tests cover lifecycle gating (`configure_surface` required before `render`), deterministic resize generation, frame/pass counter progression, CPU-oracle render input validation, and GPU budget enforcement (`REQ-ARCH-140`, `REQ-UI-040`, `REQ-UI-042`, `REQ-GPU-210`).

Current derived-object pack coverage (informative):
- `crates/pack-enhanced` tests cover fail-closed enhanced functional-group parsing for `Plane Position`, `Plane Orientation`, `Pixel Measures`, and `Pixel Value Transformation`, including per-frame item count consistency (`Per-frame Functional Groups Sequence` count must match `NumberOfFrames`), positive-value validation for `Pixel Spacing` and `Slice Thickness`, and `Frame Content` (`In-Stack Position Number`) validation (`REQ-CONF-084`, `REQ-ENH-350`).
- `crates/pack-seg` tests cover deterministic binary SEG overlay behavior for single-frame and multi-frame instances, referenced UID/frame-of-reference mismatch fail-closed handling, frame-index bounds enforcement, and no-resampling grid-mismatch failures (`REQ-CONF-086`, `REQ-SEG-301`, `REQ-SEG-302`, `REQ-SEG-303`).
- `crates/pack-rt` tests cover deterministic RT Dose scaling/alignment checks and RT Structure Set contour handling for planar contour types (`OPEN_PLANAR`, `CLOSED_PLANAR`, `CLOSEDPLANAR_XOR`) with fail-closed geometry mismatch behavior (`REQ-CONF-087`, `REQ-VOL-925`, `REQ-VOL-926`, `REQ-VOL-929`, `REQ-RT-350`, `REQ-RT-351`, `REQ-RT-353`).
- `crates/pack-sr` tests cover deterministic SR extraction for nested `NUM` measurements plus `TEXT` and `CODE` observations with provenance capture and fail-closed invalid text-value handling (`REQ-CONF-088`, `REQ-SR-300`, `REQ-MEAS-081`, `REQ-MEAS-082`).
- `crates/pack-gsps` tests cover deterministic GSPS shutter and graphics behavior for rectangular/circular/polygonal shutters and baseline graphic object types (`POINT`, `POLYLINE`, `INTERPOLATED`, `CIRCLE`, `ELLIPSE`), including fail-closed unsupported-type and invalid-topology handling (`REQ-CONF-089`, `REQ-CONF-091`, `REQ-UI-060`, `REQ-UI-061`, `REQ-UI-064`, `REQ-UI-065`, `REQ-GSPS-300`, `REQ-GSPS-301`, `REQ-GSPS-303`, `REQ-GSPS-304`).

Verification:
- Fuzz target inventory **MUST** list each target and its crate boundary.

### Limits in fuzzing (normative)

Requirements:
- **REQ-TEST-732:** Fuzz harnesses **MUST** set conservative limits lower than production defaults to maximize throughput while preserving bug discovery.
- **REQ-TEST-733:** Any out-of-limit input **MUST** return a structured error, not panic.

Verification:
- Fuzz harness configs **MUST** include explicit limit values and error assertions.

### Crash triage and regression (normative)

Requirements:
- **REQ-TEST-734:** Any crash (panic, OOM, UB in FFI) **MUST** be:
  1) minimized,
  2) classified (parser/codec/pipeline),
  3) added as a regression test or corpus seed.
- **REQ-TEST-735:** Regression artifacts **MUST NOT** contain PHI/PII.

Verification:
- Regression corpus review **MUST** confirm minimization metadata and PHI/PII exclusion.

---

## 5. Determinism evaluation

### Determinism contract

Requirements:
- **REQ-TEST-740:** For Tier 0 inputs, pixel pipeline output **MUST** be byte-for-byte identical for:
  - repeated runs on the same platform (multi-run fixed-input checks),
  - different native platforms (where feasible),
  - native vs wasm (preferred).

Verification:
- Golden corpus tests **MUST** compare output hashes across supported targets.
- CI reproducibility matrices **MUST** include native x86_64, native arm64, and wasm targets and publish per-target hash manifests for release evidence.
- CI determinism gates **MUST** include native golden-corpus tests, repeated-run hash checks, and deterministic release-manifest verification.

### How to test

Requirements:
- **REQ-TEST-741:** If cross-platform differences occur, they **MUST** be investigated and either:
  - fixed, or
  - explicitly documented as a permissible backend variance with a dedicated test that demonstrates the variance is bounded and understood.

Verification:
- Any permitted variance **MUST** have a dedicated test and a documented rationale.
- CPU-boundary hash mismatch budget is `0`; any non-zero mismatch count across target manifests **MUST** fail the reproducibility gate.

---

## 6. Requirements traceability

DiCCY treats traceability as an engineering control: it makes behavioral guarantees auditable and helps prevent silent envelope creep.

Requirements:

- **REQ-TEST-701:** New normative requirements added to `docs/` **MUST** include a REQ identifier per `docs/16-Requirements-Index.md`.
- **REQ-TEST-702:** Each unit/integration/corpus test that verifies normative behavior **MUST** reference at least one REQ identifier (e.g., in a Rust docstring, test name, or a structured annotation comment `// REQ-...`).
- **REQ-TEST-703:** CI **MUST** generate a traceability report mapping REQ identifiers to tests and **MUST** fail when any MUST-level REQ has zero test references.

Verification:

- The traceability report artifact **MUST** be produced by CI for every merge commit and for every release tag.
- Reviewers **MUST** treat missing REQ references for new behavior as a correctness failure.

---

## 7. Agent skill evals

Codex agents are evaluated on reproducible criteria to avoid \"looks good\" merges.

### Evaluation dimensions (normative)

Requirements:
- **REQ-TEST-750:** Each agent-driven change **MUST** be scored on:
  1. **Spec conformance** (docs updated when behavior changes; no contradictions),
  2. **Verification** (safe command set executed; relevant tests added/updated),
  3. **Scope discipline** (no unrelated edits),
  4. **Security discipline** (limits preserved or tightened; threat model updated if needed).
- **REQ-TEST-751:** The PR template **MUST** include a checklist reflecting these dimensions.

Verification:
- PR template review **MUST** confirm the checklist is present and aligned to this section.

### Reproducible scoring

- The repository **SHOULD** implement an eval script (optional) that computes a score from:
  - test pass/fail,
  - clippy pass/fail,
  - diff scope heuristics (file allowlist),
  - presence of required doc updates when certain modules are touched.

---

## 8. Verification checklist (for reviewers)

Normative: Review checklists must cover these items (see **REQ-TEST-760**) for changes affecting conformance, pixels, security, or APIs.


Verification:
- **REQ-TEST-760:** Review checklists **MUST** include the items above or an equivalent set.

---

## 9. Human interface concrete mapping baseline (REQ-HI-100..439)

This section defines concrete REQ-to-test mapping targets for `REQ-HI-*` implementation work tracked in `HI_REQ.md`.

Execution model:
- Implement and verify chunks in order: Viewer -> Workflow -> Security -> Interoperability/Runtime -> Usability.
- Each chunk must keep fail-closed and deterministic invariants intact.
- Each mapped test must include `REQ-HI-*` references in test names, doc comments, or structured annotations.
- Range anchors for this baseline include `REQ-HI-100` and `REQ-HI-439`.

### 9.1 Chunk-to-suite mapping

| Chunk ID | REQ-HI range | Concrete implementation scope | Concrete test mapping (required) | Gate command(s) |
|---|---|---|---|---|
| HI-CHUNK-01 (Viewer) | `REQ-HI-145..164`, `REQ-HI-180..194` (viewer/error subset), `REQ-HI-244` | Viewer interaction determinism, overlays, measurement gating, deterministic warning/error rendering | Existing: `crates/viewer-core/tests/measurement_integration.rs`, `crates/viewer-wgpu/tests/lifecycle_integration.rs`, `crates/dicom-pixel/tests/golden_corpus.rs`; Add: `crates/viewer-core/tests/hi_viewer_determinism.rs`, `crates/viewer-core/tests/hi_overlay_ordering.rs`, `crates/viewer-core/tests/hi_error_rendering.rs`, `crates/viewer-core/tests/hi_viewer_controls.rs`, `crates/viewer-core/tests/hi_error_safety_controls.rs`, `crates/viewer-core/tests/hi_comparison_layout_controls.rs` | `cargo test -p viewer-core -p viewer-wgpu -p dicom-pixel` |
| HI-CHUNK-02 (Workflow) | `REQ-HI-130..144`, `REQ-HI-165..179`, `REQ-HI-246`, `REQ-HI-248..249` | Clinical context integrity, workflow-state transitions, report/export provenance, tuple safety checks | Existing: `crates/dicom-web/src/lib.rs` tests, `crates/dicom-web-server/src/main.rs` tests, `crates/dicom-workflow-server/src/main.rs` tests, `crates/dicom-worklist/src/lib.rs` tests, `crates/dicom-mpps/src/lib.rs` tests; Add: `crates/dicom-workflow-server/tests/hi_workflow_state.rs`, `crates/dicom-web/tests/hi_context_tuple_integrity.rs`, `crates/dicom-web/tests/hi_context_safety_controls.rs`, `crates/dicom-visualizer/tests/hi_export_controls.rs` | `cargo test -p dicom-web -p dicom-web-server -p dicom-workflow-server -p dicom-worklist -p dicom-mpps -p dicom-visualizer` |
| HI-CHUNK-03 (Security) | `REQ-HI-100..129`, `REQ-HI-195..209`, `REQ-HI-245`, `REQ-HI-247`, `REQ-HI-252` | Identity/session UX controls, policy visibility, redaction-safe support output, transport/auth indicators and blocking behavior | Existing: `crates/dicom-auth/src/lib.rs` tests, `crates/dicom-audit/src/lib.rs` tests, `crates/viewer-wasm/src/lib.rs` tests, `tools/tests/test_claim_surface_lint.py`; Add: `crates/dicom-auth/tests/hi_governance_identity_controls.rs`, `crates/dicom-web-server/tests/hi_auth_session_controls.rs`, `crates/dicom-workflow-server/tests/hi_privacy_modes.rs`, `tools/tests/test_hi_req_traceability.py` | `cargo test -p dicom-auth -p dicom-audit -p dicom-web-server -p dicom-workflow-server -p viewer-wasm && python3 tools/claim_surface_lint.py` |
| HI-CHUNK-04 (Interoperability/Runtime) | `REQ-HI-255..439` | Service capability exposure, ingest/retrieve constraints, runtime preflight/recovery UX, protocol-level route/content/auth semantics, deterministic operational ordering, batch manifest/export controls, WASM/GPU boundary contracts, strict runtime-config parsing/serialization behavior, advanced pack semantics, and persisted MWL/MPPS workflow-state controls | Existing: `crates/dicom-web/src/lib.rs` tests, `crates/dicom-web-server/src/main.rs` tests, `crates/dicom-dimse-service/src/lib.rs` tests, `crates/dicom-storage/src/lib.rs` tests, `crates/dicom-workflow-server/src/main.rs` tests, `crates/dicom-net/src/lib.rs` tests, `crates/dicom-query/src/lib.rs` tests, `crates/dicom-visualizer/src/main.rs` tests, `crates/diccy/src/lib.rs` tests, `crates/viewer-wasm/src/lib.rs` tests, `crates/viewer-wgpu/src/lib.rs` tests, `crates/pack-enhanced/src/lib.rs` tests, `crates/pack-gsps/src/lib.rs` tests, `crates/pack-seg/src/lib.rs` tests, `crates/pack-rt/src/lib.rs` tests, `crates/pack-sr/src/lib.rs` tests, `crates/dicom-worklist/src/lib.rs` tests, `crates/dicom-mpps/src/lib.rs` tests; Add: `crates/dicom-web/tests/hi_interop_runtime_controls.rs`, `crates/dicom-web/tests/hi_http_response_contract.rs`, `crates/dicom-web-server/tests/hi_service_preflight_resilience.rs`, `crates/dicom-web-server/tests/hi_http_status_contract.rs`, `crates/dicom-workflow-server/tests/hi_workflow_runtime_contract.rs`, `crates/dicom-dimse-service/tests/hi_dimse_protocol_contract.rs`, `crates/dicom-net/tests/hi_association_contract.rs`, `crates/dicom-query/tests/hi_query_contract_controls.rs`, `crates/dicom-storage/tests/hi_storage_runtime_integrity.rs`, `crates/dicom-visualizer/tests/hi_batch_manifest_contract.rs`, `crates/diccy/tests/hi_runtime_config_contract.rs`, `crates/viewer-wasm/tests/hi_wasm_boundary_contract.rs`, `crates/viewer-wgpu/tests/hi_gpu_runtime_contract.rs`, `crates/pack-enhanced/tests/hi_enhanced_geometry_contract.rs`, `crates/pack-gsps/tests/hi_gsps_contract.rs`, `crates/pack-seg/tests/hi_seg_overlay_contract.rs`, `crates/pack-rt/tests/hi_rt_overlay_contract.rs`, `crates/pack-sr/tests/hi_sr_extraction_contract.rs`, `crates/dicom-worklist/tests/hi_worklist_persistence_contract.rs`, `crates/dicom-mpps/tests/hi_mpps_persistence_contract.rs`, `tools/tests/test_hi_interop_runtime_register.py` | `cargo test -p dicom-web -p dicom-web-server -p dicom-dimse-service -p dicom-storage -p dicom-workflow-server -p dicom-net -p dicom-query -p dicom-visualizer -p viewer-wasm -p viewer-wgpu -p pack-enhanced -p pack-gsps -p pack-seg -p pack-rt -p pack-sr -p dicom-worklist -p dicom-mpps -p diccy` |
| HI-CHUNK-05 (Usability) | `REQ-HI-210..239`, `REQ-HI-240..254` (usability/lifecycle subset), `REQ-HI-251` | Critical-task usability controls, accessibility/localization checks, release readiness and PMCF/CAPA linkage | Existing evidence gates: `reports/clinical/usability-summative-bundle-*.md`, `reports/clinical/CLI-030.md`; Add automation/tests: `tools/tests/test_hi_accessibility_checklist.py`, `tools/tests/test_hi_localization_controls.py`, `tools/tests/test_hi_release_gate_matrix.py`, `tools/tests/test_hi_critical_task_register.py`, `tools/tests/test_hi_accessibility_interaction_layout.py`, `tools/tests/test_hi_negative_path_matrix.py`, `tools/tests/test_hi_change_control_register.py` | `cargo test && python3 tools/traceability_report.py && python3 tools/req_completeness_lint.py` |

### 9.2 Traceability anchors per chunk

- HI-CHUNK-01 must anchor to `REQ-UI-*`, `REQ-MEAS-*`, `REQ-PIX-*`, and `REQ-HI-145..194`.
- HI-CHUNK-02 must anchor to `REQ-WEB-*`, `REQ-WL-*`, `REQ-MPPS-*`, `REQ-STOR-*`, and `REQ-HI-130..179`.
- HI-CHUNK-03 must anchor to `REQ-AUTH-*`, `REQ-AUDIT-*`, `REQ-TEL-*`, `REQ-SEC-*`, and `REQ-HI-100..129`, `REQ-HI-195..209`.
- HI-CHUNK-04 must anchor to `REQ-WEB-*`, `REQ-DIMSE-*`, `REQ-NET-*`, `REQ-HTTP-*`, `REQ-QR-*`, `REQ-STOR-*`, `REQ-WASM-*`, `REQ-GPU-*`, `REQ-SOP-*`, `REQ-ENH-*`, `REQ-GSPS-*`, `REQ-SEG-*`, `REQ-RT-*`, `REQ-SR-*`, `REQ-WL-*`, `REQ-MPPS-*`, and `REQ-HI-255..439`.
- HI-CHUNK-05 must anchor to `REQ-TEST-*` plus `REQ-HI-210..254` through explicit clinical/usability and release-gate artifacts.

### 9.3 Completion criteria for REQ-HI mapping activation

- Every `REQ-HI-*` requirement must map to at least one concrete test artifact path.
- Every chunk must include at least one negative-path fail-closed test set.
- `python3 tools/traceability_report.py` output must include `REQ-HI-*` references before chunk closure is marked complete.
