# S16-T1: JS/WASM Embedding SDK (dicom-viewer-sdk)

## Task: Implement high-level JavaScript/TypeScript SDK via wasm-pack

### Completed Implementation

1. **Created `/home/z/diccy/crates/dicom-viewer-sdk/Cargo.toml`**
   - cdylib + rlib crate types for WASM and native builds
   - Dependencies: wasm-bindgen, js-sys, web-sys, serde, serde_json, viewer-core

2. **Created `/home/z/diccy/crates/dicom-viewer-sdk/src/lib.rs`**
   - `DicomViewer` class with `#[wasm_bindgen(constructor)]`
   - `load_study(wado_rs_url)` — loads study from WADO-RS URL
   - `set_window_level(center, width)` — sets explicit window/level
   - `add_measurement_listener()` / `add_study_loaded_listener()` / `add_viewport_changed_listener()` / `add_segmentation_updated_listener()`
   - `remove_event_listener()` — removes listener by type + ID
   - `get_viewport_state()` — returns JSON state snapshot
   - `destroy()` — cleans up all resources
   - `ViewerModelState` — deterministic state snapshot for SDK boundary
   - `EventListenerMap` — stores listener entries keyed by event type
   - Native API: `emit_event()`, `create_measurement_event()`, `model_mut()`
   - Helper: `extract_study_uid_from_url()` — parses study UID from WADO-RS URL

3. **Created `/home/z/diccy/crates/dicom-viewer-sdk/src/events.rs`**
   - `SdkEventType` enum: StudyLoaded, ViewportChanged, MeasurementCreated, MeasurementUpdated, MeasurementDeleted, SegmentationUpdated, AnnotationCreated, Error
   - `SdkEvent` struct with factory methods for each event type
   - `SdkEventDataBuilder` for constructing complex event payloads
   - Event type roundtrip parsing (as_str / from_str_opt)

4. **Created `/home/z/diccy/crates/dicom-viewer-sdk/src/react.rs`**
   - `generate_typescript_types()` — full TypeScript type definitions
   - `generate_react_component()` — React component wrapper with hooks
   - `generate_vue_component()` — Vue 3 component wrapper
   - `generate_svelte_component()` — Svelte component wrapper
   - `generate_embedding_guide()` — iframe-less integration guide

5. **Added to workspace Cargo.toml**

### Test Results
- 32 tests passing across dicom-viewer-sdk and dicom-pwa
- Tests cover: constructor, load_study, set_window_level, listeners, destroy, event emission, URL parsing, state serialization, event type roundtrip, React/Vue/Svelte generation
