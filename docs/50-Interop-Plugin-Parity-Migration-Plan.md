# Interop plugin parity migration plan (OHIF/Orthanc patterns)

Status: **Planned** (As of 2026-02-24)
Reference: `docs/31-Implementation-Status.md`

## 1. Scope

This plan defines acceptance criteria for converging the workflow interoperability plugin surface with
operator expectations established by OHIF/Orthanc-style deployments.

## 2. Target capabilities

- Normalized connector adapter model for HIS/RIS and DIMSE-facing routes.
- Plugin metadata contract (`path`, `version`, compatibility range) exposed in connector diagnostics.
- Health endpoint with standardized error payloads for unconfigured or failing connector states.
- Callback delivery idempotency keyed by event and subscription.
- Feature-flagged FHIR ingest skeleton path for controlled rollout.

## 3. Migration phases

1. Contract baseline:
- Lock endpoint and env-contract shape in `docs/12-API-Surface-and-Crate-Boundaries.md`.
- Add route-contract assertions for connector health and FHIR skeleton endpoints.

2. Runtime hardening:
- Enforce plugin path validation and compatibility metadata parsing.
- Add tenant-aware policy object for per-tenant quotas and baseline throttles.

3. UI/operator surface:
- Ship connector admin panel for plugin path and rollout controls.
- Ship tenant health dashboard with role-aware read-only mode.

4. Verification:
- Integration tests for enterprise-HIS routing fallback.
- Error-path coverage for connector health and plugin validation.
- Publish dated interoperability or compatibility evidence under `reports/interoperability/` or `reports/compatibility/`.

## 4. Acceptance criteria

- `GET /interop/connectors/health` returns standardized connector health payloads.
- `GET /interop/connectors/features` includes adapter kind and plugin compatibility metadata.
- `POST /interop/fhir` fails closed when `DICOM_WORKFLOW_FHIR_INGEST_ENABLED=false` and accepts skeleton payloads when enabled.
- Connector callbacks are idempotent for duplicate event/subscription tuples.
- Tenant policy limits are applied to task and subscription creation paths.
- Verification artifacts are published with dated evidence references suitable for external review.
