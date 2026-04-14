# Human Interface Negative-Path Matrix (RC-2026.02.14)

Status: Complete (Signed)
Decision: Negative-path coverage accepted for HI release gate
Signature ID: SIG-VER-2026-02-14-HINP-01

Covered requirement: `REQ-HI-243`.

## Matrix

| NP ID | High-risk workflow | Negative-path condition | Test/Evidence mapping | Status |
|---|---|---|---|---|
| HINP-001 | Session control | Timeout and lockout transition to blocked state | `crates/dicom-web-server/tests/hi_auth_session_controls.rs` | PASS |
| HINP-002 | AuthZ controls | Denied privileged action blocks execution | `crates/dicom-web-server/tests/hi_auth_session_controls.rs` | PASS |
| HINP-003 | Context integrity | Study/series/instance mismatch fails closed | `crates/dicom-web/tests/hi_context_tuple_integrity.rs` | PASS |
| HINP-004 | Workflow context switch | Mixed-patient context switch blocked | `crates/dicom-web/tests/hi_context_safety_controls.rs` | PASS |
| HINP-005 | Measurement safety | Non-finite geometry produces invalid-state block | `crates/viewer-core/tests/hi_viewer_controls.rs` | PASS |
| HINP-006 | Export controls | Identified export denied without elevated privilege | `crates/dicom-visualizer/tests/hi_export_controls.rs` | PASS |
| HINP-007 | Derived object import | Unreviewed diff acceptance blocked | `crates/dicom-visualizer/tests/hi_export_controls.rs` | PASS |
| HINP-008 | Security diagnostics | UID/path/host tokens redacted | `crates/dicom-web-server/src/main.rs` + tests | PASS |
| HINP-009 | Blocking warnings | Unresolved blocking issues block signoff/export | `crates/viewer-core/tests/hi_error_safety_controls.rs` | PASS |

## Conclusion

Negative-path tests exist across high-risk human interaction workflows and are linked to concrete deterministic test artifacts.
Each mapped negative-path scenario enforces explicit fail closed behavior where safety policy requires blocking outcomes.
