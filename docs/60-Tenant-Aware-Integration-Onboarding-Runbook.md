# Tenant-Aware Integration Onboarding Runbook

Status: operator runbook for enterprise connector onboarding in multi-tenant deployments.

## 1. Inputs and ownership

- Target tenant ID and owner.
- Connector alias/endpoint (`DICOM_WORKFLOW_HL7_CONNECTOR_<ALIAS>`).
- Required transport mode (MLLP, file-drop, HTTPS bridge).
- Auth strategy (`DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY`).
- Rollout target and rollback owner.

## 2. Tenant baseline and isolation checks

1. Confirm ingress header contract for tenant propagation:
   - `x-tenant`, `x-workflow-tenant`, `x-tenant-id`, `x-workflow-tenant-id`.
2. Confirm tenant policy baseline:
   - task/subscription quota and rate-limit defaults.
3. Confirm tenant-specific overrides (if used):
   - `DICOM_WORKFLOW_TENANT_TASK_QUOTA_<TENANT>`
   - `DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_<TENANT>`
   - `DICOM_WORKFLOW_TENANT_QUERY_RATE_LIMIT_<TENANT>`
   - `DICOM_WORKFLOW_TENANT_MUTATION_RATE_LIMIT_<TENANT>`
   - `DICOM_WORKFLOW_TENANT_UPLOAD_CAP_BYTES_<TENANT>`
4. Verify cross-tenant deny behavior with integration tests before enabling traffic.

## 3. Connector setup sequence

1. Set connector target and optional plugin metadata.
2. Validate compatibility bounds at startup (`compatible_min`, `compatible_max`).
3. Register callback subscriptions with explicit `source` and `event_filter`.
4. Confirm capability/health endpoints:
   - `/interop/connectors/capabilities`
   - `/interop/connectors/status`
   - `/interop/connectors/health`
   - `/interop/connectors/features`

## 4. Auth strategy and secret posture

1. Select auth mode and fail closed:
   - `DICOM_WORKFLOW_AUTH_MODE=token` (or approved equivalent).
2. Set webhook auth strategy:
   - `none` or `hmac-sha256`.
3. If `hmac-sha256` is selected, provision secret before traffic cutover.
4. Validate denied-role and invalid-secret paths return deterministic failures.

## 5. Rollout strategy

1. Start connector feature flag and rollout at conservative baseline.
2. Use `POST /interop/connectors/rollout` with admin role.
3. Rollout API compatibility:
   - request accepts `rollout_percent` (canonical) and `percent` (legacy).
4. Confirm restart persistence:
   - `<DICOM_WORKFLOW_AUDIT_PATH>.connector-rollout`
5. Define rollback threshold and execute rollback drill before production enablement.

## 6. Health checks and evidence

- Validate `/interop/hl7/failures` is monitored and replay workflow is known.
- Validate callback policy and bounds:
  - `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS`
  - `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD`
  - `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS`
  - `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS`
- Confirm MLLP/file-drop mode decision is signed off by deployment owner.
- Record final go-live evidence in:
  - `reports/interoperability/external-site-readiness-checklist-<release-id>.md`

