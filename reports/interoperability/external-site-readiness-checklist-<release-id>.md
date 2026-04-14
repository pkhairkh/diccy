# External Site Readiness Checklist - <release-id>

Status: production go-live artifact for enterprise interoperability onboarding.

## 1. Site metadata

- Site name:
- Release ID: `<release-id>`
- Environment: `stage|prod`
- Tenant(s):
- Primary connector alias(es):
- Deployment owner:
- Interoperability owner:
- Security owner:

## 2. Connector and transport readiness

- [ ] Connector targets configured and validated.
- [ ] Feature flags and rollout percentages explicitly set.
- [ ] Transport mode approved (MLLP/file-drop/HTTPS bridge).
- [ ] MLLP ACK/NACK behavior validated (AA/AE/AR + deterministic ERR codes).
- [ ] Dual-mode ingest conflict behavior validated (MLLP + file-drop).

## 3. Auth and secret posture

- [ ] Auth mode validated fail-closed.
- [ ] Webhook auth strategy (`none` or `hmac-sha256`) approved.
- [ ] Secret/certificate rotation rollback plan documented.
- [ ] Denied-role and invalid-secret negative paths validated.

## 4. Tenant isolation and policy controls

- [ ] Tenant propagation contract validated.
- [ ] Cross-tenant isolation tests passed.
- [ ] Tenant quota/rate overrides reviewed and approved.
- [ ] Reconciliation idempotency behavior verified (`x-idempotency-key`).

## 5. Operability and recovery

- [ ] Connector health/status/features/capabilities endpoints monitored.
- [ ] DLQ persistence and replay workflow verified.
- [ ] Rollout persistence verified across restart.
- [ ] On-call escalation mapping and runbooks linked.

## 6. Evidence links

- Interop API verification report:
- Security gate decision:
- Performance/readiness evidence:
- Incident runbook link:
- Rotation runbook execution record:

## 7. Sign-off

- Go/No-Go decision:
- Approved by (Deployment Owner):
- Approved by (Interoperability Lead):
- Approved by (Security Lead):
- Decision timestamp (UTC):

