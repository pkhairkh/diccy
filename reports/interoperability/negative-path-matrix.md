# Interoperability Negative-Path Matrix

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed for Sprint 05 wave-2)
Owner: Security Verification Lead

## Schema

- Interface Surface ID
- Use Context
- Input Profile
- Expected Behavior
- Required Limits/Policies
- Test Artifact ID
- Result
- Reproducibility Hash
- Reviewer Sign-off

## Baseline Rows (Sprint 04 / Wave-1)

| Interface Surface ID | Use Context | Input Profile | Expected Behavior | Required Limits/Policies | Test Artifact ID | Result | Reproducibility Hash | Reviewer Sign-off |
|---|---|---|---|---|---|---|---|---|
| IFS-WEB-NEG-UNSUPPORTED-METHOD | TGT-004 unsupported HTTP method | `PUT /studies` on external peer endpoint | Reject unsupported method deterministically (fail-closed) | `REQ-HTTP-300`, `ARS-IOP-004` | `IOP-S02-NEG-001` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `153768ae5d99b5f67270b8a4c95b4d06db9fea9b5d76a03215c653235a911ab5` | Signed (Security Lead) |
| IFS-WEB-NEG-MALFORMED-UID | TGT-004 malformed external route UID | malformed UID retrieve path | Reject with typed decode error (fail-closed) | `REQ-CONF-002`, `REQ-SCOPE-008` | `IOP-S02-NEG-002` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `95ea9203816724404dbf110b4f46adcda8b12dc3ed6a162d1315be1189916040` | Signed (Security Lead) |
| IFS-WEB-NEG-TLS-REQUIRED | TGT-004 insecure transport attempt | TLS required but plain HTTP attempt | Deny request and emit policy event | `REQ-NET-308`, `REQ-AUTH-300` | `IOP-S02-NEG-003` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `6f5efbfeb514da97ec24c8a23546cea5eb6332ef0fa58e39b98fe00d864e9e20` | Signed (Security Lead) |
| IFS-WEB-NEG-AUTH-TOKEN | TGT-005 missing auth token | authenticated route without required token | Deny by default and audit reject path | `REQ-AUTH-300`, `REQ-HTTP-303` | `IOP-S02-NEG-004` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `6a50876bbfff2d46799bf5d65cb58419ac1e4eaa98eb173871da9bb5bca5879c` | Signed (Security Lead) |
| IFS-DIMSE-NEG-BAD-COMMAND | TGT-005 unsupported DIMSE command | invalid command field in command set | Reject command with structured decode error | `REQ-DIMSE-301`, `REQ-DIMSE-304` | `IOP-S02-NEG-005` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `c460de4017049e18b205d740fd8f815ee81671219137559cd5e982774d1011ea` | Signed (Interop Lead) |
| IFS-DIMSE-NEG-AE-TITLE | TGT-005 policy reject for unknown AE | association request with unknown AE title | Reject association per allowlist policy | `REQ-NET-309`, `ARS-IOP-005` | `IOP-S02-NEG-006` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `5c31c9524de90993e5188d8ec6cbe271702faa30d1c67723641ac25a6e9b2e18` | Signed (Security Lead) |
| IFS-DIMSE-NEG-TLS-POLICY | TGT-006 insecure DIMSE association | non-TLS association attempt | Deny association and preserve fail-closed posture | `REQ-NET-308`, `ARS-IOP-006` | `IOP-S02-NEG-007` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `fac6ee69ec82e71c272d72de0294eb2c9331196faa5eb1409d998a54326a6eb9` | Signed (Security Lead) |
| IFS-WL-NEG-OVERSIZED-FILTER | TGT-006 oversized external MWL filter | unsupported/oversized query keys | Fail closed with bounded limit error | `REQ-WL-302`, `ARS-IOP-006` | `IOP-S02-NEG-008` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `87ac496bf9d12a21bd1e6835167fd84af8c7dc4e91c706c135404c77c99d8ef4` | Signed (Workflow Lead) |
| IFS-MPPS-NEG-TERMINAL-REVERT | TGT-005 invalid MPPS transition | terminal-state reversion attempt | Reject with deterministic transition-integrity error | `REQ-MPPS-352`, `ARS-IOP-005` | `IOP-S02-NEG-009` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `277075da8d2fab4de9164a6be4b953e59fcd1b4a38906f138c16a69b8464c716` | Signed (Workflow Lead) |
| IFS-WEB-NEG-TRANSFER-SYNTAX | TGT-004 unsupported TS request | out-of-envelope transfer syntax input | Reject out-of-envelope request fail-closed | `REQ-CONF-002`, `ARS-IOP-004` | `IOP-S02-NEG-010` (`reports/interoperability/execution/negative-capture-matrix-RC-2026.02.12.md`) | PASS | `bcf6ade72916f5a6818319638f5b2b9a1be164f16d2b2cb6cdac6d08c10e2cba` | Signed (Interop Lead) |
| IFS-WL-NEG-PEER-003 | TGT-005 workflow negative control | MWL oversized filter (workflow campaign lane) | Fail-closed query reject in workflow lane | `REQ-WL-302`, `REQ-WL-301` | `IOP-S02-WF-003` (`reports/interoperability/execution/workflow-capture-matrix-RC-2026.02.12.md`) | PASS | `955c04288cc61f2915093ea1bd19b7fd856b9891f4c57ccc5cf857adce941fd9` | Signed (Workflow Lead) |
| IFS-MPPS-NEG-PEER-004 | TGT-005 workflow negative control | MPPS terminal reversion attempt (workflow lane) | Fail-closed transition reject in workflow lane | `REQ-MPPS-352`, `REQ-MPPS-351` | `IOP-S02-WF-004` (`reports/interoperability/execution/workflow-capture-matrix-RC-2026.02.12.md`) | PASS | `ebe052b18419b7f060d2e55b72cd7eb58e00dc67317e600535a096296cc603f8` | Signed (Workflow Lead) |

## Boundary Extension Rows (Sprint 05 / Wave-2)

| Interface Surface ID | Use Context | Input Profile | Expected Behavior | Required Limits/Policies | Test Artifact ID | Result | Reproducibility Hash | Reviewer Sign-off |
|---|---|---|---|---|---|---|---|---|
| IFS-DIMSE-NEG-PDU-LIMIT | TGT-006 PDU size boundary | DIMSE association with PDU beyond configured max bytes | Reject association fail-closed before command processing | `REQ-NET-309`, `REQ-CORE-500` | `IOP-S03-NEG-011` (`reports/interoperability/execution/negative-capture-matrix-wave2-RC-2026.02.12.md`) | PASS | `d59bd41ac90a79caccd08c9f724070a9d614f77a09e475d89de093e57f88684e` | Signed (Security Lead) |
| IFS-DIMSE-NEG-PDV-LIMIT | TGT-006 PDV length boundary | DIMSE command with PDV payload beyond configured limit | Reject command fail-closed with bounded parser behavior | `REQ-NET-309`, `REQ-CORE-500` | `IOP-S03-NEG-012` (`reports/interoperability/execution/negative-capture-matrix-wave2-RC-2026.02.12.md`) | PASS | `8f5e7ee0093edc83e5d17f038d6687ee80f8ae77d67efd01d94b3c49eda2807c` | Signed (Security Lead) |
| IFS-WEB-NEG-ROUTE-CANONICALIZATION | TGT-004 route parsing boundary | non-canonical route encoding (`..`, mixed separators, escaped segments) | Reject route parsing fail-closed with typed path error | `REQ-HTTP-301`, `REQ-SCOPE-008` | `IOP-S03-NEG-013` (`reports/interoperability/execution/negative-capture-matrix-wave2-RC-2026.02.12.md`) | PASS | `fcb4c8845b985a8542da988c807c362326b7c8b3a6981f4cae7e310372372c08` | Signed (Security Lead) |
| IFS-WEB-NEG-QUERY-BOUNDARY | TGT-005 query boundary parsing | excessive query key/value length and parameter count boundary input | Reject request fail-closed with bounded parse limits | `REQ-HTTP-302`, `REQ-HTTP-303` | `IOP-S03-NEG-014` (`reports/interoperability/execution/negative-capture-matrix-wave2-RC-2026.02.12.md`) | PASS | `e8212956ee0e56dd75a577b172d478adf98b8b7aca7708a7e0a9172fb426b4c9` | Signed (Security Lead) |
| IFS-AUTH-NEG-POLICY-TRANSITION | TGT-005 auth policy transition | token revoked during active policy transition window | Deny access fail-closed and audit policy-transition rejection | `REQ-AUTH-300`, `REQ-SEC-405` | `IOP-S03-NEG-015` (`reports/interoperability/execution/negative-capture-matrix-wave2-RC-2026.02.12.md`) | PASS | `59c5617aac8a32d74d2047fd56dc3be70d20d1b2bf8b4bf32ae0bda782c2a8d3` | Signed (Security Lead) |

## Sign-off

- Reviewer: Security Verification Lead
- Reviewer: Interop Lead
- Reviewer: QA/RA Lead
- Decision: Approved for Sprint 05 wave-2 negative-path baseline
- Signature ID: SIG-IOP-NEG-20260212-S03
- Signed At (UTC): 2026-02-12T02:04:00Z
