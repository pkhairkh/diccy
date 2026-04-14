# Interoperability Protocol Profile Pack

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed for Sprint 10)
Owner: Interoperability Engineering Lead

## Purpose

Define per-target protocol profiles (DIMSE, DICOMweb, MWL, MPPS) and acceptance rules for campaign execution.

## Per-Target Protocol Profiles

| Target ID | DIMSE Profile | DICOMweb Profile | MWL Profile | MPPS Profile | Acceptance Rule Set ID |
|---|---|---|---|---|---|
| TGT-001 | Verification/Storage/Query-Retrieve profile with explicit command + SOP checks | QIDO/WADO/STOW routes with strict method and UID validation | N/A | N/A | ARS-IOP-001 |
| TGT-002 | Verification/Storage/Query-Retrieve profile with deterministic status sequencing | N/A | N/A | N/A | ARS-IOP-002 |
| TGT-003 | N/A | N/A | MWL deterministic ordering + filter profile | MPPS transition + terminal-state profile | ARS-IOP-003 |
| TGT-004 | Peer DIMSE interoperability profile (store/query baseline) | Peer DICOMweb interoperability profile (QIDO/WADO/STOW baseline) | Site-declared | Site-declared | ARS-IOP-004 |
| TGT-005 | Peer DIMSE interoperability profile (full service baseline) | Peer DICOMweb interoperability profile (full baseline) | Site-declared | Site-declared | ARS-IOP-005 |
| TGT-006 | Modality gateway DIMSE profile (association + store/query) | N/A | Site-declared | Site-declared | ARS-IOP-006 |

## Acceptance Rules

| Rule ID | Rule Description |
|---|---|
| ARS-IOP-001 | DICOMweb QIDO/WADO/STOW success path must pass with typed fail-closed behavior for unsupported methods/routes and malformed UIDs. |
| ARS-IOP-002 | DIMSE C-ECHO/C-STORE/C-FIND/C-MOVE/C-GET must pass negotiated-context checks with deterministic pending/final sequencing and fail-closed unsupported command handling. |
| ARS-IOP-003 | MWL query ordering and MPPS transition controls must satisfy deterministic ordering/state invariants under persisted workflow service operation. |
| ARS-IOP-004 | External peer baseline must satisfy declared conformance profile without accepting out-of-envelope requests; unsupported cases must fail closed. |
| ARS-IOP-005 | External enterprise peer baseline must satisfy positive-path transaction acceptance and documented negative-path deny behavior with auditable outcomes. |
| ARS-IOP-006 | Modality simulation profile must satisfy association policy, transfer syntax negotiation constraints, and deterministic reject behavior for policy violations. |

## Sign-off

- Reviewer: Interoperability Engineering Lead
- Reviewer: V&V Lead
- Decision: Protocol profile pack approved for campaign execution planning
- Signature ID: SIG-IOP-PROFILES-20260211
- Signed At (UTC): 2026-02-11T20:14:00Z
