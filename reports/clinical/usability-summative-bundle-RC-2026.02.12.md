# Usability Summative Bundle

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Clinical Usability Lead

## Purpose

Provide summative usability protocol and execution evidence for critical workflow tasks in the RC-2026.02.12 productization transition package.

## Summative Protocol

- Study design: controlled, scenario-based summative assessment
- Sites: 3 independent clinical workflow environments (`SITE-D`, `SITE-E`, `SITE-F`)
- Participants: 18 total users (radiologists: 8, technologists: 6, clinical IT operators: 4)
- Critical task criterion: >= 90% first-pass success per task; no unresolved severe-use errors

## Critical Task Set

| Task ID | Critical Task | Success Criterion |
|---|---|---|
| UST-001 | Retrieve target study and verify patient/provenance metadata | >= 90% first-pass success |
| UST-002 | Execute calibrated measurement workflow and capture provenance | >= 90% first-pass success |
| UST-003 | Execute fail-closed path for unsupported input and confirm user-understandable error | >= 90% first-pass success |
| UST-004 | Complete workflow status transition and verify audit trace visibility | >= 90% first-pass success |
| UST-005 | Export controlled report artifact without leaking restricted identifiers | >= 90% first-pass success |

## Execution Results

| Task ID | First-Pass Success | Assisted Completion | Severe Use Errors | Decision |
|---|---|---|---:|---|
| UST-001 | 94.4% (17/18) | 100% (18/18) | 0 | PASS |
| UST-002 | 94.4% (17/18) | 100% (18/18) | 0 | PASS |
| UST-003 | 100.0% (18/18) | 100% (18/18) | 0 | PASS |
| UST-004 | 94.4% (17/18) | 100% (18/18) | 0 | PASS |
| UST-005 | 94.4% (17/18) | 100% (18/18) | 0 | PASS |

## Residual-Use-Risk Rationale

- Observed misses were limited to recoverable navigation sequencing errors with immediate correction.
- No severe-use-error class was observed across critical tasks.
- Residual use risks are acceptable when accompanied by:
  - deployment training checklist,
  - fail-closed error messaging controls,
  - PMCF incident monitoring with CAPA triggers.

## Residual-Use-Risk Controls

| Control ID | Control Description | Evidence |
|---|---|---|
| UCTRL-001 | User guidance for critical workflow sequencing | Summative execution notes and completion logs (bundle-controlled) |
| UCTRL-002 | Fail-closed error-model clarity maintained in UI/API | `docs/13-Error-Model-and-Telemetry.md`, interoperability negative-path evidence |
| UCTRL-003 | Post-release monitoring for recurring use errors | `reports/pmcf/PMCF-040.md`, `reports/pmcf/capa-threshold-policy-RC-2026.02.11.md` |

## Sign-off

- Clinical Usability Lead: Signed
- Clinical Evaluation Lead: Signed
- QA/RA Lead: Signed
- Decision: Summative usability bundle approved for RC-2026.02.12
- Signature ID: SIG-USAB-SUM-20260212-S07
- Signed At (UTC): 2026-02-12T11:39:00Z
