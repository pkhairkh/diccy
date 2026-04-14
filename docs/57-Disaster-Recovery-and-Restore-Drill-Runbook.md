# Disaster Recovery and Restore Drill Runbook

Status: **Implemented** (As of 2026-02-24)  
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## 1. Scope

This runbook defines the recovery process for:

- workflow snapshots (`worklist`, `mpps`, `sr`)
- WAL-backed stores (web storage WAL and workflow-adjacent persistent logs)

It includes an executable restore drill used as release evidence.

## 2. Recovery prerequisites

Before executing recovery:

1. Confirm release identifier and incident ticket.
2. Freeze mutation traffic for affected services.
3. Capture current on-disk state paths and permissions.
4. Confirm backup set integrity checksum for all targeted files.

## 3. Restore workflow (operational)

1. Identify baseline backup set for target release.
2. Stop affected services.
3. Restore backup files to active state paths.
4. Validate file ownership, permissions, and writable parent directories.
5. Restart services in dependency order:
   - `dicom-workflow-server`
   - `dicom-web-server`
   - `dicom-dimse-service` (profile-gated)
6. Verify service health/readiness and representative read-path checks.
7. Re-enable controlled mutation traffic.

## 4. Executable restore drill

Use deterministic drill automation:

```bash
python3 tools/workflow_restore_drill.py --release-id <release-id>
```

Outputs:

- `reports/release/workflow-restore-drill-<release-id>.json`
- `reports/release/workflow-restore-drill-<release-id>.md`

Pass condition:

- every restored file SHA-256 equals the original seeded SHA-256.

## 5. Failure handling

If drill or live restore fails:

1. Keep deployment in rollback/hold state.
2. Preserve failed artifact outputs for forensic review.
3. Re-run restore from last known-good backup set.
4. Escalate to Workflow Lead and Release Manager with evidence links.

## 6. Release evidence requirement

Each production release must include:

- restore drill artifact pair (`.json` + `.md`) under `reports/release/`
- explicit pass/fail disposition in release checklist notes.
