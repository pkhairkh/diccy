# Startup Contract Snapshot: `backend-services`

Generated for production-readiness tracking. Scope: `dicom-web-server` + `dicom-workflow-server`.

## Profile composition

- Included services:
  - `dicom-web-server`
  - `dicom-workflow-server`
- Excluded services:
  - `dicom-dimse-service`

## Startup fail-closed contract

### `dicom-web-server`

- Auth and transport posture must remain fail-closed by default.
- Required startup preflight outcomes:
  - persistence paths writable and rotation-bounded
  - configured bind value syntactically valid
  - TLS/auth contract violations fail startup
- Operational readiness routes:
  - `GET|HEAD /healthz`
  - `GET|HEAD /readyz`

### `dicom-workflow-server`

- Auth and transport posture must remain fail-closed by default.
- Required startup preflight outcomes:
  - worklist, MPPS, SR, and audit persistence paths writable and rotation-bounded
  - connector rollout snapshot path writable when configured
  - HL7 callback DLQ persistence path writable when configured
  - reconciliation run idempotency snapshot path writable when configured
  - tenant rate-limit override snapshot path writable when configured
- Operational readiness routes:
  - `GET|HEAD /healthz`
  - `GET|HEAD /readyz`

## Evidence mapping

- Startup preflight smoke workflow:
  - `.github/workflows/backend-startup-preflight-smoke.yml`
- Startup preflight checker:
  - `tools/startup_preflight_check.sh`
- Runtime contract sources:
  - `crates/dicom-web-server/src/main.rs`
  - `crates/dicom-workflow-server/src/main.rs`
  - `crates/dicom-workflow-server/src/lib.rs`
