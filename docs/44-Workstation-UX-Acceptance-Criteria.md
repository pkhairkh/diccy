# Workstation UX Acceptance Criteria

Status: **Implemented baseline acceptance criteria** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## 1. Objective criteria

A workstation build is accepted only when all criteria pass on the same dataset and configuration.

| Area | Metric | Target | Evidence command |
|---|---|---|---|
| Startup | Host boot to "WASM module loaded" | <= 5 seconds on baseline dev machine | `./tools/browser_smoke_renderer.sh` |
| Study load | File select to first rendered preview | <= 3 seconds for 1-frame sample P10 | `./tools/browser_smoke_renderer.sh` |
| Visual regression | MPR baseline + slab baseline + fusion baseline screenshot checks | zero mismatches and successful run | `DICCY_WASM_SMOKE_URL=http://127.0.0.1:4173 ./tools/browser_ui_flow_screenshot.sh` |
| Viewport interaction | Zoom + recenter response | <= 200 ms p95 interaction latency | `./tools/browser_perf_renderer.sh --output reports/performance/web-renderer-harness.json` |
| Fallback resilience | WebGPU fallback latency | <= configured fallback latency budget and zero budget violations | `./tools/ux_benchmark_suite.sh` |
| Reporting loop | SR create -> update -> retrieve | All 3 operations return 2xx and monotonic version increments | `cargo test -p dicom-workflow-server sr_http_flow_create_update_retrieve_is_deterministic -- --exact` |

## 2. Required interaction coverage

- Ingest one DICOM P10 file through host upload flow.
- Render preview and exercise zoom/recenter controls.
- Execute tri-planar MPR render with slab controls.
- Execute SR create, SR update, and SR retrieve actions from host SR panel.
- Validate backend diagnostics surface reports active backend state and fallback counters.

## 3. Failure policy

- Any failed criterion is a **NO-GO** for workstation release candidate sign-off.
- Failed criteria must map to tracked acceptance gaps in review notes and include evidence artifact paths under `reports/`.
- Benchmark evidence bundle output: `reports/performance/ux-benchmark-suite.json`.
