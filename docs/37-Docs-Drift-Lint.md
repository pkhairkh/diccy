# Docs drift lint

Status: **Implemented** (As of 2026-02-21)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## Purpose

`tools/docs_drift_lint.py` detects contradictions between symbol status declarations in `docs/31-Implementation-Status.md` and claims made in `README.md` or `docs/*.md`.

The lint is designed to prevent stale claims such as "deferred until implementation" for symbols marked implemented.

## How to run

```bash
python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json
```

Outputs:
- Console `PASS/FAIL` summary
- Machine-readable report at `reports/docs/drift-report.json`

## Rule model

- Implemented symbols:
  - fail when symbol lines contain denylist contradiction phrases
- Deferred symbols:
  - allow deferred/planned wording from a strict whitelist
  - fail on direct implementation claims outside whitelist context

## CI integration

Workflow: `.github/workflows/docs-drift-gate.yml`

- Runs on pull requests that touch `README.md`, `docs/**`, or drift-lint tooling files
- Publishes `reports/docs/drift-report.json` as `docs-drift-report` artifact
- Runs monthly on schedule (`0 6 1 * *`) for rule-maintenance cadence

## Interpreting failures

Each violation contains:
- `file`, `line`
- `symbol`, `status`
- `kind`
- `message`
- offending `text`

Recommended resolution flow:
1. Confirm `docs/31-Implementation-Status.md` status is correct.
2. Update contradictory claim in `README.md` or `docs/*.md`.
3. Re-run `docs_drift_lint.py` and verify report is clean.

## Optional pre-commit hook

Install optional local hook:

```bash
./tools/install_precommit_hooks.sh
```

Behavior:
- Runs docs drift lint only when staged files include `README.md` or `docs/` paths.

