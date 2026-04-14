# Accessibility Baseline Checklist

Date: 2026-02-14
Release Candidate: RC-2026.02.14
Status: Complete (Signed)
Owner: Clinical Usability Lead

## Purpose

Establish and verify accessibility baseline controls for critical workflows before human-interface release gate approval.

## Control Matrix

| Control ID | REQ-HI | Verification Method | Acceptance Threshold | Result | Evidence |
|---|---|---|---|---|---|
| ACC-001 | REQ-HI-225 | Accessibility walkthrough against critical workflow screens | WCAG 2.1 AA-equivalent baseline met for applicable controls | PASS | `reports/clinical/usability-summative-bundle-RC-2026.02.12.md` |
| ACC-002 | REQ-HI-226 | Programmatic-label audit for action controls | All critical controls expose labels for assistive technologies | PASS | Viewer/workflow UI test logs |
| ACC-003 | REQ-HI-227 | Keyboard-only traversal test on critical tasks | No critical task requires pointer-only action | PASS | Keyboard traversal checklist execution notes |
| ACC-004 | REQ-HI-228 | Focus-order and focus-visibility audit | Focus order logical/deterministic and visible in all critical dialogs | PASS | Focus-order capture and reviewer sign-off |
| ACC-005 | REQ-HI-229 | Status-cue review for warnings/errors | Critical cues not color-only; text/icon channel present | PASS | Error/warning rendering evidence |
| ACC-006 | REQ-HI-230 | Text scaling test at 200% | Controls remain usable at 200% scaling for critical tasks | PASS | Responsive scaling screenshots and checklist |
| ACC-007 | REQ-HI-231 | Contrast review for critical text/actions | Contrast thresholds met for critical controls | PASS | Contrast measurement records |
| ACC-008 | REQ-HI-232 | Accessibility-mode error comprehension test | Error messages remain readable/actionable in accessibility modes | PASS | Summative usability notes |

## Residual Risk and Controls

- No unresolved critical accessibility blocker remains for RC-2026.02.14.
- Residual minor findings are tracked in usability backlog with deterministic remediation targets.

## Sign-off

- Clinical Usability Lead: Signed
- QA Lead: Signed
- Decision: Accessibility baseline accepted for release gate
- Signature ID: SIG-ACC-BASE-20260214
- Signed At (UTC): 2026-02-14T16:10:00Z
