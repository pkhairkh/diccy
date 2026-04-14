# Glossary

This document defines **canonical terminology** for this repository. Terms are normative unless explicitly labeled “informative”.

## Glossary governance (normative)

Requirements:

- **REQ-TERM-001:** `docs/00-Glossary.md` **MUST** be treated as the authoritative source of terminology for all repository documentation and public APIs.
- **REQ-TERM-002:** Any new normative term introduced elsewhere in `docs/` **MUST** be added to this glossary before it is used in requirements or API definitions.
- **REQ-TERM-003:** If a term is used with a meaning that differs from its glossary definition, the differing meaning **MUST** be explicitly defined in the same section and **MUST** include a cross-link back to the glossary entry.

Verification:

- Review diffs for new/changed terms and confirm corresponding glossary entries exist.
- Run a documentation lint (manual or scripted) that flags new terms in `docs/` lacking glossary entries.

## Core objects

Requirements:
- **REQ-TERM-004:** All documentation and APIs in this repository **MUST** use these definitions when referring to objects like Instance, Frame, Series, Study, and Volume.

Verification:
- Documentation review: confirm any new or modified doc/API usage of core object terms matches these definitions.

- **DICOM file (P10)**: A byte stream following DICOM Part 10 with File Meta Information + Dataset.
- **Dataset**: A collection of DICOM elements (tag, VR, value) after meta header.
- **Element**: A single DICOM data element `(gggg,eeee)` with a Value Representation (VR) and Value Length (VL).
- **Instance**: A decoded dataset representing a single SOP Instance (often one file), identified by **SOP Instance UID**.
- **Frame**: A single 2D image plane ready for presentation, derived from:
  - a single-frame Instance, or
  - one frame of a multi-frame Instance.
- **Series**: A deterministic grouping of Instances/Frames sharing Series Instance UID and consistent geometry policy.
- **Study**: A deterministic grouping of Series sharing Study Instance UID.
- **Volume**: A 3D sampling grid assembled from a Series using patient-space geometry.
- **Viewport**: A presentation surface with a camera transform mapping from image/volume space to screen space.

## Pixel and presentation pipeline

Requirements:
- **REQ-TERM-005:** Any reference to pipeline stages **MUST** use the stage names and ordering defined here and in `docs/05-Pixel-Pipeline.md`.

Verification:
- Documentation review: compare pipeline stage names and ordering in specs/API docs against this list and `docs/05-Pixel-Pipeline.md`.

- **Transfer Syntax**: Specifies dataset encoding and pixel encoding/compression.
- **Decode**: Convert pixel encoding (possibly compressed) into raw samples with defined endianness and bit depth.
- **Photometric transform**: Convert raw samples into a canonical color model (e.g., MONOCHROME2 luminance or RGB).
- **Modality transform**: Apply Modality LUT or Rescale Slope/Intercept to map stored values to modality values (e.g., Hounsfield Units).
- **VOI transform**: Apply VOI LUT or window center/width to map modality values to display intensities.
- **Overlay**: A 1-bit plane or annotation rendered above the image.
- **GSPS**: Grayscale Softcopy Presentation State (optional, not core).

## Derived and clinical objects

- **SEG (Segmentation)**: DICOM Segmentation Storage objects containing label maps or fractional segmentations referenced to source images.
- **SR (Structured Report)**: DICOM Structured Report objects containing text and coded measurements that reference images and measurements.
- **RT Dose**: DICOM RT Dose Storage objects defining dose grids and scaling metadata.
- **RT Structure Set**: DICOM RT Structure Set objects defining contours and ROIs referenced to images.
- **RT Plan**: DICOM RT Plan objects describing treatment plans and beam geometry.
- **US (Ultrasound)**: DICOM Ultrasound Image Storage objects, including multi-frame variants.
- **NM (Nuclear Medicine)**: DICOM Nuclear Medicine Image Storage objects and associated acquisition metadata.
- **XA/XRF**: DICOM X-Ray Angiographic / X-Ray Radiofluoroscopic Image Storage objects.

## Coordinate systems

Requirements:
- **REQ-TERM-006:** Geometry computations and volume assembly **MUST** use patient space (LPS) as the canonical coordinate system unless explicitly documented otherwise.

Verification:
- Spec review: any geometry rule or implementation note references LPS or explicitly documents deviations.

- **Patient space (LPS)**: The canonical coordinate system used by this framework:
  - +X = Left, +Y = Posterior, +Z = Superior.
- **Image plane space**: Pixel indices `(i,j)` (column,row) in the stored frame.
- **World transform**: The affine transform from image plane indices to patient space derived from IOP/IPP and spacing.

## Conformance envelope

Requirements:
- **REQ-TERM-007:** Any claim of DICOM support **MUST** be represented as an explicit envelope entry in `docs/03-DICOM-Conformance-Envelope.md` or a modality/codec pack manifest.

Verification:
- Documentation review: ensure any stated support claim in README/docs is reflected in the envelope or a pack manifest.

- **Conformance envelope**: The exact set of SOP Classes, Transfer Syntaxes, and behaviors the framework claims to support.
- **Tier 0 / Tier 1+**: Incremental support tiers. Tier 0 is the minimal “must work correctly” envelope.
- **Modality pack**: An opt-in crate/feature that extends the envelope for a modality (MG/PET/CT/XR etc) without enlarging the core by default.

## Determinism

Requirements:
- **REQ-TERM-008:** Tests and regression artifacts **MUST** treat “deterministic” as defined here; any permitted variance **MUST** be documented and covered by dedicated tests.

Verification:
- Test review: confirm determinism-related tests and variance notes align with this definition and `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

- **Deterministic**: For fixed input bytes and fixed configuration, the framework produces:
  - identical decoded metadata,
  - identical pixel outputs (byte-for-byte at the CPU boundary),
  - and equivalent rendered results within explicitly documented GPU/backend tolerances validated by tests.

## Requirement language

Requirements:
- **REQ-TERM-009:** This repository **MUST** use RFC 2119 keywords for normative requirements.

This repository uses **RFC 2119** keywords:
- **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, **MAY**.

All MUST/SHOULD requirements in this document set are intended to be testable.

Verification:
- Documentation review: confirm normative statements use RFC 2119 keywords consistently.

## Display-first posture
A default operating posture where the framework focuses on deterministic decoding and rendering for visualization, while quantitative/decision-adjacent features are feature-gated and disabled by default.

## envelope_version
A version identifier for the conformance and behavior envelope (supported SOP/transfer syntaxes, default limits, and output-determining semantics). Changing default limits or pixel semantics requires a corresponding `envelope_version` increment (see `docs/14`).
