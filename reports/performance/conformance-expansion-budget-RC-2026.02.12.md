# Conformance Expansion Budget Report (S05)

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Sprint: 07 (Conformance Expansion Wave 2 / S05)
Status: Complete (Signed)
Owner: Performance + Conformance

## Scope

Performance/scalability budget verification for S05 activation of Tier 2 conformance packs in the workstation-completeness profile.

Activated S05 packs:
- `pack-enhanced`, `pack-us`, `pack-nm`, `pack-xa`, `pack-seg`, `pack-rt`, `pack-sr`, `gsps`

Measured workloads:
- Full `dicom-io` workstation-completeness conformance test suite (latency proxy for parse/validate path).
- Default `dicom-dimse` deferred-command negative suite (latency proxy for fail-closed DIMSE path).
- RSS telemetry from targeted codec/parse fuzz-wave logs.

## Method

Deterministic command set:
- `cargo test -p dicom-io --features "tier1-deflate codec-jpegls codec-j2k modality-pet modality-xr pack-enhanced pack-us pack-nm pack-xa pack-seg pack-rt pack-sr gsps"` (5 runs)
- `cargo test -p dicom-dimse` (3 runs)

Raw timing artifact:
- `reports/performance/conformance-expansion-budget-RC-2026.02.12.json`

Memory telemetry source:
- `reports/security/fuzz/logs/s04-wave1-RC-2026.02.12/*.log`

## Budget policy and verdict

| Budget item | Limit | Observed | Verdict |
|---|---:|---:|---|
| `dicom-io` warm-run p95 latency | <= 0.30 s | 0.1739 s | PASS |
| `dicom-io` cold-run max latency | <= 3.00 s | 2.3021 s | PASS |
| `dicom-dimse` p95 latency | <= 0.30 s | 0.1663 s | PASS |
| Max observed fuzz RSS | <= 64 MB | 27 MB | PASS |

Gate log:
- `reports/analytical/gates/s05-conformance-expansion-budget-RC-2026.02.12.log`

## Decision

S05 conformance expansion remains within defined latency and memory budgets for RC-2026.02.12.

Rollback decision:
- Not required (all SLO budget checks passed).

## Sign-off

- Performance Lead: Signed
- Conformance Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-PERF-S05-BUDGET-20260212
- Signed At (UTC): 2026-02-12T03:52:00Z
