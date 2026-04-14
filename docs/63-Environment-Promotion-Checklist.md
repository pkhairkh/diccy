# Environment Promotion Checklist (dev -> stage -> prod)

Status: production promotion control checklist.

## 1. Promotion prerequisites

- [ ] Release ID is assigned and consistent across all artifacts.
- [ ] `docs/14` release checklist items are complete.
- [ ] Conformance envelope and limits docs are updated (if changed).
- [ ] Migration manifest reviewed:
  - `docs/62-State-Schema-and-Migration-Manifest.md`
  - `reports/docs/state-schema-migration-manifest.v1.json`

## 2. Dev -> Stage

- [ ] Startup preflight smoke passes for target profile(s).
- [ ] Security startup fail-closed gate passes.
- [ ] Profile metadata reconciliation gate passes.
- [ ] Runtime state backup manifest generated:
  - `reports/release/state-backup-manifest-<release-id>.json`
- [ ] Restore verification report generated:
  - `reports/release/state-restore-verification-<release-id>.md`

## 3. Stage -> Prod

- [ ] External site readiness checklist completed:
  - `reports/interoperability/external-site-readiness-checklist-<release-id>.md`
- [ ] Tenant onboarding/rotation runbooks approved:
  - `docs/60-Tenant-Aware-Integration-Onboarding-Runbook.md`
  - `docs/61-Connector-Certificate-and-Secret-Rotation-Runbook.md`
- [ ] Snapshot integrity verification report is attached.
- [ ] Promotion sign-off includes deployment, interoperability, and security owners.

## 4. Required gate evidence links

- Release controls gate report.
- Security startup fail-closed gate report.
- Determinism and traceability reports.
- Profile metadata reconciliation report.
- State backup manifest and restore verification report.

## 5. Blockers and rollback triggers

- Any failing gate or missing artifact blocks promotion.
- Production rollout pauses when:
  - tenant isolation checks fail,
  - checksum verification fails for state snapshots,
  - unexpected test-only env vars are present.
- Rollback uses immutable backup manifest and restore drill procedure.

