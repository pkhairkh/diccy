# Regulatory Change-Control SOP (R-09)

Date: 2026-02-11
Status: Active
Owner: QA/RA + Systems Architecture

## Purpose

Define a repeatable decision process for interoperability and interface changes so release decisions, identifier impacts, evidence updates, and external review escalation are handled consistently.

This document is an operational SOP artifact for execution control (`R-09`) in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Scope

This SOP applies to any planned change that can alter external interoperability behavior, including:

- supported SOP Class sets,
- supported transfer syntax sets,
- DIMSE or DICOMweb command/endpoint surfaces,
- network policy defaults (TLS, throttling, association limits),
- serialized output/data-export structures,
- identity/authn/authz interface contracts,
- workflow interface behavior touching MWL/MPPS/query/retrieve semantics.

## Trigger Conditions

Run this SOP whenever a work item changes any API/interface behavior visible to external systems or modifies conformance declarations in:

- `./docs/03-DICOM-Conformance-Envelope.md`
- `./docs/12-API-Surface-and-Crate-Boundaries.md`
- `./docs/14-Release-and-Versioning.md`

## Roles

- Change owner: proposes and documents the change.
- Systems architect: evaluates technical boundary and compatibility impacts.
- QA/RA lead: evaluates conformity/documentation impacts and evidence updates.
- Security lead: evaluates security and threat-model impact.
- Release manager: enforces gating and release artifact completeness.

## Procedure

### Step 1. Open Change Record

Create a change record with:

- change ID,
- affected crates/modules/docs,
- expected behavior delta,
- target release.

### Step 2. Complete Impact Matrix

Evaluate and record impact for each axis:

1. Interoperability surface delta  
   Example: new endpoint, new SOP support, changed query semantics.
2. Safety/performance risk delta  
   Example: changed failure behavior, changed limits, changed parsing paths.
3. Security delta  
   Example: new attack surface or changed trust boundary.
4. Evidence delta  
   Example: new/updated validation reports, reproducibility artifacts, traceability links.
5. Identifier/versioning delta  
   Example: whether versioning and identifier updates are needed.

### Step 3. Decision Classification

Classify the change as one of:

- `C0 (No external behavior delta)`  
  Internal-only refactor or non-behavioral maintenance.
- `C1 (Bounded external delta)`  
  Backward-compatible, declared behavior extension with no safety-boundary change.
- `C2 (Significant interoperability delta)`  
  New or changed interface contract that can affect interpretation, performance, or safety behavior of integrated systems.

### Step 3a. Scope/Claim Exception Triage

If a proposed change alters intended-purpose wording, introduces new claim text, or requests exceptions to baseline claim-surface controls:

- classify the record as `C2` (mandatory, no downgrade),
- open a Scope Exception Record (`SER-<release>-<nnn>`) with:
  - exact proposed text delta,
  - impacted files/sections,
  - rationale and alternatives considered,
  - risk impact summary,
  - required evidence updates (analytical/clinical/PMCF),
  - release impact decision (`defer` or `include`),
- require synchronized updates for:
  - `docs/01-Vision-and-Scope.md`,
  - `docs/03-DICOM-Conformance-Envelope.md` (if support scope changes),
  - `docs/22-Claim-to-Evidence-Matrix.md`,
  - `docs/15-Regulatory-and-Standards-Mapping.md` (informative-only references),
- block merge/release until QA/RA and Release Manager sign-offs are attached.

### Step 4. Required Actions by Class

- `C0`
  - Update change record.
  - Run standard verification gates.
- `C1`
  - Update conformance and release docs.
  - Update tests/corpus/traceability references.
  - Produce compatibility note for integrators.
- `C2`
  - Run formal QA/RA review before merge.
  - Produce explicit identifier-impact decision memo.
  - Complete Scope Exception Record (`SER-*`) when scope/claim wording is changed.
  - Produce external-review escalation plan if required by quality system policy.
  - Block release until memo and evidence package are signed.

### Step 5. Approval and Release Gate

Before release tag:

- Architect sign-off: completed
- QA/RA sign-off: completed
- Security sign-off: completed (or N/A with rationale)
- Release manager gate: completed

## Evidence Package Checklist

  - `cargo fmt --all`
  - `cargo build`
  - `cargo test`
  - `cargo clippy --all-targets --all-features -- -D warnings`

## Decision Log Template

Use one entry per qualifying change:

- Change ID:
- Date:
- Owner:
- Classification (`C0`/`C1`/`C2`):
- External interfaces affected:
- Identifier/versioning impact summary:
- Evidence updated:
- Required follow-on actions:
- Final approvers:

## Operating Notes

- Keep this SOP procedural and evidence-driven.
- Do not infer approval from implementation completion; sign-offs are separate.
- If uncertainty exists in classification, default to `C2` and escalate.
