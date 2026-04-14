# Alert Taxonomy and Escalation Mapping

Status: **Implemented** (As of 2026-02-24)  
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## 1. Scope

This document defines alert classes and escalation for:

- connector degradation
- connector callback circuit-open events
- reconciliation backlog growth

## 2. Severity model

| Severity | Meaning | Initial response target |
|---|---|---|
| `SEV-1` | Active production outage, data-loss risk, or sustained callback blackout | 15 minutes |
| `SEV-2` | Production degradation with partial functionality loss | 30 minutes |
| `SEV-3` | Early warning trend that can become production impact | 4 hours |

## 3. Alert taxonomy

| Alert ID | Trigger condition | Severity | Owner |
|---|---|---|---|
| `ALERT-CONNECTOR-DEGRADED-RATE` | Connector degradation ratio exceeds threshold in rolling window | `SEV-2` | Workflow on-call |
| `ALERT-CONNECTOR-CIRCUIT-OPEN` | Circuit breaker open interval active for any connector alias | `SEV-1` | Workflow on-call |
| `ALERT-RECONCILIATION-BACKLOG-GROWTH` | Reconciliation queued-minus-completed delta grows for consecutive windows | `SEV-2` | Integration on-call |
| `ALERT-RECONCILIATION-BACKLOG-EARLY-WARN` | Backlog trend slope positive over warning threshold only | `SEV-3` | Integration on-call |

## 4. Escalation routing

### 4.1 Connector degradation

1. Workflow on-call investigates connector health/status and callback failure stream.
2. If unresolved after one response window, escalate to Integration Lead.
3. If tenant-impact spans critical tenants, escalate to Release Manager.

### 4.2 Connector circuit-open

1. Workflow on-call acknowledges and confirms circuit-open duration.
2. Trigger immediate connector rollback policy review.
3. Escalate to Security Lead if auth/signing regressions are suspected.
4. Escalate to Release Manager for release hold decision when persistent.

### 4.3 Reconciliation backlog growth

1. Integration on-call validates job schedule, throughput, and tenant scope.
2. Apply backlog drain controls and retry policy tuning.
3. Escalate to Workflow Lead when backlog remains above threshold for two windows.
4. Escalate to Release Manager if backlog blocks release/restore objectives.

## 5. Required incident artifacts

For each alert-driven incident:

- incident ticket with alert ID and severity
- affected connector/tenant identifiers
- mitigation timeline
- closure decision and follow-up tasks

## 6. Operational check

This taxonomy must be reviewed each release cycle and aligned with:

- `docs/56-Incident-Triage-Runbook-Gate-Domains.md`
- runtime monitoring artifact templates in `reports/monitoring/`
