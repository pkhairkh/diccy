# Performance Capacity Model

Status: production planning model for backend-services and backend-services-with-dimse.

## 1. Modeling assumptions

- Workloads are deterministic and seed-driven.
- Tenant traffic is uneven (top tenants produce most mutation load).
- Connector callback behavior includes transient timeout bursts.
- Reconciliation jobs are periodic and bounded by configured intervals.

## 2. Capacity dimensions

| Dimension | Baseline planning unit | Growth driver |
|---|---|---|
| Tenant count | active tenants per profile | onboarding velocity |
| Connector count | configured connector aliases + subscriptions | external integration breadth |
| Reconciliation workload | jobs per tenant x run interval | cross-system data drift |
| DICOMweb API load | query/retrieve/ingest request mix | client/viewer adoption |

## 3. Planning envelopes

### Backend-services

- Tenant envelope: up to 250 active tenants before horizontal split planning.
- Connector envelope: up to 120 configured connectors.
- Reconciliation envelope: up to 1000 enabled jobs, minimum 60s interval.

### Backend-services-with-dimse

- Tenant envelope: up to 150 active tenants (DIMSE overhead factor).
- Connector envelope: up to 100 configured connectors.
- Reconciliation envelope: up to 800 enabled jobs.
- DIMSE association envelope follows `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS`.

## 4. Scaling strategy

1. Scale read/query traffic first by horizontal web/workflow replicas.
2. Isolate connector-heavy tenants using profile-level segmentation.
3. Keep callback circuit-breaker and rollout controls active during scale events.
4. Use reconciliation throttling before raising global limits.

## 5. Validation and evidence

- Validate envelopes through performance harness profiles:
  - `reports/performance/profiles/profile-backend-services-RC-2026.02.24.json`
  - `reports/performance/profiles/profile-backend-services-with-dimse-RC-2026.02.24.json`
- Validate sustained behavior with 24h soak artifact generation:
  - `tools/generate_soak_reliability_artifact.py`
- Validate resource budgets:
  - `reports/performance/resource-budget-baseline.json`
  - `tools/resource_budget_gate.py`

