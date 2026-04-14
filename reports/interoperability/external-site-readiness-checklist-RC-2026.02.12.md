# External Site Readiness Checklist

Date: 2026-02-12  
Release Candidate: RC-2026.02.12  
Status: Complete (Signed)  
Owner: Interoperability Program

## Purpose

Define and capture third-party site readiness controls required before counting evidence as external-independent.

## Site Readiness Matrix

| Control ID | Control Description | SITE-D | SITE-E | SITE-F |
|---|---|---|---|---|
| ESR-001 | Signed legal/data-use agreement on file | PASS | PASS | PASS |
| ESR-002 | De-identification SOP acknowledged and approved | PASS | PASS | PASS |
| ESR-003 | Secure transport and auth policy baseline validated | PASS | PASS | PASS |
| ESR-004 | Environment baseline (versions/config hashes) signed | PASS | PASS | PASS |
| ESR-005 | Test account separation from production PHI systems | PASS | PASS | PASS |
| ESR-006 | Independent operator availability for witnessed runs | PASS | PASS | PASS |
| ESR-007 | Clock sync and timestamp source documented | PASS | PASS | PASS |
| ESR-008 | Artifact export and hash packaging validated | PASS | PASS | PASS |

## Blocking Conditions

A site is NOT ready when any `ESR-*` control is `FAIL` or `N/A` without signed waiver.

## Verification Evidence

- `reports/interoperability/environment-matrix.md`
- `reports/interoperability/execution/*`
- `reports/interoperability/hash-verification-register-RC-2026.02.11.md`

## Sign-off

- Interop Lead: Signed
- Security Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-ESR-20260212
- Signed At (UTC): 2026-02-12T00:29:30Z
