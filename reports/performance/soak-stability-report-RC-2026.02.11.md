# Soak Stability Report

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Performance and Reliability Board

## Inputs

- Profile: `reports/performance/profiles/soak-profile-RC-2026.02.11.json`
- Harness output: `reports/performance/soak-results-RC-2026.02.11.md`
- Harness summary: `reports/performance/soak-results-RC-2026.02.11.json`
- Execution log: `reports/analytical/gates/s12-performance-soak-RC-2026.02.11.log`

## Soak metrics snapshot

| Workload ID | p95 latency (ms) | Throughput (ops/s) | RSS growth (%) | Latency growth (%) | Result |
|---|---:|---:|---:|---:|---|
| web-sustained-parse | 254.270 | 6.676 | 0.346 | -48.939 | PASS |
| dimse-sustained-cecho | 497.723 | 4.018 | -0.101 | -20.973 | PASS |
| workflow-sustained-restart | 335.440 | 4.726 | -0.276 | 31.364 | PASS |

## Stability verdict

- All sustained workloads passed their RSS growth and latency-growth limits.
- No unbounded memory growth was observed in the soak evidence set.
- Error budgets remained at 0.0% on all sustained paths.

Locked soak digest: `sha256:3999b76489f19f73d2270f9073a8e82bc7cfe4c9c4976359ce408ab3e297f4f2`.

## Signature

- Decision: Approved soak stability evidence for Sprint 12
- Performance Lead: Signed
- V&V Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-PERF-SOAK-20260211-RC2026.02.11
- Signed At (UTC): 2026-02-11T23:03:00Z
