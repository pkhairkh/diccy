# Human Interface Change-Control Register (RC-2026.02.14)

Status: Complete (Signed)
Decision: Change-control and requirement-revision governance accepted
Signature ID: SIG-CC-2026-02-14-HICR-01

Covered requirements: `REQ-HI-247`, `REQ-HI-248`, `REQ-HI-249`, `REQ-HI-254`.

| CCR ID | REQ(s) | Control objective | Verification artifact | Status |
|---|---|---|---|---|
| HICR-247 | `REQ-HI-247` | UI string changes pass claim-surface lint and controlled wording review before release. | `tools/claim_surface_lint.py` + approval log `CC-WORDING-247` | PASS |
| HICR-248 | `REQ-HI-248` | Interface changes are classified by external impact class (`C0/C1/C2`). | Change-control log `CC-IMPACT-CLASS-248` | PASS |
| HICR-249 | `REQ-HI-249` | Claim/conformance boundary changes trigger envelope/evidence review. | Evidence ticket register `CC-EVID-REVIEW-249` | PASS |
| HICR-254 | `REQ-HI-254` | HI requirement revisions are versioned with approver signature and effective date. | Requirement revision register `CC-REQ-REV-254` | PASS |

