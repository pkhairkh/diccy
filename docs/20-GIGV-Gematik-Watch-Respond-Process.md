# GIGV/Gematik Watch-and-Respond Process (R-11)

Date: 2026-02-11  
Status: Active (operational baseline)  
Owner: Regulatory Operations + Interoperability Architecture

## Purpose

Define a repeatable process to monitor German interoperability governance updates and convert relevant changes into controlled engineering and release actions.

This document is the execution artifact for `R-11` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Scope

This process covers external change signals from:

- GIGV legal text and Annex 1 updates,
- gematik Interoperability Navigator (INA) profile updates,
- BfArM and Ministry publications relevant to health IT interface obligations,
- related national profile updates affecting interoperability expectations.

## Roles

- Watch owner: performs source monitoring and log updates.
- Triage owner: classifies each signal and proposes response.
- Architecture owner: assesses applicability and technical impact.
- QA/RA owner: confirms process and release impacts.
- Release owner: enforces gating for binding obligations.

## Monitoring Cadence

- Weekly scan: source updates and publication changes.
- Monthly review: consolidated applicability review with owners.
- Release-candidate gate: explicit check that no unaddressed binding obligation exists.

## Authoritative Source Set

Track and version the following sources:

1. GIGV legal text and annex updates.
2. gematik INA portal and profile notices.
3. BfArM pages for medical-device interoperability and notification operations.
4. Federal and gematik implementation notices impacting interface obligations.

Record exact URL, retrieval date, and publication version in the watch log.

## Trigger Classification

Classify each observed change as one of:

- `T0 (Informational)`: no immediate product impact detected.
- `T1 (Potentially applicable)`: potential impact, clarification or design work needed.
- `T2 (Binding and applicable)`: mandatory obligation with explicit or implied compliance timeline.

If classification is uncertain, default to `T2` until resolved.

## Response Procedure

### Step 1. Log Intake

- capture source URL, observed date, publication date, and summary,
- assign record ID and owner,
- classify initial trigger (`T0`/`T1`/`T2`).

### Step 2. Applicability Assessment

- architecture and QA/RA assess technical/process impact,
- record applicability (`Not applicable`, `Monitor`, `Action required`),
- assign due date for required actions.

### Step 3. Action Mapping

- map required actions to docs, tests, and release controls,
- assign accountable implementer and reviewer,
- define evidence artifacts needed for closure.

### Step 4. Delivery and Evidence

- execute mapped actions,
- link resulting commits/docs/reports to watch record,
- update closure status and closure date.

### Step 5. Governance Review

- run monthly governance review of open `T1`/`T2` records,
- escalate overdue `T2` records,
- block release if binding records are unresolved.

## Required Artifacts

- watch log (with minimum schema),
- applicability decision records,
- mapped action/evidence links,
- monthly review minutes,
- release-gate disposition for open records.

## Watch Log Minimum Schema

Each record must include:

- record ID,
- source URL,
- publication date,
- observed date,
- trigger class (`T0`/`T1`/`T2`),
- applicability decision,
- owner,
- due date (if action required),
- closure status and closure date.

## Completion Criteria for R-11

R-11 is complete for a release when:

- weekly scans are up to date for the release period,
- all `T2` records have closure evidence or approved deferral,
- open `T1` records have owner and due date,
- release gate includes explicit watch-and-respond sign-off.

## Reference Links

- GIGV text:  
  https://www.buzer.de/gesetz/16836/index.htm
- gematik Interoperability Navigator:  
  https://www.ina.gematik.de/
- BfArM Europe and EUDAMED overview:  
  https://www.bfarm.de/EN/Medical-devices/Overview/Europe-and-EUDAMED/_node.html

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-11 artifact created | Regulatory Operations |
| 2026-02-11 | v0.2 | Operationalized response procedure and completion criteria | Regulatory Operations |
