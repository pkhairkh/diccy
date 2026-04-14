# Critical Task Usability Register (RC-2026.02.14)

Status: Complete (Signed)
Decision: Critical-task usability controls accepted for release gate
Signature ID: SIG-HF-2026-02-14-CTU-01

## Scope

This register covers critical-task definition, acceptance criteria, formative/summative sequencing, use-error taxonomy, guided workflow controls, consistency controls, discoverability, long-running operation behavior, undo/redo safety, contextual help, onboarding, simulation isolation, alert-fatigue controls, and CAPA intake.

Covered requirements: `REQ-HI-210`, `REQ-HI-211`, `REQ-HI-212`, `REQ-HI-213`, `REQ-HI-214`, `REQ-HI-215`, `REQ-HI-216`, `REQ-HI-217`, `REQ-HI-218`, `REQ-HI-219`, `REQ-HI-220`, `REQ-HI-221`, `REQ-HI-222`, `REQ-HI-223`, `REQ-HI-224`.

## Control Matrix

| Control ID | REQ(s) | Control objective | Verification artifact | Status |
|---|---|---|---|---|
| CTU-210 | `REQ-HI-210` | Critical task catalog is explicitly versioned and linked to release scope. | `reports/clinical/CLI-030.md` + release notes trace IDs | PASS |
| CTU-211 | `REQ-HI-211` | Each critical task defines objective success criteria and failure criteria. | Task acceptance rubric annex `HF-RUBRIC-210` | PASS |
| CTU-212 | `REQ-HI-212` | Formative evaluation completes before summative for changed high-risk flows. | Study chronology log `HF-EVAL-SEQUENCE-2026Q1` | PASS |
| CTU-213 | `REQ-HI-213` | Summative acceptance requires zero unresolved severe use errors. | Summative defect ledger signoff `HF-SUMMATIVE-EXIT` | PASS |
| CTU-214 | `REQ-HI-214` | Use-error taxonomy captures severity, recurrence, and hazard linkage. | Taxonomy dictionary `HF-TAXONOMY-V4` | PASS |
| CTU-215 | `REQ-HI-215` | High-risk workflows enforce guided stepwise progression. | Step-sequencing checklist `HF-GUIDED-FLOW-CTL` | PASS |
| CTU-216 | `REQ-HI-216` | Equivalent workflows maintain consistent placement and terminology. | UI consistency review `HF-CONSISTENCY-2026-02` | PASS |
| CTU-217 | `REQ-HI-217` | Keyboard shortcuts for frequent critical actions are discoverable in-context. | Shortcut inventory + in-product help audit | PASS |
| CTU-218 | `REQ-HI-218` | Long-running operations show progress, ETA, and cancellation controls. | Runtime operation UX verification `HF-LRO-ETA-CANCEL` | PASS |
| CTU-219 | `REQ-HI-219` | Non-destructive actions support deterministic undo/redo behavior. | Deterministic replay checks + undo/redo script | PASS |
| CTU-220 | `REQ-HI-220` | Contextual help is available at point-of-use for high-risk controls. | Point-of-use help index `HF-HELP-INDEX-14` | PASS |
| CTU-221 | `REQ-HI-221` | Role-specific onboarding checklists are version-controlled and auditable. | Onboarding records `HF-ONBOARD-ROLE-MATRIX` | PASS |
| CTU-222 | `REQ-HI-222` | Training/simulation mode is isolated to synthetic datasets. | Isolation attestation `HF-SIM-ISOLATION-ATTEST` | PASS |
| CTU-223 | `REQ-HI-223` | Alert-fatigue controls provide deduplication and bounded escalation tuning. | Alert burden tuning bounds `HF-ALERT-FATIGUE-BOUNDS` | PASS |
| CTU-224 | `REQ-HI-224` | Recurrent use-error patterns feed CAPA and PMCF planning inputs. | CAPA/PMCF intake feed `HF-RECURRING-ERROR-FEED` | PASS |

## Acceptance Summary

- Formative and summative sequencing controls are complete.
- Critical-task success and failure criteria are objective and versioned.
- Alert-fatigue, onboarding, contextual help, and simulation controls are verified.
- CAPA and PMCF workflows receive recurrent use-error inputs.

