# Runtime SLO Monitoring Artifact Template

- Release ID: `<release-id>`
- Environment: `<dev|stage|prod>`
- Profile: `<backend-services|backend-services-with-dimse>`
- Observation window: `<start UTC>` to `<end UTC>`
- Owner: `<team or on-call owner>`

## 1. Availability

| Service | Target | Observed | Error budget consumed | Status |
|---|---|---|---|---|
| `dicom-web-server` | `>= 99.9%` | `<value>` | `<value>` | `<PASS|FAIL>` |
| `dicom-workflow-server` | `>= 99.9%` | `<value>` | `<value>` | `<PASS|FAIL>` |
| `dicom-dimse-service` (if enabled) | `>= 99.5%` | `<value>` | `<value>` | `<PASS|FAIL|N/A>` |

## 2. Error rate

| Surface | Target | Observed | Sample size | Status |
|---|---|---|---|---|
| DICOMweb request errors | `<= 1.0%` | `<value>` | `<value>` | `<PASS|FAIL>` |
| Workflow mutation errors | `<= 0.5%` | `<value>` | `<value>` | `<PASS|FAIL>` |
| Connector callback errors | `<= 1.0%` | `<value>` | `<value>` | `<PASS|FAIL>` |

## 3. Latency p95

| Route/Operation | Budget p95 | Observed p95 | Status |
|---|---|---|---|
| `/studies` query (QIDO) | `250 ms` | `<ms>` | `<PASS|FAIL>` |
| Workflow read endpoint | `12000 ms` | `<ms>` | `<PASS|FAIL>` |
| Workflow mutation endpoint | `15000 ms` | `<ms>` | `<PASS|FAIL>` |
| Connector callback dispatch | `15000 ms` | `<ms>` | `<PASS|FAIL>` |

## 4. Queue depth and backlog

| Queue/Backlog | Budget | Observed max | Observed p95 | Status |
|---|---|---|---|---|
| Web accept queue depth | `<value>` | `<value>` | `<value>` | `<PASS|FAIL>` |
| Connector callback backlog | `<value>` | `<value>` | `<value>` | `<PASS|FAIL>` |
| Reconciliation backlog | `<value>` | `<value>` | `<value>` | `<PASS|FAIL>` |

## 5. Alert and incident summary

| Alert ID | Count | Highest severity | Notes |
|---|---|---|---|
| Connector degradation | `<value>` | `<SEV-X>` | `<notes>` |
| Connector circuit-open | `<value>` | `<SEV-X>` | `<notes>` |
| Reconciliation backlog growth | `<value>` | `<SEV-X>` | `<notes>` |

## 6. Decision

- Release readiness posture: `<GO|HOLD|ROLLBACK>`
- Open risks:
  - `<risk 1>`
  - `<risk 2>`
- Required follow-up actions:
  - `<action 1>`
  - `<action 2>`
