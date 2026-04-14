# Performance Harness Report

Run Label: baseline-RC-2026.02.11
Generated At (UTC): 2026-02-11T21:38:16Z
Report Digest (sha256): `cf6be0b46329092cf3391fc635ac753e113e2faf0c28e9da3800a41c9733ea03`

## Summary

- Workloads: 5
- Violations: 0

## Workload Results

| Workload ID | Category | Iterations | p95 Latency (ms) | Throughput (ops/s) | Error Rate (%) | RSS Growth (%) | Latency Growth (%) | Result |
|---|---|---|---|---|---|---|---|---|
| web-small-parse | small | 3 | 554.946 | 3.968 | 0.000 | 0.731 | -89.382 | PASS |
| dimse-medium-cecho | medium | 3 | 113.519 | 9.995 | 0.000 | 0.711 | -17.162 | PASS |
| workflow-large-restart | large | 3 | 117.687 | 9.731 | 0.000 | 0.104 | 9.859 | PASS |
| webserver-burst-preflight-rotation | burst | 4 | 117.938 | 9.350 | 0.000 | -0.436 | -15.084 | PASS |
| workflow-sustained-auth-default | sustained | 6 | 174.819 | 7.411 | 0.000 | -0.993 | 41.229 | PASS |
