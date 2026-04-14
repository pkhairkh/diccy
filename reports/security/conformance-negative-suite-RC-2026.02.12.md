# Conformance Negative Suite (S05)

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Sprint: 07 (Conformance Expansion Wave 2 / S05)
Status: Complete (Signed)
Owner: Conformance + Security

## Scope

Fail-closed verification for every non-activated `Deferred` row after S05 activation decisions.

Non-activated deferred rows in RC-2026.02.12:
- Mammography SOP classes in `docs/03-DICOM-Conformance-Envelope.md` (remain deferred in IO envelope).
- DIMSE `C-FIND`, `C-MOVE`, `C-GET` command rows (remain deferred behind explicit DIMSE feature flags).

## Execution Matrix

| Deferred row | Expected fail-closed behavior | Verification command/log | Result |
|---|---|---|---|
| Mammography X-Ray SOPs (`1.2.840.10008.5.1.4.1.1.1.2`, `...1.2.1`) | `UnsupportedSopClass` | `reports/analytical/gates/s05-conformance-negative-mg-sop-RC-2026.02.12.log` | PASS |
| DIMSE `C-FIND` when `dimse-c-find` disabled | `DecodeError` | `reports/analytical/gates/s05-conformance-negative-cfind-RC-2026.02.12.log` | PASS |
| DIMSE `C-MOVE` when `dimse-c-move` disabled | `DecodeError` | `reports/analytical/gates/s05-conformance-negative-cmove-RC-2026.02.12.log` | PASS |
| DIMSE `C-GET` when `dimse-c-get` disabled | `DecodeError` | `reports/analytical/gates/s05-conformance-negative-cget-RC-2026.02.12.log` | PASS |

## Gate Summary

- Suite gate log: `reports/analytical/gates/s05-conformance-negative-suite-RC-2026.02.12.log`
- Gate result: PASS
- Open non-deterministic rejection paths: 0

## Decision

All non-activated deferred rows reject deterministically with fail-closed behavior for RC-2026.02.12.

## Sign-off

- Conformance Lead: Signed
- Security Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-CONF-NEG-S05-20260212
- Signed At (UTC): 2026-02-12T03:40:00Z
