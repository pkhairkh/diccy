# ADR: Desktop Shell Strategy for Workstation Packaging

Date: 2026-02-22
Status: Accepted

## Context

The workstation profile currently runs as a browser-hosted WASM application. Competitive positioning requires a turnkey operator experience with deterministic behavior and fail-closed controls.

## Decision

Adopt a **canonical script-hosted default** and keep native packaging as an explicit opt-in extension:

- Baseline distribution is browser-hosted (`viewer-wasm` runtime contract + local backend services).
- Artifact model for workstation is now:
  - `viewer-wasm`, `viewer-wgpu`, `viewer-core` are treated as shared libraries.
  - Front-end start is via `./tools/run_viewer_wasm_frontend.sh`.
  - No guaranteed `bin/viewer-*` payload in the default `workstation` archive.
- Native host wrappers, if added in future milestones, are out-of-band additions and remain explicitly documented as exceptions per profile.

## Rationale

- Keeps one primary UI/runtime implementation path.
- Reduces divergence risk across deployment channels.
- Preserves deterministic behavior and existing test coverage investments.
- Supports phased rollout to native shell without blocking workstation baseline maturity.
- Aligns release posture and claim tables with observed build artifacts.

## Consequences

- `tools/profile_matrix.json` records workstation with script-hosted/runtime-library intent.
- Packaging and docs paths must treat workstation as an artifact-posture exception where binaries are optional, not mandatory.
- Native shell packaging work is additive and scoped as a separate productization track.
