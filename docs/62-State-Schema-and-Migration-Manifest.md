# State Schema and Migration Manifest

Status: active migration manifest for persisted runtime state.

## 1. Scope

This manifest governs persisted state compatibility for:

- workflow snapshots and interop state files,
- web runtime WAL persistence,
- DIMSE runtime WAL persistence.

Machine-readable source:
- `reports/docs/state-schema-migration-manifest.v1.json`

## 2. Versioned state inventory

| State artifact | Current schema | N-1 schema | Upgrade policy | Downgrade policy |
|---|---|---|---|---|
| Workflow worklist snapshot (`DICOM_WORKFLOW_WORKLIST_STATE_PATH`) | `2` | `1` | In-place startup upgrade accepted from `1 -> 2` | Rollback requires pre-upgrade backup restore |
| Workflow MPPS snapshot (`DICOM_WORKFLOW_MPPS_STATE_PATH`) | `2` | `1` | In-place startup upgrade accepted from `1 -> 2` | Rollback requires pre-upgrade backup restore |
| Workflow SR snapshot (`DICOM_WORKFLOW_SR_STATE_PATH`) | `2` | `1` | In-place startup upgrade accepted from `1 -> 2` | Rollback requires pre-upgrade backup restore |
| Workflow HL7 failure DLQ (`<audit>.hl7-failures.dlq`) | `2` | `1` | `1 -> 2` expands rows with deterministic defaults (`attempt=1`, `max_attempts=3`) | `2 -> 1` projection drops `attempt/max_attempts` columns |
| DICOMweb WAL (`DICOM_WEB_STORAGE_WAL`) | `1` | `1` | No schema transform required | No schema transform required |
| DIMSE WAL (`DICOM_DIMSE_STORAGE_WAL`) | `1` | `1` | No schema transform required | No schema transform required |

## 3. Upgrade and downgrade rules

1. Only `N-1 -> N` in-place upgrade is supported automatically.
2. `N-2` or older requires offline migration tooling before startup.
3. Rollback is always backup-first:
   - restore immutable backup captured before migration,
   - then restart on prior runtime profile.
4. Any schema default changes require:
   - docs updates in `docs/09`, `docs/14`, this manifest,
   - migration tests in runtime/tooling suites.

## 4. Verification requirements

- Migration tests must cover:
  - `N-1 -> N` read path,
  - rollback projection/restore behavior.
- Release evidence must include:
  - immutable backup manifest,
  - restore verification report linked to release ID.

