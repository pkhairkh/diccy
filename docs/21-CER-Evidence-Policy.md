# CER Evidence Policy (R-12)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: Clinical Evaluation Lead + QA/RA

## Purpose

Define Clinical Evaluation Report (CER) evidence rules so clinical claims are supported by owned, auditable evidence and unsupported shortcut strategies are prevented.

This document is the execution artifact for `R-12` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Scope

Applies to:

- claim selection and claim text in clinical documentation,
- evidence intake used for CER drafting,
- equivalence arguments and comparator usage,
- release and submission review gates that depend on CER evidence.

## Policy Statement

CER evidence must be claim-driven, auditable, and reproducible.

Unsupported shortcut policy:

- Do not accept competitor-equivalence claims as primary evidence without full and durable access to required technical and clinical evidence.
- Do not treat competitor certifications or marketing material as substitute proof of this product’s performance.
- Do not approve CER claim text that is not mapped to owned evidence artifacts.

## Evidence Hierarchy

Use evidence in descending preference:

1. Product-owned analytical and verification evidence.
2. Product-owned clinical performance evidence.
3. Product-owned post-market or follow-up evidence.
4. External literature for scientific validity context.

External literature may support rationale but does not replace product-owned performance evidence.

## Equivalence Decision Gate

If an equivalence path is proposed:

- confirm legal/contractual access to comparator technical documentation,
- confirm access to comparator clinical evidence suitable for full equivalence analysis,
- confirm lifecycle continuity for updates and post-market deltas,
- confirm comparability of intended use, workflow, and operating context.

If any gate item is not met, close equivalence route and proceed with product-owned evidence plan.

## Prohibited Evidence Patterns

- Marketing-only claims without technical evidence artifacts.
- Third-party certifications used as direct substitute for product-specific validation.
- Claims promoted without mapped analytical, clinical, and PMCF entries.

## Required CER Inputs per Claim

Each claim must include:

- claim ID and exact wording,
- intended-use link,
- analytical evidence references,
- clinical evidence references,
- uncertainty/limitations statement,
- reviewer approval record.

## Review Workflow

### Step 1. Claim Registration

- register claim in controlled claim matrix,
- assign owner and intended-use linkage,
- set initial status.

### Step 2. Evidence Mapping

- map claim to analytical/clinical/PMCF artifacts,
- verify artifact existence and version,
- verify traceability to requirement families.

### Step 3. Equivalence Gate Check

- run equivalence decision gate when equivalence is proposed,
- record rationale and decision,
- force owned-evidence path when gate is not fully satisfied.

### Step 4. Review Board Approval

- clinical review,
- QA/RA review,
- approval record with date and sign-off.

### Step 5. Submission Freeze Control

- freeze approved claim/evidence set for release candidate,
- block release on unresolved claim-evidence gaps,
- carry approved freeze set into submission package.

## Artifacts and Records

- claim register and claim-to-evidence matrix,
- analytical/clinical/PMCF evidence artifacts,
- equivalence decisions (if applicable),
- review board sign-off records,
- release freeze decision record.

## Completion Criteria for R-12

R-12 is complete for a release when:

- each active claim has mapped owned evidence,
- no prohibited evidence pattern is present,
- equivalence decisions are explicit and justified,
- review board sign-off and freeze control are complete.

## Reference Links

- MDCG 2020-1 (Clinical evaluation of medical device software):  
  https://health.ec.europa.eu/system/files/2020-09/md_mdcg_2020_1_guidance_clinic_eva_md_software_en_0.pdf
- MDCG 2020-5 (Clinical evaluation equivalence):  
  https://health.ec.europa.eu/system/files/2020-09/md_mdcg_2020_5_guidance_clinical_evaluation_equivalence_en_0.pdf

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-12 artifact created | Clinical Evaluation Lead |
| 2026-02-11 | v0.2 | Operationalized equivalence gate and CER workflow completion criteria | Clinical Evaluation Lead |
