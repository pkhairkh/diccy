# Documentation errata (RC-2026-02-21)

Date: **2026-02-21**
Source audit: `COMPARE.md`

## Summary

This errata resolves high-visibility documentation drift between claims and implemented behavior.

## Corrections applied

| Audit ID | Previous claim | Corrected statement |
|---|---|---|
| DOC-CODE-001 | Web rendering described as active `wgpu`/WebGPU (+ WebGL fallback). | Current WASM host rendering is CPU/canvas; WebGPU/WebGL browser draw path is deferred. |
| DOC-CODE-002 | `docs/06` implied fully implemented `wgpu` rendering runtime. | `docs/06` now splits implemented-now vs deferred target architecture. |
| DOC-CODE-003 | In-scope wording implied current MPR/`VolumeGrid` implementation. | Scope now states volume/MPR execution is deferred; geometry prerequisites are implemented. |
| DOC-CODE-004 | `REQ-CONF-088` implied SR create/update is currently supported. | `REQ-CONF-088` now reflects read-only SR extraction; SR write APIs explicitly deferred. |
| DOC-CODE-005 | Transfer syntax support semantics were ambiguous. | `docs/03` now separates I/O UID recognition from pixel decode support by layer. |
| DOC-CODE-009 | `docs/08` asserted a `PixelCodec` trait as current API. | `docs/08` now documents current function-level codec boundary and deferred trait plan. |
| DOC-CODE-010 | Cache policy wording implied fully implemented deterministic cache layers. | `docs/11` now marks cache layers as implemented/deferred/partial by subsystem. |

## Follow-up tracking

Open implementation work remains in the codebase and release evidence set (renderer completion, MPR, SR write APIs, IO warning channels, deterministic shared caches).

## Follow-up corrections (2026-02-21 refresh)

This follow-up aligns the errata with the latest code-backed audit updates.

| Audit ID | Follow-up correction |
|---|---|
| DOC-CODE-001 | Updated `docs/01` in-scope wording to mark baseline `VolumeGrid`/MPR as implemented, with patient-space migration staged. |
| DOC-CODE-002 | Updated `docs/01` mission wording to match native GPU + web CPU/canvas baseline with deferred production web draw path. |
| DOC-CODE-003 | Updated `docs/06` to mark WebGPU->CPU fallback orchestration as implemented baseline and added runtime behavior matrix. |
| DOC-CODE-004 | Updated `docs/07` `VolumeGrid` model section to match actual current struct fields and explicitly mark missing patient-space metadata as not yet implemented. |
| DOC-CODE-005 | Updated `docs/07` MPR section to reflect current voxel-space baseline semantics and added patient-space migration roadmap section. |
| DOC-CODE-006 | Updated `docs/07` fusion section to separate current implemented scope (`validate_fusion` + SUV helpers) from deferred target fusion pipeline. |
| DOC-CODE-007 | Updated `docs/11` to remove deferred MPR wording and added cache implementation matrix (`implemented`/`partial`/`deferred`). |
| DOC-CODE-008 | Updated `README.md` Quickstart outputs to include `manifest.integrity.json` and explain its deterministic integrity role. |

## Follow-up corrections (2026-02-22 API/deployment sync)

This follow-up resolves `COMPARE.md` findings `DOC-CODE-001` through `DOC-CODE-006`.

| Audit ID | Follow-up correction |
|---|---|
| DOC-CODE-001 | Updated `docs/12-API-Surface-and-Crate-Boundaries.md` to define `dicom-workflow-server` as MWL/MPPS/SR runtime with `SrWorkflowStore` persistence responsibilities. |
| DOC-CODE-002 | Added full SR route contract to `docs/12-API-Surface-and-Crate-Boundaries.md` (`/sr/documents`, `/sr/documents/{sop_instance_uid}`, `/sr/documents/{sop_instance_uid}/updates`) including `GET|HEAD|POST` coverage. |
| DOC-CODE-003 | Added missing SR persistence env vars in `docs/12-API-Surface-and-Crate-Boundaries.md` (`DICOM_WORKFLOW_SR_STATE_PATH`, `DICOM_WORKFLOW_SR_AUDIT_PATH`). |
| DOC-CODE-004 | Added `dicom-web-server` runtime env var coverage for `DICOM_WEB_WORKERS` and `DICOM_WEB_ACCEPT_QUEUE_DEPTH` plus deterministic queue backpressure behavior notes. |
| DOC-CODE-005 | Updated `dicom-visualizer` output contract in `docs/12-API-Surface-and-Crate-Boundaries.md` to include mandatory `manifest.integrity.json` and sidecar field semantics. |
| DOC-CODE-006 | Corrected `docs/40-Reference-Deployment-Topology.md` default `viewer-wasm` port to `127.0.0.1:4173` and added explicit `--port` override example using `./tools/run_viewer_wasm_frontend.sh`. |
| DOC-CODE-007 | Added canonical runtime deployment glossary for storage/query, workflow, and DIMSE boundaries in `docs/40-Reference-Deployment-Topology.md`. |
| DOC-CODE-008 | Added runtime env-contract table and optional DIMSE-service binary variable contract to `docs/12-API-Surface-and-Crate-Boundaries.md`. |
| DOC-CODE-009 | Added claim-parity release gate language for runtime contract checks in `docs/14-Release-and-Versioning.md`. |
| DOC-CODE-010 | Added script/entrypoint and profile source-of-truth references to `docs/33-Productization-Profiles-and-Playbooks.md`. |
| DOC-CODE-011 | Corrected `README.md`, `docs/12-API-Surface-and-Crate-Boundaries.md`, and `docs/40-Reference-Deployment-Topology.md` to reflect `dicom-dimse-service` as a runnable binary with explicit profile-driven inclusion. |

## DOC-CODE-2026 remediation ownership/status

| Audit ID | Status | Owner | Last updated |
|---|---|---|---|
| DOC-CODE-001 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-002 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-003 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-004 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-005 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-006 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-007 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-008 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-009 | Done | Documentation working group | 2026-02-21 |
| DOC-CODE-010 | Done | Documentation working group | 2026-02-22 |
| DOC-CODE-011 | Done | Documentation working group | 2026-02-23 |
