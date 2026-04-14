# Rule-11 Function-Centric Classification Dossier (R-14)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: QA/RA Lead + Clinical Safety Lead

## Purpose

Define a controlled, function-centric classification dossier so each clinical function is mapped to:

- intended-use scope,
- claim text,
- requirement IDs,
- risk controls,
- verification evidence,
- release-gate state.

This document is the execution artifact for `R-14` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Scope

This dossier applies to clinically relevant workstation-profile functions, including:

- image interpretation workflows,
- quantitative workflows,
- derived-object display and interaction workflows,
- interoperability and service workflows that feed downstream clinical use.

Out-of-envelope functions (for baseline), including AI contour assist, are explicitly tracked as deferred and excluded from claim surface until separately approved.

## Controlled Inputs

- `./docs/01-Vision-and-Scope.md`
- `./docs/03-DICOM-Conformance-Envelope.md`
- `./docs/15-Regulatory-and-Standards-Mapping.md`
- `./docs/22-Claim-to-Evidence-Matrix.md`
- Risk file index (quality-system controlled artifact set)

## ID Conventions

- Risk IDs: `RSK-<family>-<nnn>`
- Verification IDs: `VNV-<family>-<nnn>`
- Function IDs: `FUNC-<nnn>`

All IDs must remain stable once referenced by a release candidate.

## Function Dossier Matrix

| Function ID | Function Name | Product Modules | Clinical Decision Role | Severity Context | Information Significance | Claim IDs | Requirement IDs | Risk IDs | Verification IDs | Evidence IDs | Current State |
|---|---|---|---|---|---|---|---|---|---|---|---|
| FUNC-001 | Image interpretation workflow | Viewer, pixel pipeline, rendering | Provides image-derived information used in care decisions | Serious/Critical depending on use context | Inform / Drive management (by intended-use wording) | CLM-002, CLM-004 | REQ-SCOPE-001..004, REQ-UI-001..003 | RSK-INT-001..009 | VNV-INT-001..009 | ANL-010, CLI-010, PMCF-010 | Active |
| FUNC-002 | Quantitative measurements | Measurement stack, geometry, calibration | Produces numeric outputs used in care decisions | Serious/Critical depending on use context | Inform / Drive management (by intended-use wording) | CLM-001, CLM-003 | REQ-MEAS-001, REQ-MEAS-010..083 | RSK-MEA-100..129 | VNV-MEA-100..129 | ANL-001, ANL-020, CLI-001, CLI-020, PMCF-001, PMCF-020 | Active |
| FUNC-003 | Derived-object handling | SEG/RT/SR/GSPS packs | Supplies structured overlays/observations used by clinicians | Serious | Inform / Drive management | CLM-002 | REQ-CONF-086..088, REQ-SEG-300.., REQ-RT-350.., REQ-SR-300.. | RSK-DER-200..229 | VNV-DER-200..229 | ANL-011, CLI-010, PMCF-010 | Active |
| FUNC-004 | AI contour assist | AI contour module(s), reviewer workflow | Produces contour proposals requiring human review | Serious | Drive management | None (out-of-envelope baseline) | Deferred (not in baseline envelope) | RSK-AI-300..329 (reserved) | VNV-AI-300..329 (reserved) | CLI-030, PMCF-030 | Deferred |
| FUNC-005 | Interoperability services | DIMSE, DICOMweb, MWL/MPPS, storage/query | Moves/serves data into decision workflows | Serious | Inform / Drive management | CLM-002 | REQ-CONF-070..099, REQ-OPS-002 | RSK-IOP-400..429 | VNV-IOP-400..429 | ANL-010, ANL-011, CLI-010, PMCF-010 | Active |

## Procedure

### Step 1. Register Function Row

For each `FUNC-*` row:

- confirm modules and boundaries are present in `./docs/12-API-Surface-and-Crate-Boundaries.md`,
- bind to controlled claim IDs from `./docs/22-Claim-to-Evidence-Matrix.md`,
- set initial state (`Active` or `Deferred`).

### Step 2. Link Controlled Claims and Requirements

- map each function to envelope and requirement families,
- confirm out-of-envelope functions are marked `Deferred`,
- require any new function to include both claim and requirement references before release.

### Step 3. Link Risk and Verification

- assign concrete risk and verification ID families,
- map required evidence IDs,
- verify traceability by running `python3 tools/traceability_report.py` before release tagging.

### Step 4. Review and Approval

- QA/RA review: classification logic and claim-surface consistency,
- Architecture review: module-boundary correctness,
- Release review: no `Deferred` function appears in baseline claims.

## R-14 Completion Criteria

R-14 is complete for a release when:

- every `Active` function row has claim, requirement, risk, verification, and evidence links,
- every out-of-envelope function is explicitly marked `Deferred`,
- traceability report is green,
- claim-surface gate remains clean outside `./docs/15-Regulatory-and-Standards-Mapping.md`.

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline dossier artifact created for R-14 | QA/RA Lead |
| 2026-02-11 | v0.2 | Replaced placeholder rows with controlled ID families and operationalized procedure/completion criteria | QA/RA Lead |
