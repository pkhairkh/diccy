# Interoperability Hash Capture SOP

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed for Sprint 10)
Owner: Interoperability Verification Engineering

## Purpose

Define deterministic hash-capture procedure for interoperability evidence artifacts.

## Scope

Applies to request/response traces and verdict rows produced during interoperability campaign execution.

## Hashing Rules

- Hash algorithm: `SHA-256`.
- Hash encoding format: `sha256:<64 lowercase hex chars>`.
- Request hash input: canonical serialized request bytes for each case.
- Response hash input: canonical serialized response bytes for each case.
- Matrix capture digest: `SHA-256` over canonical JSON serialization of sorted capture rows.

## Capture Procedure

1. Collect raw request/response trace material per case.
2. Normalize trace rows into JSONL schema required by `tools/interoperability_capture.py`.
3. Execute capture tool:

```bash
python3 tools/interoperability_capture.py \
  --input <trace-jsonl> \
  --output-md <matrix-md> \
  --output-json <summary-json> \
  --release-id RC-2026.02.11 \
  --fail-on-non-pass
```

4. Archive generated markdown matrix and JSON summary into `reports/interoperability/`.
5. Link generated artifacts in verification/negative matrices and release evidence index.

## Required JSONL Schema

Required fields per row:

- `case_id`
- `target_system`
- `interface_id`
- `protocol`
- `phase` (`positive` or `negative`)
- `request_hash`
- `response_hash`
- `verdict` (`PASS`, `FAIL`, `BLOCKED`)

Optional fields:

- `evidence_ref`
- `note`

## Fail-Closed Conditions

Capture run must fail when:

- any required field is missing,
- hash format is invalid,
- phase or verdict values are invalid,
- `--fail-on-non-pass` is enabled and any verdict is not `PASS`.

## Sign-off

- Reviewer: Interop Verification Lead
- Reviewer: Security Lead
- Decision: Hash capture SOP approved
- Signature ID: SIG-IOP-HASH-SOP-20260211
- Signed At (UTC): 2026-02-11T20:20:00Z
