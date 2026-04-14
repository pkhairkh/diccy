# Production Readiness Sign-Off Template

Status: **Implemented** (As of 2026-02-24)  
Reference: `docs/31-Implementation-Status.md#major-subsystems`

Use this template for go-live sign-off decisions. Keep one copy per release under `reports/release/production-readiness-signoff-<release-id>.md`.

## 1. Release metadata

- Release ID:
- Environment:
- Target go-live date:
- Change window:
- Rollback owner:

## 2. Roles and approvals

| Role | Name | Decision (`approve`/`reject`) | Timestamp (UTC) | Notes |
|---|---|---|---|---|
| Engineering Lead | | | | |
| Quality Lead | | | | |
| Security Lead | | | | |
| SRE/Operations Lead | | | | |
| Product/Program Owner | | | | |

## 3. Required evidence checklist

- [ ] Release controls gate report linked (`reports/release/release-controls-gate-<release-id>.json`)
- [ ] Release-control evidence index linked (`reports/release/release-control-evidence-index-<release-id>.json`)
- [ ] Security gate decision linked (`reports/security/security-gate-decision-<release-id>.md`)
- [ ] Performance gate decision linked (`reports/performance/performance-scalability-gate-decision-<release-id>.md`)
- [ ] Traceability manifest linked (`reports/traceability/release-traceability-manifest-<release-id>.json`)
- [ ] Artifact integrity report linked (`reports/release/release-artifact-integrity-<release-id>.json`)
- [ ] Startup contract snapshots linked (`reports/docs/startup-contract-snapshot-backend-services.md`, `reports/docs/startup-contract-snapshot-backend-services-with-dimse.md`)

## 4. Open-risk ledger

| Risk ID | Description | Severity (`low`/`medium`/`high`) | Mitigation owner | Mitigation due date | Closure status |
|---|---|---|---|---|---|
| | | | | | |
| | | | | | |

## 5. Explicit go/no-go criteria

Go criteria:
- All critical gates are PASS with artifact links present.
- No unresolved `high` severity risks.
- Rollback drill artifact is present and validated.

No-go criteria:
- Any security, traceability, or release-control gate is FAIL.
- Any unresolved `high` severity risk remains open.
- Required artifact links are missing or stale.

## 6. Final decision

- Final decision (`GO`/`NO-GO`):
- Decision timestamp (UTC):
- Decision rationale:
- Follow-up actions:
