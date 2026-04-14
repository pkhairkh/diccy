# Baseline Performance Report

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Performance and Reliability Board

## Inputs

- Profile: `reports/performance/profiles/baseline-profile-RC-2026.02.11.json`
- Harness output: `reports/performance/baseline-results-RC-2026.02.11.md`
- Harness summary: `reports/performance/baseline-results-RC-2026.02.11.json`
- Execution log: `reports/analytical/gates/s12-performance-baseline-RC-2026.02.11.log`

## Baseline metrics snapshot

| Workload ID | Class | p95 latency (ms) | Throughput (ops/s) | Error rate (%) | Result |
|---|---|---:|---:|---:|---|
| web-small-parse | small | 554.946 | 3.968 | 0.000 | PASS |
| dimse-medium-cecho | medium | 113.519 | 9.995 | 0.000 | PASS |
| workflow-large-restart | large | 117.687 | 9.731 | 0.000 | PASS |
| webserver-burst-preflight-rotation | burst | 117.938 | 9.350 | 0.000 | PASS |
| workflow-sustained-auth-default | sustained | 174.819 | 7.411 | 0.000 | PASS |

## Bottleneck analysis

1. Highest p95 latency path: `web-small-parse` at 554.946 ms. Primary contributor is command invocation and crate/test harness startup overhead for the web runtime path.
2. Second-highest p95 latency path: `workflow-sustained-auth-default` at 174.819 ms. Dominant cost is workflow runtime startup and auth-policy initialization.
3. No path exceeded release SLO thresholds and all error budgets remained at 0.0%.

## Tuning outcomes and decisions

- No release-blocking bottlenecks found under the RC baseline profile.
- Retain current runtime defaults and track command-start overhead optimization as non-blocking post-RC work.
- Baseline digest locked as `sha256:cf6be0b46329092cf3391fc635ac753e113e2faf0c28e9da3800a41c9733ea03`.

## Signature

- Decision: Approved baseline performance evidence for Sprint 12
- Performance Lead: Signed
- V&V Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-PERF-BASELINE-20260211-RC2026.02.11
- Signed At (UTC): 2026-02-11T23:02:00Z
