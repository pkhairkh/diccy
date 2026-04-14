# Interoperability Campaign Test Inventory

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed for Sprint 05 wave-2)
Owner: V&V Lead

## Purpose

Define positive-path and negative-path test inventory for external interoperability execution and reproducibility verification.

## Inventory

| Case ID | Target ID | Protocol | Path Type | Scenario | Expected Verdict | Evidence Extraction Mode |
|---|---|---|---|---|---|---|
| IOP-TC-001 | TGT-004 | DICOMweb | Positive | QIDO study query on supported UID filter | PASS | request/response trace + verdict |
| IOP-TC-002 | TGT-004 | DICOMweb | Positive | STOW then WADO round-trip retrieval | PASS | request/response trace + hash |
| IOP-TC-003 | TGT-005 | DICOMweb | Positive | Enterprise peer QIDO query and metadata retrieval | PASS | request/response trace + hash |
| IOP-TC-004 | TGT-004 | DICOMweb | Negative | Unsupported method on `/studies` | PASS (fail-closed) | request/response trace + typed error verdict |
| IOP-TC-005 | TGT-004 | DICOMweb | Negative | Malformed UID in retrieve path | PASS (fail-closed) | request/response trace + typed error verdict |
| IOP-TC-006 | TGT-005 | DICOMweb | Negative | Missing auth token on protected route | PASS (deny) | request/response trace + auth verdict |
| IOP-TC-007 | TGT-004 | DIMSE | Positive | C-ECHO liveness on authorized association | PASS | association trace + status verdict |
| IOP-TC-008 | TGT-005 | DIMSE | Positive | C-STORE ingest + indexed retrieval | PASS | command trace + storage reference |
| IOP-TC-009 | TGT-006 | DIMSE | Positive | C-FIND/C-MOVE query/retrieve baseline | PASS | command trace + response sequence |
| IOP-TC-010 | TGT-005 | DIMSE | Negative | Unsupported command field | PASS (fail-closed) | command trace + decode error verdict |
| IOP-TC-011 | TGT-005 | DIMSE | Negative | Unknown AE title association request | PASS (deny) | policy event + association reject trace |
| IOP-TC-012 | TGT-006 | DIMSE | Negative | Insecure transport when TLS required | PASS (deny) | policy event + association reject trace |
| IOP-TC-013 | TGT-005 | MWL | Positive | MWL query with deterministic ordering | PASS | query trace + ordered response proof |
| IOP-TC-014 | TGT-005 | MPPS | Positive | IN PROGRESS -> COMPLETED valid transition | PASS | status transition trace |
| IOP-TC-015 | TGT-005 | MWL | Negative | Oversized/unsupported filter keys | PASS (fail-closed) | query reject trace + limit verdict |
| IOP-TC-016 | TGT-005 | MPPS | Negative | Terminal state reversion attempt | PASS (fail-closed) | transition reject trace + error verdict |
| IOP-TC-017 | TGT-006 | MWL | Positive | External MWL ordering parity check | PASS | query trace + ordering digest |
| IOP-TC-018 | TGT-006 | MPPS | Negative | Invalid status transition boundary | PASS (fail-closed) | transition reject trace + error verdict |
| IOP-TC-019 | TGT-006 | DIMSE | Negative | PDU size boundary overflow | PASS (fail-closed) | association reject trace + limit verdict |
| IOP-TC-020 | TGT-006 | DIMSE | Negative | PDV length boundary overflow | PASS (fail-closed) | command reject trace + limit verdict |
| IOP-TC-021 | TGT-004 | DICOMweb | Negative | Route canonicalization boundary input | PASS (fail-closed) | route parse reject trace |
| IOP-TC-022 | TGT-005 | DICOMweb | Negative | Query key/value boundary overflow | PASS (fail-closed) | route parse reject trace |
| IOP-TC-023 | TGT-005 | DICOMweb | Negative | Auth policy transition (token revoked) | PASS (deny) | auth transition trace |

## Minimum External Case-Volume Thresholds (Wave-2)

| Protocol Family | Minimum Cases Required | Enforced Gate |
|---|---|---|
| DICOMweb | 10 | `tools/interoperability_normalize.py --enforce-thresholds --threshold DICOMweb=10` |
| DIMSE | 8 | `tools/interoperability_normalize.py --enforce-thresholds --threshold DIMSE=8` |
| MWL | 4 | `tools/interoperability_normalize.py --enforce-thresholds --threshold MWL=4` |
| MPPS | 4 | `tools/interoperability_normalize.py --enforce-thresholds --threshold MPPS=4` |

## Gate Enforcement Record

- Command: `python3 tools/interoperability_normalize.py --input reports/interoperability/execution/raw-wave2-RC-2026.02.12/dicomweb-raw-wave2.json --input reports/interoperability/execution/raw-wave2-RC-2026.02.12/dimse-raw-wave2.csv --input reports/interoperability/execution/raw-wave2-RC-2026.02.12/workflow-raw-wave2.jsonl --input reports/interoperability/execution/raw-wave2-RC-2026.02.12/negative-raw-wave2.json --output reports/interoperability/execution/all-protocols-execution-traces-wave2-RC-2026.02.12.jsonl --summary-json reports/interoperability/execution/all-protocols-normalize-summary-wave2-RC-2026.02.12.json --release-id RC-2026.02.12 --enforce-thresholds --threshold DICOMweb=10 --threshold DIMSE=8 --threshold MWL=4 --threshold MPPS=4`
- Result: PASS
- Gate log: `reports/analytical/gates/s03-case-volume-threshold-gate-RC-2026.02.12.log`

## Coverage Check

- Required protocol families covered: DICOMweb, DIMSE, MWL, MPPS.
- Positive and negative paths covered in external targets for all families.
- Boundary negative cases added for PDU/PDV limits, route parsing, and auth policy transitions.

## Sign-off

- Reviewer: V&V Lead
- Reviewer: Interop Lead
- Decision: Wave-2 inventory and thresholds approved
- Signature ID: SIG-IOP-INVENTORY-20260212-S03
- Signed At (UTC): 2026-02-12T02:00:00Z
