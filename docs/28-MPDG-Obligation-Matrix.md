# MPDG Obligation Matrix (R-19)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: RA Compliance + Legal Operations

## Purpose

Provide a controlled obligation matrix for Germany national-law operations with:

- paragraph-level obligation extraction,
- owner assignment,
- cadence and evidence controls,
- closure tracking.

This document is the execution artifact for `R-19` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Matrix Schema

Required columns:

1. MPDG Reference
2. Obligation Summary
3. Applicability Decision
4. Process Owner
5. Execution Cadence
6. Required Evidence
7. Last Check Date
8. Status

## Initial Obligation Matrix

| MPDG Reference | Obligation Summary | Applicability Decision | Process Owner | Execution Cadence | Required Evidence | Last Check Date | Status |
|---|---|---|---|---|---|---|---|
| MPDG-001 | Manufacturer registration and responsible-party obligations | Applicable | RA Compliance | Quarterly | Controlled register + review log | 2026-02-11 | Active |
| MPDG-002 | Market-surveillance interaction and authority response obligations | Applicable | RA Compliance | Quarterly | Procedure + communication log | 2026-02-11 | Active |
| MPDG-003 | Incident and notification handling obligations | Applicable | QA/RA Ops | Monthly | Runbook + incident records | 2026-02-11 | Active |

## Procedure

### Step 1. Paragraph Extraction

- extract relevant MPDG paragraph references,
- assign stable `MPDG-<nnn>` IDs,
- record extraction date and legal reviewer.

### Step 2. Operational Mapping

- map each obligation to process owner and cadence,
- define evidence artifacts for each obligation,
- classify applicability and rationale.

### Step 3. Execution Control

- run cadence reviews and update status,
- escalate overdue items to RA governance review,
- block release when critical applicable obligations lack current evidence.

## R-19 Completion Criteria

R-19 is complete for a release when:

- all baseline `MPDG-*` rows are active and owner-assigned,
- cadence checks are current,
- evidence links exist for each applicable row,
- no critical obligation row is overdue at release gate.

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-19 artifact created | RA Compliance |
| 2026-02-11 | v0.2 | Replaced placeholder rows and operationalized execution controls | RA Compliance |
