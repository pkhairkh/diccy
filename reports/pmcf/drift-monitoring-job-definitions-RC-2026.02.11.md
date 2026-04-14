# Drift Monitoring Job Definitions

Release Candidate: RC-2026.02.11
Generated At (UTC): 2026-02-11T22:24:03Z

| Job ID | Domain | Cadence | Owner | Threshold |
|---|---|---|---|---|
| DRIFT-REP-001 | reproducibility | daily | Reproducibility Lead | mismatch_count <= 0 |
| DRIFT-IOP-002 | interoperability | weekly | Interoperability Lead | non_pass_cases <= 0 |
| DRIFT-TRC-003 | traceability | weekly | Traceability Lead | missing_references_count <= 0 and invalid_links_count <= 0 |
| DRIFT-RPK-004 | release-package | on every release or hotfix cut | Release Manager | missing_required_count <= 0 and missing_index_reference_count <= 0 |

## Commands

- `DRIFT-REP-001`: `python3 tools/reproducibility_compare.py --manifest arm64=reports/analytical/reproducibility/actual-aarch64-apple-darwin-RC-2026.02.11.toml --manifest x86_64-projected=reports/analytical/reproducibility/projected-x86_64-unknown-linux-gnu-RC-2026.02.11.toml --manifest wasm32-projected=reports/analytical/reproducibility/projected-wasm32-unknown-unknown-RC-2026.02.11.toml --output-md reports/analytical/reproducibility/matrix-RC-2026.02.11.md --output-json reports/analytical/reproducibility/matrix-RC-2026.02.11.json --mismatch-budget 0`
- `DRIFT-IOP-002`: `python3 tools/interoperability_hash_verify.py --summary reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.11.json --summary reports/interoperability/execution/dimse-capture-summary-RC-2026.02.11.json --summary reports/interoperability/execution/workflow-capture-summary-RC-2026.02.11.json --summary reports/interoperability/execution/negative-capture-summary-RC-2026.02.11.json --output-md reports/interoperability/hash-verification-register-RC-2026.02.11.md --output-json reports/interoperability/hash-verification-register-RC-2026.02.11.json --fail-on-non-pass`
- `DRIFT-TRC-003`: `python3 tools/traceability_report.py --linkage-manifest reports/traceability/release-traceability-manifest-RC-2026.02.11.json --baseline-json reports/traceability/traceability-baseline-RC-2026.02.11.json --output-md reports/traceability/traceability-snapshot-RC-2026.02.11.md --output-json reports/traceability/traceability-snapshot-RC-2026.02.11.json --fail-on-invalid-links`
- `DRIFT-RPK-004`: `python3 tools/release_artifact_integrity.py --release-id RC-2026.02.11 --output-md reports/release/release-artifact-integrity-RC-2026.02.11.md --output-json reports/release/release-artifact-integrity-RC-2026.02.11.json --fail-on-errors`
