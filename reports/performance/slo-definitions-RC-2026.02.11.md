# Performance and Scalability SLO Definitions

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Performance and Reliability Board

## Scope

These SLOs define the release gate for the major runtime service paths:

- DICOMweb request parse/runtime path
- DIMSE verification/runtime path
- Workflow runtime persistence/auth path
- Runtime preflight storage-path safety checks

## Global SLO policy

- Error budget is release-gating and fail-closed for all listed paths.
- Thresholds are evaluated using deterministic command profiles in `reports/performance/profiles/`.
- SLO evaluation artifacts are release-specific and hash-recorded in generated JSON summaries.

## Release SLO table

| Service path | Metric | SLO target (RC-2026.02.11) | Gate |
|---|---|---|---|
| DICOMweb parse/runtime | p95 latency | <= 2,000 ms | FAIL if exceeded |
| DICOMweb parse/runtime | throughput | >= 0.20 ops/s | FAIL if below target |
| DICOMweb parse/runtime | error budget | <= 0.0% | FAIL if exceeded |
| DIMSE verification/runtime | p95 latency | <= 5,000 ms | FAIL if exceeded |
| DIMSE verification/runtime | throughput | >= 0.15 ops/s | FAIL if below target |
| DIMSE verification/runtime | error budget | <= 0.0% | FAIL if exceeded |
| Workflow runtime (persistence/restart/auth) | p95 latency | <= 8,000 ms | FAIL if exceeded |
| Workflow runtime (persistence/restart/auth) | throughput | >= 0.10 ops/s | FAIL if below target |
| Workflow runtime (persistence/restart/auth) | error budget | <= 0.0% | FAIL if exceeded |
| Runtime preflight safety checks | p95 latency | <= 3,500 ms | FAIL if exceeded |
| Runtime preflight safety checks | throughput | >= 0.20 ops/s | FAIL if below target |
| Runtime preflight safety checks | error budget | <= 0.0% | FAIL if exceeded |

## Signature

- Decision: Approved for Sprint 12 release gating use
- Performance Lead: Signed
- V&V Lead: Signed
- Security Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-PERF-SLO-20260211-RC2026.02.11
- Signed At (UTC): 2026-02-11T22:58:00Z
