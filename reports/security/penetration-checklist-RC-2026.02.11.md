# Structured Penetration Checklist

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Security Testing

## Method

Deterministic negative-path penetration checks executed through release test suites and mapped to attack classes.

## Checklist

| Check ID | Attack class | Validation method | Result | Evidence |
|---|---|---|---|---|
| PEN-001 | Unsupported HTTP method/path abuse | DICOMweb parser rejects unsupported method/path | PASS | `reports/analytical/gates/s13-pen-web-full-RC-2026.02.11.log` |
| PEN-002 | Query amplification/input abuse | DICOMweb query key/count limits enforced | PASS | `reports/analytical/gates/s13-pen-web-full-RC-2026.02.11.log` |
| PEN-003 | Transport downgrade | TLS policy reject paths on web/DIMSE/workflow | PASS | `reports/analytical/gates/s13-pen-web-full-RC-2026.02.11.log`, `reports/analytical/gates/s13-pen-dimse-full-RC-2026.02.11.log`, `reports/analytical/gates/s13-pen-workflow-full-RC-2026.02.11.log` |
| PEN-004 | Unauthorized command execution | Auth deny with audit evidence on DIMSE | PASS | `reports/analytical/gates/s13-pen-dimse-full-RC-2026.02.11.log` |
| PEN-005 | Payload accumulation abuse | Command/data limits enforced before accumulation | PASS | `reports/analytical/gates/s13-pen-dimse-full-RC-2026.02.11.log` |
| PEN-006 | Malformed workflow request parameters | Non-ASCII and invalid pair parsing rejection | PASS | `reports/analytical/gates/s13-pen-workflow-full-RC-2026.02.11.log` |
| PEN-007 | Unsafe persistence target path | Directory-as-file preflight rejection | PASS | `reports/analytical/gates/s13-pen-workflow-full-RC-2026.02.11.log` |

## Signature

- Pen-test Lead: Signed
- Security Lead: Signed
- Signature ID: SIG-SEC-PEN-CHECKLIST-20260211
- Signed At (UTC): 2026-02-11T22:07:00Z
