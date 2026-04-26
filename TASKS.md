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

- [ ] **S3-T1** Implement Hanging Protocol matching engine
  - Parse DICOM Supplement 60 Hanging Protocol IOD
  - Match rules: modality, body part, laterality, study description
  - Image set definition and display set assignment
  - Fallback protocol selection
  - Integrate with existing `HangingProtocolTemplate` and `HangingProtocolApplication`
  - **Acceptance:** Mammography (CC/MLO, current/prior) auto-arranged
  - **Estimated effort:** 6 days

- [ ] **S3-T2** Implement study prefetching engine
  - Rule-based prefetch on worklist entry (fetch priors by modality/body part)
  - Priority queue with STAT escalation
  - DICweb WADO-RS prefetch with progressive loading
  - Cache integration with existing `DeterministicCache`
  - **Acceptance:** Priors loaded before radiologist opens study
  - **Estimated effort:** 5 days

- [ ] **S3-T3** Implement FHIR R4 adapter crate (`dicom-fhir`)
  - New crate `crates/dicom-fhir`
  - Map DICOM Patient/Study/Series to FHIR Patient/ImagingStudy
  - Map DICOM SR to FHIR Observation (per HL7 DICOM-SR on FHIR IG)
  - REST client for FHIR server communication
  - **Acceptance:** DICOM SR measurement report retrievable as FHIR Observation
  - **Estimated effort:** 6 days

- [ ] **S3-T4** Implement HL7 v2 adapter crate (`dicom-hl7`)
  - New crate `crates/dicom-hl7`
  - ADT message parsing (A01/A02/A03/A08) for patient sync
  - ORM message parsing for order entry
  - ORU message generation for result delivery
  - MLLP transport layer
  - **Acceptance:** Patient demographics synced from ADT feed, orders received via ORM
  - **Estimated effort:** 6 days

- [ ] **S3-T5** Implement display calibration (GSDF)
  - New module `viewer-core::gsdf` or crate `dicom-display`
  - DICOM Grayscale Standard Display Function (GSDF) implementation
  - Monitor luminance measurement input (min/max LL)
  - LUT generation for calibrated grayscale rendering
  - Per-viewport calibration application
  - **Acceptance:** GSDF LUT generated and applied to diagnostic viewport
  - **Estimated effort:** 3 days

**Sprint 3 Deliverable:** Workflow automation (hanging protocols, prefetching),
interoperability (FHIR/HL7), display calibration — closing gaps G7, G8, G9, G12.

---

### SPRINT 4 (Weeks 13–16): Image Registration & AI Pipeline

**Goal:** Multi-modality fusion and AI inference integration.

**Tasks:**

- [ ] **S4-T1** Implement rigid image registration
  - New crate `crates/dicom-registration`
  - Rigid registration (6 DOF: 3 translation + 3 rotation)
  - Based on mutual information or cross-correlation metric
  - Multi-resolution pyramid for speed
  - Output as DICOM Registration IOD
  - Connect to existing `FusionOverlayState` and `FusionRegistrationState`
  - **Acceptance:** CT-MR rigid registration produces correct overlay
  - **Estimated effort:** 6 days

- [ ] **S4-T2** Implement deformable image registration
  - B-spline deformable registration
  - DVF (Deformation Vector Field) output
  - DICOM Deformable Spatial Registration IOD encoding
  - Apply DVF for PET-CT or CT-MR fusion overlay
  - **Acceptance:** Deformable CT-MR registration with sub-voxel accuracy on phantom
  - **Estimated effort:** 7 days

- [ ] **S4-T3** Implement AI inference runtime crate (`dicom-inference`)
  - New crate `crates/dicom-inference`
  - ONNX Runtime bindings (via `ort` crate) for model inference
  - Model manifest: input/output specifications, modality constraints
  - Preprocessing pipeline: resampling, windowing, normalization
  - Postprocessing: contour extraction from probability maps
  - **Acceptance:** ONNX model runs on CT volume, outputs segmentation
  - **Estimated effort:** 6 days

- [ ] **S4-T4** Implement DICOM AI Results (AIR) IOD
  - New crate or module in `pack-sr` extension
  - DICOM Supplement 228 (AI Results) encoding
  - CADe/CADx finding encoding with probability and algorithm identity
  - Integration with `SegmentationStore` for AI-generated segments
  - **Acceptance:** AI inference results stored as DICOM AIR, viewable in OHIF
  - **Estimated effort:** 4 days

- [ ] **S4-T5** Implement AI worklist prioritization
  - Analyze incoming studies for AI triage flags
  - Priority scoring (critical finding detection)
  - Worklist reordering in `dicom-worklist` integration
  - Notification hooks for critical findings
  - **Acceptance:** Studies with suspected critical findings surfaced first
  - **Estimated effort:** 3 days

**Sprint 4 Deliverable:** Rigid/deformable registration, AI inference pipeline,
AI results encoding — closing gaps G10, G11.

---

### SPRINT 5 (Weeks 17–20): 3D Printing & Extended Reality

**Goal:** STL export for surgical planning and XR visualization.

**Tasks:**

- [ ] **S5-T1** Implement mesh generation from segmentation
  - Marching cubes algorithm on segmentation labelmaps
  - Mesh simplification (decimation) for performance
  - Mesh smoothing (Laplacian or Taubin)
  - **Acceptance:** Smooth mesh generated from bone segmentation of CT
  - **Estimated effort:** 5 days

- [ ] **S5-T2** Implement STL/3MF/OBJ export
  - Binary STL export
  - 3MF export with units and metadata
  - OBJ export with materials
  - DICOM encapsulation of 3D model (Supplement 205)
  - **Acceptance:** STL file 3D-printable from bone segmentation
  - **Estimated effort:** 4 days

- [ ] **S5-T3** Implement XR visualization crate (`dicom-xr`)
  - New crate `crates/dicom-xr` (rename existing `modality-xr` to `modality-cr` first)
  - OpenXR / WebXR rendering pipeline
  - Volume rendering in stereoscopic view
  - Hand-tracking for interactive clipping and measurement
  - 4D DICOM support (time-series volume playback)
  - **Acceptance:** CT volume viewable in VR headset with clip plane interaction
  - **Estimated effort:** 8 days

- [ ] **S5-T4** Implement AR holographic overlay
  - Mixed-reality overlay of 3D models on patient
  - Coordinate system registration (DICOM patient → world space)
  - HoloLens / Apple Vision Pro target support
  - Surgical navigation marker tracking interface
  - **Acceptance:** 3D bone model overlaid on phantom in AR view
  - **Estimated effort:** 6 days

**Sprint 5 Deliverable:** 3D printing pipeline, XR visualization — closing gaps G13, G14.

---

### SPRINT 6 (Weeks 21–24): Cloud-Native & Collaboration

**Goal:** Kubernetes deployment, real-time collaboration, and multi-tenancy.

**Tasks:**

- [ ] **S6-T1** Containerize and create Helm chart
  - Multi-stage Dockerfile for `dicom-web-server`
  - Helm chart with values for storage backend, auth, TLS
  - Horizontal Pod Autoscaler for DICOMweb endpoints
  - Liveness/readiness probes using existing `readiness_contract` tests
  - **Acceptance:** `helm install diccy` deploys working PACS on K8s
  - **Estimated effort:** 4 days

- [ ] **S6-T2** Implement object storage backend
  - S3-compatible backend for `dicom-storage`
  - Multipart upload for large DICOM instances
  - Lifecycle policies for tiered storage
  - **Acceptance:** 1TB study stored/retrieved from MinIO
  - **Estimated effort:** 4 days

- [ ] **S6-T3** Implement real-time collaboration
  - WebSocket-based sync layer (`dicom-collab` crate)
  - Shared viewport state (pan, zoom, window/level)
  - Cursor sharing with user identity
  - Measurement annotation broadcasting
  - Conflict resolution using CRDT (operational transform)
  - **Acceptance:** Two radiologists view same study, see each other's cursor
  - **Estimated effort:** 6 days

- [ ] **S6-T4** Implement teleradiology gateway
  - Bandwidth-adaptive streaming (JPEG 2000 progressive)
  - Low-latency interaction forwarding
  - Offline mode with sync on reconnect
  - **Acceptance:** Radiologist reads study over 10 Mbps link without perceptible lag
  - **Estimated effort:** 5 days

**Sprint 6 Deliverable:** Cloud-native deployment, collaboration — closing gaps G15, G16.

---

### SPRINT 7 (Weeks 25–28): Sub-Specialty Modules

**Goal:** Cardiovascular quantification, mammography, and pathology.

**Tasks:**

- [ ] **S7-T1** Implement calcium scoring module
  - New module in `modality-ct` or new crate `dicom-cardio`
  - Agatston score calculation on non-contrast cardiac CT
  - Automatic coronary artery calcium detection
  - DICOM Supplement 97 TID 3905 encoding
  - **Acceptance:** Agatston score computed and stored as DICOM SR
  - **Estimated effort:** 6 days

- [ ] **S7-T2** Implement coronary artery analysis
  - Centerline extraction via vessel tracking
  - Curved MPR along vessel centerline
  - Stenosis measurement tool
  - Vessel diameter quantification
  - **Acceptance:** Curved MPR of LAD with stenosis measurement
  - **Estimated effort:** 7 days

- [ ] **S7-T3** Implement ejection fraction calculation
  - LV/RV contour detection on cardiac MR
  - Simpson's method volume calculation
  - ED/ES frame detection
  - EF% with uncertainty bounds
  - **Acceptance:** EF% computed from short-axis cardiac MR
  - **Estimated effort:** 5 days

- [ ] **S7-T4** Implement mammography workflow module
  - Expand `modality-mg` beyond SOP gating
  - Tomosynthesis (3D mammography) slice navigation
  - CADe integration hooks for breast lesion detection
  - Dual-monitor hanging protocol (CC/MLO arrangement)
  - MQSA compliance display controls
  - **Acceptance:** Tomosynthesis stack navigable with CADe overlay
  - **Estimated effort:** 6 days

- [ ] **S7-T5** Implement whole-slide imaging (WSI) module
  - New crate `crates/dicom-wsi`
  - DICOM Supplement 145 WSI IOD parsing
  - Pyramid/tile-based streaming rendering
  - Deep zoom with on-demand tile retrieval
  - Pathology measurement tools (cell counting, area)
  - **Acceptance:** WSI slide navigable at 40x with sub-second tile load
  - **Estimated effort:** 7 days

**Sprint 7 Deliverable:** Cardiovascular, mammography, pathology modules — closing gaps G17, G18, G19.

---

### SPRINT 8 (Weeks 29–32): Encapsulation & Enterprise Integration

**Goal:** DICOM encapsulation, enterprise VNA features, and final polish.

**Tasks:**

- [ ] **S8-T1** Implement DICOM encapsulation crate (`dicom-encapsulate`)
  - PDF → DICOM Encapsulated Document
  - JPEG/TIFF → DICOM Secondary Capture
  - Video (MP4/AVI) → DICOM Video Photographic Image
  - CDA document → DICOM Encapsulated CDA
  - **Acceptance:** PDF report encapsulated as DICOM, queryable via QIDO-RS
  - **Estimated effort:** 4 days

- [ ] **S8-T2** Implement Vendor Neutral Archive features
  - Deduplication with canonical hash (partially exists)
  - Retention policy engine (configurable per-tenant rules)
  - Study lifecycle management (archive, purge, legal hold)
  - XDS-I integration profile for cross-enterprise sharing
  - **Acceptance:** Multi-tenant VNA with retention policies enforced
  - **Estimated effort:** 6 days

- [ ] **S8-T3** Implement IHE integration profiles
  - IHE Scheduled Workflow (SWF) profile
  - IHE Patient Information Reconciliation (PIR)
  - IHE Access to Radiology Information (ARI)
  - IHE Cross-enterprise Document Sharing (XDS-I.b)
  - IHE AI Results (AIR) profile
  - **Acceptance:** IHE Connectathon test suite passes for SWF profile
  - **Estimated effort:** 5 days

- [ ] **S8-T4** Performance optimization sprint
  - GPU volume rendering benchmark suite (512x512x2048 CT)
  - Memory-mapped volume loading for >4GB studies
  - Parallel MPR slice computation (rayon)
  - WGPU render pass batching
  - WASM bundle size optimization
  - **Acceptance:** 512x512x2048 CT loads in <3s, MPR scrolls at 60fps
  - **Estimated effort:** 5 days

- [ ] **S8-T5** Integration testing and documentation
  - End-to-end test: DIMSE C-STORE → Index → QIDO → WADO → Render → SR Writeback
  - API documentation audit (all `pub` items documented)
  - Architecture decision records for new crates
  - Regulatory conformance envelope update (docs/03)
  - **Acceptance:** Full round-trip test passes, docs complete
  - **Estimated effort:** 5 days

**Sprint 8 Deliverable:** Encapsulation, VNA, IHE profiles, performance — closing gap G20.

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
