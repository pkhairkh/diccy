# DICOM conformance envelope

## Purpose

This document defines the **only** DICOM features the framework claims to support. Anything outside this envelope **MUST** fail closed with a typed error (see `docs/13-Error-Model-and-Telemetry.md`).

## Envelope model

The envelope is defined by:

1. **SOP Classes** (what kinds of objects we accept)
2. **Transfer Syntaxes** (how datasets/pixels are encoded)
3. **Semantic interpretation rules** (geometry, pixel transforms)
4. **Explicit non-support list** + required failure behavior
5. **Default limits** (resource and decompression limits that bound behavior)

## Envelope version

- `envelope_version`: `1.1`
  - Declares the workstation profile plus the workstation-completeness profile for Tier 2 clinical packs.

## Build profiles and applicability

- **Core build (repository default):** minimal-core feature set with strict fail-closed behavior.
- **Workstation profile build:** profile build that enables the required interoperability and clinical packs for workstation conformance.
- **Workstation-completeness profile build:** workstation profile plus Tier 2 clinical completeness packs (`pack-enhanced`, `pack-us`, `pack-nm`, `pack-xa`, `pack-seg`, `pack-rt`, `pack-sr`, `gsps`).
- Normative requirements marked as workstation baseline/profile apply to the workstation profile build, not to the bare minimal-core build.

### Normative requirements

- **REQ-CONF-001:** The repository **MUST** version the envelope as `envelope_version` (see `docs/14`) and treat default limits changes as envelope changes (see `docs/09`).

- **REQ-CONF-002:** The framework **MUST** validate SOP Class UID and Transfer Syntax UID before attempting pixel decode.
- **REQ-CONF-003:** The framework **MUST** reject datasets with missing required tags for the chosen SOP Class tier.
- **REQ-CONF-004:** The framework **MUST** provide deterministic ordering and interpretation for series/frames in-envelope.

Verification:
- Release checklist **MUST** confirm `envelope_version` is present and updated when defaults/semantics change.
- Unit/integration tests **MUST** cover unsupported SOP Class/Transfer Syntax early rejection and missing-required-tag failures.
- Series assembly tests **MUST** validate deterministic ordering for Tier 0 SOP classes (see `docs/04`).

---

## RC-2026.02.12 release matrices (normative)

The following release matrix is the executable conformance baseline for RC-2026.02.12. Inputs outside active rows **MUST** fail closed.

Status semantics:

- `Supported`: active in current release baseline.
- `Deferred`: implemented only behind explicit feature/profile enablement; not active in default baseline.
- `Unsupported`: outside release scope; fail closed.

### SOP Class matrix (supported/deferred/unsupported)

| SOP Class | SOP Class UID | RC-2026.02.12 status | Activation rule | Fail-closed behavior when inactive |
|---|---:|---|---|---|
| CT Image Storage | `1.2.840.10008.5.1.4.1.1.2` | Supported | Core baseline | `UnsupportedSopClass` |
| MR Image Storage | `1.2.840.10008.5.1.4.1.1.4` | Supported | Core baseline | `UnsupportedSopClass` |
| Secondary Capture Image Storage | `1.2.840.10008.5.1.4.1.1.7` | Supported | Core baseline | `UnsupportedSopClass` |
| Multi-frame Grayscale Byte SC | `1.2.840.10008.5.1.4.1.1.7.2` | Supported | Core baseline | `UnsupportedSopClass` |
| Multi-frame Grayscale Word SC | `1.2.840.10008.5.1.4.1.1.7.3` | Supported | Core baseline | `UnsupportedSopClass` |
| Multi-frame True Color SC | `1.2.840.10008.5.1.4.1.1.7.4` | Supported | Core baseline | `UnsupportedSopClass` |
| PET Image Storage | `1.2.840.10008.5.1.4.1.1.128` | Supported | Workstation baseline (`modality-pet`) | `UnsupportedSopClass` |
| Computed Radiography Image Storage | `1.2.840.10008.5.1.4.1.1.1` | Supported | Workstation baseline (`modality-xr`) | `UnsupportedSopClass` |
| Digital X-Ray (For Presentation) | `1.2.840.10008.5.1.4.1.1.1.1` | Supported | Workstation baseline (`modality-xr`) | `UnsupportedSopClass` |
| Enhanced CT Image Storage | `1.2.840.10008.5.1.4.1.1.2.1` | Supported | Workstation-completeness baseline (`pack-enhanced`) | `UnsupportedSopClass` |
| Enhanced MR Image Storage | `1.2.840.10008.5.1.4.1.1.4.1` | Supported | Workstation-completeness baseline (`pack-enhanced`) | `UnsupportedSopClass` |
| Ultrasound Image Storage | `1.2.840.10008.5.1.4.1.1.6.1` | Supported | Workstation-completeness baseline (`pack-us`) | `UnsupportedSopClass` |
| Ultrasound Multi-frame Image Storage | `1.2.840.10008.5.1.4.1.1.3.1` | Supported | Workstation-completeness baseline (`pack-us`) | `UnsupportedSopClass` |
| Nuclear Medicine Image Storage | `1.2.840.10008.5.1.4.1.1.20` | Supported | Workstation-completeness baseline (`pack-nm`) | `UnsupportedSopClass` |
| XA Image Storage | `1.2.840.10008.5.1.4.1.1.12.1` | Supported | Workstation-completeness baseline (`pack-xa`) | `UnsupportedSopClass` |
| XRF Image Storage | `1.2.840.10008.5.1.4.1.1.12.2` | Supported | Workstation-completeness baseline (`pack-xa`) | `UnsupportedSopClass` |
| Segmentation Storage | `1.2.840.10008.5.1.4.1.1.66.4` | Supported | Workstation-completeness baseline (`pack-seg`) | `UnsupportedSopClass` |
| RT Dose / Structure / Plan | `1.2.840.10008.5.1.4.1.1.481.2/.3/.5` | Supported | Workstation-completeness baseline (`pack-rt`) | `UnsupportedSopClass` |
| Basic Text SR / Comprehensive SR | `1.2.840.10008.5.1.4.1.1.88.11/.33` | Supported | Workstation-completeness baseline (`pack-sr`) | `UnsupportedSopClass` |
| GSPS | `1.2.840.10008.5.1.4.1.1.11.1` | Supported | Workstation-completeness baseline (`gsps`) | `UnsupportedSopClass` |
| Mammography X-Ray (Presentation/Processing) | `1.2.840.10008.5.1.4.1.1.1.2/.2.1` | Deferred | `modality-mg` remains non-activated for RC-2026.02.12 completeness profile | `UnsupportedSopClass` |
| Any SOP Class UID not listed above | n/a | Unsupported | n/a | `UnsupportedSopClass` |

### Transfer Syntax matrix (supported/deferred/unsupported)

| Transfer Syntax | UID | RC-2026.02.12 status | Activation rule | Fail-closed behavior when inactive |
|---|---:|---|---|---|
| Implicit VR Little Endian | `1.2.840.10008.1.2` | Supported | Core baseline | `UnsupportedTransferSyntax` |
| Explicit VR Little Endian | `1.2.840.10008.1.2.1` | Supported | Core baseline | `UnsupportedTransferSyntax` |
| RLE Lossless | `1.2.840.10008.1.2.5` | Supported | Core baseline | `UnsupportedTransferSyntax` |
| JPEG Baseline (Process 1) | `1.2.840.10008.1.2.4.50` | Supported | Core baseline | `UnsupportedTransferSyntax` |
| Deflated Explicit VR Little Endian | `1.2.840.10008.1.2.1.99` | Supported | Workstation baseline (`tier1-deflate`) | `UnsupportedTransferSyntax` |
| JPEG-LS Lossless / Near Lossless | `1.2.840.10008.1.2.4.80/.81` | Supported | Workstation baseline (`codec-jpegls`) | `UnsupportedTransferSyntax` |
| JPEG 2000 Lossless / Lossy | `1.2.840.10008.1.2.4.90/.91` | Supported | Workstation baseline (`codec-j2k`) | `UnsupportedTransferSyntax` |
| MPEG-2 / H.264 / HEVC families | `1.2.840.10008.1.2.4.100..108` (+ fragmentable variants) | Deferred (feature-gated I/O recognition only) | I/O layer may recognize UID when codec feature is enabled; pixel decode remains out-of-envelope | `UnsupportedTransferSyntax` at pixel decode boundary |
| Explicit VR Big Endian | `1.2.840.10008.1.2.2` | Unsupported | n/a | `UnsupportedTransferSyntax` |
| Any Transfer Syntax UID not listed above | n/a | Unsupported | n/a | `UnsupportedTransferSyntax` |

### Transfer syntax semantics by processing layer (normative)

To remove ambiguity, transfer syntax handling is split into two enforcement layers:

- **I/O recognition layer (`dicom-io`)**: may recognize additional transfer syntax UIDs when an explicit codec feature is enabled.
- **Pixel decode layer (`dicom-pixel`)**: accepts only syntaxes with implemented decoders; recognized-but-undecodable syntaxes fail closed.

Rules:
- The I/O layer **MUST NOT** imply pixel decode support by UID recognition alone.
- The pixel layer **MUST** return typed fail-closed errors for recognized-but-undecodable transfer syntaxes.
- Release claims **MUST** treat a syntax as `Supported` only when both layers are implemented and verified.

### DIMSE command matrix (service class + negotiated contexts)

| DIMSE command | Service class | Required abstract syntax negotiation | Required transfer syntax negotiation | RC-2026.02.12 status | Fail-closed behavior when unavailable |
|---|---|---|---|---|---|
| C-ECHO | Verification | `1.2.840.10008.1.1` | LE syntaxes from association policy | Supported | `DecodeError` / rejected presentation context |
| C-STORE | Storage | Storage SOP Class negotiated in association policy | LE syntaxes from association policy | Supported | `UnsupportedSopClass` or association rejection |
| C-FIND | Study Root Query/Retrieve Information Model - FIND | `1.2.840.10008.5.1.4.1.2.2.1` | LE syntaxes from association policy | Deferred | Enable `dimse-c-find`; else `DecodeError` for unsupported command |
| C-MOVE | Study Root Query/Retrieve Information Model - MOVE | `1.2.840.10008.5.1.4.1.2.2.2` | LE syntaxes from association policy | Deferred | Enable `dimse-c-move`; else `DecodeError` for unsupported command |
| C-GET | Study Root Query/Retrieve Information Model - GET | `1.2.840.10008.5.1.4.1.2.2.3` | LE syntaxes from association policy | Deferred | Enable `dimse-c-get`; else `DecodeError` for unsupported command |
| Any other DIMSE command field | n/a | n/a | n/a | Unsupported | `DecodeError` |

### DICOMweb route/method matrix

| Route | Allowed methods | Operation | Feature dependency | RC-2026.02.12 status | Fail-closed behavior for non-matrix input |
|---|---|---|---|---|---|
| `/studies` | `GET`, `HEAD` | QIDO studies | `qido` | Supported (packaged runtime) | unsupported method/path -> `DecodeError` |
| `/studies` | `POST` | STOW studies | `stow` | Supported (packaged runtime) | unsupported query/content-type -> `DecodeError` |
| `/series` | `GET`, `HEAD` | QIDO all series | `qido` | Supported (packaged runtime) | unsupported method -> `DecodeError` |
| `/instances` | `GET`, `HEAD` | QIDO all instances | `qido` | Supported (packaged runtime) | unsupported method -> `DecodeError` |
| `/studies/{StudyUID}` | `POST` | STOW scoped by study UID | `stow` | Supported (packaged runtime) | unsupported query/content-type -> `DecodeError` |
| `/studies/{StudyUID}` | `GET`, `HEAD` | WADO study retrieve | `wado` | Supported (packaged runtime) | unsupported method/query/path -> `DecodeError` |
| `/studies/{StudyUID}/series/{SeriesUID}` | `GET`, `HEAD` | WADO series retrieve | `wado` | Supported (packaged runtime) | unsupported method/query/path -> `DecodeError` |
| `/studies/{StudyUID}/series` | `GET`, `HEAD` | QIDO series by study | `qido` | Supported (packaged runtime) | unsupported method -> `DecodeError` |
| `/studies/{StudyUID}/instances` | `GET`, `HEAD` | QIDO instances by study | `qido` | Supported (packaged runtime) | unsupported method -> `DecodeError` |
| `/studies/{StudyUID}/series/{SeriesUID}/instances` | `GET`, `HEAD` | QIDO instances by study+series | `qido` | Supported (packaged runtime) | unsupported method -> `DecodeError` |
| `/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}` | `GET`, `HEAD` | WADO instance retrieve | `wado` | Supported (packaged runtime) | unsupported method/query/path -> `DecodeError` |
| Any route/method pair not listed above | n/a | n/a | n/a | Unsupported | `DecodeError` |

Verification:
- Unit tests **MUST** cover representative `UnsupportedSopClass`, `UnsupportedTransferSyntax`, unsupported DIMSE command, and unsupported DICOMweb route/method rejection paths for this matrix.
- Corpus manifest **MUST** include release-tagged rows mapping supported matrix coverage plus representative reject cases.

---

## Transfer Syntax support tiers

### Tier 0 (core, MUST)

Tier 0 is the minimal support set required for the framework to be considered functional.

| Transfer Syntax | UID | Pixel encoding | Status |
|---|---:|---|---|
| Implicit VR Little Endian | `1.2.840.10008.1.2` | uncompressed | MUST |
| Explicit VR Little Endian | `1.2.840.10008.1.2.1` | uncompressed | MUST |
| RLE Lossless | `1.2.840.10008.1.2.5` | RLE | MUST |
| JPEG Baseline (Process 1) | `1.2.840.10008.1.2.4.50` | JPEG baseline 8-bit | MUST |

Tier 0 constraints:
- **REQ-TS-201:** JPEG Baseline support **MUST** be limited to the DICOM constraints for Process 1.
- **REQ-TS-202:** RLE support **MUST** implement DICOM RLE framing and segment rules and **MUST** validate segment sizes against limits before allocation.

### Tier 1 (workstation baseline, MUST)

Tier 1 expands compatibility and is required for workstation conformance.

| Transfer Syntax | UID | Status |
|---|---:|---|
| Deflated Explicit VR Little Endian | `1.2.840.10008.1.2.1.99` | MUST (workstation profile) |
| JPEG-LS Lossless | `1.2.840.10008.1.2.4.80` | MUST (workstation profile) |
| JPEG-LS Near Lossless | `1.2.840.10008.1.2.4.81` | MUST (workstation profile) |
| JPEG 2000 Lossless | `1.2.840.10008.1.2.4.90` | MUST (workstation profile) |
| JPEG 2000 Lossy | `1.2.840.10008.1.2.4.91` | MUST (workstation profile) |

Assumption: JPEG-LS and JPEG2000 will require either dedicated pure-Rust implementations or isolated FFI.
- Justification: mature, fully-conformant pure-Rust decoders may not exist for all codecs.
- **REQ-TS-203:** Codec pack acceptance **MUST** include differential tests against reference decoders and fuzzing coverage at the boundary (see `docs/10` and `docs/09`).
- **REQ-TS-204:** Deflated Explicit VR LE support **MUST** enforce zlib/deflate decoding with output bounded by `max_decompressed_bytes` and **MUST** reject invalid streams.
When the `codec-jpegls` or `codec-j2k` features are enabled, the corresponding decoders are available in `dicom-pixel` and are exercised by codec-boundary fuzz targets and corpus samples.

Verification:
- Unit tests **MUST** cover JPEG Baseline constraints and RLE framing/segment validation.
- Corpus tests **MUST** include Tier 0 JPEG Baseline and RLE samples with known outputs.

### Video transfer syntaxes (feature gated, NOT supported by default)

The following video transfer syntaxes are defined by the DICOM standard and are **out-of-envelope** unless the corresponding codec feature is enabled. The default build **MUST** reject them.

MPEG-2:
- MPEG-2 Main Profile / Main Level: `1.2.840.10008.1.2.4.100` (and fragmentable `1.2.840.10008.1.2.4.100.1`) — feature `codec-mpeg2`.
- MPEG-2 Main Profile / High Level: `1.2.840.10008.1.2.4.101` (and fragmentable `1.2.840.10008.1.2.4.101.1`) — feature `codec-mpeg2`.

H.264 / AVC:
- H.264 High Profile / Level 4.1: `1.2.840.10008.1.2.4.102` (and fragmentable `1.2.840.10008.1.2.4.102.1`) — feature `codec-h264`.
- H.264 BD-compatible High Profile / Level 4.1: `1.2.840.10008.1.2.4.103` (and fragmentable `1.2.840.10008.1.2.4.103.1`) — feature `codec-h264`.
- H.264 High Profile / Level 4.2: `1.2.840.10008.1.2.4.104` (and fragmentable `1.2.840.10008.1.2.4.104.1`) — feature `codec-h264`.
- H.264 High Profile / Level 3.2: `1.2.840.10008.1.2.4.105` (and fragmentable `1.2.840.10008.1.2.4.105.1`) — feature `codec-h264`.
- H.264 Stereo High Profile / Level 4.2: `1.2.840.10008.1.2.4.106` (and fragmentable `1.2.840.10008.1.2.4.106.1`) — feature `codec-h264`.

HEVC / H.265:
- HEVC Main Profile / Level 5.1: `1.2.840.10008.1.2.4.107` (and fragmentable `1.2.840.10008.1.2.4.107.1`) — feature `codec-hevc`.
- HEVC Main 10 Profile / Level 5.1: `1.2.840.10008.1.2.4.108` (and fragmentable `1.2.840.10008.1.2.4.108.1`) — feature `codec-hevc`.

- **REQ-TS-205:** Video transfer syntax UIDs **MUST** be rejected as `UnsupportedTransferSyntax` unless the corresponding codec feature is enabled.

Note: Enabling the feature gates permits transfer syntax recognition for metadata parsing, but the core pixel pipeline does not implement video decoding.

### Out-of-envelope transfer syntaxes (informative)

The following transfer syntaxes are defined in the DICOM standard but are **out-of-envelope** for this project unless explicitly added as features. Per **REQ-CONF-002**, these UIDs **MUST** be rejected as `UnsupportedTransferSyntax`:
- JPEG Extended (Process 2/4).
- Explicit VR Big Endian (`1.2.840.10008.1.2.2`).
- JPEG Lossless (Process 14 and 14 [SV1]).
- JPEG 2000 Multi-component (Part 2).
- MPEG-2, MPEG-4 AVC/H.264, and HEVC/H.265 video transfer syntaxes.
- JPEG XL and other emerging transfer syntaxes not explicitly listed in Tier 0/1.

---

## SOP Class support tiers

### Tier 0 (core, MUST)

Tier 0 focuses on commonly used cross-sectional imaging and secondary capture.

| SOP Class | SOP Class UID | Status |
|---|---:|---|
| CT Image Storage | `1.2.840.10008.5.1.4.1.1.2` | MUST |
| MR Image Storage | `1.2.840.10008.5.1.4.1.1.4` | MUST |
| Secondary Capture Image Storage | `1.2.840.10008.5.1.4.1.1.7` | MUST |
| Multi-frame Grayscale Byte SC | `1.2.840.10008.5.1.4.1.1.7.2` | MUST |
| Multi-frame Grayscale Word SC | `1.2.840.10008.5.1.4.1.1.7.3` | MUST |
| Multi-frame True Color SC | `1.2.840.10008.5.1.4.1.1.7.4` | MUST |

Tier 0 constraints:
- **REQ-CONF-010:** CT/MR Tier 0 support assumes “classic” (non-enhanced) single-frame Instances.
- **REQ-CONF-011:** Multi-frame support in Tier 0 is limited to the SC multi-frame SOP Classes above.

### Tier 1 (workstation baseline)

| SOP Class | SOP Class UID | Status |
|---|---:|---|
| PET Image Storage | `1.2.840.10008.5.1.4.1.1.128` | MUST (workstation profile) |
| Computed Radiography Image Storage | `1.2.840.10008.5.1.4.1.1.1` | MUST (workstation profile) |
| Digital X-Ray Image Storage - For Presentation | `1.2.840.10008.5.1.4.1.1.1.1` | MUST (workstation profile) |

Assumption: Mammography SOP Class set is implemented via an MG pack and declared there.
- Justification: MG introduces additional presentation/processing variants and hanging protocol expectations that should not enlarge core scope.
- **REQ-CONF-012:** MG pack **MUST** include a manifest listing SOP Class UIDs and a test that validates them against a vendor-neutral registry (manual or CI-enabled, per project policy).

Verification:
- Tier 0 corpus tests **MUST** include classic CT/MR single-frame and SC multi-frame samples.
- If an MG pack is enabled, its manifest test **MUST** validate SOP Class UID declarations.

---

### Tier 2 (clinical completeness packs, workstation baseline)

Tier 2 defines **clinical completeness packs** that expand beyond core imaging into enhanced multi-frame objects, ultrasound/NM/XA, and derived objects (SEG/SR/RT/GSPS). For RC-2026.02.12 workstation-completeness builds, these packs are activated and **MUST** be verified with deterministic/fail-closed tests.

| Pack | SOP Classes (examples) | Status |
|---|---|---|
| Enhanced CT/MR pack | Enhanced CT (`1.2.840.10008.5.1.4.1.1.2.1`), Enhanced MR (`1.2.840.10008.5.1.4.1.1.4.1`) | MUST (workstation-completeness profile RC-2026.02.12) |
| Ultrasound pack | Ultrasound Image Storage (`1.2.840.10008.5.1.4.1.1.6.1`), Ultrasound Multi-frame (`1.2.840.10008.5.1.4.1.1.3.1`) | MUST (workstation-completeness profile RC-2026.02.12) |
| Nuclear Medicine pack | NM Image Storage (`1.2.840.10008.5.1.4.1.1.20`) | MUST (workstation-completeness profile RC-2026.02.12) |
| XA/XRF pack | XA Image Storage (`1.2.840.10008.5.1.4.1.1.12.1`), XRF Image Storage (`1.2.840.10008.5.1.4.1.1.12.2`) | MUST (workstation-completeness profile RC-2026.02.12) |
| Segmentation pack | Segmentation Storage (`1.2.840.10008.5.1.4.1.1.66.4`) | MUST (workstation-completeness profile RC-2026.02.12) |
| RT pack | RT Dose (`1.2.840.10008.5.1.4.1.1.481.2`), RT Structure Set (`1.2.840.10008.5.1.4.1.1.481.3`), RT Plan (`1.2.840.10008.5.1.4.1.1.481.5`) | MUST (workstation-completeness profile RC-2026.02.12) |
| SR pack | Basic Text SR (`1.2.840.10008.5.1.4.1.1.88.11`), Comprehensive SR (`1.2.840.10008.5.1.4.1.1.88.33`) | MUST (workstation-completeness profile RC-2026.02.12) |
| Presentation State pack | Grayscale Softcopy Presentation State (`1.2.840.10008.5.1.4.1.1.11.1`) | MUST (workstation-completeness profile RC-2026.02.12) |

Requirements:
- **REQ-CONF-083:** Each clinical completeness pack **MUST** include a manifest that lists supported SOP Class UIDs and required tags; the manifest **MUST** be validated by tests before enabling the pack in a release.
- **REQ-SOP-300:** Imaging pack manifests **MUST** enumerate supported SOP Class UIDs, and tests **MUST** validate the manifest contents against pack constants before release.
- **REQ-SOP-301:** When an imaging pack feature is disabled, its SOP Class UIDs **MUST** fail closed with `UnsupportedSopClass`.
- **REQ-CONF-084:** Enhanced CT/MR pack implementations **MUST** honor per-frame functional group semantics and **MUST** fail closed if required shared/per-frame groups are incomplete or contradictory. The current scope includes `Plane Position`, `Plane Orientation`, `Pixel Measures`, `Pixel Value Transformation`, and `Frame Content` (`In-Stack Position Number`) validation; other functional groups are out-of-envelope until explicitly added.
- **REQ-ENH-350:** Enhanced CT/MR parsing **MUST** validate shared/per-frame functional groups for `Plane Position`, `Plane Orientation`, `Pixel Measures`, and `Pixel Value Transformation`; when `Frame Content Sequence` is present it **MUST** include a positive `In-Stack Position Number`. The number of per-frame functional-group items **MUST** equal `NumberOfFrames`. `Pixel Spacing` values **MUST** be finite and > 0, and `Slice Thickness` (if present) **MUST** be > 0. Missing or inconsistent groups **MUST** fail closed.
- **REQ-CONF-085:** Ultrasound/NM/XA packs **MUST** document required calibration and timing tags for measurements; missing or invalid calibration **MUST** disable physical-unit measurements (pixel-only fallback) and surface structured warnings.
- **REQ-CONF-086 / REQ-SEG-301:** Segmentation pack implementations **MUST** validate referenced SOP Instance UID and Frame of Reference UID relationships to source images and **MUST** fail closed on mismatches. The current scope supports binary, uncompressed single-frame and multi-frame segmentations with `Bits Allocated = 1` (`Bits Stored = 1`, `High Bit = 0`); multi-frame overlays **MUST** fail closed when frame selection is out of range.
- **REQ-CONF-087:** RT pack implementations **MUST** validate spatial registration and Frame of Reference consistency before overlaying structures or dose grids; missing or invalid registration **MUST** fail closed. The initial scope supports RT Dose with exact pixel spacing and orientation matches and RT Structure Set planar contours (`OPEN_PLANAR`, `CLOSED_PLANAR`, `CLOSEDPLANAR_XOR`) that lie on the referenced image plane (no resampling).
- **REQ-CONF-088 / REQ-SR-300:** SR pack implementations **MUST** support deterministic SR ingestion/extraction for in-envelope content while rejecting references to non-existent or out-of-envelope images. The current implemented scope includes read extraction of `NUM`/`TEXT`/`CODE` items, baseline deterministic SR authoring/update APIs (`SrAuthoringBuilder`, `apply_sr_update`), and workflow-service create/update/retrieve commit endpoints with fail-closed auth/idempotency/version-conflict handling.
- **REQ-CONF-089:** Presentation State pack implementations **MUST** apply GSPS/shutter/annotation rules deterministically and **MUST NOT** modify source datasets. The initial scope supports rectangular, circular, and polygonal shutters plus deterministic graphic annotations.
- **REQ-CONF-090 / REQ-RT-354:** RT Plan parsing **MUST** require a referenced Structure Set SOP Instance UID and **MUST** fail closed when the reference is missing or mismatched.
- **REQ-CONF-091:** GSPS graphic annotations **MUST** support the workstation baseline graphic object set (`POINT`, `POLYLINE`, `INTERPOLATED`, `CIRCLE`, `ELLIPSE`) with deterministic rendering and clipping; unsupported types **MUST** fail closed. `CIRCLE` objects **MUST** provide exactly two points (center + perimeter), and `ELLIPSE` objects **MUST** provide exactly four points defining orthogonal major/minor axes with a shared center.

Verification:
- Pack manifest tests **MUST** validate SOP Class declarations and required-tag checklists (REQ-CONF-083).
- Enhanced multi-frame tests **MUST** include incomplete functional group cases that fail closed (REQ-CONF-084).
- Segmentation/RT/SR packs **MUST** include reference-resolution and UID mismatch tests that fail closed (REQ-CONF-086..REQ-CONF-090).
- GSPS/presentation packs **MUST** include deterministic render tests for ordering, shutter rules, and graphic annotation placement (REQ-CONF-089..REQ-CONF-091).

---

## Required tags and interpretation rules

This section defines per-SOP minimal requirements. Missing required tags **MUST** cause a failure.

### Common required tags (all in-envelope images)

- **REQ-CONF-020:** For all in-envelope images, the framework **MUST** require the following common tags, including the Image Pixel Module fields listed below.

- (0008,0016) **SOP Class UID** — MUST
- (0008,0018) **SOP Instance UID** — MUST
- (0020,000D) **Study Instance UID** — MUST (for study/series browsing)
- (0020,000E) **Series Instance UID** — MUST (for series assembly)
- (7FE0,0010) **Pixel Data** — MUST (Float/Double Pixel Data are out-of-envelope)

Image Pixel Module:
- (0028,0002) **Samples per Pixel** — MUST
- (0028,0004) **Photometric Interpretation** — MUST
- (0028,0010) **Rows** — MUST
- (0028,0011) **Columns** — MUST
- (0028,0100) **Bits Allocated** — MUST
- (0028,0101) **Bits Stored** — MUST
- (0028,0102) **High Bit** — MUST
- (0028,0103) **Pixel Representation** — MUST for MONOCHROME*
- (0028,0006) **Planar Configuration** — MUST if Samples per Pixel > 1
- (0028,0008) **Number of Frames** — MAY (if absent, treated as 1)

Under **REQ-CONF-020**, workstation-profile photometric handling is constrained as follows:
- Accepted Photometric Interpretation values are `MONOCHROME1`, `MONOCHROME2`, `RGB`, `YBR_FULL`, and `YBR_FULL_422`.
- `YBR_FULL` **MUST** be accepted only for native uncompressed/deflated Little Endian (`1.2.840.10008.1.2`, `1.2.840.10008.1.2.1`, `1.2.840.10008.1.2.1.99`) or `RLE Lossless` (`1.2.840.10008.1.2.5`) transfer syntaxes; other combinations **MUST** fail closed.
- `YBR_FULL_422` **MUST** be accepted only for native uncompressed/deflated Little Endian transfer syntaxes (`1.2.840.10008.1.2`, `1.2.840.10008.1.2.1`, `1.2.840.10008.1.2.1.99`); other combinations **MUST** fail closed.
- `YBR_FULL_422` **MUST** enforce `Samples per Pixel = 3`, `Planar Configuration = 0`, and even `Columns`; violations **MUST** fail closed.

VOI/Modality:
- (0028,1050)/(0028,1051) **Window Center/Width** — MAY
- (0028,1052)/(0028,1053) **Rescale Intercept/Slope** — MAY
- (0028,3010) **VOI LUT Sequence** — MAY
- (0028,3000) **Modality LUT Sequence** — MAY
- (0028,1056) **VOI LUT Function** — MAY (default `LINEAR`)

Verification:
- Unit tests **MUST** validate missing-required-tag failures for each Tier 0 SOP Class and for Image Pixel Module requirements.

### CT Image Storage (Tier 0)

Required for correct geometry:
- (0020,0032) **Image Position (Patient)** — MUST
- (0020,0037) **Image Orientation (Patient)** — MUST
- (0028,0030) **Pixel Spacing** — MUST

Recommended (for series quality checks):
- (0020,0013) Instance Number — SHOULD
- (0018,0050) Slice Thickness — SHOULD
- (0018,0088) Spacing Between Slices — MAY

Interpretation requirements:
- **REQ-CONF-030:** The series assembler **MUST** order slices by projection of IPP onto the normal vector derived from IOP.
- **REQ-CONF-031:** The IOP direction cosines **MUST** be validated as approximately orthonormal within a configured epsilon.
- **REQ-CONF-032:** If required geometry tags are missing or invalid, the Instance **MUST** be rejected for CT series assembly.

### MR Image Storage (Tier 0)

Same geometry requirements as CT (IPP/IOP/Pixel Spacing) **MUST** be present for geometry-aware operations.

If geometry tags are missing:
- The viewer **MAY** permit 2D display as “unplaced image” (no patient-space positioning), but
- **REQ-CONF-040:** measurements and volume assembly **MUST** be disabled and must surface a structured error state.

### Secondary Capture (SC) (Tier 0)

- Geometry tags are not required.
- **REQ-CONF-050:** SC images **MUST** be renderable in 2D without patient-space transforms.
- **REQ-CONF-051:** Measurements on SC images **MUST** be disabled unless Pixel Spacing is present and validated.

Verification:
- Series assembly tests **MUST** validate CT ordering/geometry rejection behavior.
- UI/API tests **MUST** validate MR “unplaced image” measurement disablement.
- SC unit tests **MUST** validate 2D rendering eligibility and measurement gating.

---

## Multi-frame handling policy (normative)

- **REQ-CONF-060:** If Number of Frames is absent, the Instance is treated as single-frame.
- For SC multi-frame SOP Classes:
  - **REQ-CONF-061:** Shared attributes apply to all frames.
  - **REQ-CONF-062:** Frames are ordered by frame index (1..N) and exposed as independent Frames with stable Frame IDs.
- For enhanced multi-frame SOP Classes (Tier 2 packs):
  - **REQ-CONF-063:** The implementation **MUST** follow per-frame functional group semantics.
  - **REQ-CONF-064:** If functional groups are present but incomplete, the decoder **MUST** fail closed rather than guessing.

Active-baseline constraint (informative):
- Encapsulated multi-frame pixel data requiring Basic/Extended Offset Table indexing is currently out-of-envelope in the active baseline; decode attempts fail closed with deterministic typed errors.

Verification:
- Multi-frame unit tests **MUST** cover absent Number of Frames defaulting, SC frame ordering, and stable Frame IDs.
- Enhanced multi-frame pack tests **MUST** include incomplete functional group cases that fail closed.
- Encapsulated codec tests **MUST** assert fail-closed behavior for multi-frame encapsulated payloads when offset-table decoding support is not enabled.

---

## Networking support (DIMSE) (normative)

Networking support is part of the workstation baseline. Workstation-profile builds **MUST** expose DIMSE APIs through `dicom-net` and `dicom-dimse`, including Query/Retrieve services (C-FIND/C-MOVE/C-GET) with policy controls and limits.

Association profile (informative):
- Application Context UID: `1.2.840.10008.3.1.1.1`.
- `dicom-dimse-service::DimseServerConfig::default()` uses a workstation policy profile that includes Verification, core imaging storage SOP classes, Study Root Query/Retrieve abstract syntaxes, and Little Endian transfer syntaxes with max PDU length 16,384.
- C-STORE acceptance is policy-driven; integrators include Storage SOP Class UIDs in `AssociationPolicy::supported_abstract_syntaxes`.

Supported DIMSE command set (informative):
- Workstation baseline: C-ECHO, C-STORE, C-FIND, C-MOVE, C-GET command parsing and service dispatch.

- **REQ-NET-300:** UL PDU parsing **MUST** validate PDU length fields and **MUST** enforce `max_pdu_bytes` before allocation.
- **REQ-NET-301:** AE Titles **MUST** be ASCII, space-padded to 16 bytes, and the trimmed value **MUST** be non-empty.
- **REQ-NET-302:** The Application Context UID **MUST** equal the DICOM application context (`1.2.840.10008.3.1.1.1`) or fail closed.
- **REQ-NET-303:** P-DATA PDV lengths **MUST** be validated against `max_pdv_bytes` and buffer length; invalid lengths **MUST** fail closed.
- **REQ-NET-304:** UIDs in association items **MUST** be strictly validated; invalid UIDs **MUST** fail closed.
- **REQ-NET-305:** Association negotiation **MUST** select the first supported transfer syntax from policy order and **MUST** record result codes for unsupported abstract/transfer syntaxes.
- **REQ-NET-306:** Association lifecycle PDUs **MUST** follow the UL state machine; out-of-order PDUs **MUST** fail closed.
- **REQ-NET-307:** Presentation context IDs **MUST** be odd and unique within an association; duplicates or even IDs **MUST** fail closed.
- **REQ-NET-308:** DIMSE association handling **MUST** enforce TLS policy; when TLS is required and the transport is insecure, the association **MUST** be rejected (fail closed) with `DecodeError`.
- **REQ-NET-309:** DIMSE association handling **MUST** enforce connection throttling via `max_connections`; when exceeded, the association **MUST** be rejected and surface `LimitExceeded(max_connections)`.

- **REQ-DIMSE-300:** The workstation profile **MUST** support the Verification SOP Class (C-ECHO) for SCU/SCP with no data set.
- **REQ-DIMSE-301:** Unsupported DIMSE commands or SOP Classes **MUST** be rejected with typed errors.
- **REQ-DIMSE-302:** The workstation profile **MUST** support the Storage SOP Class (C-STORE) for SCU/SCP with required SOP Class and SOP Instance UIDs present.
- **REQ-DIMSE-303:** C-STORE data set bytes **MUST** be bounded by `max_input_bytes` before accumulation and **MUST** return `LimitExceeded` on violation.
- **REQ-DIMSE-304:** Unsupported DIMSE command fields **MUST** fail closed before data set handling.
- **REQ-DIMSE-310:** C-FIND request/response command sets **MUST** parse deterministically and **MUST** require a command data set type.
- **REQ-DIMSE-320:** C-MOVE request/response command sets **MUST** parse deterministically and **MUST** require a Move Destination AE title.
- **REQ-DIMSE-330:** C-GET request/response command sets **MUST** parse deterministically and **MUST** require a command data set type.
- **REQ-DIMSE-340:** C-FIND requests **MUST** dispatch identifier data sets to the handler and **MUST** return a deterministic status sequence: zero or more pending responses followed by exactly one final non-pending response. Workstation baseline responses use `command_data_set_type = 0x0101`.
- **REQ-DIMSE-341:** C-MOVE requests **MUST** dispatch Move Destination and identifier data sets to the handler and **MUST** return a deterministic status sequence: zero or more pending responses followed by exactly one final non-pending response. Workstation baseline responses use `command_data_set_type = 0x0101`.
- **REQ-DIMSE-342:** C-GET requests **MUST** dispatch identifier data sets to the handler and **MUST** return a deterministic status sequence: zero or more pending responses followed by exactly one final non-pending response. Workstation baseline responses use `command_data_set_type = 0x0101`.

Verification:
- Unit tests **MUST** cover PDU length enforcement, association negotiation, state machine sequencing, UID validation, PDV length validation, TLS policy enforcement, and association throttling.
- DIMSE unit tests **MUST** cover C-ECHO/C-STORE/C-FIND/C-MOVE/C-GET request parsing, unsupported command rejection, and deterministic pending/final handler response sequencing.

---

## DICOMweb support (normative)

DICOMweb support is part of the workstation baseline. Workstation-profile builds **MUST** expose DICOMweb APIs through `dicom-web` with QIDO-RS/WADO-RS/STOW-RS endpoint coverage and policy controls.

- **REQ-HTTP-300:** DICOMweb parsing **MUST** accept only HTTP `GET`, `HEAD`, and `POST` methods; other methods **MUST** fail closed.
- **REQ-HTTP-301:** Request URIs and query keys/values **MUST** be ASCII and bounded by `max_string_bytes`, and query parameter count **MUST** be bounded by `max_dataset_elements`.
- **REQ-HTTP-302:** `GET` and `HEAD` requests **MUST NOT** include a body; non-empty bodies **MUST** fail closed.
- **REQ-HTTP-303:** DICOMweb handling **MUST** enforce an explicit TLS policy; if TLS is required and the request transport is insecure, the request **MUST** fail closed.
- **REQ-HTTP-304:** DICOMweb handling **MUST** enforce a request throttling decision; rejected requests **MUST** return `LimitExceeded` with the policy-provided limit metadata.

Implementation note (informative):
- `dicom-web::DicomWebServiceConfig::default()` is secure-by-default (`RequireTls` + deny-all authorization). Any permissive transport/auth posture requires explicit operator configuration.

- **REQ-WEB-300:** Supported DICOMweb endpoints are limited to:
  - QIDO-RS: `/studies`, `/series`, `/instances`, `/studies/{StudyUID}/series`, `/studies/{StudyUID}/instances`, `/studies/{StudyUID}/series/{SeriesUID}/instances`
  - WADO-RS: `/studies/{StudyUID}`, `/studies/{StudyUID}/series/{SeriesUID}`, `/studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}`
  - STOW-RS: `/studies`, `/studies/{StudyUID}`
  Unsupported endpoints or query parameters **MUST** fail closed.
- **REQ-WEB-301:** UID segments in DICOMweb paths **MUST** be strictly validated; invalid UIDs **MUST** fail closed.
- **REQ-WEB-302:** STOW-RS requests **MUST** declare a `content-type` of `application/dicom` or `multipart/related` with `type=application/dicom` and a `boundary` parameter; invalid or missing content types **MUST** fail closed.
- **REQ-WEB-303:** DICOMweb request bodies **MUST** be bounded by `max_input_bytes` before allocation and **MUST** return `LimitExceeded` on violation.
- **REQ-WEB-304:** Routed DICOMweb operations **MUST** run through a service runtime layer that enforces authorization and emits deterministic authz audit events for allow/deny outcomes when audit callbacks are configured.
- **REQ-WEB-305:** Service runtime execution **MUST** be deterministic:
  - QIDO-RS queries **MUST** execute against storage-backed datasets using the same supported query-key constraints as `REQ-QR-300`.
  - WADO-RS instance-level retrieval **MUST** resolve by `(StudyUID, SeriesUID, InstanceUID)` tuple and **MUST** fail closed if the tuple does not match indexed metadata.
  - WADO-RS study/series retrieval **MUST** emit deterministic `multipart/related; type="application/dicom"` payloads in storage order and **MUST** fail closed when no matching instances are found.
  - STOW-RS ingestion **MUST** delegate to deterministic storage ingestion semantics (`REQ-STOR-300` family).

Verification:
- Unit tests **MUST** cover method validation, URI/query limits, TLS policy enforcement, throttling enforcement, UID validation, content-type parsing, and body limit enforcement.
- Integration tests **MUST** cover QIDO/WADO/STOW service execution with auth allow/deny paths and audit event assertions.
- Fuzz targets **MUST** cover DICOMweb request parsing with conservative limits.

---

## Workflow service (normative)

Workflow operations are part of the core runtime and are subject to explicit authorization, parsing, and audit invariants.

- **REQ-AUTH-300:** Mutation routes on `dicom-workflow-server` (`/mpps/updates`, `/sr/documents`, `/workflow/tasks`, `/workflow/tasks/*`, `/ups`, `/ups/*`, `/interop/*`, and `/workflow/audit`) MUST enforce fail-closed role checks. Accepted request-role headers are `x-sr-role` and `x-auth-role`, and accepted roles are `writer`, `admin`, and `operator`.
- **REQ-WF-401:** Workflow query operations for MWL/MPPS/SR/tasks MUST only expose resources scoped to the caller tenant (`x-tenant`, `x-workflow-tenant`, `x-tenant-id`, or `x-workflow-tenant-id`, defaulting to `tenant-default`); cross-tenant access attempts MUST return `DVF.WORKFLOW.SR.AUTH_DENIED`.
- **REQ-WF-402:** Workflow mutation endpoints that address existing MWL/MPPS identifiers (`/worklist/items`, `/mpps/updates`) MUST reject mismatched tenant headers with `DVF.WORKFLOW.SR.AUTH_DENIED` even when payload is otherwise valid.
- **REQ-HTTP-301:** Workflow request parsing for form and query maps MUST enforce a pair count bound of `max_workflow_pair_count` (effective cap derived from `min(max_dataset_elements, 256)`), and violations MUST return `LimitExceeded` with `limit_name = "max_workflow_pair_count"`.
- **REQ-AUDIT-351:** `/workflow/audit` returns event payloads and, when `verify=true` or `verify=1`, returns deterministic chain verification status and detailed `verification.failures` entries for any detected tampering.

### FHIR ingest envelope (typed minimal contract)

- `POST /interop/fhir` is in-envelope only when `DICOM_WORKFLOW_FHIR_INGEST_ENABLED=true`.
- Accepted request shape is form-encoded and typed (`tenant`, `resource_type`, `source_system|source`, optional `content_sha256`, optional `dry_run`).
- Supported `resource_type` values: `Bundle`, `Patient`, `Encounter`, `Observation`, `DiagnosticReport`.
- FHIR ingest compatibility matrix (fail-closed contract):

| Resource type | Required typed fields | Deterministic fail-closed reason |
|---|---|---|
| `Bundle` | `bundle_type`, `entry_count` (`> 0`) | Missing/invalid fields return `400` decode error (`DVF.WORKFLOW.HTTP.DECODE_ERROR`) |
| `Patient` | `patient_id`, `patient_name` | Missing/invalid fields return `400` decode error (`DVF.WORKFLOW.HTTP.DECODE_ERROR`) |
| `Encounter` | `encounter_id`, `subject_id` | Missing/invalid fields return `400` decode error (`DVF.WORKFLOW.HTTP.DECODE_ERROR`) |
| `Observation` | `observation_code`, `subject_id`, `observed_at` | Missing/invalid fields return `400` decode error (`DVF.WORKFLOW.HTTP.DECODE_ERROR`) |
| `DiagnosticReport` | `report_code`, `subject_id`, `issued_at` | Missing/invalid fields return `400` decode error (`DVF.WORKFLOW.HTTP.DECODE_ERROR`) |
- Fail-closed mapping:
  - disabled endpoint: `404` with `DVF.WORKFLOW.FHIR.DISABLED`,
  - unsupported `resource_type`: `400` decode error,
  - invalid `content_sha256` (must be 64 hex chars): `400` decode error,
  - invalid `dry_run` value (must be `true|false|1|0`): `400` decode error.
- Successful responses return a typed acceptance payload with `mode="typed"` and explicit validation profile metadata.

Verification:
- Unit tests **MUST** cover mutation role acceptance for `writer|admin|operator`, pair-count rejections, tenant-scoped isolation for MWL/MPPS query and mutation paths, and audit-chain verification behavior (`verify` query flag).

---

## Query/Retrieve support (normative)

Query/Retrieve support is part of the workstation baseline. Implementations **MUST** enforce the following limits and deterministic behavior for identifier datasets and matching.

- **REQ-QR-300:** Supported query keys are limited by level:
  - Study: Study Instance UID (0020,000D), Patient ID (0010,0020), Accession Number (0008,0050), Study Date (0008,0020)
  - Series: Study Instance UID (0020,000D), Series Instance UID (0020,000E), Patient ID (0010,0020), Modality (0008,0060), Accession Number (0008,0050), Study Date (0008,0020)
  - Instance: Study Instance UID (0020,000D), Series Instance UID (0020,000E), SOP Instance UID (0008,0018), Patient ID (0010,0020), Modality (0008,0060), Accession Number (0008,0050), Study Date (0008,0020)
  Unsupported keys **MUST** fail closed.
- **REQ-QR-301:** Query results **MUST** be ordered deterministically by Study/Series/Instance UID.
- **REQ-QR-302:** Query key values **MUST** be bounded by `max_string_bytes`; UID keys **MUST** be validated as strict UIDs; text keys **MUST** be validated as ASCII graphic strings; `Study Date (0008,0020)` values **MUST** be exact valid `YYYYMMDD`; and query result count **MUST** be bounded by `max_dataset_elements`.
- **REQ-QR-303:** Candidate datasets **MUST** include required UIDs for the query level; missing or invalid UIDs **MUST** fail closed.

Verification:
- Unit tests **MUST** cover supported key matching (UID and text keys), deterministic ordering, invalid UID rejection, and result limit enforcement.

---

## Worklist and MPPS support (normative)

Worklist (MWL) and MPPS support are part of the workstation baseline. Implementations **MUST** enforce the following deterministic validation and limits.

### Modality Worklist (MWL)

- **REQ-WL-300:** MWL entries **MUST** include a Scheduled Procedure Step Sequence (0040,0100) with exactly one item containing:
  - Modality (0008,0060)
  - Scheduled Procedure Step ID (0040,0009)
  - Scheduled Procedure Step Start Date (0040,0002)
  - Scheduled Procedure Step Start Time (0040,0003)
  Missing or invalid fields **MUST** fail closed.
- **REQ-WL-301:** MWL response ordering **MUST** be deterministic by start date, start time, Scheduled Procedure Step ID, then Modality.
- **REQ-WL-302:** MWL string values **MUST** be bounded by `max_string_bytes`, and the number of response items **MUST** be bounded by `max_dataset_elements`.
- **REQ-WL-303:** Persisted MWL workflows **MUST** support deterministic upsert/query behavior with optional filter keys (`Modality`, `Scheduled Procedure Step ID`, `Patient ID`, `Requested Procedure ID`) and **MUST** preserve optional fields (`Scheduled Station AE Title`, `Patient ID`, `Accession Number`) in query responses when present.

### MPPS ingestion

- **REQ-MPPS-350:** MPPS updates **MUST** include SOP Instance UID (0008,0018), Performed Procedure Step Status (0040,0252), Performed Procedure Step ID (0040,0253), and start date/time (0040,0244/0040,0245).
- **REQ-MPPS-351:** MPPS status **MUST** be one of `IN PROGRESS`, `COMPLETED`, or `DISCONTINUED`. Terminal statuses **MUST** include end date/time (0040,0250/0040,0251).
- **REQ-MPPS-352:** MPPS ingestion **MUST** enforce deterministic transitions: terminal statuses **MUST NOT** revert, and identifiers **MUST** remain stable across updates. Violations **MUST** fail closed with `IntegrityError`.
- **REQ-MPPS-353:** The MPPS store **MUST** enforce `max_dataset_elements` as a bound on total tracked instances.
- **REQ-MPPS-354:** Persisted MPPS service ingest operations **MUST** emit deterministic service-audit events when audit callbacks are configured, and emitted sensitive identifiers **MUST** be redacted per `docs/13`.

Verification:
- Unit tests **MUST** cover MWL required tags, ordering, limit enforcement, persisted query/upsert filtering, MPPS status validation, transition rules, store limits, and MWL/MPPS audit event emission.

---

## Non-support items (explicit)

Normative: These items **MUST** be rejected (fail closed) in the workstation profile unless explicitly added with deterministic requirements and tests.

- **REQ-CONF-070:** Workstation builds **MUST** reject the following out-of-envelope items unless a documented extension explicitly adds deterministic semantics and tests.

The following are out-of-envelope in the workstation profile:

- Autonomous clinical-decision inference or treatment recommendation engines.
- Treatment device control, dose delivery actuation, or radiation plan execution.
- Unbounded custom/private-tag-driven automation without declared envelope extensions.
- Transfer syntaxes and SOP classes not enumerated in the workstation profile tables.
- Float Pixel Data / Double Float Pixel Data (7FE0,0008 / 7FE0,0009) unless a deterministic extension is specified.

Verification:
- Unit tests **MUST** validate that workstation builds reject each listed out-of-envelope item with stable error kinds/codes.

---

## Failure behavior (normative)

When encountering out-of-envelope data, the framework:

- **REQ-CONF-080:** The framework **MUST** return a typed error with:
  - a stable error kind (e.g., `UnsupportedTransferSyntax`, `UnsupportedSopClass`, `MissingRequiredTag`, `InvalidGeometry`, `LimitExceeded`),
  - the relevant UID/tag identifiers,
  - and a non-PII context string.
- **REQ-CONF-081:** The framework **MUST NOT** panic or crash on malformed inputs (bugs are treated as security issues).
- **REQ-CONF-082:** The framework **MUST** enforce resource limits before allocating based on declared sizes (see `docs/09`).

Verification:
- Unit tests **MUST** validate error kinds/codes for unsupported SOP Class, unsupported Transfer Syntax, missing required tags, invalid geometry, and limit exceeded.
