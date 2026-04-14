# DICOM I/O and series assembly

## Overview

Normative: Any implementation of ingestion, grouping, or ordering **MUST** follow the rules in this document to preserve determinism and security limits.

This document specifies:

1. How DICOM Part 10 (P10) files and byte buffers are ingested.
2. How Instances are grouped into Studies/Series.
3. How Frames are ordered and validated (geometry, duplicates, multi-frame).

The system is designed for **untrusted inputs** and **deterministic outputs**.

### Raw-mode fallback policy (normative)

- Default `P10Reader` behavior remains strict and fail-closed for malformed/absent P10 meta.
- Raw-dataset fallback is available only through explicit reader options.
- Raw-mode parsing still enforces configured limits and emits structured warnings for fallback and charset handling.
- Raw-mode does not alter conformance-envelope allow/deny behavior for unsupported SOP/transfer syntax.

---

## 1. Input sources and trust model

### Supported sources (normative)

- Native:
  - file paths (`std::fs`) **MAY** be supported in `dicom-io`.
  - directory scanning **MAY** be supported.
- Portable core:
  - byte buffers (`&[u8]`, `Bytes`) **MUST** be supported.
    - **REQ-IO-001:** The portable core **MUST** accept byte buffer inputs without relying on native filesystem access.
      - Verification: unit test that decodes a minimal in-envelope dataset from an in-memory buffer on all targets (native + WASM).

### Trust model (normative)

- All bytes are untrusted.
- **REQ-IO-002:** Input handling **MUST** enforce limits prior to allocations (see `docs/09`) and **MUST** fail closed with `LimitExceeded` when limits are exceeded.
  - Verification: unit tests that feed oversized declared lengths and assert `LimitExceeded` error kind and code.

---

## 1.4 DIMSE UL ingress (normative)

In the workstation profile, UL PDUs are a baseline untrusted input boundary. The core requirements for UL parsing and validation are defined in **REQ-NET-300** through **REQ-NET-304** in `docs/03-DICOM-Conformance-Envelope.md`. This section clarifies the parsing boundary:

- Association requests **MUST** include an application context, at least one presentation context, and user-info with a max PDU length.
- **REQ-NET-307:** Presentation context IDs **MUST** be odd and unique, and abstract/transfer syntax UIDs **MUST** be validated.
- Unsupported UL item types **MUST** be rejected to preserve fail-closed behavior.
- **REQ-NET-306:** Association lifecycle PDUs **MUST** follow the UL state machine and reject out-of-order PDUs.
- **REQ-NET-308:** TLS policy **MUST** be enforced at the association boundary; insecure transports **MUST** be rejected when TLS is required.
- **REQ-NET-309:** Association throttling **MUST** be enforced via `max_connections` before acceptance; rejected associations **MUST** surface `LimitExceeded(max_connections)`.
- DIMSE command sets **MUST** be parsed as implicit VR little endian, and C-STORE data set bytes **MUST** honor `max_input_bytes`.

Verification:
- Unit tests **MUST** cover association parsing, UID validation, and fail-closed behavior on unsupported items.

---

## 1.5 DICOMweb ingress (normative)

In the workstation profile, HTTP request parsing is a baseline untrusted input boundary. DICOMweb routing and validation requirements are defined in **REQ-HTTP-300** through **REQ-HTTP-304** and **REQ-WEB-300** through **REQ-WEB-303** in `docs/03-DICOM-Conformance-Envelope.md`. This section clarifies the parsing boundary:

- Request methods are limited to `GET`, `HEAD`, and `POST` and **MUST** fail closed otherwise (REQ-HTTP-300).
- Request URIs and query keys/values are bounded by `max_string_bytes`, and query parameter count is bounded by `max_dataset_elements` (REQ-HTTP-301).
- `GET` and `HEAD` requests **MUST NOT** include a body (REQ-HTTP-302).
- TLS policy enforcement **MUST** reject insecure transports when TLS is required (REQ-HTTP-303).
- Throttling decisions **MUST** be enforced before routing, returning `LimitExceeded` on rejection (REQ-HTTP-304).
- DICOMweb endpoints are limited to the explicit QIDO-RS/WADO-RS/STOW-RS paths listed in `docs/03` (REQ-WEB-300).
- UID path segments **MUST** be strictly validated (REQ-WEB-301).
- STOW-RS content types **MUST** be validated for `application/dicom` or `multipart/related` with a boundary (REQ-WEB-302).
- Request bodies are bounded by `max_input_bytes` before allocation (REQ-WEB-303).

Verification:
- Unit tests **MUST** cover method validation, URI/query limits, TLS policy enforcement, throttling enforcement, UID validation, content-type parsing, and body limit enforcement.

---

## 1.6 Storage ingestion and metadata indexing (normative)

Storage ingestion and metadata indexing are deterministic operations that sit after DICOM parsing. They must treat inputs as untrusted and fail closed on conflicts.

- **REQ-STOR-300:** Storage ingestion **MUST** compute a canonical SHA-256 hash of the input bytes and use it for deduplication; identical hashes **MUST** return a duplicate outcome without re-indexing.
- **REQ-STOR-301:** If the same SOP Instance UID is ingested with a different canonical hash, storage **MUST** fail closed with `IntegrityError`.
- **REQ-STOR-302:** Storage ingestion **MUST** append to a write-ahead log before mutating the metadata index; replay **MUST** rebuild the index deterministically and **MUST** fail closed with `IntegrityError` on hash mismatches.
- **REQ-STOR-303:** Storage ingestion **MUST** enforce `max_input_bytes` per input and `max_cache_bytes` across stored bytes before accepting data.

- **REQ-META-300:** Metadata extraction **MUST** require Study/Series/SOP Instance UIDs and SOP Class UID; missing or invalid UIDs **MUST** fail closed.
- **REQ-META-301:** The metadata index **MUST** provide deterministic ordering by UID for studies, series, and instances.
- **REQ-META-302:** The metadata index **MUST** enforce `max_dataset_elements` as a bound on total indexed instances.

Verification:
- Unit tests **MUST** cover hashing/deduplication outcomes, SOP UID conflict failures, write-ahead log replay, concurrent ingest/replay consistency, durable WAL recovery/corruption fail-closed behavior, limit enforcement, and deterministic ordering.

---

## 1.7 Query/Retrieve matching (normative)

Query/Retrieve matching is a workstation baseline operation and **MUST** be deterministic and fail closed on unsupported keys.

- Queries **MUST** only accept supported UID/text keys listed in `docs/03` (REQ-QR-300).
- Supported text-key filters include Patient ID (0010,0020), Modality (0008,0060), Accession Number (0008,0050), and Study Date (0008,0020) as constrained by query level in `docs/03` (REQ-QR-300).
- Result ordering **MUST** be deterministic by UID (REQ-QR-301).
- Query key values **MUST** be bounded by `max_string_bytes`; UID keys **MUST** be strict UIDs; text keys **MUST** be ASCII graphic strings; and Study Date values **MUST** be exact valid `YYYYMMDD` (REQ-QR-302).
- Missing or invalid required UIDs **MUST** fail closed (REQ-QR-303).

Verification:
- Unit tests **MUST** cover supported key matching (UID/text), deterministic ordering, invalid UID rejection, and result limit enforcement.

---

## 1.8 Worklist and MPPS datasets (normative)

Worklist and MPPS dataset validation is a workstation baseline operation and **MUST** be deterministic and fail closed.

### Modality Worklist (MWL)

- **REQ-WL-300:** MWL entries **MUST** include a Scheduled Procedure Step Sequence (0040,0100) with exactly one item containing Modality (0008,0060), Scheduled Procedure Step ID (0040,0009), and start date/time (0040,0002/0040,0003). Missing or invalid fields **MUST** fail closed.
- **REQ-WL-301:** MWL response ordering **MUST** be deterministic by start date, start time, Scheduled Procedure Step ID, then Modality.
- **REQ-WL-302:** MWL string values **MUST** be bounded by `max_string_bytes`, and response item counts **MUST** be bounded by `max_dataset_elements`.
- **REQ-WL-303:** Persisted MWL workflows **MUST** support deterministic upsert/query semantics with optional filters (`Modality`, `Scheduled Procedure Step ID`, `Patient ID`, `Requested Procedure ID`) and preserve optional response fields (`Scheduled Station AE Title`, `Patient ID`, `Accession Number`) when present.

### MPPS ingestion

- **REQ-MPPS-350:** MPPS updates **MUST** include SOP Instance UID (0008,0018), Performed Procedure Step Status (0040,0252), Performed Procedure Step ID (0040,0253), and start date/time (0040,0244/0040,0245).
- **REQ-MPPS-351:** MPPS status **MUST** be one of `IN PROGRESS`, `COMPLETED`, or `DISCONTINUED`. Terminal statuses **MUST** include end date/time (0040,0250/0040,0251).
- **REQ-MPPS-352:** MPPS ingestion **MUST** enforce deterministic transitions: terminal statuses **MUST NOT** revert, and identifiers **MUST** remain stable across updates. Violations **MUST** fail closed with `IntegrityError`.
- **REQ-MPPS-353:** MPPS store size **MUST** be bounded by `max_dataset_elements`.
- **REQ-MPPS-354:** Persisted MPPS service ingest operations **MUST** emit deterministic service-audit events when audit callbacks are configured, and sensitive identifiers in those events **MUST** be redacted.

Verification:
- Unit tests **MUST** cover required tags, deterministic ordering, persisted MWL query/upsert behavior, durable snapshot recovery/corruption fail-closed behavior, limit enforcement, status validation, transition rules, store limits, and MWL/MPPS audit event emission.

---

## 2. P10 parsing requirements

### File Meta Information

- **REQ-IO-010:** The parser **MUST** validate the DICOM prefix and meta header structure for P10 inputs.
  - Verification: corpus test with malformed P10 preamble asserts `DecodeError` with `DVF.DICOM.DECODE_ERROR`.
- **REQ-IO-011:** The parser **MUST** read:
  - (0002,0010) Transfer Syntax UID
  - (0002,0002) Media Storage SOP Class UID (optional but SHOULD be present)
  - (0002,0003) Media Storage SOP Instance UID (optional but SHOULD be present)
  - Verification: unit test asserts meta tags are surfaced in header-only mode.
- **REQ-IO-012:** If the meta header is malformed, the input **MUST** be rejected unless explicitly configured to accept raw datasets (advanced/debug mode only) and **MUST** return `DecodeError`.
  - Verification: unit test for malformed meta header asserts `DecodeError` kind and code; separate test verifies debug/raw-dataset configuration path.

### Dataset decoding

- **REQ-IO-013:** The decoder **MUST** support the VR/VL encoding dictated by Transfer Syntax.
  - Verification: unit tests for implicit vs explicit VR round-tripping of representative tags.

Implementation note (informative):
- For implicit VR transfer syntax, undefined-length containers with item/sequence delimiters are parsed as sequences.
- Undefined-length containers that are neither encapsulated Pixel Data nor item/delimiter-backed sequences remain fail-closed (`DecodeError`).
- **REQ-IO-014:** The decoder **MUST** track element offsets/sizes for structured debug metadata but **MUST NOT** expose raw pointers beyond safe lifetimes.
  - Verification: lint/compile-time safety gate and a unit test ensuring offsets are captured in structured debug metadata without exposing borrowed raw buffers.
- **REQ-IO-015:** Unknown tags **MUST** be preserved in a generic representation (tag + VR + raw bytes or parsed scalar), but rendering logic **MUST NOT** depend on private tags by default.
  - Verification: unit test asserting private tag presence does not affect render pipeline decisions.

### Character sets

Assumption: For minimal clinical display, full charset support is non-essential, but robustness is required.

Requirements:
- **REQ-IO-020:** The system **MUST** decode ASCII and UTF-8 correctly.
  - Verification: corpus cases for ASCII and UTF-8 patient/study strings with stable decoding.
- **REQ-IO-021:** For other encodings referenced by Specific Character Set (0008,0005), the system **SHOULD** support a conservative subset (e.g., ISO 8859 variants) if feasible.
  - Verification: optional corpus test per supported charset variant with deterministic output.
- **REQ-IO-022:** Unsupported encodings **MUST** be surfaced as a structured warning and decoded with replacement characters (U+FFFD), never causing crashes.
  - Verification: unit test that injects an unsupported charset and asserts warning + replacement behavior.

---

## 3. Instance model extraction

From each dataset in the envelope, the framework **MUST** extract a minimal header model used for browsing and series assembly:

- SOP Class UID, SOP Instance UID
- Study Instance UID, Series Instance UID
- Modality (0008,0060) (optional but SHOULD)
- Series Number (0020,0011) (optional)
- Instance Number (0020,0013) (optional)
- Image pixel module fields required by the envelope
- Geometry fields required by the SOP class (e.g., CT/MR)

- **REQ-IO-030:** The extractor **MUST** be able to run in “header-only” mode without decoding Pixel Data.
  - Verification: unit test that parses headers without touching Pixel Data for an in-envelope dataset.
- **REQ-IO-031:** The extractor **MUST** validate required tags per `docs/03-DICOM-Conformance-Envelope.md` before any pixel decode, failing closed with `MissingRequiredTag` or `UnsupportedSopClass` as applicable.
  - Verification: unit tests for missing required tags and unsupported SOP Class UID.

---

## 4. Study/Series assembly

### Grouping rules (normative)

- **REQ-SER-201:** Instances **MUST** be grouped by Study Instance UID and Series Instance UID.
  - Verification: unit test that groups mixed Study/Series UIDs into deterministic buckets.
- **REQ-SER-202:** If either UID is missing, the Instance **MUST** be rejected (out-of-envelope), except for explicitly configured debug modes, and **MUST** return `MissingRequiredTag`.
  - Verification: unit test with missing Study/Series UID asserting `MissingRequiredTag`.

### Duplicate handling (normative)

- If multiple files present the same SOP Instance UID:
  - **REQ-SER-210:** the assembler **MUST** choose one deterministically using:
    1) prefer valid + in-envelope,
    2) prefer larger Pixel Data length if both valid (heuristic),
    3) tie-break by stable path sort (native) or stable input order (bytes).
    - Verification: unit test with duplicates asserting deterministic selection under path and buffer input orders.
  - **REQ-SER-211:** the assembler **MUST** record a structured warning listing duplicate UIDs and **MUST** attach it to the Series warnings collection.
    - Verification: unit test asserting duplicate warning presence and stable ordering.

---

## 5. Frame extraction and ordering

### Frame identity (normative)

Each renderable frame **MUST** have a stable identity:

- `InstanceUid`
- `FrameIndex` (0-based) for multi-frame, absent/0 for single-frame
- `SeriesUid`

These form a `FrameKey` used for caching and deterministic comparisons.

- **REQ-SER-220:** `FrameKey` construction **MUST** be deterministic for a given input set and **MUST** be stable across native and WASM targets.
  - Verification: cross-target test that compares serialized `FrameKey` lists for a fixed corpus input.

### Ordering for CT/MR single-frame series (normative)

Compute:
- `row_dir`, `col_dir` from Image Orientation (Patient) (IOP).
- `normal = row_dir × col_dir` (right-handed cross product).
- `slice_coord = dot(normal, Image Position (Patient))`.

Requirements:
- **REQ-GEOM-301:** Frames **MUST** be sorted by `(slice_coord, InstanceNumber, SOPInstanceUID)` with a stable total order.
  - Verification: unit test with synthetic geometry asserts ordering and tie-break stability.
- **REQ-GEOM-302:** If the normal vector is invalid (non-finite, near-zero magnitude), the series **MUST** be rejected as invalid geometry and **MUST** return `InvalidGeometry`.
  - Verification: unit test with degenerate IOP yields `InvalidGeometry`.
- **REQ-GEOM-303:** If `slice_coord` values indicate non-monotonic order beyond epsilon (e.g., alternating), the assembler **MUST** reject volume assembly, but **MAY** still present as a 2D stack with a warning.
  - Verification: corpus test that triggers non-monotonic ordering and asserts 2D fallback warning.

### Ordering for SC multi-frame (normative)

- **REQ-SER-230:** Frame order **MUST** be the natural frame index order.
  - Verification: unit test for SC multi-frame uses frame index ordering.
- **REQ-SER-231:** Frame time metadata (if present) **MAY** be surfaced but does not reorder frames unless explicitly configured.
  - Verification: unit test that frame time does not alter ordering under default configuration.

---

## 6. Multi-frame policy hooks

- **REQ-SER-240:** The series layer **MUST** expose a “frame provider” abstraction so that:

- Tier 0 SC multi-frame is supported with simple shared-attribute semantics.
- Enhanced multi-frame (Tier 1 packs) can implement per-frame attribute resolution without changing core series logic.
  - Verification: compile-time interface test in `dicom-series` crate ensuring the abstraction is exercised by both SC and enhanced paths.

---

## 7. Verification requirements

- **REQ-SER-290:** Series assembly behavior **MUST** be validated with:
  - unit tests for ordering (synthetic geometry),
  - corpus tests for real datasets (de-identified or synthetic),
  - fuzz tests ensuring malformed geometry does not crash the assembler.
  - Verification: CI gate that runs unit + corpus + fuzz suites for series assembly targets.

See `docs/10-Testing-Corpus-Fuzzing-Evals.md`.
