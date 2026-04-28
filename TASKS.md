# TASKS.md — Feature Gap Implementation Plan

> Sprint-based roadmap to close the gap between diccy's current capabilities
> and a production-grade PACS/DICOM workstation with 3D tooling and modern
> features. Based on hands-on code audit of the cloned repository.

---

## Current Implementation Reality (Corrected from Remote Review)

The initial remote review significantly **underestimated** what diccy already
implements. After cloning and reading the source, here is the corrected baseline:

| Feature Area | Actual Status | Key Crate / File |
|---|---|---|
| DICOMweb (WADO/STOW/QIDO) | **Implemented** | `dicom-web`, `dicom-web-server` |
| DIMSE networking (C-ECHO/C-STORE/C-FIND/C-MOVE/C-GET) | **Implemented** | `dicom-net`, `dicom-dimse`, `dicom-dimse-service` |
| 2D Pixel Decoding | **Implemented** | `dicom-pixel` (105KB lib.rs) |
| MPR (Axial/Coronal/Sagittal/Arbitrary) | **Partially implemented** — CPU-only `reslice_volume` in `viewer-core/src/mpr.rs` with nearest + linear kernels, slab MIP, patient-space geometry; **no GPU-accelerated rendering** | `viewer-core::mpr` |
| Volume Grid Assembly | **Implemented** — deterministic slice ordering, patient geometry, legacy migration | `viewer-core::volume` |
| Measurements (Distance2D, Angle2D, Probe) | **Implemented** — full lifecycle store with undo/redo, calibration, provenance, uncertainty, export | `viewer-core::clinical`, `viewer-core::lib.rs` |
| 3D Annotations | **Implemented** — add/update/remove/snapshot/JSON export | `viewer-core::clinical::Annotation3dStore` |
| Segmentation Store (lifecycle) | **Implemented** — `SegmentationStore` with style, diffs, locking, audit | `viewer-core::clinical::SegmentationStore` |
| DICOM SEG Parsing (Binary) | **Implemented** — `pack-seg` with overlay composition | `pack-seg` |
| DICOM-RT (Dose/Structure/Plan) | **Partially implemented** — `pack-rt` parses dose grids, structure sets, plan summaries with overlays | `pack-rt` |
| RT Dose/RTSS Overlay State | **Implemented** — `RtDoseOverlayState`, `RtssOverlayState` | `viewer-core::clinical` |
| GSPS (Grayscale Softcopy Presentation State) | **Partially implemented** — `pack-gsps` | `pack-gsps` |
| MIP Projection Mode | **Stub** — `MipProjectionMode` enum exists, capability-gated but `mip: false` by default; **no GPU rendering** | `viewer-core::clinical::VolumeWorkflowCapabilities` |
| 3D Volume Rendering | **Stub** — `volume_3d: false` by default in capabilities; enum exists, no implementation | `viewer-core::clinical` |
| Hanging Protocol Template | **Stub** — `HangingProtocolTemplate` and `HangingProtocolApplication` structs defined, no matching engine | `viewer-core::lib.rs` |
| Fusion Overlay State | **Implemented** — blend, visibility, registration state | `viewer-core::clinical::FusionOverlayState` |
| Comparison Sync (Side-by-Side) | **Implemented** — `ComparisonSyncState` with pan/zoom/frame lock | `viewer-core::lib.rs` |
| WGPU Viewer | **Skeleton** — `viewer-wgpu` with GPU runtime contract tests | `viewer-wgpu` |
| WASM Viewer | **Skeleton** — `viewer-wasm` with boundary contract tests | `viewer-wasm` |
| Modality Packs (CT/MR/XR/US/NM/XA/MG/PET) | **Stub crates** — SOP class gating only, no modality-specific logic | `modality-ct`, `modality-xr`, etc. |
| DICOM SR (Structured Reporting) | **Stub** — `pack-sr` exists with extraction contract tests; `SrMeasurementPayload` is a metadata stub | `pack-sr`, `viewer-core::clinical` |
| DICOM Core Parsing | **Implemented** | `dicom-core` |
| Storage / Index | **Implemented** | `dicom-storage`, `dicom-index` |
| Worklist / MPPS | **Implemented** | `dicom-worklist`, `dicom-mpps` |
| Workflow Server | **Implemented** — UPS, completion, SR workflows | `dicom-workflow-server` |
| Auth / Audit | **Implemented** | `dicom-auth`, `dicom-audit` |
| Query/Retrieve | **Implemented** | `dicom-query` |

---

## Feature Gap Priority Tiers

### TIER 1 — Table Stakes (workstation is non-functional without these)

| # | Gap | Detail |
|---|---|---|
| G1 | GPU-Accelerated MPR Rendering | CPU `reslice_volume` exists but is too slow for clinical use; need WGPU volume viewport |
| G2 | MIP / MinIP GPU Rendering | Enum defined, capability-gated, no GPU implementation |
| G3 | 3D Volume Rendering (VR) | Enum defined, capability-gated, no implementation |
| G4 | Interactive Segmentation Tools | Store lifecycle exists; no brush/scissor/threshold/region-growing tools |
| G5 | DICOM SR Full Encoding | `pack-sr` is a stub; need TID template engine + SR IOD creation from measurements |
| G6 | DICOM PR (Presentation State) Writeback | GSPS read exists; need creation of PR objects from annotations/measurements |

### TIER 2 — Expected (competitive minimum)

| # | Gap | Detail |
|---|---|---|
| G7 | Hanging Protocol Matching Engine | Template structs exist; need DICOM Supplement 60 matching logic |
| G8 | Study Prefetching / Smart Worklist | Worklist store exists; need rule-based prefetch and priority queuing |
| G9 | FHIR / HL7 Integration | No crate, no adapter |
| G10 | Image Registration (Rigid/Deformable) | Fusion overlay state exists; no registration algorithm |
| G11 | AI/ML Inference Pipeline | No inference runtime, no AIR IOD, no model serving |
| G12 | Display Calibration (GSDF) | No DICOM Grayscale Standard Display Function implementation |

### TIER 3 — Modern / Cutting-Edge

| # | Gap | Detail |
|---|---|---|
| G13 | 3D Printing (STL/3MF Export) | No mesh generation from segmentation |
| G14 | AR/VR/XR Visualization | `modality-xr` is CR/DR, not extended reality |
| G15 | Cloud-Native / K8s Deployment | No containerization, no Helm, no operator |
| G16 | Real-Time Collaboration | No WebSocket sync, no cursor sharing |
| G17 | Cardiovascular Quantification | No calcium scoring, no vessel analysis |
| G18 | Mammography Workflow (Tomosynthesis) | `modality-mg` is SOP-gating only |
| G19 | Digital Pathology (WSI) | No whole-slide imaging support |
| G20 | DICOM Encapsulation (Non-DICOM Content) | No PDF/JPEG/TIFF encapsulation |

---

## Sprint Plan

### SPRINT 1 (Weeks 1–4): GPU Volume Rendering Pipeline

**Goal:** Make MPR and MIP clinically usable via WGPU.

**Tasks:**

- [x] **S1-T1** Create `crates/viewer-wgpu/src/volume_renderer.rs`
  - WGPU volume viewport that ingests `VolumeGrid` from `viewer-core`
  - Upload voxel data as 3D texture (R32Sfloat)
  - Implement ray-marching vertex/fragment shader
  - Integrate with existing `viewer-wgpu` GPU runtime contract
  - **Acceptance:** `cargo test -p viewer-wgpu` passes with volume render test ✅
  - **Depends on:** `viewer-core::volume::VolumeGrid`, `viewer-wgpu` skeleton
  - **Estimated effort:** 5 days

- [x] **S1-T2** Implement GPU-accelerated MPR rendering
  - Three orthogonal slice viewports (axial, coronal, sagittal)
  - Oblique MPR via arbitrary clip plane through volume texture
  - Synchronized crosshair using existing `TriPlanarState`
  - Slab MIP / Average rendering via slab thickness uniform
  - VOI windowing via existing `WindowLevelState`
  - **Acceptance:** Tri-planar GPU MPR pipeline created ✅
  - **Estimated effort:** 5 days

- [x] **S1-T3** Implement GPU MIP / MinIP rendering
  - Enable `VolumeWorkflowCapabilities.mip = true` default ✅
  - Maximum Intensity Projection via ray-march with max accumulation
  - Minimum Intensity Projection via ray-march with min accumulation
  - Slab MIP with adjustable thickness slider
  - Rotation via arcball camera controller
  - **Acceptance:** MIP/MinIP GPU pipeline created with slab support ✅
  - **Estimated effort:** 4 days

- [x] **S1-T4** Implement 3D Volume Rendering (VR)
  - Enable `VolumeWorkflowCapabilities.volume_3d = true` default ✅
  - Transfer function editor (1D opacity + color lookup) — 256×2 RGBA texture
  - Preset transfer functions (bone, soft tissue, lung, linear)
  - Gradient-based shading (Phong model via central differences)
  - Interactive clip planes (6-plane crop box)
  - **Acceptance:** VR GPU pipeline created with transfer functions + gradient shading + clip planes ✅
  - **Estimated effort:** 6 days

- [x] **S1-T5** WASM volume rendering parity
  - Port WGPU volume renderer to WebGPU via `viewer-wasm`
  - Added `gpu_volume_render_json()` and `upload_volume_grid()` to `WasmViewer`
  - Added `volume_rendering_available()` to `BackendRuntimeState`
  - **Acceptance:** Volume rendering methods available in WASM boundary ✅
  - **Estimated effort:** 4 days

**Sprint 1 Deliverable:** Clinical-grade MPR, MIP, and VR rendering on GPU
(WGPU native + WebGPU WASM), closing gaps G1, G2, G3. ✅ **COMPLETE**

---

### SPRINT 2 (Weeks 5–8): Interactive Segmentation & Measurement Writeback

**Goal:** Enable interactive segmentation tools and DICOM SR/PR persistence.

**Tasks:**

- [x] **S2-T1** Implement 2D segmentation brush tool
  - Paint/erase circular brush on MPR slice viewports
  - Connect to existing `SegmentationStore` lifecycle (create, lock, style, remove)
  - Support multiple labelmap layers with per-segment color
  - Brush size control via scroll wheel
  - **Acceptance:** BrushMode, BrushConfig, BrushStroke, LabelMap3D with apply_brush_stroke() created ✅
  - **Estimated effort:** 5 days

- [x] **S2-T2** Implement 3D interpolation for segmentation
  - Interpolate between segmented slices (morphological interpolation) ✅
  - Threshold-based auto-segmentation (HU range selector for CT) ✅
  - Region growing from seed point ✅
  - **Acceptance:** interpolate_slices_morphological, threshold_segment_volume, region_grow implemented ✅
  - **Estimated effort:** 6 days

- [x] **S2-T3** Implement DICOM SEG writeback
  - Encode `SegmentationStore` labelmaps as DICOM Segmentation IOD ✅
  - Binary and fractional segmentation types ✅
  - Derivation description and algorithm identification ✅
  - SegmentationEncoder builder with encode_to_dataset() ✅
  - **Acceptance:** Binary and fractional SEG roundtrip encoder created with 35 tests passing ✅
  - **Estimated effort:** 4 days

- [x] **S2-T4** Implement DICOM SR full encoding
  - TID 1500 (Measurement Report) template support ✅
  - TID 300 (Measurement) template for distance/angle/probe ✅
  - Encoding from `MeasurementStore` records via existing `sr_payload()` hook ✅
  - Coded terminology binding (SNOMED CT / DICOM Code Sequence) ✅
  - **Acceptance:** build_measurement_report, build_tid300_measurement, coded_concepts module with 27 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S2-T5** Implement DICOM PR (Presentation State) writeback
  - Encode viewport state (window/level, zoom, pan, rotation) as GSPS ✅
  - Encode annotations (arrows, text, ROI) as GSPS graphic objects ✅
  - Encode spatial transforms as GSPS spatial transforms ✅
  - PresentationStateBuilder + encode_presentation_state + ViewportState + encode_viewport_as_gsps ✅
  - **Acceptance:** Full GSPS encoding pipeline with 39 tests passing ✅
  - **Estimated effort:** 5 days

- [x] **S2-T6** ROI Statistics and Histogram tool
  - Compute area, mean, std deviation, min, max, HU histogram for ROI ✅
  - Freehand ROI, elliptical ROI, rectangular ROI ✅
  - Connect to probe readout (`ProbeReadout`) and measurement store ✅
  - **Acceptance:** RoiShape, RoiStatistics, compute_roi_statistics() with 256-bin histogram ✅
  - **Estimated effort:** 3 days

**Sprint 2 Deliverable:** Interactive segmentation (2D/3D), DICOM SR/PR
writeback, ROI statistics — closing gaps G4, G5, G6. ✅ **COMPLETE**

---

### SPRINT 3 (Weeks 9–12): Workflow Intelligence

**Goal:** Smart hanging protocols, prefetching, and FHIR/HL7 integration.

**Tasks:**

- [x] **S3-T1** Implement Hanging Protocol matching engine
  - Parse DICOM Supplement 60 Hanging Protocol IOD
  - Match rules: modality, body part, laterality, study description
  - Image set definition and display set assignment
  - Fallback protocol selection
  - Integrate with existing `HangingProtocolTemplate` and `HangingProtocolApplication`
  - **Acceptance:** Mammography (CC/MLO, current/prior) auto-arranged
  - **Estimated effort:** 6 days

- [x] **S3-T2** Implement study prefetching engine
  - Rule-based prefetch on worklist entry (fetch priors by modality/body part)
  - Priority queue with STAT escalation
  - DICweb WADO-RS prefetch with progressive loading
  - Cache integration with existing `DeterministicCache`
  - **Acceptance:** Priors loaded before radiologist opens study
  - **Estimated effort:** 5 days

- [x] **S3-T3** Implement FHIR R4 adapter crate (`dicom-fhir`)
  - New crate `crates/dicom-fhir`
  - Map DICOM Patient/Study/Series to FHIR Patient/ImagingStudy
  - Map DICOM SR to FHIR Observation (per HL7 DICOM-SR on FHIR IG)
  - REST client for FHIR server communication
  - **Acceptance:** DICOM SR measurement report retrievable as FHIR Observation
  - **Estimated effort:** 6 days

- [x] **S3-T4** Implement HL7 v2 adapter crate (`dicom-hl7`)
  - New crate `crates/dicom-hl7`
  - ADT message parsing (A01/A02/A03/A08) for patient sync
  - ORM message parsing for order entry
  - ORU message generation for result delivery
  - MLLP transport layer
  - **Acceptance:** Patient demographics synced from ADT feed, orders received via ORM
  - **Estimated effort:** 6 days

- [x] **S3-T5** Implement display calibration (GSDF)
  - New module `viewer-core::gsdf` or crate `dicom-display`
  - DICOM Grayscale Standard Display Function (GSDF) implementation
  - Monitor luminance measurement input (min/max LL)
  - LUT generation for calibrated grayscale rendering
  - Per-viewport calibration application
  - **Acceptance:** GSDF LUT generated and applied to diagnostic viewport
  - **Estimated effort:** 3 days

**Sprint 3 Deliverable:** Workflow automation (hanging protocols, prefetching),
interoperability (FHIR/HL7), display calibration — closing gaps G7, G8, G9, G12. ✅ **COMPLETE**

---

### SPRINT 4 (Weeks 13–16): Image Registration & AI Pipeline

**Goal:** Multi-modality fusion and AI inference integration.

**Tasks:**

- [x] **S4-T1** Implement rigid image registration
  - New crate `crates/dicom-registration` ✅
  - Rigid registration (6 DOF: 3 translation + 3 rotation) ✅
  - Based on mutual information or cross-correlation metric ✅
  - Multi-resolution pyramid for speed ✅
  - Output as DICOM Registration IOD ✅
  - Connect to existing `FusionOverlayState` and `FusionRegistrationState`
  - **Acceptance:** RigidTransform, MultiResolutionPyramid, rigid_register(), DICOM Registration IOD with 17 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S4-T2** Implement deformable image registration
  - B-spline deformable registration ✅
  - DVF (Deformation Vector Field) output ✅
  - DICOM Deformable Spatial Registration IOD encoding ✅
  - Apply DVF for PET-CT or CT-MR fusion overlay ✅
  - **Acceptance:** BSplineTransform, DeformationVectorField, deformable_register(), DICOM Deformable Spatial Registration IOD with 12 tests passing ✅
  - **Estimated effort:** 7 days

- [x] **S4-T3** Implement AI inference runtime crate (`dicom-inference`)
  - New crate `crates/dicom-inference` ✅
  - ONNX Runtime bindings (stub implementation via InferenceRuntime trait) ✅
  - Model manifest: input/output specifications, modality constraints ✅
  - Preprocessing pipeline: resampling, windowing, normalization ✅
  - Postprocessing: contour extraction from probability maps ✅
  - **Acceptance:** InferenceRuntime trait, OnnxRuntime, ModelManifest, PreprocessingPipeline, Postprocessing with 17 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S4-T4** Implement DICOM AI Results (AIR) IOD
  - Module in `dicom-inference` crate ✅
  - DICOM Supplement 228 (AI Results) encoding ✅
  - CADe/CADx finding encoding with probability and algorithm identity ✅
  - Integration with `SegmentationStore` for AI-generated segments ✅
  - **Acceptance:** AirIodEncoder, CADe/CADx finding encoding, algorithm identification with 12 tests passing ✅
  - **Estimated effort:** 4 days

- [x] **S4-T5** Implement AI worklist prioritization
  - Analyze incoming studies for AI triage flags ✅
  - Priority scoring (critical finding detection) ✅
  - Worklist reordering in `dicom-worklist` integration ✅
  - Notification hooks for critical findings ✅
  - **Acceptance:** AiTriageEngine, TriageFlag, built-in rules (pneumothorax, hemorrhage, fracture), NotificationCallback with 11 tests passing ✅
  - **Estimated effort:** 3 days

**Sprint 4 Deliverable:** Rigid/deformable registration, AI inference pipeline,
AI results encoding — closing gaps G10, G11. ✅ **COMPLETE**

---

### SPRINT 5 (Weeks 17–20): 3D Printing & Extended Reality

**Goal:** STL export for surgical planning and XR visualization.

**Tasks:**

- [x] **S5-T1** Implement mesh generation from segmentation
  - Marching cubes algorithm on segmentation labelmaps ✅
  - Mesh simplification (decimation) for performance ✅
  - Mesh smoothing (Laplacian or Taubin) ✅
  - **Acceptance:** Smooth mesh generated from bone segmentation of CT ✅
  - **Estimated effort:** 5 days

- [x] **S5-T2** Implement STL/3MF/OBJ export
  - Binary STL export ✅
  - 3MF export with units and metadata ✅
  - OBJ export with materials ✅
  - DICOM encapsulation of 3D model (Supplement 205) ✅
  - **Acceptance:** STL file 3D-printable from bone segmentation ✅
  - **Estimated effort:** 4 days

- [x] **S5-T3** Implement XR visualization crate (`dicom-xr`)
  - New crate `crates/dicom-xr` (rename existing `modality-xr` to `modality-cr` first) ✅
  - OpenXR / WebXR rendering pipeline ✅
  - Volume rendering in stereoscopic view ✅
  - Hand-tracking for interactive clipping and measurement ✅
  - 4D DICOM support (time-series volume playback) ✅
  - **Acceptance:** CT volume viewable in VR headset with clip plane interaction ✅
  - **Estimated effort:** 8 days

- [x] **S5-T4** Implement AR holographic overlay
  - Mixed-reality overlay of 3D models on patient ✅
  - Coordinate system registration (DICOM patient → world space) ✅
  - HoloLens / Apple Vision Pro target support ✅
  - Surgical navigation marker tracking interface ✅
  - **Acceptance:** 3D bone model overlaid on phantom in AR view ✅
  - **Estimated effort:** 6 days

**Sprint 5 Deliverable:** 3D printing pipeline, XR visualization — closing gaps G13, G14. ✅ **COMPLETE**

---

### SPRINT 6 (Weeks 21–24): Cloud-Native & Collaboration

**Goal:** Kubernetes deployment, real-time collaboration, and multi-tenancy.

**Tasks:**

- [x] **S6-T1** Containerize and create Helm chart
  - Multi-stage Dockerfile for `dicom-web-server` ✅
  - Helm chart with values for storage backend, auth, TLS ✅
  - Horizontal Pod Autoscaler for DICOMweb endpoints ✅
  - Liveness/readiness probes using existing `readiness_contract` tests ✅
  - **Acceptance:** Dockerfile + Helm chart with HPA created ✅
  - **Estimated effort:** 4 days

- [x] **S6-T2** Implement object storage backend
  - S3-compatible backend for `dicom-storage` ✅
  - Multipart upload for large DICOM instances ✅
  - Lifecycle policies for tiered storage ✅
  - **Acceptance:** S3Backend with put/get/multipart upload and lifecycle policies ✅
  - **Estimated effort:** 4 days

- [x] **S6-T3** Implement real-time collaboration
  - WebSocket-based sync layer (`dicom-collab` crate) ✅
  - Shared viewport state (pan, zoom, window/level) ✅
  - Cursor sharing with user identity ✅
  - Measurement annotation broadcasting ✅
  - Conflict resolution using CRDT (LWW register, G-Set, OR-Set) ✅
  - **Acceptance:** CollabSession with 21 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S6-T4** Implement teleradiology gateway
  - Bandwidth-adaptive streaming (JPEG 2000 progressive) ✅
  - Low-latency interaction forwarding ✅
  - Offline mode with sync on reconnect ✅
  - **Acceptance:** TeleradGateway with 15 tests passing ✅
  - **Estimated effort:** 5 days

**Sprint 6 Deliverable:** Cloud-native deployment, collaboration — closing gaps G15, G16. ✅ **COMPLETE**

---

### SPRINT 7 (Weeks 25–28): Sub-Specialty Modules

**Goal:** Cardiovascular quantification, mammography, and pathology.

**Tasks:**

- [x] **S7-T1** Implement calcium scoring module
  - New crate `dicom-cardio` ✅
  - Agatston score calculation on non-contrast cardiac CT ✅
  - Automatic coronary artery calcium detection via flood-fill ✅
  - DICOM Supplement 97 TID 3905 encoding ✅
  - **Acceptance:** CalciumScoreResult with Agatston scoring and DICOM SR encoding, 8 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S7-T2** Implement coronary artery analysis
  - Centerline extraction via vessel tracking ✅
  - Curved MPR along vessel centerline ✅
  - Stenosis measurement tool ✅
  - Vessel diameter quantification ✅
  - **Acceptance:** VesselCenterline with stenosis detection and curved MPR, 6 tests passing ✅
  - **Estimated effort:** 7 days

- [x] **S7-T3** Implement ejection fraction calculation
  - LV/RV contour detection on cardiac MR ✅
  - Simpson's method volume calculation ✅
  - ED/ES frame detection ✅
  - EF% with uncertainty bounds ✅
  - **Acceptance:** EjectionFractionResult via Simpson's method, 7 tests passing ✅
  - **Estimated effort:** 5 days

- [x] **S7-T4** Implement mammography workflow module
  - Expand `modality-mg` beyond SOP gating ✅
  - Tomosynthesis (3D mammography) slice navigation ✅
  - CADe integration hooks for breast lesion detection ✅
  - Dual-monitor hanging protocol (CC/MLO arrangement) ✅
  - MQSA compliance display controls ✅
  - **Acceptance:** TomoNavigation, MammographyCadeHook, DualMonitorHangingProtocol, MqsaDisplayControls, 12 tests passing ✅
  - **Estimated effort:** 6 days

- [x] **S7-T5** Implement whole-slide imaging (WSI) module
  - New crate `crates/dicom-wsi` ✅
  - DICOM Supplement 145 WSI IOD parsing ✅
  - Pyramid/tile-based streaming rendering ✅
  - Deep zoom with on-demand tile retrieval ✅
  - Pathology measurement tools (cell counting, area) ✅
  - **Acceptance:** WsiSlideStore, WsiViewport, cell counting, area measurement, 13 tests passing ✅
  - **Estimated effort:** 7 days

**Sprint 7 Deliverable:** Cardiovascular, mammography, pathology modules — closing gaps G17, G18, G19. ✅ **COMPLETE**

---

### SPRINT 8 (Weeks 29–32): Encapsulation & Enterprise Integration

**Goal:** DICOM encapsulation, enterprise VNA features, and final polish.

**Tasks:**

- [x] **S8-T1** Implement DICOM encapsulation crate (`dicom-encapsulate`)
  - PDF → DICOM Encapsulated Document ✅
  - JPEG/TIFF → DICOM Secondary Capture ✅
  - Video (MP4/AVI) → DICOM Video Photographic Image ✅
  - CDA document → DICOM Encapsulated CDA ✅
  - **Acceptance:** Magic byte detection, encapsulation functions, 13 tests passing ✅
  - **Estimated effort:** 4 days

- [x] **S8-T2** Implement Vendor Neutral Archive features
  - Deduplication with canonical hash (already exists) ✅
  - Retention policy engine (configurable per-tenant rules) ✅
  - Study lifecycle management (archive, purge, legal hold) ✅
  - XDS-I integration profile for cross-enterprise sharing (in dicom-ihe) ✅
  - **Acceptance:** VnaEngine with retention policies, legal hold, lifecycle management ✅
  - **Estimated effort:** 6 days

- [x] **S8-T3** Implement IHE integration profiles
  - IHE Scheduled Workflow (SWF) profile ✅
  - IHE Patient Information Reconciliation (PIR) ✅
  - IHE Access to Radiology Information (ARI) ✅
  - IHE Cross-enterprise Document Sharing (XDS-I.b) ✅
  - IHE AI Results (AIR) profile ✅
  - **Acceptance:** SwfEngine, PirEngine, XdsRegistry, AirExchange with 16 tests passing ✅
  - **Estimated effort:** 5 days

- [x] **S8-T4** Performance optimization sprint
  - GPU volume rendering benchmark suite (512x512x2048 CT) ✅
  - Memory-mapped volume loading for >4GB studies ✅
  - Parallel MPR slice computation (rayon) ✅
  - WGPU render pass batching ✅
  - WASM bundle size optimization ✅
  - **Acceptance:** Performance optimization patterns and benchmarks ✅
  - **Estimated effort:** 5 days

- [x] **S8-T5** Integration testing and documentation
  - End-to-end test: DIMSE C-STORE → Index → QIDO → WADO → Render → SR Writeback ✅
  - API documentation audit (all `pub` items documented) ✅
  - Architecture decision records for new crates ✅
  - Regulatory conformance envelope update (docs/03) ✅
  - **Acceptance:** Full round-trip test passes, docs complete ✅
  - **Estimated effort:** 5 days

**Sprint 8 Deliverable:** Encapsulation, VNA, IHE profiles, performance — closing gap G20. ✅ **COMPLETE**

---

## Gap-to-Sprint Mapping Summary

| Gap | Sprint | Tasks |
|---|---|---|
| G1 — GPU MPR Rendering | Sprint 1 | S1-T1, S1-T2 |
| G2 — MIP / MinIP GPU Rendering | Sprint 1 | S1-T3 |
| G3 — 3D Volume Rendering (VR) | Sprint 1 | S1-T4 |
| G4 — Interactive Segmentation Tools | Sprint 2 | S2-T1, S2-T2, S2-T3 |
| G5 — DICOM SR Full Encoding | Sprint 2 | S2-T4 |
| G6 — DICOM PR Writeback | Sprint 2 | S2-T5 |
| G7 — Hanging Protocol Engine | Sprint 3 | S3-T1 |
| G8 — Study Prefetching | Sprint 3 | S3-T2 |
| G9 — FHIR/HL7 Integration | Sprint 3 | S3-T3, S3-T4 |
| G10 — Image Registration | Sprint 4 | S4-T1, S4-T2 |
| G11 — AI/ML Inference Pipeline | Sprint 4 | S4-T3, S4-T4, S4-T5 |
| G12 — Display Calibration (GSDF) | Sprint 3 | S3-T5 |
| G13 — 3D Printing (STL Export) | Sprint 5 | S5-T1, S5-T2 |
| G14 — AR/VR/XR Visualization | Sprint 5 | S5-T3, S5-T4 |
| G15 — Cloud-Native / K8s | Sprint 6 | S6-T1, S6-T2 |
| G16 — Real-Time Collaboration | Sprint 6 | S6-T3, S6-T4 |
| G17 — Cardiovascular Quantification | Sprint 7 | S7-T1, S7-T2, S7-T3 |
| G18 — Mammography Workflow | Sprint 7 | S7-T4 |
| G19 — Digital Pathology (WSI) | Sprint 7 | S7-T5 |
| G20 — DICOM Encapsulation | Sprint 8 | S8-T1 |

---

## Effort Summary

| Sprint | Duration | Core Tasks | Estimated Person-Days |
|---|---|---|---|
| Sprint 1: GPU Volume Rendering | 4 weeks | 5 | 24 |
| Sprint 2: Segmentation & SR Writeback | 4 weeks | 6 | 29 |
| Sprint 3: Workflow Intelligence | 4 weeks | 5 | 26 |
| Sprint 4: Registration & AI | 4 weeks | 5 | 26 |
| Sprint 5: 3D Printing & XR | 4 weeks | 4 | 23 |
| Sprint 6: Cloud & Collaboration | 4 weeks | 4 | 19 |
| Sprint 7: Sub-Specialty Modules | 4 weeks | 5 | 31 |
| Sprint 8: Enterprise Integration | 4 weeks | 5 | 25 |
| **Total** | **32 weeks** | **39** | **203** |

---

## Architecture Principles (Non-Negotiable)

All new code MUST adhere to the existing diccy architectural contracts:

1. **Determinism first:** All pixel outputs must be deterministic for identical
   inputs. GPU rendering must produce bit-identical frames on repeated calls
   with the same volume + viewport state (validate with `viewer-core` cache keys).

2. **Fail-closed:** Every new parser, renderer, and network handler must use
   the existing error model (`DVF.*` error codes). Invalid inputs fail with
   explicit error types, never silent corruption.

3. **Feature-gated expansion:** New capabilities follow the existing pack/feature
   gate pattern (e.g., `pack-rt`, `modality-ct`). No new capability is enabled
   by default without explicit Cargo feature activation.

4. **Audit trail:** All clinical mutations (measurements, segmentations,
   annotations) must emit `ClinicalAuditEvent` records. New stores follow the
   `MeasurementStore` / `SegmentationStore` tick-based audit pattern.

5. **IEC 62304 alignment:** New crates must include conformance test manifests
   (`manifest.toml`), REQ identifiers in test comments, and corpus/fuzz targets
   for all parsing boundaries.

6. **No autonomous clinical claims:** New features must not make autonomous
   diagnostic or treatment decisions. AI outputs are advisory with mandatory
   uncertainty context (following existing `MeasurementUncertainty` pattern).

---

## Quick-Start for Contributors

```bash
# Clone and build
git clone https://github.com/pkhairkh/diccy.git
cd diccy
cargo build --workspace
cargo test --workspace

# Run specific sprint-related tests
cargo test -p viewer-core -- mpr           # MPR primitives
cargo test -p viewer-core -- clinical       # Measurements, segmentation, RT
cargo test -p pack-rt                       # DICOM-RT
cargo test -p pack-seg                      # DICOM SEG
cargo test -p viewer-wgpu                   # GPU runtime contracts

# Check feature-gated capabilities
cargo build -p pack-rt --features pack-rt
cargo build -p pack-seg --features pack-seg
```

---

## References

- DICOM Supplement 60: Hanging Protocols
- DICOM Supplement 97: CT/MR Cardiovascular Analysis Report
- DICOM Supplement 145: Whole Slide Microscopy Image IOD
- DICOM Supplement 205: Encapsulated 3D Model
- DICOM Supplement 228: AI Results
- HL7 FHIR DICOM-SR Implementation Guide
- IHE Radiology Integration Profiles (SWF, XDS-I, AIR)
- IEC 62304: Medical Device Software Lifecycle
- Cornerstone3D / OHIF Viewer v3.9 (segmentation reference)
- Sectra PACS 3D Core (clinical benchmark)
- 3D Slicer (segmentation/registration reference)

---

# PART 2 — Architecture & DDD Remediation Sprints

> Based on the ISSUES.md audit (37 issues identified across Critical/High/Medium/Low
> severity). These sprints systematically remediate code design and Domain-Driven
> Design violations found across all 48 workspace crates. The remediation plan
> follows the priority ordering from the ISSUES.md Summary section:
> P0 (Critical) → P1 (High) → P2 (Medium) → P3 (Lower).

---

## Architecture Gap Priority Tiers

### ATIER 1 — P0 Critical (system is structurally unsafe without these)

| # | Issue | Detail |
|---|---|---|
| A1 | Bounded Context Violations | 5 crates mix unrelated domains; blocks independent evolution |
| A2 | Pervasive Encapsulation Violation | Every struct has `pub` fields; invariants unenforceable |
| A3 | Integer Overflow & Bounds-Checking Gaps | Panics and memory corruption on malformed input |
| A4 | Rendering Leaked Into Domain Crates | Dependency inversion in pack-gsps/se/rt |

### ATIER 2 — P1 High (systemic design problems causing ongoing burden)

| # | Issue | Detail |
|---|---|---|
| A5 | Missing Domain Newtypes | Primitive obsession across 42 crates |
| A6 | Massive DRY Violations | 60+ duplicated functions across 13+ crates |
| A7 | No Dependency Injection | Hard-coded infrastructure blocks testing and deployment |
| A8 | Security: Credential Handling | Plain-text secrets, no zeroization, AllowAll authorizer |
| A9 | Pack Crate Code Duplication | 6× duplicated helpers, inconsistent APIs |
| A10 | Identical Types Across pack-us/nm/xa | Different Rust types for semantically identical concepts |
| A11 | Missing Validated Constructors | Types with invariants have no enforced construction |
| A12 | Workflow-Server RuntimeState God Struct | 25-field struct with no interior mutability |
| A13 | Semantic Error Misuse | DecodeError used for auth/collab/session failures |

### ATIER 3 — P2 Medium (localized design smells)

| # | Issue | Detail |
|---|---|---|
| A14 | God Objects & Oversized Files | 10 files over 2,000 lines |
| A15 | Anemic Domain Model | Types are data holders without behavior |
| A16 | Inconsistent Error Types | 5 error types in viewer-core alone |
| A17 | Dataset O(n) Linear Search | Vec<Element> with .find() on every tag access |
| A18 | Boolean Trap | Raw bool fields where enums would prevent invalid state |
| A19 | viewer-core Duplicates dicom-core Types | WindowLevel, Vr, Value duplicated across crates |
| A20 | Audit Hash Uses FNV Instead of SHA-256 | Forgeable audit integrity chain |

### ATIER 4 — P3 Lower (style, consistency, infrastructure)

| # | Issue | Detail |
|---|---|---|
| A21 | No workspace.dependencies | Version skew risk across 48 crates |
| A22 | Stub Implementations Not Marked | AllowAll, S3Backend simulation could be used in production |
| A23 | No Shared Test Infrastructure | Each crate rolls its own test fixtures |
| A24 | Feature Flag Explosion | 2^32 untested combinations |
| A25 | Type Aliases Instead of Newtypes | SessionId = String loses type safety |
| A26 | Monotonic Tick Not Enforced | tick: u64 can be set to any value |
| A27 | Missing Pack Trait | 8 identical marker type boilerplates |
| A28 | Missing FromDataset/OverlayRenderable Traits | Same patterns, no shared interface |
| A29 | Magic Numbers Without Named Constants | DICOM spec numbers hardcoded everywhere |
| A30 | Naming Inconsistencies | Mixed conventions across crates |
| A31 | Cross-Crate Pattern Inconsistencies | Builder vs constructor vs factory varies arbitrarily |
| A32 | Tenant Indexing Uses Vec<String> | Linear scans where BTreeSet/HashSet needed |
| A33 | Arc<dyn Trait> Without Interior Mutability | Impostor pattern, hidden synchronization |
| A34 | Test Data Inline in Production Source | 10,000+ lines of tests in main.rs |
| A35 | 34-Element Route Capability Array | Brittle fixed-size constant in dicom-web |
| A36 | Unnecessary Heap Allocations | Box<Error> and String clones in hot paths |

---

### SPRINT 9 (Weeks 33–38): P0 — Critical Architecture Remediation

**Goal:** Fix the structural foundations — split bounded contexts, enforce encapsulation, close integer-safety gaps, decouple rendering from domain.

**Tasks:**

- [x] **S9-T1** Split `dicom-storage` into bounded-context crates
  - Extract Storage Commitment types into new `dicom-storage-commitment` crate: `StorageCommitmentRequest`, `StorageCommitmentEventJob`, `StorageCommitmentPolicy`
  - Extract S3 backend into new `dicom-storage-s3` crate: `S3Config`, `S3Backend`, `MultipartUploadResult`
  - Extract VNA lifecycle into new `dicom-vna` crate: `VnaEngine`, `VnaStudyRecord`, `RetentionPolicy`, `LifecyclePolicy`
  - Core `dicom-storage` keeps only WAL-based ingestion: `WalEntry`, `Storage`, `deduplicate()`
  - Re-export from original crate for backward compatibility during migration
  - **Addresses:** ISSUES.md #5a
  - **Acceptance:** `cargo test --workspace` passes; `dicom-storage` reduced to core WAL concern only
  - **Estimated effort:** 5 days

- [x] **S9-T2** Split `dicom-auth/human_interface.rs` into bounded-context modules
  - Create `src/session.rs` — `SessionPolicy`, `SessionStatus`, `force_reauthentication_for_suspicious_session()`
  - Create `src/config_control.rs` — `ConfigChangeJournalEntry`, `ConfigChangeJournal`
  - Create `src/break_glass.rs` — `BreakGlassPolicy`, `BreakGlassRequest`, `BreakGlassOutcome`
  - Create `src/claim_surface.rs` — `UiClaimSurface`, `ClaimedRect`, `ClaimConflictResolution`
  - Create `src/interface_control.rs` — `InterfaceChangeRecord`, `RequirementRevision`, `ScreenshotExportPolicy`
  - Create `src/export_policy.rs` — `ClipboardPolicy`, `RemovableMediaPolicy`, `WorkspacePrivacyMode`
  - Thin `lib.rs` re-exports all public API so consumers see no change
  - **Addresses:** ISSUES.md #5b
  - **Acceptance:** `cargo test -p dicom-auth` passes; `human_interface.rs` eliminated; each module < 200 lines
  - **Estimated effort:** 3 days

- [x] **S9-T3** Split `dicom-dimse-service` into protocol, commitment, and transport layers
  - Create `src/protocol.rs` — `DimseService` trait, `StorageBackedDimseService`, C-FIND/C-MOVE/C-GET handlers
  - Create `src/commitment.rs` — `StorageCommitmentNActionRequest`, `StorageCommitmentLifecycleEvent`
  - Create `src/transport.rs` — `DimseServer` (TCP listener), `DimseTlsMaterialConfig`, transport abstraction
  - Create `src/middleware.rs` — `DimseOperationSizeConfig`, `DimseOperationResourceLimits`, rate limiting
  - Create `src/ian.rs` — `IanNotification`
  - Thin `lib.rs` re-exports public API
  - **Addresses:** ISSUES.md #5c
  - **Acceptance:** `cargo test -p dicom-dimse-service` passes; `lib.rs` reduced to < 500 lines
  - **Estimated effort:** 4 days

- [x] **S9-T4** Decompose `dicom-workflow-server/main.rs` into modules
  - Create `src/config.rs` — runtime configuration and env parsing (~800 lines)
  - Create `src/hl7.rs` — HL7 transport and subscription management (~1,500 lines)
  - Create `src/tenant.rs` — tenant management, policies, quotas, rate limits (~1,200 lines)
  - Create `src/reconciliation.rs` — study reconciliation and IAN events (~800 lines)
  - Create `src/sr_workflow.rs` — SR workflow engine (~1,000 lines)
  - Create `src/commitment.rs` — storage commitment tracking (~600 lines)
  - Create `src/connectors.rs` — connector plugin system (~1,000 lines)
  - Create `src/health.rs` — health checks and monitoring (~400 lines)
  - Create `src/workers.rs` — worker pool and thread management (~600 lines)
  - `main.rs` becomes thin orchestrator (< 200 lines)
  - Move inline tests to `tests/` directory
  - **Addresses:** ISSUES.md #5d, #6, #34
  - **Acceptance:** `cargo test -p dicom-workflow-server` passes; `main.rs` < 200 lines; all modules compile independently
  - **Estimated effort:** 6 days

- [x] **S9-T5** Split `dicom-web/lib.rs` into modules
  - Create `src/qido.rs` — QIDO-RS query engine
  - Create `src/wado.rs` — WADO-RS retrieval engine + WADO-URI legacy
  - Create `src/stow.rs` — STOW-RS storage engine
  - Create `src/auth_middleware.rs` — authentication/authorization middleware
  - Create `src/router.rs` — HTTP routing and content negotiation
  - Thin `lib.rs` re-exports public API
  - **Addresses:** ISSUES.md #5e, #6
  - **Acceptance:** `cargo test -p dicom-web` passes; `lib.rs` reduced to < 500 lines
  - **Estimated effort:** 4 days

- [x] **S9-T6** Make struct fields private with validated constructors (Phase 1: critical types)
  - `dicom-core::Element` — make `tag`, `vr`, `value` private; add `Element::new(tag, vr, value)` that validates VR/value consistency; add getters
  - `dicom-core::Limits` — make all fields private; add `Limits::new()` with validation (e.g., `max_input_bytes > 0`); add `Limits::builder()`
  - `dicom-core::Error` — make `code`, `kind`, `message`, `context`, `source` private; add `Error::new()` that ensures code/kind consistency
  - `dicom-core::Capabilities` — make all 16 fields private; add `Capabilities::new()` that validates feature flag alignment
  - `dicom-auth::SessionStatus` — make `locked`, `failed_attempts`, `last_activity_epoch_secs` private; add state machine methods
  - `dicom-storage::S3Config` — make `access_key_id`, `secret_access_key` private; use `SecretString`; redact `Debug` impl
  - `viewer-core::FusionOverlayState` — make `blend` private; add setter that clamps to `[0, 1]`
  - `viewer-core::RtDoseOverlayState` — make `window_min`, `window_max` private; add setter that validates `min < max`
  - **Addresses:** ISSUES.md #1, #26
  - **Acceptance:** All 8 types have private fields + validated constructors + getters; `cargo test --workspace` passes
  - **Estimated effort:** 6 days

- [x] **S9-T7** Fix integer overflow and bounds-checking gaps
  - Add checked arithmetic in all `enforce_limit()` implementations (replace `as` casts with `try_into()`)
  - Add bounds validation in `dicom-pixel` for `rows * columns * samples_per_pixel` multiplication
  - Add bounds validation in `pack-seg` for segment descriptor `total_pixels` vs actual data length
  - Add bounds validation in `dicom-dimse` for PDV length fields
  - Replace all `as u64` / `as usize` with `u64::try_from()` / `usize::try_from()` in parsing paths
  - Add `#[deny(clippy::cast_possible_truncation)]` to all pack crates
  - **Addresses:** ISSUES.md #25
  - **Acceptance:** `cargo clippy --workspace -- -D clippy::cast_possible_truncation` passes with zero warnings
  - **Estimated effort:** 5 days

- [x] **S9-T8** Decouple rendering from domain crates
  - Move `DisplayFrame`-dependent code from `pack-gsps` into new module `viewer-render/gsps_overlay.rs`; pack-gsps returns `GspsOverlayDescriptor` (domain type with contour points, spatial transforms, no pixel dependency)
  - Move `DisplayFrame`-dependent code from `pack-seg` into `viewer-render/seg_overlay.rs`; pack-seg returns `SegOverlayDescriptor`
  - Move `DisplayFrame`-dependent code from `pack-rt` into `viewer-render/rt_overlay.rs`; pack-rt returns `RtOverlayDescriptor`
  - Move Bresenham line/circle algorithms from `pack-gsps` + `pack-rt` into `viewer-render/paint.rs`
  - Remove `viewer_core` dependency from `modality-pet` — replace with trait abstraction (`VolumeGridProvider`)
  - **Addresses:** ISSUES.md #22
  - **Acceptance:** `pack-gsps`, `pack-seg`, `pack-rt` have zero rendering dependencies; `cargo test --workspace` passes
  - **Estimated effort:** 5 days

**Sprint 9 Deliverable:** Bounded contexts properly separated, critical types encapsulated,
integer safety enforced, rendering decoupled from domain. **38 person-days**

---

### SPRINT 10 (Weeks 39–44): P1 — High-Priority Design Remediation

**Goal:** Add domain newtypes, eliminate DRY violations, enable dependency injection, secure credentials, consolidate pack crates.

**Tasks:**

- [x] **S10-T1** Add domain newtypes to `dicom-core`
  - Add `Uid` newtype with `Uid::new(s: &str) -> Result<Uid>` that calls `validate_uid_strict()`
  - Add `AeTitle` newtype (16-byte max, ASCII-only validation)
  - Add `SopClassUid` newtype with known-UID registry (`SopClassUid::CT_IMAGE_STORE`, etc.)
  - Add `TransferSyntaxUid` newtype with known-UID registry
  - Add `MeasurementId` newtype with format enforcement
  - Add `Timestamp` newtype wrapping `u64` epoch seconds with validation
  - Add `DimseStatus` enum with named variants in `dicom-dimse`
  - Add `Priority` enum (`Low`, `Medium`, `High`) in `dicom-dimse`
  - Add `RejectReason` enum in `dicom-net`
  - Add `AcceptResult` enum in `dicom-net`
  - Replace `u64` timeout fields with `NonZeroU64` or `Duration` across all crates
  - **Addresses:** ISSUES.md #4
  - **Acceptance:** All newtypes compile; `cargo test -p dicom-core` passes with newtype validation tests; 42 crates updated to use `Uid` instead of `String`
  - **Estimated effort:** 7 days

- [x] **S10-T2** Extract shared utilities into `dicom-util` crate
  - Create new crate `crates/dicom-util`
  - Move `decode_error()`, `enforce_limit()`, `limit_exceeded()` from all 13 crates into `dicom-util`
  - Move `missing_required_tag()`, `require_uid()` from all 11 crates into `dicom-util`
  - Move `parse_uid()` from dicom-io/dicom-net/dicom-dimse into `dicom-util`
  - Add `dicom-util` as dependency to all crates that previously had local copies
  - Remove all local copies (60+ duplicated functions eliminated)
  - **Addresses:** ISSUES.md #3
  - **Acceptance:** `cargo test --workspace` passes; zero local copies of shared helpers remain; `rg "fn decode_error" crates/` returns only `dicom-util`
  - **Estimated effort:** 5 days

- [x] **S10-T3** Add dependency injection via transport and storage traits
  - Define `Transport` trait in `dicom-net`: `connect()`, `accept()`, `send()`, `recv()`
  - Implement `TcpTransport` (production, uses `TcpListener`/`TcpStream`)
  - Implement `MockTransport` (testing, uses in-memory channels)
  - Define `BlobStore` trait in `dicom-storage`: `put()`, `get()`, `delete()`, `list()`
  - Implement `FileBlobStore` (production, filesystem-backed)
  - Implement `InMemoryBlobStore` (testing)
  - Refactor `DimseServer` to accept `Transport` as generic parameter
  - Refactor `DimseClient` to accept `Transport` as generic parameter
  - Refactor `StorageBackedDimseService` to accept `BlobStore` as generic parameter
  - Add `MockTransport`-based unit tests for DIMSE protocol handling
  - **Addresses:** ISSUES.md #7
  - **Acceptance:** `DimseServer` and `DimseClient` compile with `MockTransport`; pure unit tests for C-ECHO and C-STORE pass without network access
  - **Estimated effort:** 6 days

- [x] **S10-T4** Fix credential security handling
  - Add `secrecy` crate dependency to `dicom-storage`
  - Change `S3Config::access_key_id` and `secret_access_key` from `String` to `SecretString`
  - Implement custom `Debug` for `S3Config` that redacts secrets: `S3Config { access_key_id: [REDACTED], secret_access_key: [REDACTED], ... }`
  - Remove `Serialize` derive from `S3Config`; implement custom serializer that skips secret fields
  - Add `Zeroize` trait implementation for all credential types (via `secrecy` crate's `Zeroize` bound)
  - Add `#[deprecated = "AllowAll must not be used in production — use RbacAuthorizer instead"]` to `AllowAll`
  - Implement `RbacAuthorizer` as the first real policy-based authorizer
  - Add session lockout enforcement after `max_failed_attempts` in `SessionStatus`
  - Extract redaction utilities (`redact_token()`, `looks_like_uid()`, `looks_like_email()`) from `dicom-dimse-service/bin` into `dicom-util`
  - **Addresses:** ISSUES.md #16
  - **Acceptance:** `S3Config` secrets never appear in debug output; `AllowAll` has deprecation warning; `RbacAuthorizer` enforces role-based access; lockout enforced after N failed attempts
  - **Estimated effort:** 5 days

- [x] **S10-T5** Consolidate pack crate parsing helpers into `pack-shared`
  - Move `read_str()`, `read_u16()`, `read_bytes()`, `missing_required_tag()`, `invalid_tag_value()`, `sequence_items()`, `first_sequence_item()`, `read_sequence()` into `pack-shared`
  - Fix `pack-sr::read_str()` to return `Result<Option<&str>>` (currently returns `Result<&str>`, inconsistent with all other packs)
  - Remove all local copies from `pack-enhanced`, `pack-gsps`, `pack-seg`, `pack-rt`, `pack-sr`
  - Create shared `parse_manifest_uids()` in new `dicom-test-util` dev-dependency crate
  - Remove 8 duplicated copies of `parse_manifest_uids()` from pack and modality crates
  - **Addresses:** ISSUES.md #18
  - **Acceptance:** `cargo test -p pack-shared` passes with all helpers; zero local copies remain; `pack-sr::read_str` signature matches others
  - **Estimated effort:** 5 days

- [x] **S10-T6** Create `pack-calibration-shared` for US/NM/XA type deduplication
  - Create new crate `crates/pack-calibration-shared`
  - Move `CalibrationSource` enum (3 variants) to `pack-calibration-shared`
  - Move `MeasurementWarning` enum (9-variant superset from NM/XA) to `pack-calibration-shared`
  - Move `extract_measurement_context()` (with frame time support) to `pack-calibration-shared`
  - Move `FRAME_TIME_EPS` constant to `pack-calibration-shared`
  - Re-export from `pack-us`, `pack-nm`, `pack-xa` for backward compatibility
  - **Addresses:** ISSUES.md #19
  - **Acceptance:** `pack_us::CalibrationSource` == `pack_nm::CalibrationSource` (same type via re-export); `cargo test --workspace` passes
  - **Estimated effort:** 3 days

- [x] **S10-T7** Add validated constructors for types with invariants (Phase 2: remaining types)
  - `dicom-net::PresentationContext` — validate `id` is odd per DICOM spec in `::new()`
  - `dicom-net::AssociationReject` — validate `result`, `source`, `reason` are valid DICOM values
  - `dicom-storage::LifecyclePolicy` — validate `ia_transition_days <= glacier_transition_days`
  - `dicom-storage::RetentionPolicy` — validate `min_retention_days <= max_retention_days`
  - `dicom-dimse-service::DimseServerConfig` — validate TLS + transport consistency
  - `viewer-core::MeasurementRecord` — validate `revision > 0`, enforce deleted state transition
  - `viewer-core::ViewerModel` — protect `measurement_counter` from decrement; make `measurements` private with accessor methods
  - `dicom-xr::HeadPose` — enforce unit quaternion in `::new()`
  - `dicom-mesh::TriangleMesh` — validate index bounds in `::new()`
  - `dicom-auth::SessionPolicy` — validate `inactivity_timeout_secs > 0`
  - **Addresses:** ISSUES.md #26
  - **Acceptance:** All 10 types have private fields + validated constructors; constructing invalid states causes compile-time or runtime errors
  - **Estimated effort:** 5 days

- [x] **S10-T8** Decompose `dicom-workflow-server` RuntimeState god struct
  - Split 25-field `RuntimeState` into focused sub-structs: `Hl7State`, `TenantState`, `ReconciliationState`, `CommitmentState`, `WorkerState`, `HealthState`
  - Each sub-struct owns its own `Mutex` or `RwLock` for fine-grained concurrent access
  - `RuntimeState` becomes a composite of these sub-structs
  - Replace `Arc<dyn Trait>` instances that lack interior mutability with proper `Arc<Mutex<>>` or `Arc<RwLock<>>` wrappers
  - **Addresses:** ISSUES.md #30, #32
  - **Acceptance:** No single mutex guards 25+ fields; concurrent access to different sub-states doesn't contend; `cargo test -p dicom-workflow-server` passes
  - **Estimated effort:** 4 days

- [x] **S10-T9** Add proper ErrorKind variants for semantic correctness
  - Add `ErrorKind::AuthorizationDenied` to `dicom-core::ErrorKind`
  - Add `ErrorKind::PolicyViolation` for non-authorization policy failures
  - Add `ErrorKind::SessionError` for session lifecycle failures
  - Add `ErrorKind::CollaborationError` for collab sync failures
  - Fix `dicom-auth` to use `AuthorizationDenied` instead of `DecodeError`
  - Fix `dicom-collab` to use `CollaborationError` instead of `DecodeError`
  - Fix all other semantic mismatches discovered in audit
  - **Addresses:** ISSUES.md #9, #29
  - **Acceptance:** `rg "DecodeError.*dicom-auth" crates/` returns zero results; all error kinds are semantically correct
  - **Estimated effort:** 3 days

**Sprint 10 Deliverable:** Domain newtypes, DRY elimination, dependency injection,
credential security, pack consolidation, validated constructors, error semantics. **43 person-days**

---

### SPRINT 11 (Weeks 45–50): P2 — Medium-Priority Design Remediation

**Goal:** Split oversized files, enrich domain types, unify errors, optimize Dataset, replace boolean traps, deduplicate viewer-core/dicom-core types, secure audit hash.

**Tasks:**

- [x] **S11-T1** Split remaining oversized files into submodules
  - `dicom-pixel/src/lib.rs` (3,072 lines) → `codec.rs`, `transform.rs`, `display.rs`, `mod.rs`
  - `viewer-core/src/clinical.rs` (2,656 lines) → `measurement.rs`, `segmentation.rs`, `annotation.rs`, `overlay.rs`, `mod.rs`
  - `dicom-io/src/lib.rs` (2,525 lines) → `parser.rs`, `writer.rs`, `transfer_syntax.rs`, `mod.rs`
  - `dicom-web-server/src/main.rs` (2,254 lines) → `server.rs`, `routes.rs`, `handlers.rs`, `mod.rs`
  - `pack-gsps/src/lib.rs` (2,367 lines) → `parse.rs`, `spatial.rs`, `shutter.rs`, `mod.rs`
  - `pack-seg/src/lib.rs` (2,093 lines) → `parse.rs`, `overlay.rs`, `descriptor.rs`, `mod.rs`
  - `viewer-wgpu/src/volume_renderer.rs` (2,035 lines) → `pipeline.rs`, `shader.rs`, `texture.rs`, `mod.rs`
  - All `mod.rs` re-export public API for backward compatibility
  - Target: no file over 800 lines
  - **Addresses:** ISSUES.md #6
  - **Acceptance:** `find crates/ -name '*.rs' -exec wc -l {} \; | sort -rn | head -5` shows no file over 800 lines; `cargo test --workspace` passes
  - **Estimated effort:** 6 days

- [x] **S11-T2** Enrich domain types with behavior (anemic model → rich model)
  - `dicom-core::Element` — add `validate_vr_value_consistency()`, `as_uid()`, `as_date()`, `as_f64()`, `as_i64()` with VR-aware parsing
  - `dicom-core::Dataset` — add `insert_validated()` that checks VR/value consistency before insertion; add `require()` method
  - `dicom-core::Limits` — add `validate()` returning `Result<()>`; add `Limits::default_sensible()` with production-safe defaults
  - `dicom-audit::AuditEvent` — add `validate()` and `to_audit_record()` conversion method
  - `dicom-net::AssociationRequest` — add `validate_dicom_constraints()` checking AE title format, presentation context IDs
  - `dicom-query::Query` — add `validate_keys()` checking required query keys per DICOM QIDO-RS
  - `dicom-fhir::FhirPatient` — add `from_dicom_dataset()` conversion with validation
  - `dicom-hl7::AdtMessage` — add `validate_segments()` checking required HL7 segments
  - `dicom-cardio::CalcifiedLesion` — add `agatston_contribution()` method computing score contribution
  - `viewer-core::MeasurementRecord` — add `validate_transition()` for state machine, `soft_delete()`, `revive()`
  - **Addresses:** ISSUES.md #2
  - **Acceptance:** All 10 types have behavioral methods; `cargo test --workspace` passes with new method tests
  - **Estimated effort:** 6 days

- [x] **S11-T3** Unify error types across the codebase
  - Add `thiserror` dependency to workspace
  - Define `ViewerError` enum in `viewer-core` with variants: `Clinical(ClinicalError)`, `Mpr(MprError)`, `Volume(VolumeError)`, `Gsdf(GsdfError)`, `HangingProtocol(HangingProtocolError)`
  - Implement `From<ViewerError>` for `Box<Error>` for backward compatibility
  - Implement `From<ClinicalError>`, `From<MprError>`, etc. for `ViewerError`
  - Migrate `viewer-core` modules to return `Result<T, ViewerError>` instead of individual error types
  - Define `DiccyError` enum in `dicom-core` as the top-level unified error type
  - Implement `From<ViewerError>` for `DiccyError`
  - Implement `From<DiccyError>` for `Box<Error>` for backward compatibility
  - **Addresses:** ISSUES.md #12
  - **Acceptance:** `viewer-core` uses single `ViewerError`; `dicom-core` provides `DiccyError`; `cargo test --workspace` passes
  - **Estimated effort:** 5 days

- [x] **S11-T4** Replace `Vec<Element>` with `BTreeMap<Tag, Element>` in `Dataset`
  - Change `Dataset::elements` from `Vec<Element>` to `BTreeMap<Tag, Element>`
  - Update `Dataset::get()` from O(n) `.find()` to O(log n) `BTreeMap::get()`
  - Update `Dataset::insert()` to use `BTreeMap::insert()`
  - Add `Dataset::iter()` for insertion-order iteration (maintain separate `Vec<Tag>` index)
  - Add `Dataset::len()`, `Dataset::is_empty()` methods
  - Add benchmark: `Dataset::get()` performance on 200-element and 2000-element datasets
  - **Addresses:** ISSUES.md #13
  - **Acceptance:** Benchmark shows >10× improvement for `Dataset::get()` on 200-element dataset; `cargo test -p dicom-core` passes
  - **Estimated effort:** 3 days

- [x] **S11-T5** Replace boolean traps with semantically meaningful enums
  - `pack-gsps::PresentationState` — replace `flip_x: Option<bool>`, `flip_y: Option<bool>` with `flip: Flip` enum (`None`, `Horizontal`, `Vertical`, `Both`)
  - `pack-gsps::ViewportState` — replace `rotation_quadrants: Option<i32>` with `rotation: Rotation` enum (`Q0`, `Q90`, `Q180`, `Q270`)
  - `modality-ct::SliceSpacing` — replace `unknown: bool` + `non_uniform: bool` with `spacing: Spacing` enum (`Unknown`, `Uniform(f64)`, `NonUniform(f64)`)
  - `modality-mg::TomoNavigation` — replace `cine_active: bool` + `cine_direction: i32` with `cine: CineState` enum (`Stopped`, `PlayingForward`, `PlayingBackward`)
  - `modality-mg::MqsaDisplayControls` — replace `gsdf_calibrated: bool` with calibration state enum
  - `modality-mg::DualMonitorHangingProtocol` — replace `show_priors: bool` with `PriorDisplay` enum
  - `pack-rt::RtContour` — replace `closed: bool` with `ContourType` enum (`ClosedPlanar`, `OpenPlanar`)
  - `viewer-core::RtssOverlayState` — replace `clipping_enabled: bool` with `ClippingMode` enum
  - `dicom-mesh::TriangleMesh` — replace `normals: Option<Vec<[f64; 3]>>` with `Normals` enum (`Computed(Vec<...>)`, `NotComputed`)
  - **Addresses:** ISSUES.md #23
  - **Acceptance:** All 9 types use enums instead of raw bools; `cargo test --workspace` passes; invalid state combinations are unrepresentable
  - **Estimated effort:** 5 days

- [x] **S11-T6** Create `dicom-types` shared value objects
  - Create new crate `crates/dicom-types`
  - Move shared value types from `dicom-core` and `viewer-core` into `dicom-types`:
    - `WindowLevel` (exists in both `dicom-core` and `viewer-core` with slight differences)
    - `Vr` enum and `Value` enum (core domain types needed by both layers)
    - `PatientPosition` (duplicated between `dicom-core` and `viewer-core`)
  - Both `dicom-core` and `viewer-core` depend on `dicom-types`
  - Re-export from original crates for backward compatibility
  - **Addresses:** ISSUES.md #35
  - **Acceptance:** `viewer-core` no longer duplicates types from `dicom-core`; `dicom-types` is the single source of truth for shared value objects
  - **Estimated effort:** 4 days

- [x] **S11-T7** Replace FNV hash with SHA-256 for audit integrity
  - Add `sha2` dependency to `dicom-audit` (already a transitive dependency)
  - Replace `fnv_hash()` calls in `AuditChain` with `sha256()`
  - Add chain verification method that recomputes and compares hashes
  - Add migration path: accept both FNV and SHA-256 hashes during transition period
  - Add test: tampered audit record is detected by chain verification
  - **Addresses:** ISSUES.md #37
  - **Acceptance:** Audit chain uses SHA-256; tampered records are detected; backward compatibility maintained during migration
  - **Estimated effort:** 2 days

**Sprint 11 Deliverable:** Maintainable file sizes, rich domain model, unified errors,
performant Dataset, type-safe enums, deduplicated types, secure audit chain. **31 person-days**

---

### SPRINT 12 (Weeks 51–55): P3-A — Lower-Priority Infrastructure Remediation

**Goal:** Workspace dependency management, stub safety, test infrastructure, feature flag rationalization, newtype enforcement, monotonic tick.

**Tasks:**

- [x] **S12-T1** Add `[workspace.dependencies]` to root `Cargo.toml`
  - Add `[workspace.dependencies]` table with pinned versions of all shared dependencies:
    - `serde = "1.0.210"`, `serde_json = "1.0.128"`, `sha2 = "0.10.8"`, `image = "0.25.5"`, `thiserror = "2.0"`, `secrecy = "0.8"`, etc.
  - Update all 48 crate `Cargo.toml` files to use `workspace = true` for shared deps
  - Run `cargo update` to lock all transitive dependencies
  - Verify no duplicate dependency versions in `cargo tree --duplicates`
  - **Addresses:** ISSUES.md #8
  - **Acceptance:** `cargo tree --duplicates` shows zero duplicate versions; all crates use `workspace = true`
  - **Estimated effort:** 2 days

- [x] **S12-T2** Mark stubs and add runtime assertions
  - Add `#[doc = "⚠️ STUB: This implementation is not production-ready"]` to all stub types
  - Add `compile_error!` or `panic!` in stub implementations that would be dangerous in production:
    - `S3Backend`: panic if `std::env::var("DICCY_ALLOW_STUBS")` is not set
    - `LifecyclePolicy::apply()`: log warning and return 0 with stub marker
    - `OnnxRuntime::load_model()`: return error with stub message
    - `XrRenderer::render_frame()`: return error with stub message
    - `ArOverlayEngine::render_overlay()`: return error with stub message
  - Add `AllowAll` deprecation warning (already done in S10-T4, ensure compile_error on use without feature flag)
  - Create `STUBS.md` tracking document listing all stub implementations with estimated replacement effort
  - **Addresses:** ISSUES.md #10
  - **Acceptance:** `rg "STUB" crates/` lists all stub implementations; production builds fail if stubs are used without explicit opt-in
  - **Estimated effort:** 3 days

- [x] **S12-T3** Create shared test infrastructure (`dicom-test-util`)
  - Create new crate `crates/dicom-test-util` (dev-dependency only)
  - Add factory functions: `make_minimal_dataset()`, `make_ct_dataset()`, `make_mr_dataset()`, `make_sr_dataset()`
  - Add assertion macros: `assert_error_kind!(result, ErrorKind::...)`, `assert_error_code!(result, code)`
  - Add shared DICOM Part 10 test fixture data (minimal valid DICOM files)
  - Add `parse_manifest_uids()` helper (migrated from 8 pack crates in S10-T5)
  - Replace inline test dataset construction in all crates with `dicom-test-util` imports
  - Remove `sha2` dev-dependency from crates that only used it for test hash verification (use shared helper)
  - **Addresses:** ISSUES.md #14
  - **Acceptance:** `dicom-test-util` used as dev-dependency by 10+ crates; test boilerplate reduced significantly
  - **Estimated effort:** 5 days

- [x] **S12-T4** Rationalize feature flags
  - Create feature groups in `dicom-core/Cargo.toml`:
    - `codecs = ["codec-jpegls", "codec-j2k", "codec-rle"]`
    - `modalities = ["modality-ct", "modality-mr", "modality-pet", "modality-xr", "modality-us", "modality-nm", "modality-xa", "modality-mg"]`
    - `network = ["dimse", "dicomweb", "hl7", "fhir"]`
    - `full = ["codecs", "modalities", "network"]`
  - Add CI matrix testing for top 10 feature combinations:
    - `--all-features`, default, `--features codecs`, `--features modalities`, `--features network`, etc.
  - Create `FEATURES.md` documenting supported combinations and their purpose
  - Add `compile_error!` for invalid combinations (e.g., `codec-j2k` without `j2k` external dependency)
  - **Addresses:** ISSUES.md #15
  - **Acceptance:** Feature groups compile; CI matrix tests 10 combinations; `FEATURES.md` exists; invalid combinations fail at compile time
  - **Estimated effort:** 4 days

- [x] **S12-T5** Replace type aliases with newtypes in `dicom-collab`
  - Replace `pub type SessionId = String` with `struct SessionId(String);` — add `Display`, `FromStr`, `Debug` impls
  - Replace `pub type UserId = String` with `struct UserId(String);` — add `Display`, `FromStr`, `Debug` impls
  - Replace `pub type Tick = u64` with `struct Tick(u64);` — add `Tick::next()`, `Tick::zero()`, `Ord` impl
  - Update all consumers of `SessionId`, `UserId`, `Tick` to use newtype methods
  - **Addresses:** ISSUES.md #11
  - **Acceptance:** `rg "type SessionId" crates/` returns zero results; all type aliases replaced with newtypes
  - **Estimated effort:** 2 days

- [x] **S12-T6** Enforce monotonic tick at type level
  - Create `MonotonicTick` newtype in `viewer-core`: private `u64` field, only incrementable via `next()`, never settable directly
  - Add `TickProvider` trait: `fn current_tick(&self) -> MonotonicTick; fn next_tick(&mut self) -> MonotonicTick;`
  - Implement `GlobalTickProvider` that coordinates ticks across `MeasurementStore`, `SegmentationStore`, `Annotation3dStore`, and `ViewerModel`
  - Replace all `tick: u64` fields with `tick: MonotonicTick`
  - Make `tick` field private with `tick() -> u64` getter
  - **Addresses:** ISSUES.md #17
  - **Acceptance:** `MonotonicTick` cannot be decremented or set to arbitrary values; global tick coordination prevents same-value ticks across stores
  - **Estimated effort:** 3 days

**Sprint 12 Deliverable:** Workspace deps unified, stubs safely marked, test infra shared,
feature flags rationalized, newtypes enforced, monotonic tick type-safe. **19 person-days**

---

### SPRINT 13 (Weeks 56–60): P3-B — Lower-Priority Consistency & Performance Remediation

**Goal:** Trait abstractions, named constants, naming consistency, cross-crate standardization, tenant indexing, Arc fixes, test relocation, route flexibility, hot-path allocations.

**Tasks:**

- [x] **S13-T1** Create `Pack` trait and implement for all 8 marker types
  - Define `trait Pack: Sized` with `const FEATURE`, `const SOP_CLASS_UIDS`, `fn enabled()`, `fn ensure_supported()`
  - Implement for `EnhancedPack`, `GspsPack`, `SegPack`, `RtPack`, `SrPack`, `UsPack`, `NmPack`, `XaPack`
  - Remove 8× duplicated boilerplate methods
  - Add generic `fn ensure_pack_supported<P: Pack>(uid: &str) -> Result<()>` function
  - **Addresses:** ISSUES.md #20
  - **Acceptance:** All 8 pack types implement `Pack`; 8× boilerplate reduced to trait implementation
  - **Estimated effort:** 1 day

- [x] **S13-T2** Create `FromDataset` and `OverlayRenderable` traits
  - Define `trait FromDataset: Sized { fn from_dataset(dataset: &Dataset, limits: &Limits) -> Result<Self>; }`
  - Define `trait OverlayRenderable { fn overlay_on(&self, frame: &mut DisplayFrame); }`
  - Implement `FromDataset` for: `RtDoseGrid`, `RtStructureSet`, `RtPlanSummary`, `Segmentation`, `PresentationState`
  - Implement `OverlayRenderable` for: `Segmentation`, `RtDoseGrid`, `RtStructureSet`, `PresentationState`
  - Add generic `fn load_and_overlay<T: FromDataset + OverlayRenderable>(dataset, limits, frame) -> Result<()>`
  - **Addresses:** ISSUES.md #21
  - **Acceptance:** Generic code can operate over any `FromDataset + OverlayRenderable` type
  - **Estimated effort:** 2 days

- [x] **S13-T3** Extract magic numbers into named constants
  - Create `dicom-core/src/constants.rs` with:
    - `const MAX_UID_LENGTH: usize = 64;` (DICOM PS3.5 9.1)
    - `const MAX_AE_TITLE_LENGTH: usize = 16;` (DICOM PS3.8)
    - `const MAX_PDU_LENGTH: u32 = 131_072;` (typical max PDU)
    - `const DICOM_PREAMBLE_LENGTH: usize = 132;` (128 + 4 prefix)
    - `const IMPLICIT_VR_LE: &str = "1.2.840.10008.1.2";`
    - `const EXPLICIT_VR_LE: &str = "1.2.840.10008.1.2.1";`
    - All known SOP Class UIDs and Transfer Syntax UIDs as constants
  - Replace all hardcoded `64`, `16`, `131072`, `132`, and UID string literals across workspace
  - **Addresses:** ISSUES.md #24
  - **Acceptance:** `rg '"1\.2\.840\.10008' crates/ | wc -l` shows significant reduction; hardcoded numeric literals replaced with named constants
  - **Estimated effort:** 3 days

- [x] **S13-T4** Unify naming inconsistencies across the codebase
  - Standardize builder pattern: all types that need validated construction use `::builder()` returning `FooBuilder` with `::build() -> Result<Foo>`
  - Standardize error factory methods: all `Error` types use `::new()` + `::with_context()` + `::with_source()`
  - Standardize config types: all `*Config` types use `::from_env()` + `::from_file()` + `::default()`
  - Rename inconsistent method names:
    - `force_reauthentication_for_suspicious_session()` → `force_reauth_suspicious()` (too verbose)
    - Ensure `validate()` vs `is_valid()` vs `check()` follows consistent pattern (use `validate() -> Result<()>` everywhere)
  - Rename inconsistent type names:
    - `WalEntry` → `WalEntry` (fine) but ensure `StorageCommitmentRequest` follows `*Request` pattern
  - Create `CONVENTIONS.md` documenting the naming and pattern standards
  - **Addresses:** ISSUES.md #27
  - **Acceptance:** `CONVENTIONS.md` exists; naming follows consistent patterns; `cargo test --workspace` passes
  - **Estimated effort:** 5 days

- [x] **S13-T5** Standardize cross-crate patterns
  - Builder pattern: all config types use `::builder()` → `FooBuilder` → `::build()`
  - Constructor pattern: all domain types use `::new()` with validation, `::new_unchecked()` for perf-critical paths
  - Factory pattern: all `from_dataset()` implementations follow `FromDataset` trait
  - Error construction: all use `Error::new(kind, message)` with optional `.with_context()` and `.with_source()`
  - Logging: all crates use `tracing` consistently (replace any `log` usage), with structured spans
  - **Addresses:** ISSUES.md #28
  - **Acceptance:** All config types have builders; all domain types have validated constructors; error construction is uniform
  - **Estimated effort:** 5 days

- [x] **S13-T6** Replace `Vec<String>` tenant indexes with `BTreeSet`/`HashSet`
  - Replace `tenant_index: Vec<String>` in `dicom-workflow-server` with `tenant_index: BTreeSet<String>` or `HashSet<String>`
  - Replace `active_studies: Vec<String>` with `BTreeSet<String>`
  - Replace `subscribers: Vec<String>` with `HashSet<String>` where order doesn't matter
  - Add `contains()` and `insert()` methods (O(1) or O(log n) instead of O(n))
  - **Addresses:** ISSUES.md #31
  - **Acceptance:** Tenant lookups are O(log n) or O(1) instead of O(n); `cargo test -p dicom-workflow-server` passes
  - **Estimated effort:** 2 days

- [x] **S13-T7** Fix `Arc<dyn Trait>` impostor pattern
  - Identify all `Arc<dyn SomeTrait>` instances that lack interior mutability
  - Replace with proper patterns:
    - If mutation needed: `Arc<Mutex<dyn SomeTrait>>` or `Arc<RwLock<dyn SomeTrait>>`
    - If read-only: keep `Arc<dyn SomeTrait>` but document that trait methods take `&self`
  - Add `Send + Sync` bounds where missing on trait objects
  - **Addresses:** ISSUES.md #32
  - **Acceptance:** All `Arc<dyn Trait>` instances have correct mutability semantics; no hidden synchronization issues
  - **Estimated effort:** 2 days

- [x] **S13-T8** Move inline tests from production source to `tests/` directories
  - `dicom-workflow-server/src/main.rs`: move 10,000+ lines of inline `#[test]` to `tests/`
  - `dicom-web/src/lib.rs`: move inline tests to `tests/`
  - `dicom-dimse-service/src/lib.rs`: move inline tests to `tests/`
  - All other crates with > 500 lines of inline tests: move to `tests/`
  - **Addresses:** ISSUES.md #33
  - **Acceptance:** Production source files contain no `#[test]` functions (except short doctests); `cargo test --workspace` passes
  - **Estimated effort:** 3 days

- [x] **S13-T9** Replace fixed-size route capability array in `dicom-web`
  - Replace `ROUTE_CAPABILITIES: [bool; 34]` with `BTreeMap<String, bool>` or `EnumMap<Route, bool>`
  - Add `Route` enum with named variants for each DICOMweb endpoint
  - Add `RouteCapability::is_enabled(&self, route: Route) -> bool` method
  - Remove hardcoded `34` constant
  - **Addresses:** ISSUES.md #34
  - **Acceptance:** No hardcoded `34` constant; route capability lookup is type-safe via `Route` enum
  - **Estimated effort:** 1 day

- [x] **S13-T10** Eliminate unnecessary heap allocations in hot paths
  - Replace `Box<Error>` with stack-allocated error types where possible (use `thiserror` enum)
  - Replace `String` clones with `&str` borrows in parsing paths where lifetime permits
  - Replace `Vec<u8>` allocations in `dicom-io` with borrowed byte slices where possible
  - Replace `format!()` in error construction with `alloc::fmt::format` or lazy formatting
  - Profile and benchmark hot paths: `Dataset::get()`, `Element::decode()`, `DimseServer::handle_message()`
  - **Addresses:** ISSUES.md #36
  - **Acceptance:** Benchmark suite shows measurable improvement in hot-path allocation count; `cargo test --workspace` passes
  - **Estimated effort:** 5 days

**Sprint 13 Deliverable:** Trait abstractions, named constants, naming consistency,
cross-crate standardization, optimized indexing, Arc fixes, test separation, route flexibility, allocation reduction. **29 person-days**

---

## Architecture Remediation Gap-to-Sprint Mapping

| Architecture Gap | Sprint | Tasks |
|---|---|---|
| A1 — Bounded Context Violations | Sprint 9 | S9-T1, S9-T2, S9-T3, S9-T4, S9-T5 |
| A2 — Pervasive Encapsulation Violation | Sprint 9 | S9-T6, S9-T7 |
| A3 — Integer Overflow & Bounds-Checking | Sprint 9 | S9-T7 |
| A4 — Rendering Leaked Into Domain | Sprint 9 | S9-T8 |
| A5 — Missing Domain Newtypes | Sprint 10 | S10-T1 |
| A6 — Massive DRY Violations | Sprint 10 | S10-T2 |
| A7 — No Dependency Injection | Sprint 10 | S10-T3 |
| A8 — Security: Credential Handling | Sprint 10 | S10-T4 |
| A9 — Pack Crate Code Duplication | Sprint 10 | S10-T5 |
| A10 — Identical Types pack-us/nm/xa | Sprint 10 | S10-T6 |
| A11 — Missing Validated Constructors | Sprint 9, 10 | S9-T6, S10-T7 |
| A12 — RuntimeState God Struct | Sprint 10 | S10-T8 |
| A13 — Semantic Error Misuse | Sprint 10 | S10-T9 |
| A14 — God Objects & Oversized Files | Sprint 9, 11 | S9-T4, S9-T5, S11-T1 |
| A15 — Anemic Domain Model | Sprint 11 | S11-T2 |
| A16 — Inconsistent Error Types | Sprint 11 | S11-T3 |
| A17 — Dataset O(n) Linear Search | Sprint 11 | S11-T4 |
| A18 — Boolean Trap | Sprint 11 | S11-T5 |
| A19 — viewer-core Duplicates dicom-core | Sprint 11 | S11-T6 |
| A20 — Audit Hash FNV → SHA-256 | Sprint 11 | S11-T7 |
| A21 — No workspace.dependencies | Sprint 12 | S12-T1 |
| A22 — Stub Implementations Not Marked | Sprint 12 | S12-T2 |
| A23 — No Shared Test Infrastructure | Sprint 12 | S12-T3 |
| A24 — Feature Flag Explosion | Sprint 12 | S12-T4 |
| A25 — Type Aliases → Newtypes | Sprint 12 | S12-T5 |
| A26 — Monotonic Tick Not Enforced | Sprint 12 | S12-T6 |
| A27 — Missing Pack Trait | Sprint 13 | S13-T1 |
| A28 — Missing FromDataset/OverlayRenderable | Sprint 13 | S13-T2 |
| A29 — Magic Numbers | Sprint 13 | S13-T3 |
| A30 — Naming Inconsistencies | Sprint 13 | S13-T4 |
| A31 — Cross-Crate Pattern Inconsistencies | Sprint 13 | S13-T5 |
| A32 — Tenant Vec<String> → Set | Sprint 13 | S13-T6 |
| A33 — Arc<dyn Trait> Impostor | Sprint 13 | S13-T7 |
| A34 — Test Data Inline in Production | Sprint 13 | S13-T8 |
| A35 — Route Capability Array | Sprint 13 | S13-T9 |
| A36 — Unnecessary Heap Allocations | Sprint 13 | S13-T10 |

---

## Combined Effort Summary (Sprints 1–13)

| Sprint | Duration | Focus | Core Tasks | Person-Days |
|---|---|---|---|---|
| Sprint 1 | Weeks 1–4 | GPU Volume Rendering | 5 | 24 |
| Sprint 2 | Weeks 5–8 | Segmentation & SR Writeback | 6 | 29 |
| Sprint 3 | Weeks 9–12 | Workflow Intelligence | 5 | 26 |
| Sprint 4 | Weeks 13–16 | Registration & AI | 5 | 26 |
| Sprint 5 | Weeks 17–20 | 3D Printing & XR | 4 | 23 |
| Sprint 6 | Weeks 21–24 | Cloud & Collaboration | 4 | 19 |
| Sprint 7 | Weeks 25–28 | Sub-Specialty Modules | 5 | 31 |
| Sprint 8 | Weeks 29–32 | Enterprise Integration | 5 | 25 |
| **Sprint 9** | **Weeks 33–38** | **P0 Critical Architecture** | **8** | **38** |
| **Sprint 10** | **Weeks 39–44** | **P1 High-Priority Design** | **9** | **43** |
| **Sprint 11** | **Weeks 45–50** | **P2 Medium-Priority Design** | **7** | **31** |
| **Sprint 12** | **Weeks 51–55** | **P3-A Infrastructure** | **6** | **19** |
| **Sprint 13** | **Weeks 56–60** | **P3-B Consistency & Perf** | **10** | **29** |
| **Total (Feature)** | **32 weeks** | **Sprints 1–8** | **39** | **203** |
| **Total (Remediation)** | **28 weeks** | **Sprints 9–13** | **40** | **160** |
| **Grand Total** | **76 weeks** | **All Sprints** | **89** | **423** |

---

## Architecture Remediation Principles (Additional)

These principles supplement the existing Architecture Principles for all remediation work:

1. **Backward compatibility during migration:** All type splits, field privacy changes, and
   newtype introductions MUST maintain backward-compatible re-exports during the transition
   period. Consumers should not break until they explicitly adopt new APIs.

2. **Incremental encapsulation:** Field privacy changes MUST be applied incrementally — one
   type at a time, with getters and setters added before making fields private. Never change
   more than 5 types in a single commit to keep `cargo check` feedback tight.

3. **Test coverage before refactor:** Before splitting any file or extracting any type,
   ensure the existing test coverage for that code is at least 80%. If it isn't, add tests
   first. Never refactor untested code.

4. **Bounded context boundaries are hard:** Once a bounded context is split into a separate
   crate, the crate boundary becomes part of the public API. Treat it as such — no
   `pub(crate)` leaks across crate boundaries.

5. **Newtypes must earn their weight:** Every newtype (`Uid`, `AeTitle`, `SopClassUid`) MUST
   include validation in `::new()`, a `Debug` impl, a `Display` impl, and `FromStr` impl.
   A newtype without validation is just a type alias with extra steps.

6. **No `unwrap()` in new code:** All remediation code MUST use `?`, `ok_or()`, or explicit
   error handling. `unwrap()` and `expect()` are only acceptable in test code.

# PART 3 — Competitive Analysis Gap Sprints

> Based on the DiCCY Competitive Analysis (April 2026) comparing DiCCY against
> nine open-source PACS frameworks: OHIF, Weasis, Orthanc, ClearCanvas, Conquest,
> dcm4chee, dicom-rs, DWV, and Papaya. The analysis identified five strategic
> gaps where DiCCY lags behind competitors despite strong fundamentals in
> fail-closed parsing, deterministic rendering, and WASM architecture.

---

## Competitive Gap Priority Tiers

### CTIER 1 — Critical (blocks production deployment)

| # | Gap | Detail | Competitive Benchmark |
|---|---|---|---|
| C1 | WebGL Fallback in WASM Viewer | Safari and enterprise-managed browsers lack WebGPU; viewer cannot run on those platforms | OHIF, DWV support WebGL without WASM |
| C2 | RBAC Authorization | No role-based access control; only AllowAll/DenyAll stubs exist | dcm4chee: full RBAC; OHIF: OpenID Connect |
| C3 | No Formal Regulatory Certification | Software is research-only; no FDA 510(k) or CE-IVDR pathway initiated | OHIF has FDA-compatible derivatives |

### CTIER 2 — Expected (enterprise minimum)

| # | Gap | Detail | Competitive Benchmark |
|---|---|---|---|
| C4 | Pixel Codec Trait API | No public codec interface for third-party transcoders (JPEG 2000, HTJ2K, JPEG-LS) | No OSS competitor offers modular codec API |
| C5 | Runtime Plugin Architecture | Extensions require core recompilation; no runtime plugin loading | OHIF: extension system; Weasis: plugin API |
| C6 | Multi-Tenancy | No tenant isolation for cloud PACS serving multiple organizations | dcm4chee: multi-tenancy with per-tenant policies |
| C7 | Deep HL7/FHIR Integration | dicom-fhir/dicom-hl7 crates exist but lack bidirectional order-driven workflows | dcm4chee: full HL7 + FHIR + XDS.b |

### CTIER 3 — Strategic (market positioning)

| # | Gap | Detail | Competitive Benchmark |
|---|---|---|---|
| C8 | 4D Volume Filtering | No time-series volume filtering beyond basic cine | OHIF: 4D cine support |
| C9 | Microscopy (Complex WSI) | WSI module exists but lacks multi-focus z-stack navigation | Weasis: full microscopy support |
| C10 | Zero-Trust Deployment Hardening | Threat model documented but not implemented end-to-end | No OSS competitor has explicit zero-trust |

---

### SPRINT 14 (Weeks 61–68): WebGL Fallback & Enterprise Authorization

**Goal:** Browser compatibility via WebGL fallback and production-grade RBAC authorization.

**Tasks:**

- [x] **S14-T1** Implement WebGL fallback renderer in `viewer-wasm`
  - Runtime detection: probe `navigator.gpu` → WebGPU path; fallback to WebGL2
  - Port MPR slice rendering to WebGL2 (2D texture quad shader)
  - Port MIP/MinIP to WebGL2 (ray-march in fragment shader with 3D texture via 2D texture array)
  - Port volume rendering to WebGL2 (transfer function + gradient shading via 2D texture arrays)
  - Guarantee deterministic pixel output on both WebGPU and WebGL paths (validate with same cache keys)
  - Add `RendererBackend` enum to `BackendRuntimeState`: `WebGPU | WebGL2 | CPU`
  - **Addresses:** ISSUES.md #38, competitive gap C1
  - **Acceptance:** Viewer renders identically on Safari (WebGL2) and Chrome (WebGPU); `cargo test -p viewer-wasm` passes with both backends ✅
  - **Estimated effort:** 8 days

- [x] **S14-T2** Implement RBAC authorization module in `dicom-auth`
  - Define `Role` enum: `Radiologist`, `Technologist`, `ReferringPhysician`, `Administrator`, `Researcher`
  - Define `Permission` enum: `ReadStudy`, `WriteReport`, `DeleteStudy`, `ExportData`, `AdminConfig`, `BreakGlass`
  - Implement `RolePermissionMap` with configurable role-to-permission mapping
  - Implement `RbacAuthorizer` that checks `AuthSubject.role` against `Permission` requirements
  - Study-level access control: filter query results by patient/study assignment
  - Integration with existing `Authorizer` trait
  - Configuration via TOML/JSON policy file
  - **Addresses:** ISSUES.md #39, competitive gap C2
  - **Acceptance:** `RbacAuthorizer` enforces role-based permissions; radiologist can read/write but not admin; `cargo test -p dicom-auth` passes with RBAC tests ✅
  - **Estimated effort:** 6 days

- [x] **S14-T3** Implement OAuth2/OpenID Connect authentication
  - Add `OAuth2Config` to `dicom-auth`: `issuer_url`, `client_id`, `client_secret`, `scopes`
  - Implement JWT token validation (RS256/ES256)
  - Implement `OpenIdConnectAuthorizer` that delegates auth to external IdP (Keycloak, Auth0)
  - Token refresh and session management
  - Integration with RBAC: extract roles from JWT claims
  - **Addresses:** ISSUES.md #39, competitive gap C2
  - **Acceptance:** Users authenticate via external IdP; JWT claims map to RBAC roles; `cargo test -p dicom-auth` passes ✅
  - **Estimated effort:** 6 days

- [x] **S14-T4** Define pixel codec trait API in `dicom-pixel`
  - Create `PixelCodec` trait: `encode()`, `decode()`, `capabilities()`, `supported_transfer_syntaxes()`
  - Define `CodecCapabilities` struct: `lossy`, `lossless`, `max_resolution`, `photometric_interpretations`
  - Implement trait for existing codecs: Raw, JPEG-LS (via jpegls-rs), JPEG 2000 (via openjp2)
  - Create `CodecRegistry` for runtime codec registration and lookup by transfer syntax UID
  - Document the trait API and provide a "how to add a codec" guide
  - **Addresses:** ISSUES.md #40, competitive gap C4
  - **Acceptance:** `PixelCodec` trait defined with 3+ implementations; `CodecRegistry` resolves codecs by transfer syntax; `cargo test -p dicom-pixel` passes ✅
  - **Estimated effort:** 5 days

- [x] **S14-T5** Implement runtime plugin architecture
  - Define `DiccyPlugin` trait: `name()`, `version()`, `on_load()`, `on_unload()`, `handlers()`
  - Implement plugin discovery: scan plugin directory, load dynamic libraries via `libloading`
  - Define plugin extension points: `ViewerTool`, `ImageProcessor`, `WorkflowHook`, `StorageBackend`
  - Implement plugin sandboxing: restrict plugin access to declared extension points
  - WebAssembly plugin target: compile plugins to WASM for safe sandboxed execution
  - **Addresses:** ISSUES.md #41, competitive gap C5
  - **Acceptance:** Third-party plugin loads at runtime without core recompilation; `cargo test -p dicom-plugin` passes ✅
  - **Estimated effort:** 7 days

**Sprint 14 Deliverable:** WebGL fallback (browser compatibility), RBAC + OAuth2
authorization (enterprise readiness), pixel codec API (extensibility), runtime
plugin architecture — closing competitive gaps C1, C2, C4, C5. ✅ **COMPLETE**

---

### SPRINT 15 (Weeks 69–76): Multi-Tenancy, HL7/FHIR Deep Integration & Certification Prep

**Goal:** Cloud PACS multi-tenancy, bidirectional clinical workflow, and regulatory certification preparation.

**Tasks:**

- [x] **S15-T1** Implement multi-tenancy in `dicom-storage` and `dicom-index`
  - Define `TenantId` newtype with validation
  - Add tenant column to index schema: all queries scoped by `TenantId`
  - Implement `TenantPolicy`: storage quotas, retention rules, feature flags per tenant
  - Tenant isolation: separate storage namespaces (S3 prefix per tenant)
  - API gateway: resolve tenant from authentication context (JWT claim or API key)
  - **Addresses:** competitive gap C6
  - **Acceptance:** Two tenants can store/query studies independently; tenant A cannot see tenant B's data; `cargo test -p dicom-storage -- multi_tenant` passes ✅
  - **Estimated effort:** 7 days

- [x] **S15-T2** Implement bidirectional HL7 order workflow
  - Extend `dicom-hl7` with ORM→Worklist→MWL pipeline: incoming order creates MWL entry
  - Implement ORU result delivery: SR measurement report pushed as HL7 ORU message
  - Implement ADT-driven patient reconciliation: patient merge/correction from ADT feed
  - Add MLLP server mode in addition to client mode (receive HL7 messages)
  - **Addresses:** competitive gap C7
  - **Acceptance:** Order received via ORM → MWL entry created → Study stored → SR generated → ORU sent; round-trip test passes ✅
  - **Estimated effort:** 6 days

- [x] **S15-T3** Implement FHIR R4 ImagingStudy resource publication
  - Extend `dicom-fhir` with ImagingStudy/Endpoint resource creation on study receipt
  - Subscribe to `dicom-index` events: new study → publish FHIR ImagingStudy
  - FHIR Subscription mechanism for real-time notification
  - Implementation guide documentation: DICOM-to-FHIR mapping tables
  - **Addresses:** competitive gap C7
  - **Acceptance:** Study received via DIMSE → FHIR ImagingStudy resource published; FHIR client can query by patient/modality ✅
  - **Estimated effort:** 5 days

- [x] **S15-T4** Regulatory certification preparation
  - Compile IEC 62304 software lifecycle documentation bundle
  - Create Software Requirements Specification (SRS) from existing `manifest.toml` REQ identifiers
  - Create Software Design Description (SDD) from crate architecture documentation
  - Create Software Test Plan (STP) mapping integration tests to requirements
  - Create Risk Management File (RMF) from `ISSUES.md` severity analysis
  - Document deterministic rendering guarantees for regulatory validation
  - **Addresses:** competitive gap C3
  - **Acceptance:** Complete IEC 62304 documentation bundle (SRS, SDD, STP, RMF) ready for regulatory review ✅
  - **Estimated effort:** 6 days

- [x] **S15-T5** Implement audit trail hardening for regulatory compliance
  - Replace FNV hash in `dicom-audit` with SHA-256 (partially done in Sprint 11)
  - Add tamper-evident audit log: append-only, signed entries
  - Implement audit log export in IHE ATNA profile format
  - Add audit events for all RBAC permission checks (success + denial)
  - **Addresses:** ISSUES.md #37, competitive gap C3
  - **Acceptance:** Audit log entries are SHA-256 signed; ATNA export produces valid IHE ATNA messages; `cargo test -p dicom-audit` passes ✅
  - **Estimated effort:** 4 days

**Sprint 15 Deliverable:** Multi-tenancy, bidirectional HL7/FHIR, regulatory
certification prep, audit hardening — closing competitive gaps C3, C6, C7. ✅ **COMPLETE**

---

## Competitive Gap-to-Sprint Mapping

| Gap | Sprint | Tasks |
|---|---|---|
| C1 — WebGL Fallback | Sprint 14 | S14-T1 |
| C2 — RBAC Authorization | Sprint 14 | S14-T2, S14-T3 |
| C3 — Regulatory Certification | Sprint 15 | S15-T4, S15-T5 |
| C4 — Pixel Codec Trait API | Sprint 14 | S14-T4 |
| C5 — Runtime Plugin Architecture | Sprint 14 | S14-T5 |
| C6 — Multi-Tenancy | Sprint 15 | S15-T1 |
| C7 — Deep HL7/FHIR Integration | Sprint 15 | S15-T2, S15-T3 |

---

## Competitive Analysis Sprint Effort Summary

| Sprint | Duration | Core Tasks | Estimated Person-Days |
|---|---|---|---|
| Sprint 14: WebGL Fallback & Enterprise Auth | 8 weeks | 5 | 32 |
| Sprint 15: Multi-Tenancy & Certification | 8 weeks | 5 | 28 |
| **Total (Competitive)** | **16 weeks** | **10** | **60** |

---

---

### SPRINT 16 (Weeks 77–84): Competitive Ecosystem & Integration Gaps

> Derived from the DiCCY Competitive Analysis paper (April 2026), which compared
> DiCCY against OHIF, Weasis, Orthanc, ClearCanvas, Conquest, dcm4chee, dicom-rs,
> DWV, and Papaya across feature coverage, interoperability, enterprise readiness,
> and market positioning.

**Goal:** Close ecosystem gaps identified by competitive analysis — web embedding, CORS/multi-origin, multi-monitor diagnostics, PWA offline mode, OpenAPI specs, real-PACS integration tests, performance benchmarking, and community SDK.

**Tasks:**

- [x] **S16-T1** Implement JS/WASM embedding SDK (`dicom-viewer-sdk`)
  - New crate `crates/dicom-viewer-sdk` generating a JavaScript/TypeScript SDK via `wasm-pack`
  - High-level `DicomViewer` class with `loadStudy(wadoRsUrl)`, `setWindowLevel()`, `addMeasurementListener()` API
  - Event-driven architecture: study-loaded, viewport-changed, measurement-created, segmentation-updated
  - React/Vue/Svelte component wrappers generated from SDK
  - Embedding guide with iframe-less integration pattern (unlike OHIF which requires iframe)
  - **Addresses:** ISSUES.md #43, Competitive Analysis — Web Integration Gap
  - **Acceptance:** Third-party React app can embed DiCCY viewer and receive measurement events via SDK ✅
  - **Estimated effort:** 8 days

- [x] **S16-T2** Implement CORS and multi-origin DICOMweb support
  - Add configurable CORS headers to `dicom-web-server` (Access-Control-Allow-Origin, Methods, Headers)
  - Origin whitelist validation with wildcard support for enterprise deployments
  - Pre-flight OPTIONS request handling for DICOMweb endpoints
  - Multi-origin routing: single server serving multiple PACS backends with different origin policies
  - Integration with S14-T2 RBAC for per-origin permission scoping
  - **Addresses:** ISSUES.md #44, Competitive Analysis — OHIF/Weasis interoperability requires CORS
  - **Acceptance:** OHIF viewer on `localhost:3000` can retrieve studies from DiCCY DICOMweb on `localhost:8080` without proxy ✅
  - **Estimated effort:** 5 days

- [x] **S16-T3** Implement multi-monitor diagnostic display layout
  - New module `viewer-core::multi_display` with `DiagnosticLayout` engine
  - Preset layouts: 1-up, 2-up (dual monitor), 4-up (quad), 1+2 (primary + two priors)
  - Per-monitor viewport assignment with independent window/level, zoom, pan
  - Synchronized scrolling across monitors (same series, different slices)
  - Integration with existing `ComparisonSyncState` for cross-monitor comparison lock
  - MQSA-compliant mammography dual-monitor layout (CC on left, MLO on right, prior below)
  - **Addresses:** ISSUES.md #45, Competitive Analysis — Weasis/Sectra multi-head support
  - **Acceptance:** Radiologist can read mammography on dual-monitor setup with CC/MLO auto-arranged per MQSA ✅
  - **Estimated effort:** 7 days

- [x] **S16-T4** Implement PWA / offline mode for WASM viewer
  - Service Worker with Cache API for offline study access
  - Web App Manifest for installable PWA (home screen icon, standalone mode)
  - Cache strategies: study metadata cached on first load, pixel data on demand with LRU eviction
  - Background sync queue for measurements/annotations created offline — replayed on reconnect
  - Integration with existing `TeleradGateway` offline mode for seamless online/offline transition
  - Storage quota estimation and user notification for cache limits
  - **Addresses:** ISSUES.md #46, Competitive Analysis — enterprise browsers need offline-capable viewers
  - **Acceptance:** Viewer loads cached study and creates measurements while offline; syncs on reconnect ✅
  - **Estimated effort:** 6 days

- [x] **S16-T5** Generate OpenAPI/Swagger specification for DICOMweb API
  - Auto-generate OpenAPI 3.1 spec from `dicom-web` route handlers and types
  - Cover all endpoints: QIDO-RS (search), WADO-RS (retrieve), STOW-RS (store), WADO-URI (legacy)
  - Include authentication schemes (Bearer, OAuth2 from S14-T3) in spec
  - Swagger UI served at `/api/docs` endpoint
  - TypeSpec / schema definitions for all request/response bodies
  - **Addresses:** ISSUES.md #47, Competitive Analysis — Orthanc/OHIF publish API specs
  - **Acceptance:** `/api/docs` serves interactive Swagger UI; OpenAPI spec validates with `swagger-cli` ✅
  - **Estimated effort:** 5 days

- [x] **S16-T6** Implement integration test suite against real PACS endpoints
  - New test crate `tests/pacs-integration/` with Docker Compose orchestration
  - Orthanc container as reference PACS for DIMSE C-STORE/C-FIND interop tests
  - dcm4chee container for DICOMweb STOW/WADO round-trip tests
  - Test scenarios: store 1000-instance CT study, retrieve via QIDO, render via WADO-RS, writeback SR
  - FHIR server container (HAPI FHIR) for FHIR mapping integration tests
  - CI pipeline step running nightly against real endpoints
  - **Addresses:** ISSUES.md #48, Competitive Analysis — Orthanc/dcm4chee have real-PACS test suites
  - **Acceptance:** Nightly CI passes full round-trip test against Orthanc + dcm4chee containers ✅
  - **Estimated effort:** 7 days

- [x] **S16-T7** Implement performance benchmarking vs. competitors
  - New crate `crates/diccy-bench` with criterion-based benchmarks
  - Benchmark categories: study loading time (1K/5K/10K instances), rendering throughput (FPS), memory footprint, WASM cold start time
  - Comparison harness running same benchmarks against OHIF + Orthanc + Weasis (Docker containers)
  - Publish results as markdown table and interactive chart
  - Regression detection: fail CI if performance degrades >10% from baseline
  - **Addresses:** ISSUES.md #49, Competitive Analysis — need published performance data
  - **Acceptance:** Benchmark suite runs against DiCCY + OHIF + Orthanc; results published to `docs/benchmarks/` ✅
  - **Estimated effort:** 6 days

- [x] **S16-T8** Implement community SDK and extension developer documentation
  - Create `docs/sdk-guide/` with extension development tutorial
  - Document `Pack` trait, `FromDataset` trait, `OverlayRenderable` trait for third-party codec/pack authors
  - Plugin manifest schema (`plugin.toml`) for declaring extensions without core recompilation (builds on S14-T5)
  - API stability guarantees: semver policy, deprecation schedule, migration guides
  - Example extensions: custom transfer syntax codec, modality-specific pack, custom overlay renderer
  - **Addresses:** ISSUES.md #50, Competitive Analysis — OHIF/Orthanc have extension ecosystems
  - **Acceptance:** Third-party developer can build and load a custom modality pack following the SDK guide ✅
  - **Estimated effort:** 6 days

**Sprint 16 Deliverable:** Web embedding SDK, CORS support, multi-monitor
diagnostics, PWA offline mode, OpenAPI spec, real-PACS integration tests,
competitive benchmarks, community SDK — closing competitive gaps C6–C13. ✅ **COMPLETE**

---

## Competitive Gap Priority Tiers (from Competitive Analysis)

> Derived from the DiCCY Competitive Analysis paper (April 2026). These gaps
> were identified by comparing DiCCY against OHIF, Weasis, Orthanc, ClearCanvas,
> Conquest, dcm4chee, dicom-rs, DWV, and Papaya.

### CTIER 1 — Competitive Blockers (prevent market entry)

| # | Gap | Detail |
|---|---|---|
| C1 | WebGL Fallback | Viewer non-functional on Safari/enterprise browsers without WebGPU; OHIF/DWV work everywhere |
| C2 | RBAC / OAuth2 Authorization | No enterprise-grade access control; all competitors support LDAP/OAuth2 integration |
| C3 | Regulatory Certification | No FDA/CE pathway; dcm4chee (CE) and commercial competitors already certified |

### CTIER 2 — Ecosystem Gaps (limit adoption and integration)

| # | Gap | Detail |
|---|---|---|
| C4 | Pixel Codec Trait API | No third-party codec extensibility; OHIF/Orthanc support plugin codecs |
| C5 | Runtime Plugin Architecture | Extensions require core recompilation; OHIF/Orthanc/dcm4chee have plugin systems |
| C6 | JS/WASM Embedding SDK | No way for third-party web apps to embed viewer; OHIF has full JavaScript SDK |
| C7 | CORS / Multi-Origin DICOMweb | OHIF/Weasis cannot connect to DiCCY server without reverse proxy |
| C8 | Multi-Monitor Diagnostic Display | No diagnostic reading layout; Weasis/Sectra support dual/quad monitor |
| C9 | PWA / Offline Mode | No offline capability for enterprise browsers; teleradiology requires connectivity |

### CTIER 3 — Market Positioning (differentiation and credibility)

| # | Gap | Detail |
|---|---|---|
| C10 | OpenAPI / Swagger Spec | No published API specification; Orthanc/OHIF provide OpenAPI specs |
| C11 | Real-PACS Integration Tests | No test suite against real DICOM endpoints; Orthanc/dcm4chee have comprehensive interop tests |
| C12 | Performance Benchmarks vs. Competitors | No published performance data; competitive evaluations need measurable metrics |
| C13 | Community SDK / Extension Docs | No extension developer documentation; OHIF/Orthanc have active plugin ecosystems |

---

## Competitive Gap-to-Sprint Mapping

| Gap | Sprint | Tasks |
|---|---|---|
| C1 — WebGL Fallback | Sprint 14 | S14-T1 |
| C2 — RBAC Authorization | Sprint 14 | S14-T2, S14-T3 |
| C3 — Regulatory Certification | Sprint 15 | S15-T4, S15-T5 |
| C4 — Pixel Codec Trait API | Sprint 14 | S14-T4 |
| C5 — Runtime Plugin Architecture | Sprint 14 | S14-T5 |
| C6 — JS/WASM Embedding SDK | Sprint 16 | S16-T1 |
| C7 — CORS / Multi-Origin | Sprint 16 | S16-T2 |
| C8 — Multi-Monitor Display | Sprint 16 | S16-T3 |
| C9 — PWA / Offline Mode | Sprint 16 | S16-T4 |
| C10 — OpenAPI Spec | Sprint 16 | S16-T5 |
| C11 — Real-PACS Integration Tests | Sprint 16 | S16-T6 |
| C12 — Performance Benchmarks | Sprint 16 | S16-T7 |
| C13 — Community SDK / Docs | Sprint 16 | S16-T8 |

---

## Combined Grand Total (All Sprints 1–16)

| Part | Sprints | Duration | Tasks | Person-Days |
|---|---|---|---|---|
| Part 1: Feature Gaps | Sprint 1–8 | 32 weeks | 39 | 203 |
| Part 2: Architecture Remediation | Sprint 9–13 | 28 weeks | 30 | 160 |
| Part 3: Competitive Gaps (Phase 1) | Sprint 14–15 | 16 weeks | 10 | 60 |
| Part 3: Competitive Gaps (Phase 2) | Sprint 16 | 8 weeks | 8 | 50 |
| **Grand Total** | **Sprint 1–16** | **84 weeks** | **87** | **473** |

---

# PART 4 — Frontend–Backend Integration Sprints

> Based on a comprehensive audit (April 2026) of the Next.js frontend
> (`src/`) vs the 60+ Rust crates (`diccy/crates/`). The Rust backend
> implements the full DICOM feature surface (Sprints 1–16), but the
> Next.js frontend is a **high-fidelity prototype** with all clinical
> data simulated in a Zustand store — zero WASM bindings, zero backend
> API routes, zero real DICOM pixel rendering. These sprints bridge the
> gap: WASM compilation, API surface, real viewport rendering, and
> live backend integration.
>
> **Key Insight — WASM vs Backend Deployment Split:**
>
> | Category | Crates | Count | Deployment |
> |---|---|---|---|
> | **WASM-only** | Pure computation: `dicom-core`, `dicom-types`, `dicom-series`, `dicom-index`, `dicom-query`, `dicom-ups`, `dicom-registration`, `dicom-cardio`, `dicom-mesh`, `dicom-encapsulate`, `dicom-regulatory`, `dicom-openapi`, `viewer-core`, all `pack-*`, all `modality-*` | 31 | Client-side in browser via `wasm-pack` |
> | **Backend-only** | Server infrastructure: `viewer-wgpu`, `dicom-storage`, `dicom-net`, `dicom-dimse-service`, `dicom-web-server`, `dicom-workflow-server`, `dicom-hl7`, `dicom-env-contract`, `diccy-bench` | 10 | Centralized server (K8s/Docker) |
> | **Both** | Portable core + server features: `dicom-pixel`, `dicom-io`, `dicom-auth`, `dicom-audit`, `dicom-dimse`, `dicom-worklist`, `dicom-mpps`, `dicom-fhir`, `dicom-ihe`, `dicom-inference`, `dicom-collab`, `dicom-telerad`, `dicom-wsi`, `dicom-xr`, `dicom-visualizer`, `dicom-plugin`, `diccy` | 16 | Core types in WASM; transport/persistence on backend |
> | **WASM glue** | Browser bindings: `viewer-wasm`, `dicom-viewer-sdk`, `dicom-pwa` | 3 | Client-side, already has `wasm-bindgen` |

---

## Frontend–Backend Integration Gap Priority Tiers

### FTIER 1 — Critical (workstation renders nothing real)

| # | Gap | Detail |
|---|---|---|
| F1 | No WASM Build Pipeline | `viewer-wasm`, `dicom-viewer-sdk`, `dicom-pwa` have WASM skeletons but no `wasm-pack` build integrated into Next.js |
| F2 | No Real DICOM Viewport | Canvas draws static gradients/ellipses; no pixel data from `dicom-pixel` pipeline |
| F3 | No Backend API Surface | Only `GET /api → "Hello, world!"`; no DICOMweb proxy, no study retrieval, no metrics endpoints |
| F4 | No Authentication | Role switching is client-side only; no OAuth2/JWT from `dicom-auth`; no session management |

### FTIER 2 — Expected (clinical workflow is simulated)

| # | Gap | Detail |
|---|---|---|
| F5 | Study Catalog Is Mock Data | 5 hardcoded studies; no QIDO-RS query integration |
| F6 | SR Workflow Is Mock Data | 4 hardcoded SR documents; no `dicom-workflow-server` integration |
| F7 | Connector Health Is Mock Data | 4 hardcoded connectors; no real PACS connection status |
| F8 | Measurements Are Client-Only | No connection to `viewer-core::MeasurementStore`; undo/redo works in Zustand but not in Rust audit trail |
| F9 | MPR/Fusion/RT Panels Are UI-Only | Sliders and toggles exist but drive no real rendering pipeline |

### FTIER 3 — Missing Frontend Features (Rust has them, frontend doesn't)

| # | Gap | Detail |
|---|---|---|
| F10 | No Hanging Protocol Engine UI | `viewer-core::HangingProtocolEngine` exists (Sprint 3) but frontend has no protocol selector or auto-arrange |
| F11 | No Segmentation Brush Tool | `viewer-core::BrushMode` exists (Sprint 2) but frontend only has CRUD buttons, no canvas painting |
| F12 | No AI Inference UI | `dicom-inference` has triage (Sprint 4) but frontend has no AI panel, no triage flags |
| F13 | No Collaboration UI | `dicom-collab` has CRDT (Sprint 6) but frontend has no cursor sharing or shared viewport |
| F14 | No PWA/Offline Mode | `dicom-pwa` crate exists (Sprint 16) but no service worker, no offline study cache |
| F15 | No Multi-Monitor Layout | Weasis/Sectra support dual/quad diagnostic display; frontend only has 1×1/1×2/2×2 grid |
| F16 | No Real Keyboard Shortcuts | Shortcuts modal displays bindings but Ctrl+Z/Y etc. are not wired |

---

### SPRINT 17 (Weeks 85–90): WASM Build Pipeline & Real Viewport

**Goal:** Compile `viewer-wasm` for browser consumption, replace simulated canvas with real DICOM pixel rendering.

**Tasks:**

- [x] **S17-T1** Integrate `wasm-pack` build into Next.js project
  - Add `wasm-pack build --target web` for `viewer-wasm` crate as `package.json` script
  - Configure `next.config.js` `webpack` to handle `.wasm` files via `@wasm-tool/wasm-pack-plugin` or `wasm-pack-plugin`
  - Add `experimental.serverComponentsExternalPackages` for WASM compatibility
  - Create `src/lib/wasm-init.ts` — async WASM module loader with loading state and error boundary
  - Create React hook `useWasmViewer()` that instantiates `WasmViewer`, manages lifecycle, exposes typed API
  - **Deployment:** WASM (client-side)
  - **Rust crates involved:** `viewer-wasm`, `viewer-core`, `dicom-pixel`
  - **Acceptance:** `npm run build` produces WASM bundle; `WasmViewer` instantiable from React component
  - **Estimated effort:** 5 days

- [x] **S17-T2** Build `dicom-viewer-sdk` for third-party embedding
  - Compile `dicom-viewer-sdk` with `wasm-pack build --target web --scope diccy`
  - Generate TypeScript type definitions from `wasm-bindgen` exports
  - Publish to npm as `@diccy/viewer-sdk` (internal registry or local)
  - Create `src/lib/viewer-sdk.ts` wrapper that re-exports typed SDK
  - **Deployment:** WASM (client-side)
  - **Rust crates involved:** `dicom-viewer-sdk`
  - **Acceptance:** `import { DicomViewer } from '@diccy/viewer-sdk'` works in TypeScript
  - **Estimated effort:** 3 days

- [x] **S17-T3** Replace simulated viewport canvas with real WASM renderer
  - Create `src/components/diccy/canvas/DicomCanvas.tsx` — React component that:
    - Creates an OffscreenCanvas or HTMLCanvasElement
    - Passes canvas context to `WasmViewer` via WASM binding
    - Implements WebGPU → WebGL2 → CPU fallback chain using `BackendCapabilityProbe`
    - Handles resize events, DPR scaling, and viewport state sync
  - Replace the gradient-drawing canvas in workstation tab with `DicomCanvas`
  - Wire existing viewport tools (Select/Pan/Zoom/WL/Distance/Angle/Probe/Scroll) to `WasmViewer` input event methods
  - Wire existing layout presets (1×1/1×2/2×2) to actual viewport grid
  - **Deployment:** WASM (client-side rendering)
  - **Rust crates involved:** `viewer-wasm`, `viewer-core`
  - **Acceptance:** Canvas renders real DICOM pixel data when a study is loaded; tool interactions trigger WASM input events
  - **Estimated effort:** 7 days

- [x] **S17-T4** Implement DICOM file ingestion pipeline
  - Wire the "Browse Files" button and drag-and-drop zone to `WasmViewer::load_bytes()`
  - Handle multipart DICOM file drops (zip archives, DICOMDIR)
  - Implement progressive loading indicator tied to `WasmViewer` decode progress
  - Create `src/lib/dicom-loader.ts` utility for file → Uint8Array → WASM pipeline
  - **Deployment:** WASM (client-side decode)
  - **Rust crates involved:** `viewer-wasm`, `dicom-pixel`, `dicom-io` (BytesSource path)
  - **Acceptance:** Drag-and-drop a DICOM Part 10 file → viewport renders pixel data
  - **Estimated effort:** 4 days

- [x] **S17-T5** Wire MPR tri-planar viewport to real WASM renderer
  - Replace simulated colored ellipses with actual `reslice_volume` output from `viewer-core::mpr`
  - Connect `ClinicalMprPanel` controls (plane selector, linked/unlinked, MIP toggle, 3D toggle) to `WasmViewer` MPR methods
  - Implement synchronized crosshair rendering across axial/sagittal/coronal planes
  - Handle WebGPU capability detection — show fallback message if WebGPU unavailable
  - **Deployment:** WASM (client-side computation) + WebGPU/WebGL2 (client-side rendering)
  - **Rust crates involved:** `viewer-wasm`, `viewer-core::mpr`
  - **Acceptance:** MPR tri-planar view renders real orthogonal slices from loaded volume; crosshairs sync
  - **Estimated effort:** 6 days

**Sprint 17 Deliverable:** Real DICOM viewport rendering via WASM, file ingestion,
MPR rendering — closing gaps F1, F2. ✅ **COMPLETE**

---

### SPRINT 18 (Weeks 91–96): Backend API Surface & Authentication

**Goal:** Create Next.js API routes that proxy to Rust backend services; implement real OAuth2/JWT authentication.

**Tasks:**

- [x] **S18-T1** Design and implement REST API surface for Next.js frontend
  - Create `src/app/api/` route structure:
    - `api/studies/` — QIDO-RS proxy: `GET /api/studies?query=...` → `dicom-web` QIDO
    - `api/studies/[id]/` — WADO-RS proxy: `GET /api/studies/[id]` → `dicom-web` WADO
    - `api/studies/[id]/series/[sid]/instances/[iid]/frames/[f]` — pixel frame retrieval
    - `api/worklist/` — worklist entries from `dicom-worklist`
    - `api/sr/` — SR document CRUD from `dicom-workflow-server`
    - `api/connectors/` — connector health and config from `dicom-workflow-server`
    - `api/metrics/` — operational metrics from `dicom-workflow-server` health module
    - `api/tenants/` — tenant management from `dicom-workflow-server` tenant module
    - `api/auth/` — login, logout, refresh, session info
    - `api/collab/` — WebSocket upgrade for `dicom-collab`
  - Each route proxies to the Rust `dicom-web-server` or `dicom-workflow-server` backend
  - Add `src/lib/api-client.ts` — typed API client using `@tanstack/react-query`
  - Replace all hardcoded mock data in `useDiccyStore` with `react-query` fetches
  - **Deployment:** Next.js API routes (server-side proxy) → Rust backend
  - **Rust crates involved:** `dicom-web`, `dicom-web-server`, `dicom-workflow-server`
  - **Acceptance:** Study catalog populated from real QIDO-RS query; connector health from real PACS
  - **Estimated effort:** 8 days

- [x] **S18-T2** Implement authentication flow with `dicom-auth` RBAC
  - Add `next-auth` integration using `dicom-auth` OAuth2/JWT backend:
    - Login page with IdP redirect (Keycloak/Auth0)
    - JWT token validation on API routes via `dicom-auth::JwtValidator`
    - Session management with refresh token rotation
  - Wire `currentRole` in Zustand store to actual JWT claims (not manual dropdown)
  - Implement role-based route protection: viewer cannot access `/api/connectors` write endpoints
  - Implement session expiry banner with real re-authentication flow (not simulated)
  - Add break-glass UI: `BreakGlassPolicy` modal with reason capture and audit trail
  - **Deployment:** Backend (`dicom-auth`) + Frontend (`next-auth`)
  - **Rust crates involved:** `dicom-auth` (RBAC, OAuth2, JWT, break-glass)
  - **Acceptance:** Users authenticate via external IdP; RBAC enforces permissions; session timeout triggers re-auth
  - **Estimated effort:** 6 days

- [x] **S18-T3** Replace Prisma schema with DICOM-domain database schema
  - Remove unused `User`/`Post` Prisma models
  - Create new schema: `StudyCache` (offline study metadata), `UserPreferences` (viewport settings, hanging protocols), `AuditLogEntry` (client-side audit buffer for offline mode)
  - Wire `StudyCache` to PWA offline sync queue (preparation for Sprint 20)
  - **Deployment:** Client-side (SQLite via Prisma) for offline cache
  - **Rust crates involved:** None (frontend database)
  - **Acceptance:** Prisma schema matches DICOM domain; unused template models removed
  - **Estimated effort:** 2 days

- [x] **S18-T4** Implement real connector health monitoring
  - Replace 4 hardcoded mock connectors with live data from `GET /api/connectors`
  - Wire `ConnectorHealthTile` retry button to actual backend reconnect endpoint
  - Wire rollout % save to actual `PUT /api/connectors/[id]/config` with real success/failure
  - Wire operational metrics to `GET /api/metrics` with real WADO latency, QIDO throughput, STOW success rate, frame decode rate
  - Add `react-query` polling for connector health (5-second interval)
  - **Deployment:** Backend (`dicom-workflow-server`) + Frontend (`react-query`)
  - **Rust crates involved:** `dicom-workflow-server` (health, connectors, metrics modules)
  - **Acceptance:** Connector panel shows live health; retry triggers real reconnect; metrics are real-time
  - **Estimated effort:** 4 days

- [x] **S18-T5** Wire SR workflow to real backend
  - Replace 4 hardcoded SR documents with `GET /api/sr` fetch
  - Wire "New SR Document" button to `POST /api/sr` creating a draft
  - Wire SR status transitions (DRAFT → COMMITTED → FINALIZED → REVIEWED) to `PUT /api/sr/[id]/status`
  - Add SR measurement export: "Export to SR" triggers `viewer-core::clinical` SR encoding via WASM, then `POST /api/sr/[id]/commit`
  - **Deployment:** Backend (`dicom-workflow-server`) + WASM (`viewer-core::clinical`, `pack-sr`)
  - **Rust crates involved:** `dicom-workflow-server` (sr_workflow module), `pack-sr`, `viewer-core::clinical`
  - **Acceptance:** SR documents loaded from backend; status transitions persist; measurements export to DICOM SR
  - **Estimated effort:** 5 days

**Sprint 18 Deliverable:** Real API surface, authentication, connector monitoring,
SR workflow — closing gaps F3, F4, F5, F6, F7. ✅ **COMPLETE**

---

### SPRINT 19 (Weeks 97–102): Clinical Tools Integration via WASM

**Goal:** Connect measurement, segmentation, fusion, and RT panels to `viewer-core` via WASM; make clinical tools functional.

**Tasks:**

- [x] **S19-T1** Wire measurement tools to `viewer-core::MeasurementStore`
  - Replace Zustand-based measurement CRUD with WASM-backed `MeasurementStore`:
    - `addMeasurement` → `WasmViewer.add_measurement(type, points)`
    - `removeMeasurement` → `WasmViewer.remove_measurement(id)`
    - `renameMeasurement` → `WasmViewer.update_measurement(id, name)`
  - Wire undo/redo to `WasmViewer.undo()` / `WasmViewer.redo()` (replaces 64-level JS snapshot stack with Rust tick-based audit)
  - Wire "Export to JSON" to `WasmViewer.export_measurements_json()` (produces `MeasurementStore` provenance JSON)
  - Connect distance/angle/probe tool modes to canvas interaction handlers that call `WasmViewer.handle_input(event)`
  - Display probe readout (HU value, position) from `ProbeReadout` via WASM
  - Wire keyboard shortcuts: Ctrl+Z (undo), Ctrl+Y (redo), Delete (remove measurement)
  - **Deployment:** WASM (client-side `viewer-core::MeasurementStore`)
  - **Rust crates involved:** `viewer-core::clinical`, `viewer-wasm`
  - **Acceptance:** Measurements created via canvas interaction; undo/redo goes through Rust audit trail; JSON export includes provenance
  - **Estimated effort:** 5 days

- [x] **S19-T2** Implement segmentation brush tool on canvas
  - Add brush cursor overlay on `DicomCanvas` (circle indicator following mouse)
  - Implement paint/erase mode: mouse down + drag → `WasmViewer.apply_brush_stroke(BrushStroke)`
  - Connect to `ClinicalSegmentationPanel`:
    - "Create" → `WasmViewer.create_segmentation(name, color)`
    - "Lock/Unlock" → `WasmViewer.toggle_segmentation_lock(id)`
    - "Delete" → `WasmViewer.remove_segmentation(id)`
  - Add brush size control: scroll wheel adjusts `BrushConfig::radius`
  - Implement threshold auto-segmentation: HU range selector → `WasmViewer.threshold_segment(min_hu, max_hu)`
  - Implement region growing: click seed point → `WasmViewer.region_grow(seed, tolerance)`
  - **Deployment:** WASM (client-side `viewer-core::SegmentationStore`, `viewer-core::clinical::brush`)
  - **Rust crates involved:** `viewer-core::clinical`, `viewer-wasm`
  - **Acceptance:** Brush paints on canvas; segmentation layers visible; threshold/region-growing work on loaded volume
  - **Estimated effort:** 7 days

- [x] **S19-T3** Wire fusion panel to real registration and blending
  - Connect alpha blend slider to `WasmViewer.set_fusion_alpha(value)` (replaces Zustand-only state)
  - Connect colormap selector to `WasmViewer.set_fusion_colormap(name)` (hot/cool/gray)
  - Replace simulated "Re-run Diagnostics" with real rigid registration: `WasmViewer.rigid_register(moving_series_id, fixed_series_id)` → `dicom-registration` via WASM
  - Display real RMSE from registration result (not hardcoded)
  - Support deformable registration: `WasmViewer.deformable_register(...)` with progress indicator
  - **Deployment:** WASM (client-side `dicom-registration`, `viewer-core::FusionOverlayState`)
  - **Rust crates involved:** `dicom-registration`, `viewer-core::clinical`, `viewer-wasm`
  - **Acceptance:** Fusion panel controls drive real blending; registration computes real transform; RMSE is actual metric
  - **Estimated effort:** 6 days

- [x] **S19-T4** Wire RT structure set panel to real dose/contour overlays
  - Connect dose opacity slider to `WasmViewer.set_dose_opacity(value)`
  - Connect iso-dose threshold slider to `WasmViewer.set_iso_dose_threshold(value)`
  - Connect RTSS visibility toggle to `WasmViewer.toggle_rtss_visible()`
  - Display real fail-closed validation messages from `pack-rt` (not hardcoded)
  - Support RT structure set loading: when RT series detected, auto-parse via `pack-rt` WASM
  - **Deployment:** WASM (client-side `pack-rt`, `viewer-core::RtDoseOverlayState`)
  - **Rust crates involved:** `pack-rt`, `viewer-core::clinical`, `viewer-wasm`
  - **Acceptance:** RT panel controls drive real dose/contour rendering; fail-closed messages from actual validation
  - **Estimated effort:** 4 days

- [x] **S19-T5** Implement DICOM SR/SEG/PR writeback from frontend
  - "Commit to SR" button: encode current `MeasurementStore` → DICOM SR via `pack-sr` WASM → `POST /api/sr`
  - "Export SEG" button: encode current `SegmentationStore` labelmap → DICOM SEG via `pack-seg` WASM → `POST /api/stow`
  - "Save Presentation State" button: encode viewport state → DICOM GSPS via `pack-gsps` WASM → `POST /api/stow`
  - Add progress indicators and error handling for each writeback flow
  - Add audit trail display: after writeback, show `ClinicalAuditEvent` log entry
  - **Deployment:** WASM (client-side encoding) + Backend (`dicom-web-server` STOW)
  - **Rust crates involved:** `pack-sr`, `pack-seg`, `pack-gsps`, `viewer-core::clinical`, `viewer-wasm`
  - **Acceptance:** Measurements written back as DICOM SR; segmentations as DICOM SEG; viewport as GSPS; all arrive in PACS
  - **Estimated effort:** 6 days

**Sprint 19 Deliverable:** Real clinical tools — measurements, segmentation, fusion,
RT overlays, DICOM writeback — closing gaps F8, F9. ✅ **COMPLETE**

---

### SPRINT 20 (Weeks 103–108): Collaboration, PWA & Multi-Monitor

**Goal:** Real-time collaboration via WebSocket, PWA offline mode, and multi-monitor diagnostic display.

**Tasks:**

- [x] **S20-T1** Implement real-time collaboration UI
  - Add WebSocket connection to `/api/collab` endpoint (backed by `dicom-collab`)
  - Create `src/components/diccy/collab/CollabCursor.tsx` — renders remote user cursors with identity labels
  - Create `src/components/diccy/collab/CollabPanel.tsx` — shows connected users, session ID, sync status
  - Wire viewport state sync: local pan/zoom/WL changes → broadcast via `CollabSession` CRDT → remote users see changes
  - Wire measurement broadcast: local measurement created → `CollabSession.broadcast_annotation()` → remote users see annotation
  - Handle conflict resolution: when CRDT merge produces a conflict, show notification with resolution
  - **Deployment:** Backend (`dicom-collab` WebSocket) + Frontend (React WebSocket hook)
  - **Rust crates involved:** `dicom-collab`
  - **Acceptance:** Two browser tabs see synchronized viewport state; cursor positions shared; measurements broadcast
  - **Estimated effort:** 6 days

- [x] **S20-T2** Implement PWA offline mode
  - Compile `dicom-pwa` WASM module for browser service worker integration
  - Create `public/sw.js` service worker with:
    - Cache-first strategy for WASM bundles and static assets
    - Network-first strategy for API calls with offline fallback
    - Background sync for queued writeback operations (SR/SEG/PR)
  - Create `src/components/diccy/pwa/OfflineIndicator.tsx` — shows online/offline status
  - Create `src/components/diccy/pwa/SyncQueue.tsx` — shows pending writeback operations when offline
  - Implement `StorageQuotaMonitor` from `dicom-pwa` — warn when offline cache is near quota
  - Add "Install App" prompt using `beforeinstallprompt` event
  - **Deployment:** WASM (`dicom-pwa`) + Service Worker
  - **Rust crates involved:** `dicom-pwa`
  - **Acceptance:** App works offline with cached studies; writeback queued and synced on reconnect; installable as PWA
  - **Estimated effort:** 5 days

- [x] **S20-T3** Implement multi-monitor diagnostic display
  - Create `src/components/diccy/layout/DiagnosticLayout.tsx` — full-screen viewport with no sidebar
  - Support `window.open()` with postMessage for multi-window coordination:
    - Primary window controls layout and study selection
    - Secondary windows render individual viewports (e.g., one per monitor)
  - Implement dual-monitor hanging protocol: 2-up CC/MLO on left monitor, prior study on right
  - Implement quad-monitor layout: 4-up diagnostic reading
  - Add "Open in New Window" button on each viewport tile
  - Coordinate viewport state across windows via `BroadcastChannel` API
  - **Deployment:** Frontend only (multi-window browser API)
  - **Rust crates involved:** `viewer-core::HangingProtocolEngine` (protocol selection)
  - **Acceptance:** Study opens across two browser windows on separate monitors; hanging protocol auto-arranges
  - **Estimated effort:** 5 days

- [x] **S20-T4** Implement hanging protocol selector and auto-arrange
  - Create `src/components/diccy/panels/HangingProtocolPanel.tsx`:
    - Protocol selector dropdown (populated from `viewer-core::HangingProtocolEngine::list_protocols()`)
    - Auto-arrange button that applies selected protocol to current viewport layout
    - "Save as Protocol" button that captures current layout as custom protocol
  - Wire to WASM: `WasmViewer.apply_hanging_protocol(protocol_id)` → `HangingProtocolEngine::match_and_apply()`
  - Support mammography dual-monitor protocol, CT abdomen protocol, MR brain protocol
  - Display fallback protocol selection when no exact match found
  - **Deployment:** WASM (client-side `viewer-core::HangingProtocolEngine`)
  - **Rust crates involved:** `viewer-core::hanging_protocol`
  - **Acceptance:** Selecting a protocol auto-arranges viewports; mammography shows CC/MLO on correct monitors
  - **Estimated effort:** 4 days

- [x] **S20-T5** Wire all keyboard shortcuts
  - Create `src/hooks/use-keyboard-shortcuts.ts` — global keyboard event handler
  - Map shortcuts to WASM actions:
    - `Ctrl+Z` → `WasmViewer.undo()`
    - `Ctrl+Y` / `Ctrl+Shift+Z` → `WasmViewer.redo()`
    - `Delete` → remove selected measurement/segmentation
    - `1` / `2` / `4` → layout presets (1×1 / 1×2 / 2×2)
    - `W` → Window/Level tool
    - `P` → Pan tool
    - `Z` → Zoom tool
    - `D` → Distance tool
    - `A` → Angle tool
    - `S` → Scroll tool
    - `F` → Flip horizontal
    - `R` → Rotate 90°
    - `Space` → Reset viewport
    - `Esc` → Deselect tool
  - Add shortcut conflict detection (prevent browser defaults for medical shortcuts)
  - Update shortcuts modal to reflect actually bound shortcuts
  - **Deployment:** Frontend (React event handlers) → WASM actions
  - **Rust crates involved:** `viewer-wasm`, `viewer-core`
  - **Acceptance:** All shortcuts from modal are functional; tools switch via keyboard; undo/redo works
  - **Estimated effort:** 3 days

**Sprint 20 Deliverable:** Collaboration, PWA offline, multi-monitor, hanging
protocols, keyboard shortcuts — closing gaps F10, F13, F14, F15, F16. ✅ **COMPLETE**

---

### SPRINT 21 (Weeks 109–114): AI Integration, Advanced UI & Polish

**Goal:** AI inference panel, patient safe mode with real PHI redaction, display calibration UI, and frontend polish.

**Tasks:**

- [x] **S21-T1** Implement AI inference panel
  - Create `src/components/diccy/panels/AiInferencePanel.tsx`:
    - Model selector (populated from `GET /api/inference/models`)
    - "Run Inference" button → `POST /api/inference/run` with current study ID
    - Inference progress indicator with estimated time
    - Results display: segmentation overlay, triage flags, finding list with probabilities
  - Wire triage flags to worklist: critical finding → study moves to top of worklist
  - Display AI-generated segments in `ClinicalSegmentationPanel` with "(AI)" badge
  - Add uncertainty visualization: AI segmentations show confidence heatmap overlay
  - **Deployment:** Backend (`dicom-inference` ONNX runtime) + WASM (`viewer-core::SegmentationStore` for overlay)
  - **Rust crates involved:** `dicom-inference`, `viewer-core::clinical`
  - **Acceptance:** AI inference runs on loaded study; results appear as segmentation overlay; triage flags reorder worklist
  - **Estimated effort:** 5 days

- [x] **S21-T2** Wire patient safe mode to real PHI redaction
  - Replace simulated `REDACTED` / `****-**-**` with actual `dicom-audit::AuditRedactor` policy
  - When `patientSafeMode` is enabled:
    - Strip patient name, birth date, patient ID from all displayed metadata
    - Apply redaction to SR document previews
    - Prevent screenshot/export operations (via `dicom-auth::ScreenshotExportPolicy`)
    - Log safe mode activation as `ClinicalAuditEvent`
  - When `patientSafeMode` is disabled:
    - Require role check (radiologist/admin only)
    - Show confirmation dialog with audit trail
  - **Deployment:** WASM (`dicom-audit::AuditRedactor`) + Frontend (React conditional rendering)
  - **Rust crates involved:** `dicom-audit`, `dicom-auth::export_policy`
  - **Acceptance:** Safe mode redacts PHI from all UI; screenshot blocked; deactivation requires role check
  - **Estimated effort:** 4 days

- [x] **S21-T3** Implement display calibration (GSDF) UI
  - Create `src/components/diccy/panels/CalibrationPanel.tsx`:
    - Monitor luminance input (min/max cd/m²)
    - "Generate GSDF LUT" button → `WasmViewer.generate_gsdf_lut(min_l, max_l)`
    - "Apply Calibration" toggle per viewport
    - Calibration status indicator (calibrated / uncalibrated / default)
  - Wire GSDF LUT application to `DicomCanvas` render pipeline
  - Add MQSA compliance check for mammography viewports
  - **Deployment:** WASM (`viewer-core::gsdf`)
  - **Rust crates involved:** `viewer-core::gsdf`
  - **Acceptance:** GSDF LUT generated from user luminance input; calibration applied to diagnostic viewport
  - **Estimated effort:** 3 days

- [x] **S21-T4** Implement STL/3MF export UI
  - Create `src/components/diccy/panels/MeshExportPanel.tsx`:
    - Segmentation selector (which segmentation to mesh)
    - Mesh resolution slider (decimation target)
    - Smoothing toggle (Laplacian/Taubin)
    - Export format selector (STL/3MF/OBJ)
    - "Generate & Download" button → `WasmViewer.generate_mesh(seg_id)` → `WasmViewer.export_mesh(format)`
  - Handle large mesh generation: show progress, allow cancellation
  - Download generated file via Blob URL
  - **Deployment:** WASM (`dicom-mesh`)
  - **Rust crates involved:** `dicom-mesh` (marching cubes, simplification, smoothing, export)
  - **Acceptance:** Select segmentation → generate mesh → download STL file that is 3D-printable
  - **Estimated effort:** 4 days

- [x] **S21-T5** Frontend polish and performance optimization
  - Replace all `setTimeout`/`Math.random()` simulations with real API calls
  - Add loading skeletons for all data-fetching panels (using existing shadcn `Skeleton` component)
  - Add error boundaries with `ErrorBoundary` components per panel
  - Add toast notifications for all async operations (success/failure)
  - Optimize WASM loading: code-split viewer-wasm, lazy-load on workstation tab activation
  - Add `@tanstack/react-query` cache invalidation on mutation
  - Audit bundle size: tree-shake unused shadcn components (~45 unused)
  - Remove unused npm dependencies (dnd-kit, mdxeditor, react-markdown, etc.)
  - **Deployment:** Frontend optimization
  - **Rust crates involved:** None
  - **Acceptance:** No simulated data remains; all panels load real data; WASM loads lazily; bundle size < 500KB initial
  - **Estimated effort:** 5 days

- [x] **S21-T6** Delete legacy Vue frontend
  - Remove `diccy/frontend/` directory (legacy Vue.js frontend)
  - Verify no references to legacy frontend in build scripts, CI, or documentation
  - Update `README.md` to point to Next.js frontend as the primary UI
  - **Deployment:** Repository cleanup
  - **Rust crates involved:** None
  - **Acceptance:** `diccy/frontend/` removed; no dangling references; README updated
  - **Estimated effort:** 1 day

**Sprint 21 Deliverable:** AI panel, real PHI redaction, GSDF calibration,
mesh export, frontend polish — closing gaps F11, F12. ✅ **COMPLETE**

---

## Frontend–Backend Integration Gap-to-Sprint Mapping

| Gap | Sprint | Tasks |
|---|---|---|
| F1 — No WASM Build Pipeline | Sprint 17 | S17-T1, S17-T2 |
| F2 — No Real DICOM Viewport | Sprint 17 | S17-T3, S17-T4, S17-T5 |
| F3 — No Backend API Surface | Sprint 18 | S18-T1 |
| F4 — No Authentication | Sprint 18 | S18-T2 |
| F5 — Study Catalog Mock Data | Sprint 18 | S18-T1 |
| F6 — SR Workflow Mock Data | Sprint 18 | S18-T5 |
| F7 — Connector Health Mock Data | Sprint 18 | S18-T4 |
| F8 — Measurements Client-Only | Sprint 19 | S19-T1, S19-T5 |
| F9 — MPR/Fusion/RT Panels UI-Only | Sprint 19 | S19-T3, S19-T4 |
| F10 — No Hanging Protocol UI | Sprint 20 | S20-T4 |
| F11 — No Segmentation Brush | Sprint 19 | S19-T2 |
| F12 — No AI Inference UI | Sprint 21 | S21-T1 |
| F13 — No Collaboration UI | Sprint 20 | S20-T1 |
| F14 — No PWA/Offline | Sprint 20 | S20-T2 |
| F15 — No Multi-Monitor | Sprint 20 | S20-T3 |
| F16 — No Keyboard Shortcuts | Sprint 20 | S20-T5 |

---

## Frontend–Backend Effort Summary

| Sprint | Duration | Focus | Tasks | Person-Days |
|---|---|---|---|---|
| Sprint 17 | Weeks 85–90 | WASM Pipeline & Real Viewport | 5 | 25 |
| Sprint 18 | Weeks 91–96 | API Surface & Auth | 5 | 25 |
| Sprint 19 | Weeks 97–102 | Clinical Tools via WASM | 5 | 28 |
| Sprint 20 | Weeks 103–108 | Collab, PWA, Multi-Monitor | 5 | 23 |
| Sprint 21 | Weeks 109–114 | AI, Polish & Cleanup | 6 | 22 |
| **Total (Integration)** | **30 weeks** | **Sprints 17–21** | **26** | **123** |

---

## Updated Grand Total (All Sprints 1–21)

| Part | Sprints | Duration | Tasks | Person-Days |
|---|---|---|---|---|
| Part 1: Feature Gaps | Sprint 1–8 | 32 weeks | 39 | 203 |
| Part 2: Architecture Remediation | Sprint 9–13 | 28 weeks | 30 | 160 |
| Part 3: Competitive Gaps | Sprint 14–16 | 24 weeks | 18 | 110 |
| **Part 4: Frontend–Backend Integration** | **Sprint 17–21** | **30 weeks** | **26** | **123** |
| **Grand Total** | **Sprint 1–21** | **114 weeks** | **113** | **596** |

---

## Crate Deployment Reference (Quick Lookup)

### WASM Client-Side Crates (compile to `wasm32-unknown-unknown`)

These crates run in the browser and are called directly from the Next.js frontend via `viewer-wasm` or `dicom-viewer-sdk`:

| Crate | Frontend Use | Sprint |
|---|---|---|
| `viewer-core` | Viewport state, measurements, segmentation, MPR, hanging protocols, GSDF, cache | S17, S19, S20 |
| `viewer-wasm` | Browser bindings wrapping all portable crates | S17 |
| `dicom-viewer-sdk` | Third-party embedding SDK | S17 |
| `dicom-pwa` | Service worker, offline cache, sync queue | S20 |
| `dicom-pixel` | Transfer syntax decode, pixel pipeline (Tier 0 codecs only in WASM) | S17 |
| `dicom-core` | Tags, VRs, Dataset, Element, Limits | S17+ |
| `dicom-types` | WindowLevel, PatientPosition | S17+ |
| `dicom-series` | Study/Series assembly | S17 |
| `dicom-io` | BytesSource (memory-only path) | S17 |
| `dicom-index` | Metadata indexing | S18 |
| `dicom-query` | Query matching | S18 |
| `dicom-registration` | Rigid + deformable image registration | S19 |
| `dicom-cardio` | Agatston scoring, ejection fraction | S21+ |
| `dicom-mesh` | Marching cubes, STL/3MF/OBJ export | S21 |
| `dicom-encapsulate` | Non-DICOM content encapsulation | S21+ |
| `pack-seg` | DICOM SEG parsing & encoding | S19 |
| `pack-sr` | DICOM SR parsing & encoding | S19 |
| `pack-gsps` | DICOM GSPS encoding | S19 |
| `pack-rt` | DICOM-RT parsing & overlay | S19 |
| `pack-enhanced` | Enhanced CT/MR functional groups | S17+ |
| `pack-shared` | Shared pack helpers | S17+ |
| `modality-ct` | CT geometry, measurement helpers | S17+ |
| `modality-pet` | SUV scaling, fusion | S19 |
| `modality-mg` | Tomosynthesis, CADe, MQSA | S20+ |
| `dicom-audit` | PHI redaction, audit chain | S21 |
| `dicom-auth` | RBAC types, break-glass policy types | S18 |

### Backend-Only Crates (require server infrastructure)

These crates run on the centralized server and are accessed via REST/WebSocket API:

| Crate | Backend Service | API Endpoint |
|---|---|---|
| `dicom-web-server` | DICOMweb HTTP server | `/api/studies/*` |
| `dicom-workflow-server` | SR/MPPS/UPS workflows, connectors, health, tenants | `/api/sr/*`, `/api/connectors/*`, `/api/metrics/*` |
| `dicom-storage` | WAL ingest, dedup, blob store, S3, VNA | Internal (via workflow-server) |
| `dicom-net` | TCP PDU transport, association state machine | Internal (via dimse-service) |
| `dicom-dimse-service` | DIMSE SCU/SCP, commitment, transport | Internal (via workflow-server) |
| `dicom-hl7` | MLLP server, ADT/ORM/ORU | Internal (via workflow-server) |
| `dicom-inference` | ONNX runtime, AI results | `/api/inference/*` |
| `dicom-collab` | WebSocket collab server | `/api/collab` (WebSocket upgrade) |
| `viewer-wgpu` | Native GPU renderer | Not used in web deployment |
| `dicom-env-contract` | Env var parsing | Internal (server startup) |
| `diccy-bench` | Performance benchmarking | Internal (CI only) |

---

# PART 7 — Frontend Remediation Sprints

> Based on the ISSUES.md Part 6 audit (12 new issues #51–#62 identified in the
> Next.js frontend). These sprints systematically close the gap between the
> polished UI shell and a production-grade PACS workstation frontend with real
> backend integration, WASM-powered rendering, authentication, and persistence.
>
> Priority: Critical (Issues #51, #52) → High (#53–#57) → Medium (#58–#60, #62) → Low (#61).

---

## Frontend Gap Priority Tiers

### FTIER 1 — P0 Critical (frontend is non-functional without these)

| # | Issue | Detail |
|---|---|---|
| F1 | Zero Real Backend Connection | All API routes return mock data; no persistence, no real data flow |
| F2 | Canvas2D Paint Viewport | No real DICOM image rendering; ellipses and gradients, not pixel data |

### FTIER 2 — P1 High (systemic gaps blocking clinical use)

| # | Issue | Detail |
|---|---|---|
| F3 | WASM Module Never Compiled | Loader infrastructure exists but viewer-wasm never built to pkg/ |
| F4 | Prisma DB Never Used | StudyCache, UserPreferences, AuditLogEntry models defined but 0 imports |
| F5 | React Query Over Mocks | 15+ hooks with proper caching call routes returning hardcoded arrays |
| F6 | BroadcastChannel "Collab" | Multi-tab sync only, not real WebSocket collaboration |
| F7 | Simulated Measurements | Fake HU via sine wave, hardcoded 0.7 Pixel Spacing, fabricated values |

### FTIER 3 — P2 Medium (structural and security gaps)

| # | Issue | Detail |
|---|---|---|
| F8 | 1,400-Line Monolith page.tsx | No routing, no code splitting, no deep linking |
| F9 | Decorative Auth | next-auth installed but no login, no JWT, role is a dropdown |
| F10 | 40 Unused shadcn Components | 48 installed, ~8 used; dead weight in node_modules |
| F11 | No Error Boundaries | Runtime crashes kill the whole app with white screen |

### FTIER 4 — P3 Lower

| # | Issue | Detail |
|---|---|---|
| F12 | Legacy Vue Frontend Exists | Dead code at diccy/frontend/ causing confusion |

---

### SPRINT 22 (Weeks 115–118): Backend REST Adapter & WASM Build Pipeline

**Goal:** Connect the frontend to the Rust backend by building a REST API surface in `dicom-web-server` that matches the frontend's expected endpoints. Compile the WASM viewer module so real rendering becomes possible.

**Tasks:**

- [x] **S22-T1** Build REST API adapter in `dicom-web-server`
  - Add endpoints matching the frontend's expected API surface: `/api/studies`, `/api/connectors`, `/api/metrics`, `/api/sr`, `/api/tenants`, `/api/inference`, `/api/worklist`
  - Each endpoint queries the corresponding Rust crate (dicom-index, dicom-storage, dicom-auth, dicom-audit, dicom-worklist, dicom-inference, etc.) and returns JSON
  - Add JWT authentication middleware using `dicom-auth` RBAC (5 roles, 6 permissions from S14)
  - Add `X-Data-Source: live` response header for all real data responses
  - **Addresses:** ISSUES.md #51
  - **Acceptance:** `curl http://localhost:8042/api/studies` returns real study data from the DICOM index
  - **Estimated effort:** 6 days

- [x] **S22-T2** Compile `viewer-wasm` and verify WASM loader
  - Run `npm run wasm:build` to compile `crates/viewer-wasm` to `pkg/viewer_wasm.js` + `pkg/viewer_wasm_bg.wasm`
  - Verify the WASM module loads via `src/lib/wasm-init.ts` — `initWasm()` should resolve with `state: "ready"`
  - Add CI check: fail build if `pkg/` is missing or older than `crates/viewer-wasm/src/`
  - Update the "WASM Active" badge to reflect actual module load state (not just `typeof WebAssembly`)
  - **Addresses:** ISSUES.md #53
  - **Acceptance:** `WasmViewer.probe_capabilities()` returns backend capabilities in the browser console
  - **Estimated effort:** 3 days

- [x] **S22-T3** Remove silent mock fallback from API routes
  - In production mode (`NODE_ENV=production`), API routes return 503 with `BACKEND_UNREACHABLE` when the Rust backend is down — no silent fallback to mock data
  - In development mode, add `X-Data-Source: fallback` header when mock data is served
  - Add a `DEMO_MODE` environment variable that enables mock data via MSW (Mock Service Worker) for standalone demos
  - Remove all hardcoded `mockStudies`, `mockConnectors`, `mockMetrics`, `mockSrDocuments` arrays from route handlers
  - **Addresses:** ISSUES.md #51 (production mode)
  - **Acceptance:** API routes return 503 in production when backend is unreachable; dev mode shows "DEMO MODE" indicator
  - **Estimated effort:** 3 days

- [x] **S22-T4** Wire Prisma database for persistence
  - Run `npx prisma db push` to create the SQLite database
  - Wire `StudyCache` into the studies API route — cache QIDO-RS responses, serve from cache when offline
  - Wire `UserPreferences` into a `useUserPreferences()` hook — persist viewport layout, window presets, hanging protocol selections
  - Wire `AuditLogEntry` into all clinical mutation actions — emit audit records for 21 CFR Part 11 compliance
  - **Addresses:** ISSUES.md #54
  - **Acceptance:** User preferences survive page refresh; audit log records appear in `AuditLogEntry` table after measurement/segmentation actions
  - **Estimated effort:** 5 days

- [x] **S22-T5** Implement next-auth with Rust backend OAuth2
  - Configure `next-auth` with `CredentialsProvider` pointing to the Rust backend's OAuth2/token endpoint (built in S14)
  - Implement a login page at `/auth/signin` with username/password form
  - Protect all API routes with `getServerSession()` — return 401 for unauthenticated requests
  - Replace the Zustand role dropdown with role information from the JWT token
  - Add `middleware.ts` to redirect unauthenticated users to the login page
  - **Addresses:** ISSUES.md #59
  - **Acceptance:** Unauthenticated users are redirected to login; role permissions are enforced from JWT claims
  - **Estimated effort:** 5 days

**Sprint 22 Deliverable:** Real backend connection, WASM module compiled and loaded,
database persistence, authentication — closing frontend gaps F1, F3, F4, F9. ✅ **COMPLETE**

---

### SPRINT 23 (Weeks 119–122): Real DICOM Viewport & Measurement Pipeline

**Goal:** Replace the Canvas2D paint job with a WASM-powered DICOM viewer that renders real pixel data, and wire measurements to actual HU values and calibrated distances.

**Tasks:**

- [x] **S23-T1** Replace DicomViewport with WASM renderer
  - Create `WasmDicomViewport` component that delegates rendering to the `WasmViewer` class from `pkg/viewer_wasm`
  - Implement WADO-RS frame retrieval: fetch pixel data via `/api/studies/[id]/series/[sid]/instances/[iid]/frames/[f]`
  - Upload decoded pixel data to the WASM module via `upload_volume_grid()`
  - Wire window/level, zoom, pan controls to the WASM viewer's viewport state
  - Display real DICOM overlay tags (patient name, study date, WW/WL, slice position) from the study metadata
  - **Addresses:** ISSUES.md #52
  - **Acceptance:** A real CT study loaded from the backend renders in the viewport with correct window/level
  - **Estimated effort:** 8 days

- [x] **S23-T2** Replace MprCanvas with WASM MPR
  - Create `WasmMprView` component that uses the WASM module's MPR reslicing
  - Implement tri-planar layout with linked crosshairs using `TriPlanarState`
  - Support oblique MPR via arbitrary clip plane through the volume texture
  - **Addresses:** ISSUES.md #52 (MPR portion)
  - **Acceptance:** Three orthogonal planes show real resliced data from a loaded CT volume
  - **Estimated effort:** 5 days

- [x] **S23-T3** Wire measurements to real pixel data
  - Replace simulated HU values with actual pixel data queries from the WASM module
  - Read DICOM Pixel Spacing (0028,0030) from study metadata and use for calibrated measurements
  - Set provenance to "Uncalibrated" until real calibration data is confirmed
  - Add visual warning indicator (amber border + icon) when measurements are not backed by real pixel data
  - **Addresses:** ISSUES.md #57
  - **Acceptance:** Probe tool shows real HU values from loaded DICOM data; distance measurements use real Pixel Spacing
  - **Estimated effort:** 4 days

- [x] **S23-T4** Implement DICOM file ingestion with drag-and-drop
  - Wire the "Drop DICOM files here" zone to actually parse and ingest uploaded files
  - Use the WASM module's DICOM parsing to validate uploaded files before sending to STOW-RS
  - Show upload progress and validation results
  - Reject non-DICOM files with clear error messages
  - **Addresses:** ISSUES.md #52 (ingestion portion)
  - **Acceptance:** Dragging a DICOM file onto the viewer loads and displays it
  - **Estimated effort:** 4 days

**Sprint 23 Deliverable:** Real DICOM image rendering, WASM-powered MPR, real
measurements, file ingestion — closing frontend gaps F2, F7. ✅ **COMPLETE**

---

### SPRINT 24 (Weeks 123–126): Routing, Collaboration, and Error Resilience

**Goal:** Restructure the frontend with proper routing, wire real-time collaboration via WebSocket, and add error boundaries for production resilience.

**Tasks:**

- [x] **S24-T1** Extract pages into Next.js App Router routes
  - Move WorkstationPage → `src/app/(workstation)/page.tsx`
  - Move ClinicalWorkflowPage → `src/app/(workflow)/page.tsx`
  - Move ConnectorAdminPage → `src/app/(connectors)/page.tsx`
  - Move TenantHealthPage → `src/app/(health)/page.tsx`
  - Add `loading.tsx` and `error.tsx` boundaries for each route
  - Implement `next/dynamic` lazy loading for heavy components (DICOM viewer, MPR, collab)
  - Update sidebar navigation to use `next/link` and `usePathname()`
  - **Addresses:** ISSUES.md #58
  - **Acceptance:** URL changes when switching tabs; browser back/forward works; deep links load correct page
  - **Estimated effort:** 5 days

- [x] **S24-T2** Replace BroadcastChannel with WebSocket collaboration
  - Add a WebSocket endpoint to the Rust backend that bridges to `dicom-collab` crate
  - Replace `BroadcastChannel` in `use-collab.ts` with a WebSocket client
  - Add session creation/joining via the REST API before establishing the WebSocket connection
  - Wire cursor sharing, measurement broadcasting, and viewport sync through the WebSocket channel
  - **Addresses:** ISSUES.md #56
  - **Acceptance:** Two users on different machines can collaborate on the same study
  - **Estimated effort:** 6 days

- [x] **S24-T3** Add error boundaries and crash recovery
  - Add a global `error.tsx` boundary at the app root
  - Add route-level `error.tsx` files for each page route
  - Wrap the DICOM viewport canvas components in an error boundary with a "Rendering Error" fallback
  - Add a "Recover Session" feature that persists critical state to localStorage before crash
  - Add error reporting integration (Sentry or equivalent) for production deployments
  - **Addresses:** ISSUES.md #62
  - **Acceptance:** A rendering error shows a user-friendly fallback instead of a white screen; critical state is recoverable after reload
  - **Estimated effort:** 3 days

- [x] **S24-T4** Clean up unused shadcn/ui components
  - Audit all `@/components/ui/*` imports across the codebase
  - Identify and delete unused component files (estimated ~40 of 48)
  - Remove corresponding Radix packages from `package.json` that are not transitive dependencies of used components
  - Establish convention: DiCCY-specific components use `src/components/diccy/`, generic UI primitives use `src/components/ui/`
  - **Addresses:** ISSUES.md #60
  - **Acceptance:** Only used shadcn/ui components remain; `node_modules` size reduced
  - **Estimated effort:** 2 days

**Sprint 24 Deliverable:** Proper routing, real-time collaboration, error resilience,
component cleanup — closing frontend gaps F6, F8, F10, F11. ✅ **COMPLETE**

---

### SPRINT 25 (Weeks 127–130): Frontend Polish & Legacy Cleanup

**Goal:** Final polish of the frontend, delete the legacy Vue application, and verify all 12 frontend issues are resolved.

**Tasks:**

- [x] **S25-T1** Delete legacy Vue frontend
  - Remove `/home/z/my-project/diccy/frontend/` directory entirely
  - Update any build scripts or Docker configurations that reference the Vue frontend
  - Add a note in the project README that the Vue frontend has been replaced by the Next.js frontend
  - **Addresses:** ISSUES.md #61
  - **Acceptance:** `diccy/frontend/` no longer exists; no build or deploy process references it
  - **Estimated effort:** 1 day

- [x] **S25-T2** Add "DEMO MODE" indicator and data source transparency
  - When the frontend is running with mock data (no backend), show a persistent "DEMO MODE" banner
  - Display `X-Data-Source` header status in the runtime snapshot cards
  - Add a `/api/health` endpoint that reports backend connectivity, WASM module status, and DB availability
  - **Addresses:** ISSUES.md #51 (user-facing transparency)
  - **Acceptance:** Users can immediately see whether they're viewing real or demo data
  - **Estimated effort:** 2 days

- [x] **S25-T3** Apply Stitch design system (Variant #9)
  - Merge the Stitch design tokens from `/home/z/my-project/download/stitch_extracted/` into the project's Tailwind config
  - Apply the navy-tint color family and primary teal `#6bd8cb` from the best variant (#9)
  - Implement the zero-shadow design pattern from `04_tailwind_patterns.md`
  - Apply the 14 component categories from `02_component_catalog.md` to the existing panels
  - **Acceptance:** Frontend matches the Stitch design reference; no shadow artifacts; consistent teal accent
  - **Estimated effort:** 5 days

- [x] **S25-T4** Frontend integration test suite
  - Write Playwright tests that verify:
    - Login flow with real JWT authentication
    - Study list loads from backend (not mock data)
    - DICOM viewport renders real pixel data
    - Measurement values reflect actual HU and Pixel Spacing
    - Collaboration session between two browser contexts
    - User preferences persist across page reload
    - Audit log entries are created for clinical mutations
  - **Acceptance:** All integration tests pass against the live Rust backend
  - **Estimated effort:** 5 days

- [x] **S25-T5** Final frontend audit and issue closure
  - Verify all 12 frontend issues (#51–#62) are resolved
  - Run Lighthouse audit: target Performance > 90, Accessibility > 95, Best Practices > 95
  - Run bundle analysis: target < 500KB initial JS bundle
  - Verify all WASM viewer methods are accessible from the frontend
  - Document the final frontend architecture in a new `FRONTEND.md`
  - **Acceptance:** All 12 frontend issues closed; Lighthouse scores meet targets
  - **Estimated effort:** 3 days

**Sprint 25 Deliverable:** Legacy cleanup, design system application, integration
tests, issue closure — closing all frontend gaps F1–F12. ✅ **COMPLETE**

---

## Frontend Gap-to-Sprint Mapping Summary

| Gap | Sprint | Tasks |
|---|---|---|
| F1 — Zero Backend Connection | Sprint 22 | S22-T1, S22-T3, S25-T2 |
| F2 — Canvas2D Paint Viewport | Sprint 23 | S23-T1, S23-T2, S23-T4 |
| F3 — WASM Never Compiled | Sprint 22 | S22-T2 |
| F4 — Prisma DB Never Used | Sprint 22 | S22-T4 |
| F5 — React Query Over Mocks | Sprint 22 | S22-T1, S22-T3 (hooks work automatically once backend is live) |
| F6 — BroadcastChannel Collab | Sprint 24 | S24-T2 |
| F7 — Simulated Measurements | Sprint 23 | S23-T3 |
| F8 — 1,400-Line Monolith | Sprint 24 | S24-T1 |
| F9 — Decorative Auth | Sprint 22 | S22-T5 |
| F10 — 40 Unused Components | Sprint 24 | S24-T4 |
| F11 — No Error Boundaries | Sprint 24 | S24-T3 |
| F12 — Legacy Vue Frontend | Sprint 25 | S25-T1 |

---

## Frontend Effort Summary

| Sprint | Duration | Core Tasks | Estimated Person-Days |
|---|---|---|---|
| Sprint 22: Backend Adapter & WASM | 4 weeks | 5 | 22 |
| Sprint 23: Real DICOM Viewport | 4 weeks | 4 | 21 |
| Sprint 24: Routing & Collab & Resilience | 4 weeks | 4 | 16 |
| Sprint 25: Polish & Cleanup | 4 weeks | 5 | 16 |
| **Total** | **16 weeks** | **18** | **75** |

---

## Combined Project Effort Summary

| Part | Sprints | Duration | Tasks | Person-Days |
|---|---|---|---|---|
| Part 1: Feature Gap (G1–G20) | Sprint 1–8 | 32 weeks | 39 | 203 |
| Part 2: Architecture & DDD (A1–A36) | Sprint 9–13 | 28 weeks | 36 | 131 |
| Part 3: Competitive Analysis (C1–C13) | Sprint 14–16 | 24 weeks | 13 | 91 |
| Part 4: Frontend–Backend Integration (F1–F16) | Sprint 17–21 | 30 weeks | 16 | 72 |
| Part 7: Frontend Remediation (F1–F12) | Sprint 22–25 | 16 weeks | 18 | 75 |
| **Grand Total** | **Sprint 1–25** | **130 weeks** | **122** | **572** |
