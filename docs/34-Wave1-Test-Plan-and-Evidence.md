# Wave 1 Test Plan and Evidence

Last updated: 2026-02-21

## Scope

Wave 1 covers tasks `TASK-3001` through `TASK-3200`.

## Test plan mapping

| Area | REQ anchor | Primary verification |
|---|---|---|
| Docs/code capability consistency | REQ-OPS-001, REQ-OPS-005 | `tools/claim_surface_lint.py`, `tools/docs_capability_lint.py`, `tools/docs_symbol_lint.py` |
| WASM backend fallback | REQ-OPS-003, REQ-OPS-006 | `cargo test -p viewer-wasm` |
| Native renderer lifecycle/cache | REQ-OPS-003, REQ-OPS-007 | `cargo test -p viewer-wgpu` |
| MPR baseline | REQ-OPS-002, REQ-OPS-003 | `cargo test -p viewer-core` |
| SR authoring/update baseline | REQ-OPS-002, REQ-OPS-008 | `cargo test -p pack-sr` |
| Raw-mode parser + warnings | REQ-OPS-002, REQ-OPS-004 | `cargo test -p dicom-io` |

## CI gate additions

- Claim-surface and capability consistency gates in `.github/workflows/claim-surface-gate.yml`.
- Determinism and traceability gates in `.github/workflows/determinism-gate.yml`.
- Reproducibility matrix in `.github/workflows/reproducibility-matrix.yml`.

## Operator incident playbook (renderer fallback/device-loss)

1. Confirm active backend via runtime snapshot export.
2. Review `backend_diagnostics` metrics for fallback count and last error code.
3. Toggle `Force CPU renderer` when GPU instability persists.
4. Attempt deterministic recovery with `Recover device`.
5. Preserve event log and runtime snapshot for incident record.

## SLO targets

| Metric | Target |
|---|---|
| Viewer startup to ready state | <= 3s on reference hardware |
| 2D re-render cadence | <= 33ms median |
| Baseline MPR request latency | <= 100ms for 512x512 output |

## Wave 1 implementation sequence

1. Truthful docs and claim-surface gates.
2. WASM backend runtime fallback and diagnostics.
3. Renderer lifecycle/cache determinism.
4. Volume/MPR baseline APIs.
5. SR author/update baseline APIs.
6. Raw-mode parser diagnostics and warning channels.
7. Release evidence and traceability publication.
