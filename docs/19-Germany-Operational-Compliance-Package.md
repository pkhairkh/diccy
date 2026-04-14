# Germany Operational Compliance Package (R-10)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: QA/RA Operations + Release Management

## Purpose

Provide a concrete operations checklist for Germany-focused launch readiness covering:

- DMIDS to EUDAMED transition execution,
- BfArM-facing operational steps,
- German-language labeling and documentation readiness.

This document is the execution artifact for `R-10` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## As-of Date and Source Anchors

As of `2026-02-11`, project planning uses the following anchors:

1. First four EUDAMED modules become mandatory on `2026-05-28` (Actor, UDI/Devices, NB/Certificates, Market Surveillance).
2. DMIDS remains mandatory before that date.
3. BfArM FAQ indicates no automatic DMIDS-to-EUDAMED data transfer; migration must be planned explicitly.

Operational rule: re-check these anchors against official sources before each release candidate.

## Workstream A: DMIDS -> EUDAMED Transition

### A.1 Registry inventory freeze

- Freeze current DMIDS-relevant actor/device/certificate inventory.
- Record accountable owners for each record family.
- Tag inventory snapshot with release-candidate identifier.

### A.2 Cutover plan

- Define migration steps with start/end dates.
- Define reconciliation approach for records with no auto-transfer.
- Define rollback and exception-handling procedure.

### A.3 Post-cutover verification

- Verify expected record counts and key fields in target modules.
- Record reconciliation outcomes and unresolved deltas.
- Escalate unresolved deltas to QA/RA governance gate.

## Workstream B: BfArM Operations Runbook

### B.1 Notification and registration operations

- Maintain current BfArM notification obligations and owner matrix.
- Keep procedure links and response SLAs versioned.
- Ensure release checklist includes BfArM operation status check.

### B.2 Incident and escalation routing

- Define event triage path and authority-notification thresholds.
- Define escalation contacts and backup ownership.
- Require closure evidence for every escalated event.

### B.3 Clinical-study operations readiness (if applicable)

- Define study-notification decision gate.
- Define required submission/evidence package for study pathways.
- Record decision outcome per release candidate.

## Workstream C: German Labeling and Documentation Readiness

### C.1 User-facing language readiness

- Verify German-language user-facing artifacts are current.
- Verify terminology consistency with controlled intended-use text.
- Record language QA sign-off.

### C.2 Label and packaging metadata readiness

- Verify packaging/metadata fields against approved release baseline.
- Verify identifier/versioning consistency with `docs/14` and `docs/25`.
- Record release manager sign-off.

### C.3 Release gate checks

- Run claim-surface gate and documentation consistency checks.
- Confirm unresolved deviations are either closed or formally deferred.
- Block release when mandatory checks are incomplete.

## Workstream D: Governance and Evidence Controls

- Maintain evidence index with links to decisions, logs, and approvals.
- Record owner/date/status for every workstream checkpoint.
- Include Germany operations readiness summary in release notes package.

## R-10 Completion Criteria

R-10 is complete for a release when:

- DMIDS/EUDAMED transition readiness is documented with owner-approved plan,
- BfArM runbook status is current,
- German labeling and metadata checks are signed,
- governance evidence index is complete and review-approved.

## Reference Links (for operator verification)

- European Commission EUDAMED overview:  
  https://health.ec.europa.eu/medical-devices-eudamed/overview_en
- European Commission UDI/Device module page:  
  https://health.ec.europa.eu/medical-devices-eudamed/udidevice-registration_en
- BfArM Europe and EUDAMED page:  
  https://www.bfarm.de/EN/Medical-devices/Overview/Europe-and-EUDAMED/_node.html
- BfArM DMIDS notifications page:  
  https://www.bfarm.de/EN/Medical-devices/Tasks/DMIDS/Notifications/_node.html

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-10 artifact created | QA/RA Operations |
| 2026-02-11 | v0.2 | Operationalized workstreams and completion criteria | QA/RA Operations |
