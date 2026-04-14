# Performance Harness Report

Run Label: failure-injection-RC-2026.02.11
Generated At (UTC): 2026-02-11T21:38:27Z
Report Digest (sha256): `9d2d813d6efbc84fafecf1eb95806243ca1b06cb9a067f7f6a594f857c57b081`

## Summary

- Workloads: 4
- Violations: 0

## Workload Results

| Workload ID | Category | Iterations | p95 Latency (ms) | Throughput (ops/s) | Error Rate (%) | RSS Growth (%) | Latency Growth (%) | Result |
|---|---|---|---|---|---|---|---|---|
| failure-restart-recovery | restart | 4 | 157.687 | 8.023 | 0.000 | 0.691 | -43.942 | PASS |
| failure-storage-path-reject | storage-path | 4 | 213.685 | 5.292 | 0.000 | 0.272 | -26.541 | PASS |
| failure-auth-deny-dimse | auth | 4 | 233.997 | 5.157 | 0.000 | -0.909 | -16.593 | PASS |
| failure-insecure-transport-reject | transport | 4 | 191.093 | 6.026 | 0.000 | -0.515 | -46.510 | PASS |
