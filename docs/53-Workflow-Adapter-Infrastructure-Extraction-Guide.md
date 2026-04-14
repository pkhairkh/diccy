# Workflow Adapter/Plugin Infrastructure Extraction Guide

## Purpose

This guide describes an incremental migration path for moving workflow adapter and plugin loading logic from `crates/dicom-workflow-server` into a dedicated infrastructure crate while preserving current runtime behavior and fail-closed defaults.

## Current Baseline

- Adapter metadata and plugin/env loading live in `crates/dicom-workflow-server/src/main.rs`.
- Runtime surfaces impacted today:
  - connector registry normalization and feature/rollout resolution,
  - plugin metadata parsing from environment variables,
  - connector status/features/health endpoints.

## Target End State

- Introduce a dedicated crate (recommended name: `crates/dicom-workflow-infra-adapters`).
- Move adapter/plugin loading and compatibility policy checks into that crate.
- Keep domain/application route logic in `dicom-workflow-server`, consuming typed infrastructure APIs.

## Recommended Extraction Stages

1. Define a stable interface in `dicom-workflow-server` for adapter resolution:
   - typed connector descriptor,
   - typed plugin metadata,
   - typed feature/rollout capability view.
2. Move pure normalization/parsing code first:
   - env-key normalization,
   - connector alias normalization,
   - plugin path/version compatibility parsing.
3. Move policy checks next:
   - `compatible_min`/`compatible_max` checks,
   - startup diagnostics payload shaping.
4. Move endpoint payload builders last:
   - connector status/features/health DTO rendering can consume infrastructure-provided snapshots.
5. Keep a compatibility shim in `dicom-workflow-server` until all call sites are switched.

## Contract and Risk Controls

- Keep environment-contract keys unchanged during extraction.
- Preserve deterministic connector ordering behavior.
- Preserve fail-closed startup behavior for invalid plugin material.
- Keep `/interop/connectors/*` response field names stable unless a versioned API change is explicitly planned.

## Validation Checklist

- Build passes for `dicom-workflow-server` and the new infrastructure crate.
- Existing connector status/features/health tests pass unchanged.
- Startup with invalid plugin path still fails closed.
- Env-contract docs and generated reports remain in sync after extraction.

## Rollback Plan

- Keep migration in small, independently reviewable commits:
  - interface scaffolding,
  - parser move,
  - policy move,
  - endpoint integration.
- If any stage regresses runtime behavior, revert only that stage and retain previously validated boundaries.

## Related References

- `docs/12-API-Surface-and-Crate-Boundaries.md`
- `docs/39-SR-Workflow-Architecture.md`
- `docs/41-Extension-Plugin-Contract.md`
- `docs/49-Runtime-Profile-Capability-Matrix.md`
