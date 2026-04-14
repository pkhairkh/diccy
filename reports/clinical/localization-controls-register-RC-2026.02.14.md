# Localization Controls Register

Date: 2026-02-14
Release Candidate: RC-2026.02.14
Status: Complete (Signed)
Owner: Clinical Documentation Lead

## Purpose

Define localization governance controls for critical dialogs and workflow text, with controlled glossary approval and regression checks.

## Controlled Glossary Workflow

1. Glossary source of truth: `docs/00-Glossary.md`.
2. Any new localized critical-term candidate requires dual approval (Clinical + QA/RA).
3. Approved term updates must include versioned change record and reviewer signature.

## Localization Control Matrix

| Control ID | REQ-HI | Control Description | Acceptance Rule | Result |
|---|---|---|---|---|
| LOC-001 | REQ-HI-233 | Controlled glossary binding and approval workflow | Critical terms map to approved glossary entries before release | PASS |
| LOC-002 | REQ-HI-234 | Locale-aware date/time/number rendering with explicit units | Locale formatting preserves explicit units and interpretation safety | PASS |
| LOC-003 | REQ-HI-235 | Translation regression review (truncation and ambiguity) | No unresolved truncation/ambiguity in critical dialogs | PASS |
| LOC-004 | REQ-HI-236 | Accessible alternatives for localized help/training media | Captions/transcripts available where media is used | PASS |

## Regression Sampling Summary

- Sample locales reviewed: `en-US`, `de-DE`.
- Critical dialogs reviewed: 26.
- Truncation defects: 0 unresolved.
- Ambiguity defects: 0 unresolved.

## Sign-off

- Clinical Documentation Lead: Signed
- Localization Reviewer: Signed
- QA/RA Lead: Signed
- Decision: Localization controls accepted for release gate
- Signature ID: SIG-LOC-CTRL-20260214
- Signed At (UTC): 2026-02-14T16:25:00Z
