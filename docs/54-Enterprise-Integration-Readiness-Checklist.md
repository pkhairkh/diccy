# Enterprise Integration Readiness Checklist

Status: integration control checklist for deployment readiness reviews.

## 1. HL7 connector readiness

- [ ] Connector aliases and targets are declared (`DICOM_WORKFLOW_HL7_CONNECTOR_*`) and reviewed for wildcard precedence.
- [ ] Feature flags and rollout percentages are explicitly set for production connectors (`DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_*`, `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_*`).
- [ ] Connector plugin compatibility bounds (`compatible_min`/`compatible_max`) are validated at startup.
- [ ] Callback delivery failure controls are active:
  - circuit-breaker/backoff policy observed in runtime behavior,
  - callback idempotency TTL persistence file is writable and rotated by operational policy.
- [ ] Connector rollout persistence is validated:
  - rollout updates survive restart via `<DICOM_WORKFLOW_AUDIT_PATH>.connector-rollout`,
  - rollback procedure includes persisted rollout-state verification.
- [ ] HL7 callback dead-letter queue controls are validated:
  - DLQ persistence path `<DICOM_WORKFLOW_AUDIT_PATH>.hl7-failures.dlq` is writable,
  - operator replay/prune workflow is documented and executable (`tools/hl7_dead_letter_replay.py`).
- [ ] Connector health/status/rollout/capabilities endpoints are reachable and monitored:
  - `/interop/connectors/health`
  - `/interop/connectors/status`
  - `/interop/connectors/features`
  - `/interop/connectors/rollout`
  - `/interop/connectors/capabilities`
- [ ] Connector admin UI is API-backed (no static connector rows) and demonstrates:
  - retry behavior for transient workflow API failures,
  - degraded-state UX preserving last-known connector state when backend calls fail.

## 2. FHIR ingest readiness

- [ ] `DICOM_WORKFLOW_FHIR_INGEST_ENABLED` is explicitly configured for the target environment.
- [ ] Inbound callers are aligned to typed `POST /interop/fhir` contract:
  - `resource_type` is restricted to in-envelope set (`Bundle`, `Patient`, `Encounter`, `Observation`, `DiagnosticReport`),
  - required typed fields per `resource_type` are supplied (for example `Observation` requires `observation_code`, `subject_id`, `observed_at`),
  - `dry_run` values are normalized to `true|false|1|0`,
  - optional `content_sha256` uses 64-hex format.
- [ ] Fail-closed behavior is validated:
  - disabled endpoint returns `DVF.WORKFLOW.FHIR.DISABLED`,
  - invalid typed fields return deterministic HTTP 400 decode errors.

## 3. Tenant control readiness

- [ ] Tenant header propagation policy is documented at ingress (`x-tenant`, `x-workflow-tenant`, `x-tenant-id`, `x-workflow-tenant-id`).
- [ ] Cross-tenant access tests are validated for workflow, MPPS, SR, and reconciliation routes.
- [ ] Tenant quota override strategy is explicitly reviewed:
  - read-only quota API `/workflow/policy/quotas`,
  - snapshot export `/workflow/policy/quotas/snapshot`,
  - environment override keys (`DICOM_WORKFLOW_TENANT_TASK_QUOTA_*`, `DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_*`).
- [ ] Tenant rate-limit override strategy is explicitly reviewed:
  - environment override keys (`DICOM_WORKFLOW_TENANT_QUERY_RATE_LIMIT_*`, `DICOM_WORKFLOW_TENANT_MUTATION_RATE_LIMIT_*`, `DICOM_WORKFLOW_TENANT_UPLOAD_CAP_BYTES_*`),
  - persisted restart artifact `<DICOM_WORKFLOW_AUDIT_PATH>.tenant-rate-limit-overrides`,
  - default/fallback policy behavior validated for tenants without explicit overrides.
- [ ] Reconciliation job guardrails are confirmed:
  - interval bounds (`60..=86400` seconds),
  - max jobs and per-tenant caps,
  - tenant-scoped job run/list isolation,
  - run endpoint idempotency (`x-idempotency-key`) replay behavior across restarts.

## 4. Security and audit readiness

- [ ] Auth mode and transport policy remain fail-closed (`deny_all` + TLS policy alignment).
- [ ] Mutation endpoints require writer/admin/operator role controls and are validated via integration tests.
- [ ] Workflow audit trail is enabled and chain verification (`/workflow/audit?verify=true`) is exercised.
- [ ] Webhook auth strategy is explicitly chosen:
  - `none` or `hmac-sha256` (`DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY`),
  - shared secret is provisioned when `hmac-sha256` is enabled.

## 5. Operational sign-off

- [ ] Deployment owner signed off on HL7 transport mode (MLLP/file-drop/HTTPS bridge).
- [ ] On-call runbook includes connector rollout update and rollback procedure.
- [ ] Tenant-aware onboarding runbook is completed for each go-live tenant:
  - `docs/60-Tenant-Aware-Integration-Onboarding-Runbook.md`.
- [ ] Connector certificate/secret rotation runbook is approved:
  - `docs/61-Connector-Certificate-and-Secret-Rotation-Runbook.md`.
- [ ] Production promotion includes evidence links for:
  - connector health and failure dashboards,
  - FHIR typed contract verification,
  - tenant isolation and quota snapshot export checks,
  - interop failure-to-audit correlation verification (DLQ `correlation_id` to workflow audit request-id hash mapping),
  - external-site readiness artifact (`reports/interoperability/external-site-readiness-checklist-<release-id>.md`).
