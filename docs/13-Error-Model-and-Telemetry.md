# Error model and telemetry

## Purpose

Normative: The shared error taxonomy and codes are mandatory (see **REQ-ERR-001**).

Requirements:
- **REQ-ERR-001:** All crates **MUST** use the shared error taxonomy and error codes defined in this document for user-visible failures.

Verification:
- Cross-crate tests **MUST** assert shared error kind and code usage.

Define a shared, stable error model that:

- supports fail-closed behavior,
- is testable and comparable across crates,
- avoids leaking PHI/PII,
- supports optional structured telemetry for debugging and performance.

---

## 1. Error taxonomy

### ErrorKind (normative)

The project defines a non-exhaustive error kind enum, including at least:

- `UnsupportedSopClass { sop_class_uid }`
- `UnsupportedTransferSyntax { transfer_syntax_uid }`
- `MissingRequiredTag { tag }`
- `InvalidTagValue { tag, detail }`
- `InvalidGeometry { detail }`
- `InvalidPixelTransform { stage, detail }` - e.g., non-finite intermediate values, invalid window/LUT preconditions.
- `DecodeError { stage, detail }`
- `LimitExceeded { limit_name, observed, allowed }`
- `IoError { detail }` (native only)
- `IntegrityError { detail }` (e.g., hash mismatch, duplicate UID resolution issues)
- `InternalError { detail }` (should be rare; indicates a bug)

Requirements:
- **REQ-ERR-002:** The error kind enum **MUST** include at least the variants listed above.
- **REQ-ERR-003:** Error kinds **MUST** be stable for programmatic matching.
- **REQ-ERR-004:** Additional kinds **MAY** be added without breaking existing callers (non-exhaustive).

Verification:
- Unit tests **MUST** assert presence of required variants and stable matching behavior.

### ErrorCode (normative)

Each error includes a stable string code:

Format:
- `DVF.<DOMAIN>.<KIND>`

Examples:
- `DVF.DICOM.UNSUPPORTED_TS`
- `DVF.DICOM.MISSING_TAG`
- `DVF.SECURITY.LIMIT_EXCEEDED`
- `DVF.GEOM.INVALID`

Requirements:
- **REQ-ERR-005:** Each error **MUST** include a stable string code.
- **REQ-ERR-006:** Error codes **MUST** follow the `DVF.<DOMAIN>.<KIND>` format.
- **REQ-ERR-007:** Error codes **MUST NOT** include input-derived PHI/PII.
- **REQ-ERR-008:** Unit tests **MUST** validate codes for major failure classes.

Verification:
- Unit tests **MUST** assert code formatting and PHI/PII exclusion.

---

## 2. Error structure

Requirements: The canonical error struct shape described in this section is mandatory (see **REQ-ERR-010**).

A canonical error struct:

- `code: &'static str`
- `kind: ErrorKind`
- `message: String` (short, non-PII)
- `context: Vec<ContextItem>` (structured key/value)
- `source: Option<Box<dyn Error + Send + Sync>>` (where applicable)

Requirements:
- **REQ-ERR-010:** The canonical error struct shape **MUST** be representable in code; any deviation **MUST** be justified and documented.

ContextItem examples (allowed):
- `tag: "0020,0032"`
- `uid_prefix: "1.2.840.10008..."` (optionally truncated)
- `limit_name: "max_decompressed_bytes"`

ContextItem forbidden (normative):
- Patient Name, Patient ID, Accession Number, any free-form DICOM PN/LO that may contain PHI.
- Full file paths by default (may leak usernames); allow only in debug builds or explicit config.

Requirements:
- **REQ-ERR-011:** Context items **MUST NOT** include PHI/PII or full file paths by default.

Verification:
- Unit tests **MUST** reject context items containing forbidden fields.

### 2.1 DIMSE/UL mapping (informative)

- UL association sequencing and PDU parsing failures are reported as `DecodeError` with stage `dicom-net`.
- DIMSE command parsing failures are reported as `DecodeError` with stage `dicom-dimse`.
- Unsupported SOP class UIDs in DIMSE commands map to `UnsupportedSopClass`.
- Networking and DIMSE size violations map to `LimitExceeded` with limit names defined in `docs/09`.
- DIMSE TLS policy failures map to `DecodeError` with stage `dicom-dimse-service`.
- DIMSE association throttling violations map to `LimitExceeded` with limit name `max_connections`.
- DICOMweb request parsing failures are reported as `DecodeError` with stage `dicom-web`.
- DICOMweb service-runtime operation failures (for example, WADO tuple misses) map to `DecodeError` with stage `dicom-web`.
- DICOMweb size violations map to `LimitExceeded` with limit names defined in `docs/09`.
- Storage ingestion integrity failures (hash mismatch or UID conflict) map to `IntegrityError`.
- Storage/index limit violations map to `LimitExceeded` with limit names defined in `docs/09`.
- Metadata extraction failures map to `MissingRequiredTag` or `InvalidTagValue`.
- Query matching failures map to `DecodeError` with stage `dicom-query`; invalid UID values map to `InvalidTagValue`, and query limits map to `LimitExceeded`.
- MWL validation failures map to `MissingRequiredTag` or `InvalidTagValue`; structural/query-filter errors map to `DecodeError` with stage `dicom-worklist`, MWL limits map to `LimitExceeded`, and persisted workflow audit callbacks propagate callback failures.
- MPPS validation failures map to `MissingRequiredTag` or `InvalidTagValue`; status and terminal validation failures map to `DecodeError` with stage `dicom-mpps`. MPPS transition violations map to `IntegrityError`, MPPS limits map to `LimitExceeded`, and persisted service audit callbacks propagate callback failures.
- Authn/authz policy failures map to `DecodeError` with stage `dicom-auth`.
- Audit retention violations map to `LimitExceeded` with limit name `max_audit_event_bytes`.

Requirements:
- **REQ-NET-307:** UL PDU parsing and association sequencing failures **MUST** map to `DecodeError` with stage `dicom-net`.
- **REQ-DIMSE-305:** DIMSE command parsing failures **MUST** map to `DecodeError` with stage `dicom-dimse`, and unsupported SOP Class UIDs **MUST** map to `UnsupportedSopClass`.
- **REQ-AUTH-302:** Authorization failures **MUST** map to `DecodeError` with stage `dicom-auth`.
- **REQ-AUDIT-352:** Audit retention violations **MUST** map to `LimitExceeded` with limit name `max_audit_event_bytes`.

Verification:
- Unit tests in `dicom-net` **MUST** assert the `DecodeError` stage for invalid association sequencing.
- Unit tests in `dicom-dimse` **MUST** assert the `DecodeError` stage for malformed command sets and `UnsupportedSopClass` for unsupported SOP UIDs.
- Unit tests in `dicom-auth` **MUST** assert error mapping for authorization denials.

### 2.2 RC-2026.02.12 conformance activation mappings (informative)

For the RC-2026.02.12 workstation-completeness profile:

- Activated pack SOP classes (`pack-enhanced`, `pack-us`, `pack-nm`, `pack-xa`, `pack-seg`, `pack-rt`, `pack-sr`, `gsps`) parse on active paths and remain covered by shared `MissingRequiredTag` / `InvalidTagValue` validation.
- If any activated pack feature is disabled in a build variant, those pack SOP class UIDs **MUST** fail closed with `UnsupportedSopClass`.
- Mammography SOP classes remain non-activated in the IO envelope for RC-2026.02.12 and **MUST** fail closed with `UnsupportedSopClass`.

---

## 3. Warning and troubleshooting model

Not all issues are fatal.

Requirements:
- **REQ-ERR-020:** The system **SHOULD** represent non-fatal issues as structured warnings attached to Instance, Series, or Frame.
- **REQ-ERR-021:** Warnings **MUST** be deterministic and stable enough for tests.

Verification:
- Tests **MUST** assert stable warning ordering and contents for deterministic inputs.

Structured warning codes currently emitted by the IO boundary include:

- `DVF.IO.RAW_MODE_META_FALLBACK` — malformed/absent P10 meta accepted only under explicit raw-mode.
- `DVF.IO.CHARSET_UNSUPPORTED` — Specific Character Set is recognized but outside conservative supported subset.
- `DVF.IO.CHARSET_INVALID` — Specific Character Set tag value type was invalid for parser policy.

---

## 4. Telemetry

Telemetry is optional and intended for developers/integrators.

### Requirements (normative)

- **REQ-TEL-504:** Telemetry **MUST** be opt-in.
- **REQ-TEL-505:** Telemetry events **MUST** be structured (key/value), not free-form logs.
- **REQ-TEL-506:** Telemetry **MUST NOT** include PHI/PII.
- **REQ-TEL-501:** Telemetry **MUST** treat the following identifiers as sensitive and **MUST NOT** emit them in raw form:
  - all DICOM UIDs (Study/Series/SOP Instance/Frame of Reference),
  - file paths, filenames, URLs, hostnames, IP addresses,
  - user identifiers and local environment identifiers.
- **REQ-TEL-502:** If correlation is required, telemetry **MUST** emit a salted keyed digest:
  - `digest = HMAC-SHA256(secret, raw_value)`,
  - output encoded as lowercase hex,
  - where `secret` is provided by the integrator/deployment and is not checked into source control.
- **REQ-TEL-503:** Telemetry schemas **MUST** define an allowlist of keys per event type; unknown keys **MUST** be rejected or dropped deterministically.

Verification additions:
- **REQ-TEL-507:** Unit tests **MUST** include a schema test that rejects any event containing forbidden keys or UID-like patterns (e.g., `^\d+(\.\d+)+$`).
- **REQ-TEL-508:** Property tests **SHOULD** fuzz telemetry events to ensure forbidden patterns cannot be emitted by any event constructor.

### Event types (suggested)

Requirements: If any of these events are emitted, they must comply with the redaction rules (see **REQ-TEL-510**).

- `decode_start` / `decode_end` with:
  - stage,
  - bytes processed,
  - elapsed_ms,
  - success/failure code.
- `cache_hit` / `cache_miss` with:
  - cache layer,
  - key type (FrameKey/SeriesUid),
  - bytes.

Requirements:
- **REQ-TEL-510:** Any emitted telemetry event type **MUST** comply with redaction and schema rules in this document.

### Verification

- Unit tests confirm telemetry redaction rules (no forbidden tags allowed).
- A \"telemetry disabled\" build compiles out telemetry dependencies where possible (see **REQ-TEL-509**).

Requirements:
- **REQ-TEL-509:** A \"telemetry disabled\" build **MUST** compile out telemetry dependencies where possible.

Verification:
- Build matrix includes a telemetry-disabled build variant.

---

## 5. Audit logging

Audit logs are distinct from telemetry but must follow the same redaction rules.

Requirements:
- **REQ-AUDIT-350:** Audit logs **MUST** redact sensitive identifiers (UIDs, paths, hostnames, IPs) using the same policy as telemetry.
- **REQ-AUDIT-351:** Audit log events **MUST** enforce the retention and size limits defined in `docs/09`.

Verification:
- Unit tests **MUST** assert redaction behavior and limit enforcement for audit records.

Operational artifact diagnostics (RC-2026.02.14):
- Batch export diagnostics (`manifest.csv` detail/status fields) surface structured fail-closed outcomes while inheriting redaction rules for operator-visible payloads.
- Batch artifact integrity sidecars (`manifest.integrity.json`) include only non-secret metadata (`application_version`, `build_id`, `envelope_version`) and deterministic `sha256` hashes for generated artifacts, aligning operator troubleshooting with audit-safe payload constraints.
- SR workflow service audit records store only hashed operational identifiers (`sop_uid_hash`, `principal_hash`, `idempotency_key_hash`) and explicit non-PHI operation/outcome/version metadata.
- Workflow audit records also persist deterministic hash-chain fields (`previous_audit_hash`, `audit_hash`) so `/workflow/audit?verify=true|1` can return a deterministic tamper-check report.

---

## 6. Verification requirements

Requirements:
- **REQ-ERR-030:** Tests **MUST** assert error kind and error code for:
  - unsupported SOP class,
  - unsupported transfer syntax,
  - missing required tags,
  - limit exceeded,
  - invalid geometry.

Verification:
- Unit tests include explicit REQ annotations for the error cases listed above.
