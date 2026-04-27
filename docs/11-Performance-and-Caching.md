# Performance and caching

## Goals

- Interactive viewing **SHOULD** be smooth for typical clinical image sizes.
- Decoding and rendering **MUST** be bounded by explicit resource budgets.
- Caching **MUST** be deterministic to support reproducible behavior and tests.

## Implementation status (RC-2026.02.12)

Status: **Partial cache implementation** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

- Implemented now: deterministic GPU texture budget accounting/lifecycle counters in `viewer-wgpu` plus shared deterministic cache utility (`viewer-core::DeterministicCache`) with stable LRU tie-break and pinned-entry support.
- Implemented now: baseline MPR cache-key quantization for deterministic reuse inputs.
- Planned: expanded dataset/pixel cache layering across ingestion and decode stages.

### Cache implementation matrix (As of 2026-02-21)

| Cache layer | Status | Notes |
|---|---|---|
| Shared deterministic cache utility | Implemented (baseline) | `viewer-core::DeterministicCache` provides stable LRU tie-break + pinning. |
| GPU texture cache in renderer lifecycle | Partially implemented | Budget enforcement and deterministic accounting are active; broader pooling/reuse hardening remains staged. |
| Dataset header cache | Deferred target architecture | Design specified; full integration remains pending. |
| Decoded pixel cache | Deferred target architecture | Design specified; full integration remains pending. |
| Volume/MPR cache layering | Partial | Baseline deterministic MPR cache-key quantization exists; fuller layered caching remains pending. |

---

## 1. Performance model

### Key operations

Normative: Performance reporting **MUST** attribute work to these operations so regressions can be localized and fixed.

- Dataset scan (header-only): enumerate studies/series and basic metadata.
- Series assembly: ordering + geometry validation.
- Pixel decode: transfer syntax decode + pipeline.
- Upload: CPU output → GPU texture.
- Interaction: pan/zoom/window-level updates.

### Requirements (normative)

- Header-only scan **MUST** not decode Pixel Data.
- Window/level interaction **SHOULD** avoid re-decoding compressed pixels if possible:
  - preferred: keep post-modality buffer and reapply VOI cheaply,
  - fallback: recompute from stored samples with bounded cost.

Assumption: storing post-modality buffers can reduce latency but increases memory.
- Verification method: benchmark both modes and enforce a configurable cache budget.

---

## 2. Caching layers

### 2.1 Dataset header cache (deferred target architecture)

Key: `InstanceUid`

Stores:
- extracted minimal header model (tags needed for browsing and decode setup).

Requirements:
- Cache **MUST** be bounded (max entries and/or bytes).
- Eviction **MUST** be deterministic (LRU with stable tie-break).

### 2.2 Decoded pixel cache (deferred target architecture)

Key: `FrameKey` (InstanceUid + FrameIndex)

Stores:
- decoded raw samples (optional),
- post-modality buffer (optional),
- final display buffer (`Luma8`/`Rgba8`) (optional).

Requirements:
- The cache **MUST**:
  - account for byte size precisely,
  - evict deterministically,
  - support “pinned” entries for currently displayed frames.

### 2.3 GPU texture cache (partially implemented)

Key: `FrameKey` + `TextureFormat`

Stores:
- `wgpu::Texture` + view + sampler metadata.

Requirements:
- A GPU budget **MUST** be enforced.
- Texture re-use **SHOULD** be employed (pooling) to reduce churn.
- Eviction policy **MUST** be deterministic.

---

## 3. Volume caching

Volume/MPR baseline behavior is implemented; the cache layering below defines current baseline plus deferred expansion targets:

- Cache assembled volumes by `SeriesUid` and config (resample mode).
- Optionally cache MPR slices by plane parameters (quantized) for repeated views.

Requirements:
- Volume cache **MUST** be bounded by bytes.
- Non-uniform spacing volumes **MUST** not be cached unless explicitly supported.

---

## 4. Concurrency and scheduling

### Native

- Decoding **MAY** occur on a bounded thread pool.
- The scheduler **MUST** prioritize:
  - current frame,
  - adjacent frames (prefetch window),
  - then background tasks.

### WASM

- Single-threaded operation **MUST** remain functional.
- Long operations **SHOULD** yield cooperatively (chunked processing) to avoid UI lockups.

---

## 5. Benchmarking and profiling

### Requirements (normative)

- The project **SHOULD** maintain micro-benchmarks for:
  - RLE decode,
  - JPEG baseline decode (if enabled),
  - VOI/window mapping,
  - volume resampling (if enabled),
  - PET/CT fusion baseline (resample + blend) with explicit latency budgets.
- Benchmarks **MUST** be reproducible:
  - fixed inputs,
  - fixed config,
  - pinned feature sets.

Assumption: Criterion-based benchmarking is sufficient for CPU micro-benchmarks.
- Verification method: include `cargo bench` harnesses (optional), and document how to run them without network access.
- Deterministic fusion harness entrypoint:
  - `./tools/fusion_perf_harness.sh` (fails non-zero when the configured budget is exceeded).

---

## 6. Verification requirements

- Cache eviction logic **MUST** be unit-tested for determinism.
- Performance regressions **SHOULD** be detected via benchmark baselines (optional but recommended).


## 7. Web performance matrix (normative)

Web viewers exhibit meaningful performance variability across browsers and OS. DiCCY treats this as a first-class engineering constraint for WASM deployments.

Reference (informative): Pereira et al., web viewer performance survey/classification: https://pubmed.ncbi.nlm.nih.gov/39349783/

Requirements:

- **REQ-PERF-701:** Any WASM deployment profile **MUST** define a browser/OS test matrix and record measured decode + interaction latency under fixed corpus inputs.
- **REQ-PERF-702:** Performance regressions **SHOULD** be detected by comparing medians and spreads across the matrix for a pinned corpus subset.
- **REQ-PERF-703:** Performance tests **MUST** run with explicit cache budgets and explicit thread settings (where applicable), and report those settings.

Verification:

- The repository **SHOULD** maintain a minimal “perf corpus subset” and a reproducible benchmark script (native and web) that reports results in a machine-readable format.
- Browser harness entrypoint (large-study interactions):
  - `./tools/browser_perf_renderer.sh --output reports/performance/web-renderer-harness.json --interactions 50 --width 512 --height 512`
- Dashboard synthesis for adoption/fallback trends:
  - `python3 tools/webgpu_dashboard_report.py --inputs 'reports/performance/web-renderer-harness*.json' --output reports/performance/webgpu-dashboard.md`

---

## 8. Release-specific budgets and tuning outcomes (RC-2026.02.11)

This section records the release-scoped performance evidence bundle for RC-2026.02.11.

Release evidence:
- SLO definitions: `reports/performance/slo-definitions-RC-2026.02.11.md`
- Runtime monitoring template: `reports/monitoring/runtime-slo-monitoring-template.md`
- Workload profile specification: `reports/performance/workload-profile-spec-RC-2026.02.11.md`
- Baseline report: `reports/performance/baseline-performance-report-RC-2026.02.11.md`
- Soak stability report: `reports/performance/soak-stability-report-RC-2026.02.11.md`
- Failure-injection report: `reports/performance/failure-injection-report-RC-2026.02.11.md`
- Gate decision: `reports/performance/performance-scalability-gate-decision-RC-2026.02.11.md`

Summary outcomes:
- Baseline workloads met release thresholds for latency, throughput, and error budget.
- Soak workloads showed bounded resource behavior under sustained loops.
- Failure-injection workloads preserved fail-closed behavior for restart, storage-path, auth, and transport rejection scenarios.
- The Sprint 12 performance/scalability gate for RC-2026.02.11 is signed as GO in the gate decision artifact.

---

## 9. Production SLO baseline (backend profiles)

These baseline SLOs apply to production promotion unless a release-specific override is approved.

| Surface | p95 target | p99 target | Error budget |
|---|---|---|---|
| QIDO (`/studies` query class) | `250 ms` | `500 ms` | `<= 1.0%` |
| WADO retrieve class | `400 ms` | `800 ms` | `<= 1.0%` |
| STOW ingest class | `1200 ms` | `2400 ms` | `<= 1.0%` |
| Workflow query APIs | `<= 12 s` | `<= 15 s` | `<= 0.5%` |
| Workflow mutation APIs | `<= 15 s` | `<= 18 s` | `<= 0.5%` |
| Connector callback dispatch | `<= 15 s` | `<= 22 s` | `<= 1.0%` |

Profile-specific harnesses:

- Workflow mutation/query harness (deterministic fixtures, fixed seed):
  - `reports/performance/profiles/workflow-api-load-profile-RC-2026.02.24.json`
  - `tools/run_workflow_api_load_harness.sh`
- Connector callback throughput/backoff/circuit harness:
  - `reports/performance/profiles/connector-callback-load-profile-RC-2026.02.24.json`
  - `tools/run_connector_callback_load_harness.sh`
- Profile-specific gate workloads:
  - `reports/performance/profiles/profile-backend-services-RC-2026.02.24.json`
  - `reports/performance/profiles/profile-backend-services-with-dimse-RC-2026.02.24.json`

Budget and regression gates:

- Sustained p95/p99 regression gate from env-contract defaults:
  - `tools/perf_budget_regression_gate.py`
- CPU/memory/GPU resource budget gate:
  - `tools/resource_budget_gate.py`
  - `reports/performance/resource-budget-baseline.json`

24h soak artifact generation:

- `tools/generate_soak_reliability_artifact.py`

Capacity assumptions and scaling model:

- `docs/64-Performance-Capacity-Model.md`
