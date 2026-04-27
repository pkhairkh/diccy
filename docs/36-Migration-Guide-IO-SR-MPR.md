# Migration Guide: IO / SR / MPR API Updates

Last updated: 2026-02-22

## Overview

Wave 1 introduced additive APIs in three areas:

- `dicom-io`: raw-mode options, structured parser warnings, debug offset metadata.
- `pack-sr`: deterministic SR authoring builder and update workflow.
- `viewer-core`/`diccy`/`viewer-wasm`: baseline MPR request/response surfaces.

Wave 2 extends those surfaces with:
- patient-space-capable `VolumeGrid` metadata and migration adapters,
- patient-space MPR request APIs + slab controls,
- baseline PET/CT fusion execution pipeline,
- SR workflow service create/update/retrieve endpoints.

## IO migration

- Existing `P10Reader::new` and `P10Reader::with_limits` behavior is unchanged.
- New API:
- `P10Reader::with_limits_and_options(...)`
- `P10Reader::read_dataset_with_diagnostics()`
- `P10Reader::warnings()`

## SR migration

- Extraction APIs remain unchanged.
- New authoring/update APIs:
- `SrAuthoringBuilder`
- `SrAuthoredDocument`
- `SrUpdateRequest`
- `apply_sr_update(...)`
- New workflow service APIs:
  - `POST /sr/documents`
  - `POST /sr/documents/{sop_instance_uid}/updates`
  - `GET /sr/documents/{sop_instance_uid}`
  - `GET /sr/documents`

## MPR migration

- New baseline API in `viewer-core`:
- `VolumeGrid::from_slices(...)`
- `reslice_volume(...)`
- New patient-space and slab APIs in `viewer-core`:
  - `PatientGeometry`
  - `VolumeGrid::voxel_to_patient_um(...)`
  - `VolumeGrid::patient_to_voxel_f64(...)`
  - `PatientMprRequest`
  - `reslice_volume_patient(...)`
  - `MprRequest::{slab_thickness, slab_mode}`
- New `diccy` facade:
- `assemble_volume_from_slices(...)`
- `request_mpr_frame(...)`
- New WASM API:
- `WasmViewer::mpr_reslice_json(...)`
  - `WasmViewer::mpr_reslice_patient_json(...)`
  - `WasmViewer::set_mpr_slab(...)`

## Fusion migration

- New baseline PET/CT fusion APIs:
  - `validate_fusion_preconditions(...)`
  - `resample_pet_to_ct_grid(...)`
  - `blend_pet_overlay(...)`
  - `execute_pet_ct_fusion(...)`
- New perf harness:
  - `./tools/fusion_perf_harness.sh`

## Compatibility notes

- All additions are backward-compatible for callers that do not adopt new APIs.
- Raw-mode must be explicitly enabled; default behavior remains strict fail-closed.
- SR update path enforces referenced-SOP validation and version checks.
- Voxel-space MPR request flows remain supported during patient-space migration.
- `LegacyVolumeGrid` adapters remain available for schema compatibility.

## Deprecation notices (soft)

No hard API removals are introduced in this wave. Soft deprecation guidance:
- New integrations should prefer patient-space MPR request surfaces over voxel-only assumptions.
- New persistence integrations should use service-level SR commit endpoints instead of ad-hoc local SR mutation.
- Future major versions may tighten compatibility around legacy volume-only metadata paths once migration adoption is complete.
