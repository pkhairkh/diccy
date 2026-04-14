# UDI and Interface Trigger Matrix (R-16)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: QA/RA Operations + Release Management

## Purpose

Operationalize explicit trigger decisions for interoperability and interface changes so each qualifying change has:

- documented impact class,
- identifier/versioning decision,
- evidence update obligations,
- release gate approval.

This document is the execution artifact for `R-16` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Trigger Matrix

| Trigger ID | Change Type | Example | Impact Class | Required Decision Record | Required Evidence Update | Release Gate |
|---|---|---|---|---|---|---|
| TRG-001 | New SOP class support | Add modality/derived-object support | C2 | UDI/certificate impact memo | Conformance + interoperability tests | Required |
| TRG-002 | New transfer syntax support | Add codec/TS acceptance | C2 | UDI/certificate impact memo | Pixel/IO tests + corpus deltas | Required |
| TRG-003 | DIMSE command surface change | Add/modify DIMSE operations | C2 | UDI/certificate impact memo | DIMSE protocol tests + integration evidence | Required |
| TRG-004 | DICOMweb route/method change | Add endpoint/method/query behavior | C2 | UDI/certificate impact memo | HTTP/service tests + interop matrix rows | Required |
| TRG-005 | Security policy change | TLS/throttle/auth default delta | C1/C2 | Change impact decision | Security verification + risk update | Required |
| TRG-006 | Internal refactor only | No external behavior change | C0 | C0 record | Standard verification only | Required |

Classification mapping:

- `C0`: no external behavior delta.
- `C1`: bounded external delta.
- `C2`: significant interoperability delta.

## Decision Record Schema

Each trigger event requires a record with:

1. Change ID
2. Trigger ID(s)
3. Impact Class
4. Identifier/versioning decision
5. Evidence updates completed
6. Approvers and date

## Procedure

### Step 1. Trigger Identification

- map each change request to one or more `TRG-*` entries,
- default uncertain mapping to the stricter class,
- store trigger mapping in release notes draft.

### Step 2. Decision and Evidence Update

- issue decision record with class and identifier impact,
- execute required evidence updates from trigger matrix,
- attach links to test runs and affected docs.

### Step 3. Release Gate Enforcement

- release manager verifies decision record completeness,
- QA/RA verifies trigger classification and evidence links,
- block release when required decision artifacts are missing.

## R-16 Completion Criteria

R-16 is complete for a release when:

- all interoperability-affecting changes have trigger mappings,
- all `C2` changes include identifier/versioning memo and approvals,
- evidence updates linked by each trigger are present,
- release gate includes explicit R-16 sign-off.

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-16 artifact created | QA/RA Operations |
| 2026-02-11 | v0.2 | Operationalized trigger workflow and completion criteria | QA/RA Operations |
