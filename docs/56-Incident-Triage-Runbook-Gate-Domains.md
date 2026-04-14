# Incident Triage Runbook by Gate Domain

Status: **Implemented** (As of 2026-02-24)  
Reference: `docs/31-Implementation-Status.md#major-subsystems`

## 1. Scope

This runbook defines production incident triage for release-governance and operational gates:

- claim-surface
- traceability
- determinism
- security
- packaging

Use this runbook whenever a gate fails in CI, pre-release checks, or post-deployment verification.

## 2. Global triage flow

1. Capture incident metadata:
   - release identifier
   - failing workflow/job URL
   - failing gate domain
   - first failing commit and actor
2. Open an incident ticket with severity:
   - `SEV-1`: production outage or active security exposure
   - `SEV-2`: release blocked with no active outage
   - `SEV-3`: non-blocking drift or warning-only signal
3. Freeze related deployments until gate status is resolved.
4. Execute domain-specific playbook below.
5. Record evidence artifacts under `reports/release/` and `reports/docs/`.

## 3. Domain playbooks

### 3.1 Claim-surface gate

Trigger:
- claim-surface lint failure or prohibited regulatory/diagnostic term outside `docs/15`.

Primary artifacts:
- `python3 tools/claim_surface_lint.py` output
- changed docs/UI strings in the failing diff

Triage steps:
1. Confirm offending phrase and exact file location.
2. Classify as:
   - unauthorized claim outside `docs/15`,
   - false positive requiring rule tuning.
3. Remediate by:
   - removing/rewriting claim text, or
   - moving standards/regulatory claim context into `docs/15` only.
4. Re-run claim-surface lint and attach clean output.

Escalation:
- compliance owner for policy exceptions,
- release manager for go/no-go decision.

### 3.2 Traceability gate

Trigger:
- missing REQ index mapping,
- missing REQ references in automated tests,
- docs/tests mismatch for normative requirements.

Primary artifacts:
- traceability gate logs
- `docs/16-Requirements-Index.md`
- relevant tests in `crates/**/tests` and `tools/tests`

Triage steps:
1. Identify missing link:
   - REQ present in docs but absent from tests, or
   - test references unknown REQ.
2. Patch smallest diff to restore one-to-one mapping.
3. Confirm index and test references are synchronized.
4. Archive updated traceability evidence in release artifacts.

Escalation:
- architecture owner for new REQ taxonomy changes.

### 3.3 Determinism gate

Trigger:
- reproducibility matrix mismatch,
- cross-target drift (native vs WASM) beyond documented exceptions,
- golden corpus output hash mismatch.

Primary artifacts:
- determinism/reproducibility workflow outputs
- corpus hash manifests and failure diffs

Triage steps:
1. Determine scope:
   - single fixture drift,
   - broad algorithmic drift.
2. Validate whether change is expected and documented.
3. If unintended:
   - revert unstable change or
   - fix canonical CPU-path logic per `docs/05`.
4. If intended:
   - update docs and corpus artifacts in the same change set.
5. Re-run deterministic checks and archive outputs.

Escalation:
- pixel pipeline owner,
- release manager if envelope-impacting.

### 3.4 Security gate

Trigger:
- startup fail-closed violations,
- hostile-input limits failures,
- dependency review gate failures,
- telemetry/privacy redaction regressions.

Primary artifacts:
- security workflow logs
- `docs/09-Security-Threat-Model.md`
- limits/docs sync outputs

Triage steps:
1. Classify incident:
   - auth/transport regression,
   - input limit regression,
   - dependency-risk regression,
   - redaction/privacy regression.
2. Contain exposure:
   - block release,
   - disable affected profile if required.
3. Patch with fail-closed defaults preserved.
4. Update threat model and tests when behavior or limits changed.
5. Re-run security gates and attach evidence.

Escalation:
- security owner immediately for `SEV-1`,
- release manager for release hold/restart.

### 3.5 Packaging gate

Trigger:
- release artifact integrity mismatch,
- profile packaging contract failures,
- missing capability manifest or signed evidence mismatch.

Primary artifacts:
- `reports/release/release-artifact-integrity-<release-id>.json`
- `reports/release/release-preflight-summary-<release-id>.json`
- profile archives under `dist/profiles/`

Triage steps:
1. Validate artifact set completeness for target release.
2. Confirm single `release_id` consistency across all generated artifacts.
3. Repair packaging script/profile metadata mismatch.
4. Re-run profile checks and evidence signing verification.
5. Attach corrected artifact manifest to incident ticket.

Escalation:
- release engineering owner,
- packaging/profile owner.

## 4. Exit criteria

Incident is closed only when all are true:

- failing gate returns passing state on re-run,
- remediation commit is linked in incident ticket,
- artifact evidence is attached and versioned,
- release manager records final disposition (`proceed`, `hold`, or `rollback`).
