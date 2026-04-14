# Runtime Persistence Migration Runbook

Date: 2026-02-11
Status: Active
Owner: Runtime Platform Team

## Purpose

Provide an operational migration path from legacy in-memory runtime state to durable release-profile persistence for:

- `dicom-web-server` WAL-backed storage,
- `dicom-workflow-server` MWL snapshot persistence,
- `dicom-workflow-server` MPPS snapshot persistence,
- DIMSE storage-backed persistence paths.

## Scope

Applies to packaged runtime deployments and integration environments moving from in-memory startup behavior to durable startup defaults.

## Pre-Migration Checklist

1. Identify current runtime mode and data flow boundaries.
2. Record current version, envelope version, and deployment config.
3. Select persistence storage location with backup and access controls.
4. Ensure runtime process identity has read/write access to persistence directories.
5. Prepare rollback plan and contact owner for release decision.
6. Generate immutable backup manifest before migration:
   - `python3 tools/state_backup_manifest.py --repo-root . --state-root state --release-id <release-id>`

## Configuration Targets

### DICOMweb runtime

- Set `DICOM_WEB_STORAGE_WAL` to a persistent path.
- Set `DICOM_WEB_STORAGE_WAL_MAX_BYTES` and `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES`.
- Keep secure defaults enabled (`DICOM_WEB_TLS_POLICY=require_tls`, `DICOM_WEB_AUTH_MODE=deny_all`) unless an explicit approved override exists.

### Workflow runtime

- Set `DICOM_WORKFLOW_WORKLIST_STATE_PATH` and `DICOM_WORKFLOW_MPPS_STATE_PATH`.
- Set `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES` and `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES`.
- Keep secure transport and auth policies enabled for production (`DICOM_WORKFLOW_TRANSPORT_SECURITY=tls`, token-based auth where needed).

## Migration Procedure

1. Stop runtime write traffic (maintenance mode or controlled drain).
2. Snapshot existing environment and collect baseline evidence artifacts.
3. Deploy persistence configuration and start runtime.
4. Verify startup preflight succeeds (no invalid-path or write-access failures).
5. Execute smoke workload:
   - web store/retrieve round trip,
   - worklist insert/query,
   - MPPS ingest/query.
6. Restart runtime process.
7. Re-run smoke workload and confirm persisted continuity.
8. Record evidence references in release/interoperability indexes.
9. Verify snapshot export/import integrity payloads (checksum + tenant isolation):
   - `python3 tools/workflow_snapshot_integrity_verify.py --repo-root . --export-json <export.json> --import-json <import.json>`

## Verification Expectations

- Persistence files exist at configured paths.
- Runtime restart preserves previously ingested state.
- Oversized persistence files follow configured rotation behavior.
- Security defaults remain active after migration.
- Backup manifest and restore verification report exist for active `release_id`.

## Rollback Procedure

1. Stop runtime.
2. Restore prior configuration and prior state backup.
3. Restart runtime in previous mode.
4. Re-run smoke checks to confirm service recovery.
5. Record rollback reason and corrective actions in release notes.

## Audit Trail Fields

- Migration ID
- Environment
- Runtime versions
- Old config hash
- New config hash
- Evidence artifact IDs
- Decision (`complete`, `rolled_back`)
- Reviewer sign-off

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Initial persistence migration runbook | Runtime Platform Team |
