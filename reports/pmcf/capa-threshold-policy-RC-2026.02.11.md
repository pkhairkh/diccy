# CAPA Threshold Policy

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Approved (Signed)
Owner: Quality Gate Board

## Purpose

Define threshold-driven CAPA escalation for scientific evidence quality degradation.

## Threshold Table

| Threshold ID | Condition | Severity floor | Required action | SLA |
|---|---|---|---|---|
| CAPA-TH-001 | Reproducibility `mismatch_count > 0` | Major (S2) | Open CAPA, freeze release progression, assign engineering owner | 24h |
| CAPA-TH-002 | Interoperability `non_pass_cases > 0` | Major (S2) | Open CAPA, run targeted interoperability rerun, issue risk memo | 24h |
| CAPA-TH-003 | Traceability `missing_references_count > 0` | Major (S2) | Open CAPA, block governance gate until zero-miss restored | 24h |
| CAPA-TH-004 | Traceability `invalid_links_count > 0` | Major (S2) | Open CAPA, repair evidence linkage and rerun traceability gate | 24h |
| CAPA-TH-005 | Missing required release artifacts > 0 | Critical (S3) | Open CAPA, revoke GO decision, regenerate signed package | 8h |
| CAPA-TH-006 | Any confirmed security fail-closed violation | Critical (S3) | Open CAPA, emergency security board, hotfix protocol | 8h |

## Escalation Pathway

1. PMCF Operations raises threshold breach incident.
2. Quality Gate Board assigns CAPA owner and due date.
3. Engineering and QA execute corrective + preventive actions with deterministic rerun evidence.
4. Regulatory/Release co-sign closure and update affected release/package records.

## Sign-off

- Reviewer: Quality Gate Chair
- Decision: CAPA Threshold Policy Approved
- Signature ID: SIG-CAPA-TH-20260211
- Signed At (UTC): 2026-02-11T22:24:00Z
