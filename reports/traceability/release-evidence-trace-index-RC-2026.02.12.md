# Release Evidence Trace Index

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Release Manager

## Traceability clusters

| Cluster ID | Claim ID | Bound release index entry | Gate outcomes |
|---|---|---|---|
| TRC-CLUSTER-S01-001 | CLM-S01-001 | `REL-IDX-S01-001` | `s01-evidence-sign-verify` PASS, `s01-evidence-realism-lint` PASS |
| TRC-CLUSTER-S01-002 | CLM-S01-002 | `REL-IDX-S01-002` | `s01-evidence-realism-lint` PASS |
| TRC-CLUSTER-S01-003 | CLM-S01-003 | `REL-IDX-S01-003` | `s01-evidence-realism-lint` PASS |
| TRC-CLUSTER-S02-001 | CLM-S02-001 | `REL-IDX-S02-001` | `s02-interoperability-capture-dicomweb` PASS, `s02-interoperability-capture-dimse` PASS, `s02-interoperability-hash-verify` PASS |
| TRC-CLUSTER-S02-002 | CLM-S02-002 | `REL-IDX-S02-001` | `s02-interoperability-capture-workflow` PASS, `s02-interoperability-capture-negative` PASS |
| TRC-CLUSTER-S02-003 | CLM-S02-003 | `REL-IDX-S02-001` | `s02-evidence-realism-lint` PASS, `s02-traceability-report` PASS |
| TRC-CLUSTER-S03-001 | CLM-S03-001 | `REL-IDX-S03-001` | `s03-reproducibility-compare` PASS, wave-2 capture/hash gates PASS |
| TRC-CLUSTER-S03-002 | CLM-S03-002 | `REL-IDX-S03-001` | `s03-interoperability-capture-negative-wave2` PASS, boundary negatives PASS |
| TRC-CLUSTER-S03-003 | CLM-S03-003 | `REL-IDX-S03-001` | `s03-case-volume-threshold-gate` PASS, `s03-trace-bindings-verify` PASS |

## Interop case binding coverage

- Case binding artifact: `reports/traceability/interoperability-case-bindings-RC-2026.02.12.json`
- Human-readable binding table: `reports/traceability/interoperability-case-bindings-RC-2026.02.12.md`
- Coverage result: `61/61` interop case IDs bound to REQ IDs and trace cluster IDs.
- Verification log: `reports/analytical/gates/s03-trace-bindings-verify-RC-2026.02.12.log`
- Unbound case IDs: `0`.

## Bound artifacts

- `reports/interoperability/release-evidence-index.md`
- `reports/interoperability/verification-matrix.md`
- `reports/interoperability/negative-path-matrix.md`
- `reports/interoperability/environment-matrix.md`
- `reports/interoperability/hash-verification-register-RC-2026.02.12.md`
- `reports/interoperability/hash-verification-register-wave2-RC-2026.02.12.md`
- `reports/interoperability/reproducibility-wave2-RC-2026.02.12.md`
- `reports/interoperability/wave2-independent-review-RC-2026.02.12.md`
- `reports/traceability/claim-evidence-trace-bundle-RC-2026.02.12.md`
- `reports/analytical/ANL-S01-CLOSE.md`
- `reports/analytical/ANL-S02-CLOSE.md`
- `reports/analytical/ANL-S03-CLOSE.md`

## Signature

- Release Manager: Signed
- Traceability Lead: Signed
- Signature ID: SIG-TRC-REL-IDX-20260212
- Signed At (UTC): 2026-02-12T02:11:00Z
