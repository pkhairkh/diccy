# HA and Rolling Upgrade Runbook

Date scope: As of 2026-02-22

## 1. Rolling upgrade goals

- Preserve in-flight ingest and query availability.
- Preserve deterministic state continuity for storage and snapshots.
- Preserve auth/transport posture (`deny_all`/TLS policy defaults) during rollout.
- Emit deterministic evidence artifacts in release notes.

## 2. Core service rollout (default web + workflow)

1. Preflight:
   - run `./tools/startup_preflight_check.sh`
   - confirm required dirs are writable
   - confirm `state/` snapshot WAL paths are mounted
2. Drain one service instance:
   - Kubernetes:
     - `kubectl rollout restart deployment/dicom-web-runtime`
     - `kubectl rollout status deployment/dicom-web-runtime`
     - `kubectl rollout restart deployment/dicom-workflow-runtime`
     - `kubectl rollout status deployment/dicom-workflow-runtime`
3. Validate:
   - `curl -sS http://<web>/healthz`
   - `curl -sS http://<workflow>/healthz`
   - execute a smoke ingest/query flow through an existing dataset
4. Roll back if any deterministic health check fails:
   - `kubectl rollout undo deployment/dicom-web-runtime`
   - `kubectl rollout undo deployment/dicom-workflow-runtime`

## 3. DIMSE-enabled rollout

1. Ensure DIMSE integration points are in fail-closed mode (`DICOM_DIMSE_AUTH_MODE=deny_all`) before start.
2. Restart DIMSE deployment separately to avoid coupling with web/workflow plane:
   - `kubectl rollout restart deployment/dicom-dimse-runtime`
   - `kubectl rollout status deployment/dicom-dimse-runtime`
3. Validate DIMSE negotiation path and fallback behavior:
   - if DIMSE negotiation fails, route through DICOMweb fallback path in `docs/51-Integration-Cookbook.md`.

## 4. Single-node vs multi-node presets

| Preset | Recommended replicas | State strategy |
|---|---|---|
| Single-node (`core`) | 1 | Local filesystem-backed state with quick snapshot intervals |
| Multi-node (`enterprise`) | 2+ for web/workflow, 1+ for DIMSE depending on topology | Shared persistent storage for WAL and snapshot directories |

## 5. Runtime resources and autoscaling guidance

- Suggested starting points:
  - web: `500m` CPU / `1Gi` memory request, `2Gi` memory limit
  - workflow: `300m` CPU / `768Mi` memory request, `2Gi` memory limit
  - DIMSE: `500m` CPU / `512Mi` memory request, `1Gi` memory limit
- Add autoscaling when p95 latency exceeds target or sustained CPU exceeds 70%:
  - web replicas: 2–8 with scale-up at 65%
  - workflow replicas: 2–6 with scale-up at 70%
  - DIMSE: typically pinned; scale only with explicit topology review
- Add resource guards:
  - `terminationGracePeriodSeconds` ≥ 45
  - `readinessProbe` on front-door endpoints (when supported by runtime image)

## 6. Evidence capture

- Add these artifacts to release notes:
  - preflight log
  - rollout completion timestamps
  - first-API response success rate for post-upgrade smoke flow
