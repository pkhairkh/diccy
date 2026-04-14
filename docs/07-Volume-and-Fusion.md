# Volume and fusion

## Overview

Status: **Implemented baseline with deferred advanced architecture** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

Normative: Volume/MPR requirements in this document define deterministic behavior.

Current implemented baseline:
- deterministic `VolumeGrid` assembly and CPU reslice APIs (`axial`, `coronal`, `sagittal`, `arbitrary`) exist in `viewer-core`,
- `VolumeGrid` includes optional patient-space metadata with deterministic voxel<->patient mapping helpers,
- voxel-space and patient-space MPR request surfaces coexist with deterministic axis-aligned equivalence coverage,
- WASM host exposes tri-planar validation interactions with synchronized crosshair + slab controls,
- native demo parity is available via `cargo run -p rdvf --example tri_planar_demo`,
- PET/CT fusion baseline executes deterministic resample + blend with fail-closed limits.

This document specifies:
- implemented baseline behavior,
- deferred target architecture,
- verification requirements.

---

## 1. Volume assembly

### Inputs

Requirements:

- **REQ-VOL-921:** Inputs **MUST** be validated against the envelope and geometry rules before volume assembly proceeds.

A `Series` of frames with valid geometry:
- Image Orientation (Patient) (IOP)
- Image Position (Patient) (IPP)
- Pixel Spacing (sx, sy)
- Deterministic slice ordering (per `docs/04`)

### Volume grid model (implemented baseline)

Requirements:

- **REQ-VOL-922:** The `VolumeGrid` metadata **MUST** be sufficient for deterministic voxel indexing, ordering provenance, and spacing-aware processing in the baseline implementation.

Current `VolumeGrid` baseline fields:
- `dimensions: [x, y, z]`
- `spacing_um: [sx, sy, sz]`
- `orientation_ras: [[i8; 3]; 3]` (baseline fixed orientation basis)
- `source_instance_uids: Vec<String>` (deterministic source ordering provenance)
- `voxels: Vec<i32>`
- `non_uniform_spacing: bool`
- `patient_geometry: Option<PatientGeometry>` where:
  - `origin_um: [i64; 3]`
  - `basis_vectors_um: [[i64; 3]; 3]`
  - `frame_of_reference_uid: Option<String>`

`VolumeGrid` evolution decision:
- baseline remains backward-compatible for voxel-space callers,
- migration path is **patient-space capable baseline** (optional metadata + adapters),
- `LegacyVolumeGrid` adapter APIs preserve serialized compatibility while enabling deterministic patient mapping.

### Assembly constraints (normative)

Requirements:

- **REQ-VOL-903:** All frames in a volume **MUST** share the same Rows/Columns, Pixel Spacing (within epsilon), and IOP (within epsilon, accounting for sign flips).
- **REQ-VOL-904:** IPP values **MUST** form a monotonic progression along the slice direction within epsilon.
- **REQ-VOL-905:** If these constraints are not met, volume assembly **MUST** fail closed with `InvalidVolumeGeometry`; the series **MAY** still be browsed as a 2D stack.

### Slice spacing (sz) (normative)

Compute slice coordinate `t_k = dot(normal, IPP_k)` (per `docs/04`).

Let deltas be `d_k = t_{k+1} - t_k` for sorted slices.

Requirements:
- **REQ-VOL-906:** If `nz < 2`, set `sz = 1.0` mm and mark spacing as “unknown” (no through-plane measurement).
- **REQ-VOL-907:** If `nz >= 2`, `sz` **MUST** be computed as the median of `|d_k|`.
- **REQ-VOL-908:** If any `|d_k - sz|` exceeds a configured tolerance, volume assembly **MUST** fail closed unless explicitly configured for non-uniform spacing (advanced mode only).

### Data population (normative)

Requirements:

- **REQ-VOL-909:** The volume **MUST** be populated from the **post-modality** values (after Modality LUT/Rescale) but **before** VOI/window, so that MPR can apply VOI consistently.
- **REQ-VOL-910:** The mapping from slices to `k` index **MUST** follow deterministic ordering.

---

## 2. Multi-planar reconstruction (MPR baseline)

### MPR planes (implemented baseline)

Requirements:

- **REQ-VOL-911:** Plane definitions **MUST** produce deterministic reslice outputs for fixed parameters.

Current baseline semantics:
- `axial`, `coronal`, and `sagittal` requests are resolved in volume voxel space,
- `arbitrary` requests currently use voxel-space origin/axes parameters,
- `PatientMprRequest` supports patient-space plane requests (`axial`, `coronal`, `sagittal`) while preserving voxel API compatibility,
- slab controls are available with deterministic `average`/`max` composition ordering and bounded memory behavior,
- baseline output determinism is defined by fixed input volume + request + limits.

Implementation strategy (baseline):
- Construct a reslice request in voxel-space coordinates.
- Sample the volume into a 2D output frame deterministically.

### Sampling modes

- **Nearest**: required (deterministic, fast; see **REQ-VOL-912**).
- **Linear**: optional if deterministic fixed-point or stable float implementation is validated.

Requirements:

- **REQ-VOL-912:** Nearest-neighbor sampling **MUST** be supported for MPR outputs.
- **REQ-VOL-913:** Sampling **MUST** be deterministic for a fixed plane definition and configuration.
- **REQ-VOL-914:** Out-of-bounds samples **MUST** be treated as background (0) or clamped, per a documented policy (default: background 0).

### Roadmap: advanced 3D behavior (deferred)

Target architecture (deferred):
- anisotropic slab interpolation and higher-order kernels beyond deterministic baseline nearest/linear policies,
- streaming/LOD assisted large-volume interactions with deterministic fallback boundaries,
- broader patient-space arbitrary-plane semantics with explicit conformance gating.

---

## 3. Fusion (PET over CT)

### Implemented current scope

Current implemented scope in `modality-pet` includes:
- fusion eligibility validation (`Frame of Reference UID` presence/match),
- deterministic SUV scaling helper validation/application,
- fail-closed precondition checks (missing geometry, size/buffer limits),
- PET-on-CT deterministic resampling on CT reference grid,
- deterministic PET overlay blending (`HotIron`, fixed alpha policy),
- deterministic output hashing/performance gates in tests.

Requirements:
- **REQ-VOL-915:** Both series **MUST** have Frame of Reference UID (0020,0052).
- **REQ-VOL-916:** The Frame of Reference UID **MUST** match exactly; otherwise fusion **MUST** be rejected (no implicit registration).
- **REQ-VOL-917:** Both series **MUST** have valid patient-space transforms.
- **REQ-VOL-920:** PET pack **MUST** document all required tags for scaling and failure behavior.

### Fusion baseline pipeline (implemented)

Status: **Implemented baseline** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

Baseline pipeline sequence:
1. Assemble CT volume grid (reference grid).
2. Assemble PET volume grid.
3. Resample PET onto CT grid using deterministic sampling.
4. Apply PET-specific scaling.
5. Blend PET color overlay on CT grayscale using explicit alpha rules.

Deferred advanced requirements:
- **REQ-VOL-918:** Fusion **MUST NOT** attempt deformable registration.
- **REQ-VOL-919:** Blending **MUST** be deterministic and parameterized (alpha, colormap selection).

### Segmentation and RT packs (optional)

Requirements:
- **REQ-VOL-923 / REQ-SEG-300:** Segmentation labelmaps **MUST** align to the referenced source image using Frame of Reference UID and pixel grid; mismatches **MUST** fail closed.
- **REQ-VOL-924 / REQ-SEG-302:** If segmentation resampling is required, it **MUST** use deterministic nearest-neighbor mapping in voxel space and preserve integer labels, with explicit provenance of source grid and resampling policy.
- **REQ-VOL-925 / REQ-RT-350:** RT Dose values **MUST** apply Dose Grid Scaling (3004,000E) in `f64`; non-finite dose values **MUST** fail closed.
- **REQ-VOL-926 / REQ-RT-351:** RT Dose alignment **MUST** use Image Position/Orientation plus Grid Frame Offset Vector; missing or invalid alignment **MUST** fail closed. The initial scope requires exact pixel spacing, orientation, and frame-of-reference matches before overlay.
- **REQ-VOL-927 / REQ-SEG-303 / REQ-RT-352:** Dose/segmentation overlays **MUST** be deterministic with explicit colormap and alpha policies.
- **REQ-VOL-928:** Enhanced multi-frame volumes **MUST** validate per-frame geometry consistency before assembly; inconsistencies **MUST** fail closed.
- **REQ-VOL-929 / REQ-RT-353:** RT Structure Set contours **MUST** be planar and aligned to the referenced image plane using Frame of Reference UID, Image Position, Image Orientation, and Pixel Spacing; mismatches **MUST** fail closed and no resampling is allowed in the initial scope. Supported contour geometric types are `OPEN_PLANAR`, `CLOSED_PLANAR`, and `CLOSEDPLANAR_XOR`.

Workstation scope note: segmentation overlays include deterministic support for both single-frame and multi-frame segmentations when alignment constraints are satisfied; frame selection outside declared `NumberOfFrames` range **MUST** fail closed.

---

## 4. Verification requirements

Verification:

- Unit tests **MUST** validate envelope/geometry preconditions and volume grid mapping (REQ-VOL-921..REQ-VOL-922).
- Unit tests **MUST** validate volume assembly constraints and spacing rules (REQ-VOL-903..REQ-VOL-908).
- Unit or corpus tests **MUST** validate post-modality population ordering and deterministic slice indexing (REQ-VOL-909..REQ-VOL-910).
- Synthetic geometry tests **MUST** validate deterministic MPR reslice outputs (REQ-VOL-911..REQ-VOL-914).
- Unit tests **MUST** validate voxel-space and patient-space axis-aligned MPR equivalence and slab determinism.
- Corpus tests **SHOULD** include:
  - CT volume with known spacing and expected MPR slices,
  - PET/CT pair with matching Frame of Reference for fusion scope checks and deterministic output hashes (REQ-VOL-915..REQ-VOL-920),
  - SEG and RT Dose samples with known alignment and deterministic overlay outputs (REQ-VOL-923..REQ-VOL-929).

## 5. Scalability and web constraints (informative)

WebGPU-based volume rendering at scale often requires residency/LOD strategies to bound memory and bandwidth. RDVF keeps baseline volume features conservative and treats advanced streaming/LOD as an optional extension.

Reference:
- Herzberger et al., “Residency Octree: A Hybrid Approach for Scalable Web-Based Multi-Volume Rendering” (IEEE TVCG, 2024): https://pubmed.ncbi.nlm.nih.gov/37889813/

Normative constraints:

- **REQ-VOL-901:** Default builds **MUST** enforce `max_cache_bytes` and `max_decompressed_bytes` (see `docs/09`) such that volume assembly cannot exceed configured budgets.
- **REQ-VOL-902:** Any streaming/LOD volume rendering strategy **MUST** be implemented behind an explicit feature/pack and **MUST** define deterministic fallback behavior when higher-resolution data is unavailable.

## Advanced visualization status matrix (2026-02-22)

| Capability | Status | Notes |
|---|---|---|
| Tri-planar linked crosshair | Implemented | `viewer-wasm` tri-planar controls synchronize axial/coronal/sagittal indices. |
| Slab controls (`slab_thickness`, `slab_mode`) | Implemented | Deterministic `average` and `max` modes wired through MPR request path. |
| Patient-space axis-aligned requests | Implemented baseline | `PatientMprRequest` path is available; broader arbitrary-plane workflows remain incremental. |
| Orientation overlays/cube UX | Implemented prototype | Host UI now renders deterministic plane orientation indicators for tri-planar navigation context. |
| PET/CT fusion execution baseline | Implemented | Deterministic resample + blend path and precondition checks are available. |
| Fusion quantification panel UX | Implemented prototype | Host UI exposes bounded fusion parameters, diagnostics, ROI summary, and deterministic export bundle outputs. |
