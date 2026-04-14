# Interoperability Release Evidence Index

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed for Sprint 09 productization transition)
Owner: Release Manager

## Required Entries

- Envelope version: `1.1`
- Verification matrix version: `reports/interoperability/verification-matrix.md` (2026-02-12, signed)
- Negative-path matrix version: `reports/interoperability/negative-path-matrix.md` (2026-02-12, signed with wave-2 boundary extensions)
- Environment matrix version: `reports/interoperability/environment-matrix.md` (2026-02-12, signed)
- Hash verification registers:
  - `reports/interoperability/hash-verification-register-RC-2026.02.12.md` (wave-1)
  - `reports/interoperability/hash-verification-register-wave2-RC-2026.02.12.md` (wave-2)
- Execution evidence sets:
  - `reports/interoperability/execution/dicomweb-execution-evidence-RC-2026.02.12.md`
  - `reports/interoperability/execution/dimse-execution-evidence-RC-2026.02.12.md`
  - `reports/interoperability/execution/workflow-execution-evidence-RC-2026.02.12.md`
  - `reports/interoperability/execution/negative-path-execution-evidence-RC-2026.02.12.md`
  - `reports/interoperability/execution/dicomweb-execution-evidence-wave2-RC-2026.02.12.md`
  - `reports/interoperability/execution/dimse-execution-evidence-wave2-RC-2026.02.12.md`
  - `reports/interoperability/execution/workflow-execution-evidence-wave2-RC-2026.02.12.md`
  - `reports/interoperability/execution/negative-path-execution-evidence-wave2-RC-2026.02.12.md`
- Reproducibility and review evidence:
  - `reports/interoperability/reproducibility-wave2-RC-2026.02.12.md`
  - `reports/interoperability/wave2-independent-review-RC-2026.02.12.md`

## Release Records

| RC ID | Execution Wave | Verification Matrix | Negative Matrix | Hash Register | Core Gate Outcomes | Decision | Approvers |
|---|---|---|---|---|---|---|---|
| RC-2026.02.12 | Wave-1 (Sprint 04 / S02) | Signed | Signed | `hash-verification-register-RC-2026.02.12.md` | `interoperability_capture.py` PASS (all sets), `interoperability_hash_verify.py` PASS, `evidence_realism_lint.py` PASS, `traceability_report.py` missing refs `0` | GO | Interop Lead; V&V Lead; Security Lead; Workflow Lead; Release Manager |
| RC-2026.02.12 | Wave-2 (Sprint 05 / S03) | Signed (reused) | Signed (boundary-extended) | `hash-verification-register-wave2-RC-2026.02.12.md` | `interoperability_normalize.py --enforce-thresholds` PASS, wave-2 capture/hash gates PASS, reproducibility variance `0.0000%` PASS, trace-bindings verify `61/61` PASS, evidence scan PASS | GO | Interop Lead; Security Lead; QA/RA Lead; Release Manager |

## Product-Claim External Dependency Lock (S07)

This section confirms that required product claims rely on external independent interoperability evidence and do not require local-only execution records.

| Claim ID | Required External Evidence | Local-Only Dependency Present | Decision |
|---|---|---|---|
| CLM-001 | `reports/interoperability/environment-matrix.md` (`SITE-D`/`SITE-E`/`SITE-F` external profiles), `reports/interoperability/verification-matrix.md` | No | PASS |
| CLM-002 | `reports/interoperability/execution/dicomweb-capture-matrix-RC-2026.02.12.md`, `reports/interoperability/execution/dimse-capture-matrix-RC-2026.02.12.md`, `reports/interoperability/negative-path-matrix.md` | No | PASS |
| CLM-003 | `reports/interoperability/execution/workflow-capture-matrix-RC-2026.02.12.md`, `reports/interoperability/verification-matrix.md` | No | PASS |
| CLM-004 | `reports/interoperability/reproducibility-wave2-RC-2026.02.12.md`, `reports/interoperability/wave2-independent-review-RC-2026.02.12.md` | No | PASS |

Dependency conclusion:

- No release-blocking claim row in the S07 productization package depends on local-only interop evidence.
- External independent targets (`TGT-004`, `TGT-005`, `TGT-006`) remain the controlling evidence source for required interop claim controls.

## Final Campaign Decision

- Decision: GO (wave-1 + wave-2 interoperability evidence complete and release-locked)
- Signature ID: SIG-IOP-REL-20260212-S03
- Signed At (UTC): 2026-02-12T02:14:00Z

## Traceability Bindings

- Traceability manifest: `reports/traceability/release-traceability-manifest-RC-2026.02.12.json`
- Claim bundle: `reports/traceability/claim-evidence-trace-bundle-RC-2026.02.12.md`
- Release trace index: `reports/traceability/release-evidence-trace-index-RC-2026.02.12.md`
- Interop case bindings: `reports/traceability/interoperability-case-bindings-RC-2026.02.12.md`
- Bound cluster IDs:
  - `TRC-CLUSTER-S02-001`, `TRC-CLUSTER-S02-002`, `TRC-CLUSTER-S02-003`
  - `TRC-CLUSTER-S03-001`, `TRC-CLUSTER-S03-002`, `TRC-CLUSTER-S03-003`

## Sign-off

- Reviewer: Release Manager
- Reviewer: Interoperability Lead
- Reviewer: QA/RA Lead
- Decision: Approved and locked for RC-2026.02.12 productization transition package
- Signature ID: SIG-IOP-REL-20260212-S07-RM
- Signed At (UTC): 2026-02-12T11:47:00Z
