# Incident Classification Matrix

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Approved (Signed)
Owner: QA/RA Operations

## Taxonomy

| Incident Class ID | Domain | Definition | Severity bands | Trigger examples | CAPA trigger |
|---|---|---|---|---|---|
| INC-IOP-001 | Interoperability | Fail-closed behavior deviation in DICOMweb/DIMSE/MWL/MPPS transactions | S1 (minor), S2 (major), S3 (critical) | unsupported method accepted, malformed DIMSE payload not rejected, unauthorized command accepted | S2/S3 mandatory CAPA |
| INC-REP-001 | Determinism | Cross-target or repeated-run reproducibility drift beyond budget | S1, S2, S3 | `mismatch_count > 0`, hash instability for fixed corpus/config | S2/S3 mandatory CAPA |
| INC-SEC-001 | Security posture | Authz/audit/TLS or dependency findings reopening release risks | S1, S2, S3 | deny-by-default bypass, audit redaction failure, blocking dependency finding | S2/S3 mandatory CAPA |
| INC-WKF-001 | Workflow/storage reliability | Durable workflow state continuity failure or invalid-path handling regression | S1, S2, S3 | rollback restore failure, persistence snapshot corruption, startup preflight bypass | S2/S3 mandatory CAPA |

## Classification Procedure

1. Intake incident with unique ID and evidence link.
2. Assign domain class and preliminary severity within one business day.
3. Confirm reproducibility using deterministic rerun command set.
4. Escalate to CAPA board for any S2/S3 incident.
5. Record closure with verified test/evidence updates.

## Sign-off

- Reviewer: QA/RA Lead
- Decision: Incident Taxonomy Approved
- Signature ID: SIG-INC-TAX-20260211
- Signed At (UTC): 2026-02-11T22:22:00Z
