# Web Renderer Maturity

Status: **Implemented (gated WebGPU production path)** (As of 2026-02-21)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## 1. Production WebGPU readiness checklist

`viewer-wasm` keeps CPU-oracle output as correctness source and treats WebGPU as presentation-only. Production activation is explicitly gated by `set_production_webgpu_renderer(true)` plus host/runtime capability checks.

| Checklist item | Requirement | Current status | Evidence |
|---|---|---|---|
| Initialization gate | Probe API + adapter, initialize device, fail closed on errors | Implemented | `crates/viewer-wasm/src/backend.rs`, `crates/viewer-wasm/web/app.js` |
| Submit path | Upload CPU-oracle RGBA into WebGPU texture and draw full-screen pass | Implemented | `crates/viewer-wasm/web/app.js` (`drawPreviewWithWebGpu`) |
| Deterministic fallback | WebGPU failures transition to CPU path with typed reason codes | Implemented | `crates/viewer-wasm/src/backend.rs` metrics + fallback lifecycle |
| Telemetry/metrics | Report transitions, fallback reasons/latency budget, render counts, chunk sizing | Implemented | `backend_selection_metrics_json()`, runtime panel in `crates/viewer-wasm/web/index.html` |
| Recovery | Simulate/recover device-loss with bounded transition behavior | Implemented | `simulate_gpu_device_lost()` / `recover_gpu_device()` + chaos test |

## 2. Texture format compatibility matrix

| Source pixel format | CPU oracle output | WebGPU upload texture | Browser path status |
|---|---|---|---|
| `Luma8` | `Rgba8` viewport bytes | `rgba8unorm` texture upload | Implemented |
| `Rgba8` | `Rgba8` viewport bytes | `rgba8unorm` texture upload | Implemented |
| `Luma16` | Converted/normalized through CPU oracle path | N/A direct upload (fail closed in backend skeleton) | Implemented (fail closed) |

Notes:
- The WebGPU host presenter consumes CPU-oracle RGBA bytes and does not bypass CPU correctness rules.
- Upload rows are padded to 256-byte alignment for deterministic WebGPU transfer behavior.

## 3. OHIF-class behavioral comparison

| Dimension | DiCCY (`viewer-wasm`) | OHIF-class expectation | Gap status |
|---|---|---|---|
| Startup path | Capability probe, gated WebGPU flag, CPU default | Dynamic GPU detection with graceful fallback | Closed (baseline parity) |
| Interaction responsiveness | CPU-oracle render + auto-tuned target frame interval + bounded upload chunk policy | Smooth interactive pan/zoom on large studies | Partial (benchmark tuning ongoing) |
| Recovery behavior | Device-loss simulation + explicit recovery + metrics for transition reasons | Robust recovery from browser GPU resets | Closed (baseline parity) |
| Determinism contract | CPU-oracle hashes remain correctness source | Determinism often implicit | Strength (explicit invariant) |

## 4. Objective pass/fail criteria

| Criterion | Pass condition | Current result |
|---|---|---|
| Backend gate correctness | Production WebGPU cannot activate unless feature flag + probe checks pass | PASS |
| Render path integrity | Uploaded DICOM frames can render via WebGPU when backend is selected | PASS |
| Oracle invariance | CPU-oracle hashes are unchanged by backend selection | PASS |
| Fallback latency budget | `last_fallback_latency_ms <= fallback_latency_budget_ms` with zero budget violations | PASS |
| Device-loss chaos | Repeated loss/recovery cycles keep bounded transitions and recover without unbounded drift | PASS |

## 5. Evidence refresh cadence

- Run smoke/integration browser coverage each release candidate.
- Run performance harness for large-study scenarios and publish JSON + dashboard deltas.
- Re-evaluate matrix/criteria quarterly or when browser backend behavior changes.
