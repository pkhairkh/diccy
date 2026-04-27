# Migration Guide: API-Only SR to Workflow Endpoints

Status: **Baseline migration guide** (As of 2026-02-22)
Reference: `docs/39-SR-Workflow-Architecture.md`

## 1. Who should use this guide

Use this guide when existing integrations call `pack-sr` APIs directly and need to move to service-backed workflow endpoints.

## 2. Endpoint mapping

| Existing integration pattern | Workflow endpoint pattern |
|---|---|
| Local SR create helper call | `POST /sr/documents` |
| Local SR update helper call | `POST /sr/documents/{sop_instance_uid}/updates` |
| Local SR reload from file/state | `GET /sr/documents/{sop_instance_uid}` |
| Local in-memory SR index | `GET /sr/documents?study_instance_uid=...` |

## 3. Required request headers

- `x-sr-principal`: caller identity.
- `x-sr-role`: must be `writer` for create/update.
- `x-idempotency-key`: required for create/update, deterministic per request payload.

## 4. Migration checklist

1. Add workflow base URL configuration (`http://127.0.0.1:8082` default local).
2. Add deterministic idempotency-key generation in client write path.
3. Add explicit `expected_version` handling for update requests.
4. Add read/list routes for retrieval and search.
5. Add conflict handling for `409` responses and retry handling for transient errors (`429`, `503`, etc.).
6. Add non-PHI audit telemetry for UI/service action traceability.

## 5. Validation commands

```bash
cargo test -p dicom-workflow-server sr_http_flow_create_update_retrieve_is_deterministic -- --exact
DICCY_WORKFLOW_INTEGRATION=1 ./tools/browser_sr_workflow.sh
```
