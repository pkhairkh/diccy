# Failure Injection Report

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Performance and Security Board

## Inputs

- Profile: `reports/performance/profiles/failure-injection-profile-RC-2026.02.11.json`
- Harness output: `reports/performance/failure-injection-results-RC-2026.02.11.md`
- Harness summary: `reports/performance/failure-injection-results-RC-2026.02.11.json`
- Execution log: `reports/analytical/gates/s12-performance-failure-injection-RC-2026.02.11.log`

## Injected failure classes and outcomes

| Workload ID | Injected class | Verified behavior | p95 latency (ms) | Error rate (%) | Result |
|---|---|---|---:|---:|---|
| failure-restart-recovery | restart | Persistence state recovers after restart | 157.687 | 0.000 | PASS |
| failure-storage-path-reject | storage-path | Directory-path storage misconfiguration rejected fail-closed | 213.685 | 0.000 | PASS |
| failure-auth-deny-dimse | auth | Unauthorized DIMSE request denied with audit capture | 233.997 | 0.000 | PASS |
| failure-insecure-transport-reject | transport | Insecure transport rejected fail-closed | 191.093 | 0.000 | PASS |

## Failure-injection verdict

- Restart, storage-path error, auth deny, and insecure-transport reject scenarios all passed deterministic gate thresholds.
- Fail-closed behavior remained intact for injected negative-path scenarios.
- Error budgets remained at 0.0%.

Locked failure digest: `sha256:9d2d813d6efbc84fafecf1eb95806243ca1b06cb9a067f7f6a594f857c57b081`.

## Signature

- Decision: Approved failure-injection evidence for Sprint 12
- Security Lead: Signed
- Performance Lead: Signed
- V&V Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-PERF-FAILINJ-20260211-RC2026.02.11
- Signed At (UTC): 2026-02-11T23:04:00Z
