# Startup Contract Snapshot: `backend-services-with-dimse`

Generated for production-readiness tracking. Scope: `dicom-web-server` + `dicom-workflow-server` + `dicom-dimse-service`.

## Profile composition

- Included services:
  - `dicom-web-server`
  - `dicom-workflow-server`
  - `dicom-dimse-service`

## Startup fail-closed contract

### `dicom-web-server`

- Required startup preflight outcomes:
  - storage path and WAL controls writable under configured bounds
  - TLS/auth contract violations fail startup
  - bind and worker contract invalid values fail startup
- Operational readiness routes:
  - `GET|HEAD /healthz`
  - `GET|HEAD /readyz`

### `dicom-workflow-server`

- Required startup preflight outcomes:
  - worklist/MPPS/SR persistence and audit path preflight pass
  - connector rollout, HL7 DLQ, reconciliation idempotency, and tenant rate-limit snapshot artifacts are writable when configured
  - invalid route policy/env contracts fail startup
- Operational readiness routes:
  - `GET|HEAD /healthz`
  - `GET|HEAD /readyz`

### `dicom-dimse-service`

- Required startup preflight outcomes:
  - DIMSE storage root and optional audit paths writable
  - presentation context and association-limit contracts validated
  - TLS/auth policy violations fail startup where configured
- Operational readiness routes:
  - DIMSE runtime health endpoint contract validated by integration tests

## Evidence mapping

- Startup preflight smoke workflow:
  - `.github/workflows/backend-startup-preflight-smoke.yml`
- Startup preflight checker:
  - `tools/startup_preflight_check.sh`
- Runtime contract sources:
  - `crates/dicom-web-server/src/main.rs`
  - `crates/dicom-workflow-server/src/main.rs`
  - `crates/dicom-workflow-server/src/lib.rs`
  - `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`
