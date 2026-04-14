# Performance Workload Profile Specification

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Performance and Reliability Board

## Purpose

Define deterministic workload classes and execution profiles used by `tools/performance_harness.py` for Sprint 12 evidence.

## Workload classes

| Class | Intent | Typical scale |
|---|---|---|
| small | Minimal single-path verification load | 3 iterations, 1 op/iteration |
| medium | Representative service-path load | 3-6 iterations, 1 op/iteration |
| large | Heavier path involving persistence/restart logic | 3-8 iterations, 1 op/iteration |
| burst | Short high-frequency command burst | 4-8 iterations, minimal think time |
| sustained | Longer steady-state repetition for growth checks | 8-12 iterations with think time |

## Profile set

| Profile ID | File | Coverage |
|---|---|---|
| perf-baseline-rc-2026-02-11 | `reports/performance/profiles/baseline-profile-RC-2026.02.11.json` | small + medium + large + burst + sustained baseline metrics |
| perf-soak-rc-2026-02-11 | `reports/performance/profiles/soak-profile-RC-2026.02.11.json` | sustained stability and growth checks |
| perf-failure-injection-rc-2026-02-11 | `reports/performance/profiles/failure-injection-profile-RC-2026.02.11.json` | restart/storage/auth fail-closed paths |

## Determinism controls

- All workloads use explicit command arrays and fixed iteration counts.
- Thresholds are encoded in profile JSON and evaluated by the harness.
- Generated reports include a deterministic digest over sorted workload results.

## Signature

- Decision: Approved for Sprint 12 execution
- Performance Lead: Signed
- V&V Lead: Signed
- Release Manager: Signed
- Signature ID: SIG-PERF-WL-20260211-RC2026.02.11
- Signed At (UTC): 2026-02-11T22:59:00Z
