# Implementation status

As of date: **2026-02-24**

This document is the canonical status ledger for capability-claim alignment.

## Onboarding guidance and positioning

- Current scope remains **DICOMweb + workflow service first** with optional DIMSE integration via explicit profile.
- Enterprise HIS/RIS/HL7 orchestration, multi-tenant policy planes, and fully bundled PACS-era monolith behavior are explicit out-of-scope items unless marked otherwise in open backlog items.
- For each onboarding cycle, operators should verify profile claims against:
  - `docs/12-API-Surface-and-Crate-Boundaries.md`
  - `docs/40-Reference-Deployment-Topology.md`
  - `docs/33-Productization-Profiles-and-Playbooks.md`
  - `tools/runtime_env_contract.py --check-docs` outputs in `reports/docs/runtime-env-contract.json`

## Packaging claim alignment

- Backend services currently ship `dicom-web-server` and `dicom-workflow-server` as the default packaged artifacts for service operations.
- `dicom-dimse-service` is implemented as a runnable service binary (`dicom-dimse-service`) and is available via explicit profile enabling; it is not included by default in backend packaging.
- `docs/12-API-Surface-and-Crate-Boundaries.md` and `docs/40-Reference-Deployment-Topology.md` are the canonical sources for runtime artifact claims.
- Workstation visualization crates (`viewer-wasm`, `viewer-wgpu`) are currently library/runtime integration crates and are not documented here as default shipped binaries.
- `dicom-visualizer` remains the documented packaged binary for `framework-core`; viewer front-end launch is currently script-hosted (`tools/run_viewer_wasm_frontend.sh`).
- 2026-02-23: `DOC-CODE-011` follow-up corrected DIMSE posture docs from "standalone/optional planned" phrasing to explicit profile-gated runnable binary status in `docs/12`, `docs/33`, and `docs/40`.
- Runtime parser source-of-truth references for onboarding and env-contract checks:
  - `crates/dicom-web-server/src/main.rs`
  - `crates/dicom-workflow-server/src/main.rs`
  - `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`

- Environment contract checks are now part of implementation sign-off for backend profiles:
  - `python3 tools/runtime_env_contract.py --repo-root . --check-docs --report reports/docs/runtime-env-contract.json`
  - `python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json`
  - `python3 tools/docs_env_review_gate.py --repo-root . --report reports/docs/docs-env-review-gate.json`

## Major subsystems

| Subsystem | Status | Notes |
|---|---|---|
| DICOM IO parse + envelope validation | Implemented | Fail-closed parse path and envelope checks are active. |
| Pixel decode + CPU oracle pipeline | Implemented | Deterministic CPU boundary output is the correctness source. |
| Native GPU renderer (`viewer-wgpu`) | Implemented (baseline) | Lifecycle validation, deterministic budget accounting, and offscreen acquire/draw/present command encoding are active; advanced pooling/recovery hardening remains staged. |
| WASM host rendering (`viewer-wasm`) | Implemented (CPU/canvas default + gated WebGPU) | CPU/canvas remains fail-closed default; gated WebGPU presentation path is available with runtime flag + capability checks. |
| WASM WebGPU/WebGL draw path | Implemented (WebGPU gated baseline) | Browser WebGPU draw path is wired for production-gated use; WebGL fallback remains deferred. |
| VolumeGrid assembly + MPR reslice | Implemented (patient-space-capable baseline) | Deterministic baseline `VolumeGrid` + CPU reslice APIs are available with optional patient-space metadata, voxel/patient mapping helpers, synchronized tri-planar controls, and slab composition controls. |
| PET/CT fusion pipeline | Implemented (baseline) | Deterministic PET-on-CT resample + blend pipeline, fail-closed fusion preconditions, deterministic output hash tests, and perf budget harness are available. |
| SR extraction (`NUM`/`TEXT`/`CODE`) | Implemented | Read-only deterministic extraction is active. |
| SR creation/update write APIs | Implemented (API + service baseline) | Deterministic builder/update APIs with version/reference checks are available, plus workflow-service create/update/retrieve commit routes with fail-closed auth and idempotency/version-conflict handling. |
| Deterministic dataset/pixel cache layers | Implemented (baseline utility) | Shared deterministic cache utility with stable LRU + pinning is available for integration. |
| GPU texture budget enforcement | Implemented | Budget reservation/release path is active in renderer lifecycle checks. |

## Native vs WASM compatibility matrix

| Capability | Native | WASM | Status date |
|---|---|---|---|
| CPU pixel pipeline output (`Luma8`/`Rgba8`) | Yes | Yes | 2026-02-21 |
| GPU renderer draw/present pipeline | Yes (baseline offscreen path in `viewer-wgpu`) | Yes (gated WebGPU in `viewer-wasm`) | 2026-02-21 |
| CPU/canvas interactive rendering host | N/A | Yes (`viewer-wasm`) | 2026-02-21 |
| MPR baseline runtime APIs | Yes | Yes (CPU-only binding) | 2026-02-22 |
| SR extraction APIs | Yes | Yes (via core crates) | 2026-02-21 |
| SR creation/update APIs | Yes (API + workflow service baseline) | Yes (core bindings + web prototype client) | 2026-02-22 |

## Symbol status map

This table is used by `tools/docs_symbol_lint.py`.

| Symbol | Status | Expected location |
|---|---|---|
| `WasmViewer` | Implemented | `crates/viewer-wasm/src/lib.rs` |
| `WgpuRenderer` | Implemented | `crates/viewer-wgpu/src/lib.rs` |
| `PixelPipeline` | Implemented | `crates/dicom-pixel/src/lib.rs` |
| `VolumeGrid` | Implemented | `crates/viewer-core/src/volume.rs` |
| `PixelCodec` | Deferred | (planned) `crates/dicom-pixel` public codec trait |

## Capability assertion policy

- Every major capability assertion in `README.md`, `docs/01-Vision-and-Scope.md`, `docs/06-Rendering-and-Interaction.md`, and `docs/08-WASM-Target.md` must include an explicit status state (`Implemented`, `Partially implemented`, `Deferred`, or `Planned`) and an explicit date stamp.
- When status changes, update this document and linked claims in the same change set.

## Advanced Visualization Matrix (As of 2026-02-22)

| Feature | Status | Evidence |
|---|---|---|
| Tri-planar linked controls | Implemented | `crates/viewer-wasm/web/index.html`, `crates/viewer-wasm/tests/hi_wasm_boundary_contract.rs` |
| Slab controls | Implemented | `crates/viewer-wasm/src/lib.rs`, `crates/viewer-core/src/mpr.rs` |
| Patient-space request path | Implemented baseline | `WasmViewer::mpr_reslice_patient_json(...)` and patient-space tests |
| Fusion preconditions and execution | Implemented baseline | `crates/modality-pet/src/lib.rs` + fusion harness |
| Orientation overlays and ROI quant UX panels | Implemented prototype | `crates/viewer-wasm/web/index.html`, `crates/viewer-wasm/web/app.js` |
