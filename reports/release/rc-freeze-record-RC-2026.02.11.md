# RC Freeze Record

Release Candidate: RC-2026.02.11
Date: 2026-02-11
Status: Complete (Signed)
Owner: Release Manager

## Freeze Inputs

- Conformance envelope source: `docs/03-DICOM-Conformance-Envelope.md`
- Release/versioning source: `docs/14-Release-and-Versioning.md`
- Release notes: `reports/release/release-notes-RC-2026.02.11.md`

## Envelope Lock

- Frozen `envelope_version`: `1.0`
- Freeze decision: no envelope bump required for governance-only Sprint 15 artifacts.

## Signed Evidence Reference Lock

| Domain | Frozen evidence reference |
|---|---|
| Interoperability | `reports/interoperability/release-evidence-index.md` |
| Security | `reports/security/security-gate-decision-RC-2026.02.11.md` |
| Performance | `reports/performance/performance-scalability-gate-decision-RC-2026.02.11.md` |
| Traceability | `reports/traceability/traceability-gate-decision-RC-2026.02.11.md` |
| Governance | `reports/release/rc-governance-gate-decision-RC-2026.02.11.md` |

## Hash Lock Inputs

- Artifact integrity report: `reports/release/release-artifact-integrity-RC-2026.02.11.md`
- Artifact integrity register JSON: `reports/release/release-artifact-integrity-RC-2026.02.11.json`

Frozen digest anchors (sha256):

| Artifact | sha256 |
|---|---|
| `reports/release/release-notes-RC-2026.02.11.md` | `39360f4df76337a7c6860e7571cad6ce653602be765a02e8ca3b5f2a3e890da4` |
| `reports/security/sbom-RC-2026.02.11.json` | `b926e55b22f4f73d1d7f7cf6e19d59f39e0cdfa728b6fadb97eb476919724956` |
| `reports/security/security-gate-decision-RC-2026.02.11.md` | `e3889b13fb5b7e867becdba7d3f2bc45d3c9b22520c821e9cfab61b16972f598` |
| `reports/performance/performance-scalability-gate-decision-RC-2026.02.11.md` | `6f224a95e8f4e39615662cb00a5155c5ac5cbe8b2023f9a3081f03ff3379cbe0` |
| `reports/interoperability/release-evidence-index.md` | `cb70f1619d5399d304bf109bc7ee0cacefdc782511be6305fb8e87926718ab1d` |
| `reports/traceability/release-evidence-trace-index-RC-2026.02.11.md` | `3bf6130b79aa73029b8f5e601a08cb791e6fc46b22f8f916f47f93e6b3dcebe5` |

## Decision

- RC freeze package is locked for governance sign-off.
- Any post-freeze change requires reopening GO/NO-GO board minutes and issuing a superseding signed decision artifact.

## Sign-off

- Release Manager: Signed
- Traceability Lead: Signed
- Security Lead: Signed
- Signature ID: SIG-RCFREEZE-RC-20260211-S15
- Signed At (UTC): 2026-02-11T23:49:00Z
