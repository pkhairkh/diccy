# PMCF Operating Schedule

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Approved (Signed)
Owner: PMCF Operations

## Objective

Define post-release surveillance cadence, indicators, and accountable owners for scientific-grade monitoring continuity.

## Monitoring Calendar

| Window | Cadence | Scope | Primary owner | Backup owner | Evidence output |
|---|---|---|---|---|---|
| Weekly | every 7 days | Drift checks (repro/interoperability/traceability/release package) | PMCF Operations | Release Manager | `reports/pmcf/post-release-monitoring-cycle-<n>-RC-2026.02.11.md` |
| Monthly | every 30 days | Incident classification and CAPA trend review | QA/RA Operations | Security Lead | `reports/pmcf/incident-review-log-RC-2026.02.11.md` |
| Quarterly | calendar quarter close | PMCF protocol review (PMCF-001/010/020/030/040) | PMCF Lead | Clinical Evaluation Lead | `reports/pmcf/pmcf-quarterly-review-RC-2026.02.11.md` |
| Hotfix-triggered | per release hotfix | Immediate safety and drift reassessment | Release Manager | PMCF Lead | `reports/pmcf/post-hotfix-monitoring-RC-2026.02.11.md` |

## Indicators and Alert Thresholds

| Indicator ID | Indicator | Threshold | Escalation |
|---|---|---|---|
| PMCF-IND-001 | Reproducibility mismatch count | > 0 | Immediate CAPA escalation |
| PMCF-IND-002 | Interoperability non-pass cases | > 0 | Immediate CAPA escalation |
| PMCF-IND-003 | Traceability missing references | > 0 | Release governance block |
| PMCF-IND-004 | Traceability invalid links | > 0 | Release governance block |
| PMCF-IND-005 | Missing required release artifacts | > 0 | Release governance block |
| PMCF-IND-006 | Missing release index references | > 0 | Release governance block |

## Ownership Matrix

- PMCF Operations: executes weekly monitoring jobs and compiles cycle evidence.
- QA/RA Operations: validates incident classification and CAPA pathway assignments.
- Release Manager: enforces freeze/re-open policy when thresholds are breached.
- Traceability Lead: validates evidence linkage integrity for all cycle outputs.

## Sign-off

- Reviewer: PMCF Lead
- Decision: Operating Schedule Approved and Activated
- Signature ID: SIG-PMCF-SCHED-20260211
- Signed At (UTC): 2026-02-11T22:20:00Z
