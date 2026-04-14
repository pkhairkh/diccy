# Claim-to-Evidence Matrix (R-13)

Date: 2026-02-12  
Status: Updated (RC-2026.02.12 scientific scale-up cohort signed)  
Owner: Clinical Evaluation Lead + Systems Validation Lead

## Purpose

Provide a single matrix that maps each product claim to:

- analytical evidence,
- clinical evidence,
- PMCF evidence path,
- approval and release-gate status.

This document is the execution artifact for `R-13` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Usage Rules

- Add one row per claim; do not merge multiple claims into one row.
- Keep wording identical to controlled claim text.
- Evidence IDs must point to immutable artifacts (report IDs, protocol IDs, dataset IDs, test report IDs).
- A claim cannot be approved when any required evidence cell is empty.
- Any claim wording change requires a new revision entry and re-approval.

## Matrix Schema

Required columns:

1. Claim ID
2. Claim Text (controlled wording)
3. Intended-Use Link
4. Analytical Evidence IDs
5. Clinical Evidence IDs
6. PMCF Path IDs
7. Acceptance Criteria ID
8. Current Status
9. Owner
10. Reviewer Sign-off
11. Last Updated

## Claim-to-Evidence Matrix

| Claim ID | Claim Text | Intended-Use Link | Analytical Evidence IDs | Clinical Evidence IDs | PMCF Path IDs | Acceptance Criteria ID | Current Status | Owner | Reviewer Sign-off | Last Updated |
|---|---|---|---|---|---|---|---|---|---|---|
| CLM-001 | Quantitative output calculations are reproducible within declared tolerance bounds for supported configurations. | IU-001 | ANL-001, ANL-002, ANL-048, ANL-049, ANL-058 | CLI-001, CLI-041, CLI-042 | PMCF-001 | AC-001 | Approved (RC-2026.02.12 + EV-C2) | Validation Lead | Signed (Clinical Evaluation Lead, QA Lead, Scientific Review Board) | 2026-02-12 |
| CLM-002 | Supported interoperability transactions execute with declared fail-closed behavior under in-envelope and out-of-envelope inputs. | IU-002 | ANL-010, ANL-011, ANL-048, ANL-058 | CLI-010, CLI-041, CLI-042 | PMCF-010 | AC-010 | Approved (RC-2026.02.12 + EV-C2) | Interop Lead | Signed (Interop Lead, Security Lead, Scientific Review Board) | 2026-02-12 |
| CLM-003 | Measurement workflows provide explicit provenance for calibration source and method. | IU-003 | ANL-020, ANL-048, ANL-049, ANL-058 | CLI-020, CLI-041, CLI-042 | PMCF-020 | AC-020 | Approved (RC-2026.02.12 + EV-C2) | Measurement Lead | Signed (Measurement Lead, Clinical Evaluation Lead, Scientific Review Board) | 2026-02-12 |
| CLM-004 | User-facing workflow outputs remain stable across release-defined environments according to approved acceptance thresholds. | IU-004 | ANL-030, ANL-049, ANL-058 | CLI-030, CLI-041, CLI-042 | PMCF-030 | AC-030 | Approved (RC-2026.02.12 + EV-C2) | Product Validation Lead | Signed (Product Validation Lead, Release Manager, Scientific Review Board) | 2026-02-12 |

## External Validation Outcome Update (Sprint 08 / S06)

This section records scaled-cohort external validation outcomes used to update claim row status.

| Claim ID | External Validation Outcome | Decision Record | Reviewer Sign-off |
|---|---|---|---|
| CLM-001 | Scaled-cohort reproducibility endpoint passed with subgroup/sensitivity confirmation. | ANL-058, CLI-042 | Signed (Analytics Validation Lead, Clinical Evaluation Lead) |
| CLM-002 | Scaled-cohort interoperability endpoint passed for valid-path and invalid-path cases. | ANL-058, CLI-042 | Signed (Interop Lead, Security Lead) |
| CLM-003 | Scaled-cohort provenance completeness endpoint passed with uncertainty bounds within limits. | ANL-058, CLI-042 | Signed (Measurement Lead, Biostatistics Lead) |
| CLM-004 | Scaled-cohort workflow stability endpoint passed across site profiles. | ANL-058, CLI-042 | Signed (Product Validation Lead, Release Manager) |

## Quantification Module Claim Alignment (R-17)

This section links quantification modules from `docs/26-Scientific-Grade-Quantification-Validation-Program.md` to controlled claim rows.

| Module ID | Claim IDs | Evidence IDs | Alignment Status |
|---|---|---|---|
| QNT-001 | CLM-001, CLM-003 | ANL-001, ANL-058, CLI-042, PMCF-001 | Aligned (Signed RC-2026.02.12) |
| QNT-002 | CLM-001 | ANL-002, ANL-058, CLI-042, PMCF-001 | Aligned (Signed RC-2026.02.12) |
| QNT-003 | CLM-001 | ANL-002, ANL-058, CLI-042, PMCF-001 | Aligned (Signed RC-2026.02.12) |
| QNT-004 | N/A (Deferred) | N/A (out-of-envelope baseline) | Deferred |

## Repository Claim Index Coverage (RC-2026.02.12)

This section maps every active repository claim surface entry to a controlled identifier and confirms no unindexed active claims remain.

| Claim Surface Entry | Control ID | Source | Status |
|---|---|---|---|
| Framework Intended Purpose (verbatim) | REQ-SCOPE-001 / REQ-SCOPE-017 | `README.md`, `docs/01-Vision-and-Scope.md` | Indexed (Controlled) |
| Quantitative reproducibility claim | CLM-001 | `docs/22-Claim-to-Evidence-Matrix.md` | Indexed (Active) |
| Interoperability fail-closed claim | CLM-002 | `docs/22-Claim-to-Evidence-Matrix.md` | Indexed (Active) |
| Measurement provenance claim | CLM-003 | `docs/22-Claim-to-Evidence-Matrix.md` | Indexed (Active) |
| Workflow stability claim | CLM-004 | `docs/22-Claim-to-Evidence-Matrix.md` | Indexed (Active) |

Completeness rule:

- Any new active claim wording in `README.md` or `docs/` must be indexed here (or in an approved successor section) before release.

## Evidence ID Registry

| Evidence ID | Evidence Type | Artifact Path/Locator | Status |
|---|---|---|---|
| ANL-001 | Analytical test report | `reports/analytical/ANL-001.md` | Complete (Signed) |
| ANL-002 | Reproducibility benchmark report | `reports/analytical/ANL-002.md` | Complete (Signed) |
| CLI-001 | Clinical evaluation report section | `reports/clinical/CLI-001.md` | Complete (Signed) |
| PMCF-001 | PMCF protocol | `reports/pmcf/PMCF-001.md` | Active Monitoring Plan (Signed) |
| ANL-010 | Interoperability verification report | `reports/analytical/ANL-010.md` | Complete (Signed) |
| ANL-011 | Negative-path robustness report | `reports/analytical/ANL-011.md` | Complete (Signed) |
| CLI-010 | Clinical workflow fit assessment | `reports/clinical/CLI-010.md` | Complete (Signed) |
| PMCF-010 | Interoperability follow-up protocol | `reports/pmcf/PMCF-010.md` | Active Monitoring Plan (Signed) |
| ANL-020 | Measurement provenance validation report | `reports/analytical/ANL-020.md` | Complete (Signed) |
| CLI-020 | Clinical measurement usage review | `reports/clinical/CLI-020.md` | Complete (Signed) |
| PMCF-020 | Measurement follow-up protocol | `reports/pmcf/PMCF-020.md` | Active Monitoring Plan (Signed) |
| ANL-030 | Environment stability report | `reports/analytical/ANL-030.md` | Complete (Signed) |
| CLI-030 | Workflow outcome consistency review | `reports/clinical/CLI-030.md` | Complete (Signed) |
| PMCF-030 | Environment drift follow-up protocol | `reports/pmcf/PMCF-030.md` | Active Monitoring Plan (Signed) |
| PMCF-040 | Post-release integrated monitoring baseline | `reports/pmcf/PMCF-040.md` | Post-Release Monitoring Baseline (Signed) |
| ANL-048 | External validation primary endpoint report | `reports/analytical/ANL-048.md` | Complete (Signed) |
| ANL-049 | External validation subgroup and uncertainty report | `reports/analytical/ANL-049.md` | Complete (Signed) |
| CLI-041 | External validation clinical evidence update | `reports/clinical/CLI-041.md` | Complete (Signed) |
| ANL-058 | Scaled-cohort endpoint/subgroup/sensitivity report | `reports/analytical/ANL-058.md` | Complete (Signed) |
| CLI-042 | Scaled-cohort clinical evidence update | `reports/clinical/CLI-042.md` | Complete (Signed) |

## Approval Workflow

### Step 1. Register Claim

- assign claim owner,
- bind claim to intended-use link,
- set initial status to `Active` or `Deferred`.

### Step 2. Map Evidence

- link analytical, clinical, and PMCF IDs,
- verify corresponding artifact files exist,
- record acceptance criteria ID.

### Step 3. Validate Completeness

- ensure no required evidence cell is empty,
- ensure all IDs map to controlled artifacts,
- ensure row status aligns with claim-surface policy.

### Step 4. Review and Sign-off

- clinical owner review,
- validation owner review,
- QA/RA release-gate review.

### Step 5. Release Gate

- block release when any active claim row is incomplete,
- require signed decision for deferred claims.

## Completion Criteria for R-13

R-13 is complete for a release when:

- all active claims have complete evidence mappings,
- all evidence IDs resolve to complete signed artifacts,
- release gate decision is recorded with reviewer sign-off.
