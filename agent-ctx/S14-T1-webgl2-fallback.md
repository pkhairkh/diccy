# S14-T1: WebGL Fallback Renderer in viewer-wasm

## Task Summary
Implemented WebGL2 fallback renderer in the `viewer-wasm` crate, adding a complete WebGPU → WebGL2 → CPU cascade for backend selection.

## Files Modified

### `/home/z/diccy/crates/viewer-wasm/src/backend.rs`
- Added `WebGL2` variant to `RendererBackend` enum (with `as_str()` returning `"WebGL2"`)
- Added `WebGL2Lifecycle` enum: `Uninitialized | Ready | ContextLost | Failed`
- Added `WebGL2InitFailed` and `WebGL2ContextLost` variants to `BackendErrorCode`
- Added `WebGL2Backend` struct implementing `WasmRenderBackend` trait
- Added GLSL ES 3.00 shader sources:
  - `WEBGL2_MPR_VERT_GLSL` / `WEBGL2_MPR_FRAG_GLSL` — 2D texture quad shader for MPR slice rendering
  - `WEBGL2_MIP_VERT_GLSL` / `WEBGL2_MIP_FRAG_GLSL` — Ray-marching shader using `sampler2DArray` for MIP/MinIP
  - `WEBGL2_VR_VERT_GLSL` / `WEBGL2_VR_FRAG_GLSL` — Volume rendering with transfer function + Phong gradient shading using `sampler2DArray`
- Added public functions: `webgl2_mpr_shader_sources_json()`, `webgl2_mip_shader_sources_json()`, `webgl2_vr_shader_sources_json()`
- Updated `BackendSelectionMetrics` with `webgl2_init_attempts`, `webgl2_init_failures`, `webgl2_present_count`, `webgl2_backend_flag_enabled`
- Updated `ConfigureDirective` with `AttemptWebGL2Init` and `AttemptWebGpuThenWebGL2` variants
- Updated `RecoverDirective` with `AttemptWebGL2Init`
- Updated `BackendPolicy::configure_directive()` for cascade logic
- Updated `BackendPolicy::recover_directive()` for WebGL2 context loss recovery
- Updated `BackendPolicy::submit_failure_mapping()` for WebGL2 error codes
- Updated `BackendRuntimeState`:
  - Added `webgl2: WebGL2Backend` and `webgl2_lifecycle: WebGL2Lifecycle` fields
  - Added `webgl2_backend_enabled` field (defaults to `cfg!(feature = "webgl2-backend")`)
  - Added `set_webgl2_backend_enabled()`, `webgl2_backend_enabled()` methods
  - Added `simulate_webgl2_context_lost()` method
  - Added `webgl2_shader_sources_json()` method
  - Updated `configure_backend()` for WebGPU → WebGL2 → CPU cascade
  - Updated `record_present_with_timing()` for WebGL2 backend
  - Updated `volume_rendering_available()` to include WebGL2
  - Updated `metrics_json()` with WebGL2 fields

### `/home/z/diccy/crates/viewer-wasm/src/lib.rs`
- Updated re-exports to include `WebGL2Lifecycle`, `webgl2_mip_shader_sources_json`, `webgl2_mpr_shader_sources_json`, `webgl2_vr_shader_sources_json`
- Updated `configure_renderer_backend()` doc comment
- Updated `active_renderer_backend()` doc comment
- Added `set_webgl2_renderer_enabled()` / `webgl2_renderer_enabled()` methods
- Added `simulate_webgl2_context_lost()` method
- Added `webgl2_shader_sources_json()` method

### `/home/z/diccy/crates/viewer-wasm/Cargo.toml`
- Added `webgl2-backend` feature flag

### `/home/z/diccy/crates/viewer-wasm/tests/webgl2_fallback.rs` (new)
- 32 integration tests covering:
  - WebGL2 backend selection when WebGPU unavailable
  - Explicit WebGL2 preference
  - Feature flag gating
  - Full cascade: WebGPU → WebGL2 → CPU
  - MPR cache key determinism across backends
  - Viewport pixel output determinism
  - WebGL2 context loss and recovery
  - WebGL2 lifecycle state transitions
  - WebGL2 metrics tracking
  - Volume rendering availability with WebGL2
  - WebGL2 shader source validation
  - Fail-closed: WebGL2 init failure falls back to CPU

### `/home/z/diccy/crates/viewer-wasm/tests/backend_tests.rs`
- Updated `production_webgpu_flag_blocks_backend_activation_when_disabled` to account for WebGL2 cascade (feature-flag conditional)

### `/home/z/diccy/crates/viewer-wasm/tests/hi_wasm_boundary_contract.rs`
- Updated `backend_selection_and_fallback_are_deterministic` for WebGL2 cascade behavior

## Test Results
All tests pass with all feature flag combinations:
- `cargo test -p viewer-wasm` (no features) ✅
- `cargo test -p viewer-wasm --features webgl2-backend` ✅
- `cargo test -p viewer-wasm --features webgpu-backend` ✅
- `cargo test -p viewer-wasm --features "webgpu-backend,webgl2-backend"` ✅
- `cargo test -p viewer-wasm --all-features` ✅

## Architecture Decisions
1. **2D Texture Arrays**: WebGL2 lacks native 3D textures, so all 3D volume shaders use `sampler2DArray` where each layer = one volume slice. Z-interpolation is done manually in the shader.
2. **Determinism**: MPR cache keys are computed from the same CPU volume data regardless of backend, ensuring bit-identical results.
3. **Fail-closed**: If WebGL2 init fails, the system falls back to CPU, never silently corrupts.
4. **Feature-gated**: WebGL2 support is behind the `webgl2-backend` feature flag, defaulting to disabled.
5. **Cascade order**: WebGPU → WebGL2 → CPU, with explicit `"webgl2"` preference support.
