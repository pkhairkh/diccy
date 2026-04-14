# Interoperability Environment Matrix

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed for Sprint 04 execution)
Owner: DICOM Interop Lead

## Schema

- Environment ID
- Target ID
- Runtime/Profile
- Protocol Surface
- Limits/Policies
- Result
- Evidence Link
- Reviewer

## Rows

| Environment ID | Target ID | Runtime/Profile | Protocol Surface | Limits/Policies | Result | Evidence Link | Reviewer |
|---|---|---|---|---|---|---|---|
| ENV-IOP-201 | TGT-004 | Orthanc peer profile (SITE-D, external independent) | DICOMweb + DIMSE baseline | TLS required, deny-by-default auth, out-of-envelope fail-closed enforcement | PASS | `reports/interoperability/external-site-readiness-checklist-RC-2026.02.12.md`, `reports/interoperability/execution/dicomweb-execution-evidence-RC-2026.02.12.md`, `reports/interoperability/execution/dimse-execution-evidence-RC-2026.02.12.md` | Interop Lead |
| ENV-IOP-202 | TGT-005 | dcm4chee enterprise peer profile (SITE-E, external independent) | DICOMweb + DIMSE + MWL/MPPS | TLS/auth policy lock, workflow transition policy checks, fail-closed unsupported-path requirement | PASS | `reports/interoperability/external-site-readiness-checklist-RC-2026.02.12.md`, `reports/interoperability/execution/dicomweb-execution-evidence-RC-2026.02.12.md`, `reports/interoperability/execution/dimse-execution-evidence-RC-2026.02.12.md`, `reports/interoperability/execution/workflow-execution-evidence-RC-2026.02.12.md` | Interop Lead |
| ENV-IOP-203 | TGT-006 | Modality gateway simulation profile (SITE-F, external independent) | DIMSE + workflow touchpoints | AE-title allowlist, TLS policy enforcement, bounded command/query semantics | PASS | `reports/interoperability/external-site-readiness-checklist-RC-2026.02.12.md`, `reports/interoperability/execution/dimse-execution-evidence-RC-2026.02.12.md`, `reports/interoperability/execution/workflow-execution-evidence-RC-2026.02.12.md`, `reports/interoperability/execution/negative-path-execution-evidence-RC-2026.02.12.md` | Security Lead |

## Sign-off

- Reviewer: DICOM Interop Lead
- Reviewer: Security Lead
- Reviewer: Release Manager
- Decision: Approved external-site environment baseline for Sprint 04 wave-1 execution
- Signature ID: SIG-IOP-ENV-20260212-S02
- Signed At (UTC): 2026-02-12T01:28:00Z
