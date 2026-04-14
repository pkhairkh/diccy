# Performance Harness Report

Run Label: soak-RC-2026.02.11
Generated At (UTC): 2026-02-11T21:38:24Z
Report Digest (sha256): `3999b76489f19f73d2270f9073a8e82bc7cfe4c9c4976359ce408ab3e297f4f2`

## Summary

- Workloads: 3
- Violations: 0

## Workload Results

| Workload ID | Category | Iterations | p95 Latency (ms) | Throughput (ops/s) | Error Rate (%) | RSS Growth (%) | Latency Growth (%) | Result |
|---|---|---|---|---|---|---|---|---|
| web-sustained-parse | sustained | 10 | 254.270 | 6.676 | 0.000 | 0.346 | -48.939 | PASS |
| dimse-sustained-cecho | sustained | 10 | 497.723 | 4.018 | 0.000 | -0.101 | -20.973 | PASS |
| workflow-sustained-restart | sustained | 8 | 335.440 | 4.726 | 0.000 | -0.276 | 31.364 | PASS |
