# Contributing

This repository is **docs-as-source-of-truth**. Contributions are accepted only when they preserve correctness, determinism, and security posture.

## Development rules (normative)

- **REQ-OPS-020:** Changes **MUST** be scoped and reviewable.
- **REQ-OPS-021:** Any change that affects behavior **MUST** include:
  - tests (unit/integration/corpus),
  - documentation updates in `docs/`,
  - an explicit statement of conformance impact (if any).
- **REQ-OPS-022:** New public APIs **MUST** be specified in `docs/12-API-Surface-and-Crate-Boundaries.md`.

Verification:
- PR checklist review confirms scope, tests, docs, and conformance impact statement (REQ-OPS-020, REQ-OPS-021).
- API review confirms `docs/12` updated for any new public API surface (REQ-OPS-022).

## Local checks (required)

- **REQ-OPS-023:** Before opening a PR, contributors **MUST** run:

```bash
cargo build
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

- **REQ-OPS-024:** If fuzzing is being modified, contributors **MUST** run the relevant fuzz target(s) per `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

Verification:
- CI or review confirms the command outputs are attached or reproducible (REQ-OPS-023).
- Fuzzing change review confirms relevant fuzz targets were executed (REQ-OPS-024).

## Documentation updates

- **REQ-OPS-025:** Specs in `docs/` **MUST** use RFC 2119 language (MUST/SHOULD/MAY).
- **REQ-OPS-026:** Specs **MUST NOT** contain placeholders such as “to be determined”.
- **REQ-OPS-027:** When repository reality diverges from specs, contributors **MUST** either:
  - update implementation to match specs, **or**
  - update specs and the conformance envelope, plus tests.

Verification:
- Documentation lint or review confirms RFC 2119 language and no placeholders (REQ-OPS-025, REQ-OPS-026).
- Conformance review confirms spec/implementation alignment and envelope updates when needed (REQ-OPS-027).

## Claim surface lint (required when applicable)

- **REQ-OPS-028:** If `README.md` or any file under `docs/` is modified, contributors **MUST** run `python3 tools/claim_surface_lint.py` and record the result in the PR or change summary.

Verification:
- PR review confirms the denylist scan command/output is recorded when `README.md` or `docs/` changes are present (REQ-OPS-028).

## Security and privacy

- Do not commit datasets containing PHI/PII.
- Public corpora references are permitted only via hash manifests and download instructions outside CI, per `docs/10`.

Verification:
- Scan changes for any dataset additions or references that violate PHI/PII constraints.
- Confirm corpus references are via hashes and external download instructions only.

See `SECURITY.md`.

## Docs drift lint (required when applicable)

- **REQ-OPS-029:** If `README.md` or any file under `docs/` is modified, contributors **MUST** run `python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json` and include the result (or artifact) in the PR/check summary.
- **REQ-OPS-030:** Pull requests that modify `README.md` or `docs/` **SHOULD** attach or reference the latest `reports/docs/drift-report.json` artifact.

Verification:
- PR review confirms docs-drift lint output is recorded for docs/README changes (REQ-OPS-029).
- PR review confirms the latest drift report artifact is attached or referenced when required (REQ-OPS-030).

See also: `docs/37-Docs-Drift-Lint.md`.
