# WASM target

## Overview

Status: **Implemented CPU path with gated WebGPU draw path** (As of 2026-02-21)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

Normative: WASM builds **MUST** preserve the same conformance envelope semantics and deterministic CPU-boundary outputs as native builds, within documented platform constraints.

This document defines portability constraints and the separation between:

- **portable core logic** (WASM-compatible),
- **platform glue** (browser integration, rendering surface setup).

---

## 1. Target constraints

### Runtime constraints (normative)

- WASM builds **MUST** assume:
  - no filesystem access (`std::fs` unavailable),
  - limited and quota-managed memory,
  - single-threaded execution by default.
- Any code requiring:
  - OS paths,
  - native threads,
  - memory-mapped IO,
  **MUST** be isolated outside core crates.

### Toolchain constraints (assumption + verification)

Assumption: primary WASM target is `wasm32-unknown-unknown` using `wasm-bindgen`-based packaging.

Justification: it is the most common Rust→web toolchain and supports `wgpu` web.

Verification method:
- CI **MUST** include `cargo build --target wasm32-unknown-unknown` for all portable crates.

---

## 1.1 Determinism and numerics (normative)

WebAssembly execution is mostly deterministic, but the core specification permits nondeterministic NaN payload selection in some cases (see WebAssembly Numerics). DiCCY avoids this by preventing NaN/Infinity generation in the pixel pipeline and by treating the CPU boundary output as the correctness oracle.

Primary reference:
- WebAssembly core spec, Numerics: https://webassembly.github.io/spec/core/exec/numerics.html

Requirements:

- **REQ-WASM-301:** The pixel pipeline **MUST** avoid generating NaN or Infinity and **MUST** fail closed if any intermediate non-finite value is observed.
- **REQ-WASM-302:** WASM builds **MUST** treat CPU boundary outputs (`Luma8`/`Rgba8`) as the determinism oracle. GPU rendering is presentation-only and constrained per `docs/05`; cross-target reproducibility decisions are made from CPU-boundary hash sets, not screenshots.
- **REQ-WASM-303:** Any future feature that depends on NaN payload preservation **MUST NOT** be enabled in the default WASM build. If such a feature is added behind an explicit pack, it **MUST** define a deterministic canonicalization policy and include cross-VM regression tests.

Verification:

- Cross-target tests **MUST** compare CPU boundary hashes between native and WASM for Tier 0 corpus subsets.
- Reproducibility matrix jobs **MUST** run across native x86_64, native arm64, and wasm targets, publish per-target hash manifests, and fail when mismatch budget is exceeded.
- Property tests **SHOULD** assert that intermediate values in the pixel pipeline are finite for all in-envelope inputs.

---

## 2. Crate portability split

### Portable crates (MUST build on wasm32)

Requirements: These crates **MUST NOT** depend on OS-only APIs and **MUST** compile under `wasm32-unknown-unknown` with default features.

- `dicom-core`
- `dicom-series`
- `dicom-pixel`
- `viewer-core`
- `viewer-wgpu` (GPU renderer crate; WASM host now includes gated browser WebGPU draw activation)

### Platform glue crates

Requirements: Platform glue **MUST** encapsulate browser-specific APIs so portable crates remain host-agnostic.

- `viewer-wasm`:
  - browser bindings,
  - file/drag-drop adapters,
  - UI integration hooks.

### Requirements (normative)

- Portable crates **MUST NOT** depend on:
  - `std::fs`,
  - `std::process`,
  - native windowing toolkits,
  - environment-variable configuration for critical security limits (limits must be programmable).
- Platform glue **MAY** depend on `web_sys`, `js_sys`, and `wasm-bindgen`.

---

## 3. Input ingestion in the browser

### Byte-oriented ingestion (normative)

- The WASM viewer **MUST** accept inputs as byte buffers provided by the host (browser).
- Supported host mechanisms **MAY** include:
  - file picker uploads,
  - drag-and-drop,
  - in-memory fetch (only if explicitly enabled; see security section below).

### Archive handling (normative)

If archives (zip) are supported:
- Archive parsing **MUST** enforce:
  - maximum entry count,
  - maximum compressed size,
  - maximum decompressed size per entry,
  - maximum total decompressed size.
- Path traversal inside archives **MUST** be prevented (ignore directory components; reject `..`).

---

## 4. Codec strategy (plugin boundary)

### Default (normative)

- WASM builds **MUST** support Tier 0 transfer syntaxes using **memory-safe Rust decoders** only.
- Any FFI-based codec **MUST** be disabled by default on WASM.

### Codec abstraction (normative)

The current implementation uses a function-level codec boundary in `dicom-pixel` (`PixelPipeline::decode` and codec feature gates). A dedicated public `PixelCodec` trait is deferred until a multi-backend decode plugin surface is implemented.

Requirements:
- Codec selection **MUST** be driven by Transfer Syntax UID.
- Codec implementations **MUST** be registered via compile-time features to avoid dynamic loading in WASM.
- Decoders **MUST** enforce decompression limits.

---

## 5. Rendering in web environments

### Web rendering path and fallback status

Current RC-2026.02.21 implementation:
- Primary active path: `viewer-wasm` CPU/canvas rendering from CPU-oracle display frames.
- Gated production WebGPU draw path is available when runtime feature flag and probe checks pass.

Target architecture:
- Primary target: `wgpu` WebGPU backend in browser environments where supported.
- Target fallback: CPU path by default; optional WebGL backend only when explicitly implemented and validated.

Requirements:
- The runtime **MUST** probe capabilities before activating any GPU path.
- GPU activation **MUST** fail closed to CPU presentation when required capabilities are missing or initialization fails.
- Capability claims **MUST** match `docs/31-Implementation-Status.md` and include an explicit release date stamp.

### Renderer backend contract (RC-2026.02.12)

The WASM host runtime exposes deterministic backend coordination APIs:

- `configure_renderer_backend(preferred, webgpu_api, adapter_available, webgl2_api, max_texture_dimension_2d)`
- `active_renderer_backend()`
- `backend_capability_probe_json()`
- `backend_selection_metrics_json()`
- `set_force_cpu_renderer(enabled)`
- `set_render_frame_interval_ms(interval_ms)`
- `simulate_gpu_device_lost()`
- `recover_gpu_device()`
- `csp_safe_shader_source()`

Current implementation guarantees:
- CPU-oracle output remains the only correctness source; backend selection affects presentation path only.
- WebGPU backend is implemented with typed error codes (`DVF.WASM.GPU.INIT_FAILED`, `DVF.WASM.GPU.DEVICE_LOST`, `DVF.WASM.GPU.SUBMIT_FAILED`, `DVF.WASM.GPU.FLAG_DISABLED`).
- GPU path failover to CPU is automatic for probe/init/submit failures.
- Upload budgeting and chunking are deterministic and bounded by `max_gpu_texture_bytes` with policy-driven chunk sizing.
- Runtime diagnostics include transition counters, fallback reason fields, latency budgets, render timing counters, and explicit device-loss recovery attempts.
- Shader source is embedded and exposed through a strict-CSP-safe path (no runtime shader fetch/eval).

Production readiness checklist, compatibility matrix, and objective pass/fail criteria:
- `docs/38-Web-Renderer-Maturity.md`

### Deterministic rendering contract

- CPU boundary output (pipeline output bytes) **MUST** be deterministic.
- Screen rendering **SHOULD** be visually stable, but exact per-pixel screen captures are not guaranteed across GPU drivers/backends.
- If the project introduces a “render capture” test mode:
  - it **MUST** be explicitly labeled backend-dependent and used only for smoke tests, not as the primary correctness oracle.
  - backend variance policy **MUST** be bounded and explicit: capture-based checks may tolerate at most 1 LSB absolute difference for at least 99% of pixels and 2 LSB maximum absolute difference for any pixel.

---

## 6. Security considerations for WASM

- Network access in browsers is inherently possible.
- Default viewer builds **MUST** not perform any network requests.
- If network loading is enabled:
  - it **MUST** require explicit host permission and be documented,
  - it **MUST** enforce the same input limits and origin restrictions,
  - it **MUST** avoid sending PHI/PII and must warn the integrator.

See `docs/09-Security-Threat-Model.md`.

---

## 7. Verification requirements

- A WASM build check **MUST** exist for portable crates.
- Golden corpus tests **MUST** validate pixel pipeline output determinism in WASM for Tier 0 samples (hash comparison at CPU boundary).
- Release pipelines **MUST** archive a signed cross-target reproducibility matrix with per-target manifest hashes and mismatch verdict.

---

## 8. Local interactive host harness (informative)

Repository path:
- `crates/viewer-wasm/web` (HTML/CSS/JS host harness)

Runner:
- `tools/run_viewer_wasm_frontend.sh`

Local startup:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
./tools/run_viewer_wasm_frontend.sh --port 4173
```

Open `http://127.0.0.1:4173`.

The host provides deterministic interaction coverage for:
- file upload to `WasmViewer::load_bytes`,
- viewport resize and zoom controls,
- compile-time gated network toggle (`--features network`),
- metadata HTML escaping checks (`escapeMetadataHtml`).
