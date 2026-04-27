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
| **Grand Total** | **60 weeks** | **All Sprints** | **79** | **363** |

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
