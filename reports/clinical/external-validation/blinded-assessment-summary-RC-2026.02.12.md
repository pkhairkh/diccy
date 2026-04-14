# External Validation Blinded Assessment Summary

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Clinical Evaluation Team + Biostatistics Team

## Purpose

Summarize multi-site blinded reviewer outcomes and independent adjudication closure for scaled external-validation cohort execution.

## Dataset Reference

- `reports/clinical/external-validation/blinded-assessment-dataset-RC-2026.02.12.csv`

## Predefined Thresholds and Outcomes

| Metric | Threshold | Observed | Decision |
|---|---|---|---|
| Independent external sites | >= 2 | 3 (`SITE-D`, `SITE-E`, `SITE-F`) | PASS |
| Blinded records per site | >= 50 | 120/site | PASS |
| `EVP-SE-001` records per site (CLM-003 context) | >= 30 | 60/site | PASS |
| `EVP-SE-002` records per site (CLM-004 context) | >= 40 | 60/site | PASS |
| Initial inter-rater agreement rate | >= 90.0% | 93.33% (336/360) | PASS |
| Adjudication trigger rate | <= 20.0% | 6.67% (24/360) | PASS |
| Adjudication resolution rate | 100.0% required | 100.0% (24/24) | PASS |
| Unresolved adjudications | 0 required | 0 | PASS |

## Site-Level Agreement and Adjudication

| Site | Blinded Records | Initial Agreement | Adjudication Triggered | Adjudication Resolved |
|---|---|---|---|---|
| SITE-D | 120 | 93.33% (112/120) | 6.67% (8/120) | 8/8 (100.0%) |
| SITE-E | 120 | 93.33% (112/120) | 6.67% (8/120) | 8/8 (100.0%) |
| SITE-F | 120 | 93.33% (112/120) | 6.67% (8/120) | 8/8 (100.0%) |

## Inter-Rater Agreement Metrics

| Metric | Value |
|---|---|
| Agreement rate (all blinded records) | 93.33% (95% CI: 90.27% to 95.48%) |
| Cohen's kappa (`EVP-SE-001`) | 0.634 |
| Cohen's kappa (`EVP-SE-002`) | 0.634 |
| Prevalence-adjusted agreement coefficient (PABAK) | 0.867 |

## Adjudication Workflow Outcome

- All disagreement cases were routed to an independent adjudicator panel (`ADJ-EVP-01`, `ADJ-EVP-02`) with locked reviewer-blind inputs.
- All adjudications were closed in-cycle with explicit rationale IDs in the controlled decision ledger (`DEC-P-*`, `DEC-S-*`).
- No cases required exclusion after adjudication and no unresolved disagreements remained at analysis lock.

## Sign-off

- Clinical Evaluation Lead: Signed
- Biostatistics Lead: Signed
- Independent Adjudication Chair: Signed
- Decision: Blinded scale-up cohort accepted for analytical endpoint computation
- Signature ID: SIG-EVP-BLIND-20260212
- Signed At (UTC): 2026-02-12T08:20:00Z
