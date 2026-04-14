# Runtime Secure-Defaults Final Verification

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Security Lead + Runtime Owners

## Purpose

Provide final submission-candidate confirmation that packaged runtimes enforce secure defaults: TLS required and deny-by-default authorization.

## Verification Scope

| Runtime | Security Control | Evidence | Result |
|---|---|---|---|
| `dicom-web` | TLS required by policy; secure default config active | `reports/analytical/gates/s08-runtime-secure-defaults-web-RC-2026.02.12.log` | PASS |
| `dicom-dimse-service` | Fail-closed server config + TLS required | `reports/analytical/gates/s08-runtime-secure-defaults-dimse-RC-2026.02.12.log` | PASS |
| `dicom-workflow-server` | Auth defaults to deny-all; insecure transport rejected; token auth validated on TLS | `reports/analytical/gates/s08-runtime-secure-defaults-workflow-RC-2026.02.12.log` | PASS |

## Gate Commands

- `cargo test -p dicom-web --all-features`
- `cargo test -p dicom-dimse-service --all-features`
- `cargo test -p dicom-workflow-server`

## Decision

Runtime secure-default verification is **Approved** for submission candidate RC-2026.02.12.

## Sign-off

- Security Lead: Signed
- Runtime Lead (Web): Signed
- Runtime Lead (Workflow/DIMSE): Signed
- Signature ID: SIG-SEC-RUNTIME-FINAL-20260212
- Signed At (UTC): 2026-02-12T13:10:00Z
