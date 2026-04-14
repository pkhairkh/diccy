# Release Evidence Trace Index

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Release Manager

## Traceability clusters

| Cluster ID | Claim ID | Bound release index entry | Gate outcomes |
|---|---|---|---|
| TRC-CLUSTER-001 | CLM-TRC-001 | `REL-IDX-SEC-002` | `s13-security-fuzz-campaign` PASS, `s13-security-dependency-review` PASS |
| TRC-CLUSTER-002 | CLM-TRC-002 | `REL-IDX-TRACE-003` | `s14-traceability-report` PASS, `s14-cargo-test` PASS |
| TRC-CLUSTER-003 | CLM-TRC-003 | `REL-IDX-INTEROP-001` | `s11-traceability` PASS, interoperability capture gates PASS |
| TRC-CLUSTER-004 | CLM-TRC-004 | `REL-IDX-TRACE-003` | `s14-req-completeness-lint` PASS, `s14-traceability-report` PASS |
| TRC-CLUSTER-005 | CLM-TRC-005 | `REL-IDX-GOV-004` | `s15-release-preflight` PASS, `s15-release-artifact-integrity` PASS |
| TRC-CLUSTER-006 | CLM-TRC-006 | `REL-IDX-PMCF-005` | `s16-post-release-monitor` PASS, `s16-traceability-report` PASS |

## Bound artifacts

- `reports/interoperability/release-evidence-index.md`
- `reports/security/security-gate-decision-RC-2026.02.11.md`
- `reports/traceability/traceability-snapshot-RC-2026.02.11.md`
- `reports/traceability/claim-evidence-trace-bundle-RC-2026.02.11.md`
- `reports/release/rc-governance-gate-decision-RC-2026.02.11.md`
- `reports/pmcf/PMCF-040.md`
- `reports/release/master-grade-closeout-decision-RC-2026.02.11.md`

## Signature

- Release Manager: Signed
- Traceability Lead: Signed
- Signature ID: SIG-TRC-REL-IDX-20260211
- Signed At (UTC): 2026-02-11T22:31:00Z
