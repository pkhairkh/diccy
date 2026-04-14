# Scientific-Grade Quantification Validation Program (R-17)

Date: 2026-02-12  
Status: Complete (RC-2026.02.12 scientific scale-up signed)  
Owner: Clinical Science Lead + Analytics Validation Lead

## Purpose

Define the controlled validation program for quantitative outputs with explicit separation of:

- analytical validation,
- clinical validation,
- uncertainty declarations,
- limits-of-use declarations.

This document is the execution artifact for `R-17` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Program Structure

### Track A: Analytical Validation

Objective: verify calculation correctness, repeatability, and robustness under controlled conditions.

Required outputs:

- protocol IDs,
- dataset and ground-truth provenance,
- bias/repeatability/reproducibility metrics,
- uncertainty statements.

### Track B: Clinical Validation

Objective: verify performance in intended clinical-use contexts.

Required outputs:

- population and setting definition,
- endpoint definition,
- acceptance criteria and statistical methods,
- reviewer approvals.

### Track C: Limits and Uncertainty Control

Objective: publish boundaries where outputs are valid and how uncertainty is represented.

Required outputs:

- limits-of-use table,
- uncertainty declaration table,
- release-claim gating rules.

## Module Matrix (Operational Baseline)

| Module ID | Quantification Area | Analytical Protocol ID | Clinical Protocol ID | Uncertainty ID | Limits-of-Use ID | Evidence IDs | Status |
|---|---|---|---|---|---|---|---|
| QNT-001 | Geometry-based distance/area/volume | ANP-001 | CLP-001 | UNC-001 | LIM-001 | ANL-001, ANL-058, CLI-042, PMCF-001 | Active (Signed RC-2026.02.12) |
| QNT-002 | Time-series and flow metrics | ANP-002 | CLP-002 | UNC-002 | LIM-002 | ANL-002, ANL-058, CLI-042, PMCF-001 | Active (Signed RC-2026.02.12) |
| QNT-003 | Mapping-derived metrics | ANP-003 | CLP-003 | UNC-003 | LIM-003 | ANL-002, ANL-058, CLI-042, PMCF-001 | Active (Signed RC-2026.02.12) |
| QNT-004 | AI contour-assisted metrics | ANP-004 | CLP-004 | UNC-004 | LIM-004 | N/A (out-of-envelope) | Deferred (out-of-envelope baseline) |

## Module Status Table (Signed RC-2026.02.12)

| Module ID | Status Decision | Status Rationale | Decision Record | Decision Sign-off |
|---|---|---|---|---|
| QNT-001 | Active | In-envelope geometry-driven quantification path remains active with scaled-cohort analytical + clinical evidence refresh and signed uncertainty/limits controls. | R17-MOD-001 | Signed (Clinical Science Lead, Analytics Validation Lead) |
| QNT-002 | Active | In-envelope time-series metric path remains active with scaled-cohort analytical + clinical evidence refresh and signed uncertainty/limits controls. | R17-MOD-002 | Signed (Clinical Science Lead, Analytics Validation Lead) |
| QNT-003 | Active | In-envelope mapping-derived metric path remains active with scaled-cohort analytical + clinical evidence refresh and signed uncertainty/limits controls. | R17-MOD-003 | Signed (Clinical Science Lead, Analytics Validation Lead) |
| QNT-004 | Deferred | AI contour-assisted path remains outside baseline envelope and claim surface; activation requires envelope + claim + evidence expansion controls. | R17-MOD-004 | Signed (Clinical Science Lead, QA/RA Lead) |

## Evidence Linkage Register (Signed Artifacts)

| Module ID | Evidence ID | Artifact Path | Artifact Status |
|---|---|---|---|
| QNT-001 | ANL-001 | `reports/analytical/ANL-001.md` | Signed |
| QNT-001 | CLI-001 | `reports/clinical/CLI-001.md` | Signed |
| QNT-001 | PMCF-001 | `reports/pmcf/PMCF-001.md` | Signed |
| QNT-002 | ANL-002 | `reports/analytical/ANL-002.md` | Signed |
| QNT-002 | CLI-001 | `reports/clinical/CLI-001.md` | Signed |
| QNT-002 | PMCF-001 | `reports/pmcf/PMCF-001.md` | Signed |
| QNT-003 | ANL-002 | `reports/analytical/ANL-002.md` | Signed |
| QNT-003 | ANL-058 | `reports/analytical/ANL-058.md` | Signed |
| QNT-003 | CLI-042 | `reports/clinical/CLI-042.md` | Signed |
| QNT-003 | PMCF-001 | `reports/pmcf/PMCF-001.md` | Signed |
| QNT-002 | ANL-058 | `reports/analytical/ANL-058.md` | Signed |
| QNT-002 | CLI-042 | `reports/clinical/CLI-042.md` | Signed |
| QNT-001 | ANL-058 | `reports/analytical/ANL-058.md` | Signed |
| QNT-001 | CLI-042 | `reports/clinical/CLI-042.md` | Signed |
| QNT-004 | N/A (deferred) | N/A (out-of-envelope baseline) | Deferred |

## Claim Alignment Register (R-13 linkage)

| Module ID | Claim IDs | Alignment Rationale | Alignment Status |
|---|---|---|---|
| QNT-001 | CLM-001, CLM-003 | Geometry-derived quantification is tied to reproducibility and measurement provenance claim controls with scaled-cohort confirmation. | Aligned (Signed RC-2026.02.12) |
| QNT-002 | CLM-001 | Time-series quantification contributes to reproducibility claim controls for supported configurations with scaled-cohort confirmation. | Aligned (Signed RC-2026.02.12) |
| QNT-003 | CLM-001 | Mapping-derived quantification contributes to reproducibility claim controls for supported configurations with scaled-cohort confirmation. | Aligned (Signed RC-2026.02.12) |
| QNT-004 | N/A (deferred) | Deferred module is explicitly excluded from baseline claim rows. | Deferred |

## Uncertainty and Limits Validation Register (RC-2026.02.12)

| Module ID | Uncertainty ID | Uncertainty Evidence ID | Limits-of-Use ID | Limits Evidence ID | Validation Outcome | Sign-off |
|---|---|---|---|---|---|---|
| QNT-001 | UNC-001 | ANL-058 | LIM-001 | CLI-042 | PASS (declared uncertainty and limits hold on scaled cohort) | Signed (Clinical Science Lead, Biostatistics Lead) |
| QNT-002 | UNC-002 | ANL-058 | LIM-002 | CLI-042 | PASS (declared uncertainty and limits hold on scaled cohort) | Signed (Clinical Science Lead, Biostatistics Lead) |
| QNT-003 | UNC-003 | ANL-058 | LIM-003 | CLI-042 | PASS (declared uncertainty and limits hold on scaled cohort) | Signed (Clinical Science Lead, Biostatistics Lead) |
| QNT-004 | UNC-004 | N/A (deferred) | LIM-004 | N/A (deferred) | Deferred | Signed (Clinical Science Lead, QA/RA Lead) |

## Completeness Gate

Release completeness checks for this module matrix are automated by:

- `python3 tools/quant_module_lint.py`

The lint validates:

- active modules have Analytical/Clinical Protocol IDs populated,
- active modules have Uncertainty/Limits IDs populated,
- active modules reference signed evidence IDs with existing artifacts,
- active modules are aligned to claim rows in `docs/22-Claim-to-Evidence-Matrix.md`.

## Procedure

### Step 1. Freeze Metrics and Acceptance Criteria

- map each enabled module to a single acceptance profile,
- freeze metric definitions per release candidate,
- explicitly mark deferred modules as out-of-envelope.

### Step 2. Execute Analytical Track

- run analytical protocol for each active module,
- record reproducibility and error metrics,
- record uncertainty statement IDs.

### Step 3. Execute Clinical Track

- execute clinical protocol for each active module,
- collect endpoints and acceptance outcomes,
- document reviewer sign-off and limitations.

### Step 4. Enforce Limits-of-Use

- update limits-of-use table for each quantified output,
- verify claim gating against declared limits,
- reject release if limits declarations are incomplete.

### Step 5. Module activation/deferment sign-off workflow

- create or update a module decision record (`R17-MOD-*`) for every status decision,
- require Clinical Science + Analytics Validation review for active modules,
- require Clinical Science + QA/RA review for deferred modules,
- block release when any active module lacks signed decision record,
- block release when a deferred module is referenced by active claim rows.

## R-17 Completion Criteria

R-17 is complete for a release when:

- every non-deferred module has analytical and clinical protocol links,
- uncertainty and limits IDs are populated,
- acceptance outcomes are recorded,
- deferred modules are explicitly excluded from baseline claims,
- evidence-link and claim-alignment lint (`tools/quant_module_lint.py`) passes.

## RC-2026.02.12 Sign-off

- Reviewer: Clinical Science Lead
- Reviewer: Analytics Validation Lead
- Decision: Approved for RC-2026.02.12 scientific scale-up closure
- Signature ID: SIG-QNT-20260212-CSL
- Signed At (UTC): 2026-02-12T08:52:00Z

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-17 artifact created | Clinical Science Lead |
| 2026-02-11 | v0.2 | Operationalized module states and validation procedure | Clinical Science Lead |
| 2026-02-11 | v0.3 | Replaced seeded placeholders with release-specific signed evidence mappings and RC sign-off | Clinical Science Lead |
| 2026-02-11 | v0.4 | Added signed module-status rationale table, evidence/claim alignment registers, activation/deferment sign-off workflow, and completeness lint hook | Clinical Science Lead |
| 2026-02-12 | v0.5 | Added scaled-cohort uncertainty/limits validation register and refreshed active-module evidence bindings (`ANL-058`, `CLI-042`) | Clinical Science Lead |
