# GO/NO-GO Decision Template (Controlled)

Template ID: GNG-TPL-v1
Status: Controlled Template (Signed)
Owner: Regulatory/Quality Lead
Effective Date: 2026-02-11

## Header

- Release Candidate: `<release-id>`
- Decision Session ID: `<session-id>`
- Decision Date (UTC): `<timestamp>`
- Chair: `<name>`

## Gate Checklist

- [ ] REL-GATE-01 safe command gates passed (`fmt/build/test/clippy`).
- [ ] REL-GATE-02 claim surface + REQ completeness gates passed.
- [ ] REL-GATE-03 traceability missing references = 0 and invalid links = 0.
- [ ] REL-GATE-04 determinism manifest + cross-target compare passed.
- [ ] REL-GATE-05 artifact integrity report passed (SBOM + index + hashes).
- [ ] REL-GATE-06 GO/NO-GO board minutes approved and signed.
- [ ] REL-GATE-07 rollback/hotfix drill evidence approved.
- [ ] REL-GATE-08 RC freeze package locked (`envelope_version`, release notes, signed evidence references).

## Decision Outcome

- [ ] GO
- [ ] NO-GO

Decision rationale:

```
<concise rationale with top residual risks and controls>
```

## Required Evidence Bindings

- Preflight summary: `reports/release/release-preflight-summary-<release-id>.md`
- Artifact integrity report: `reports/release/release-artifact-integrity-<release-id>.md`
- Freeze record: `reports/release/rc-freeze-record-<release-id>.md`
- Governance decision record: `reports/release/rc-governance-gate-decision-<release-id>.md`

## Signatures

- Release Manager: `<signed>`
- Security Lead: `<signed>`
- Traceability Lead: `<signed>`
- Regulatory/Quality Lead: `<signed>`

Archive rule:

- Finalized decision records are stored in `reports/release/` and linked from the release analytical closure report.
