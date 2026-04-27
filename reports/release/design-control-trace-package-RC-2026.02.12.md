# Design-Control Trace Package

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Systems Engineering Lead + QA/RA Lead

## Purpose

Provide a release-scoped design-control trace package linking requirements, architecture boundaries, verification evidence, and risk controls for productization transition.

## Scope

- requirements baseline from `docs/01`, `docs/03`, `docs/05`, `docs/09`, `docs/14`,
- architecture boundary mapping from `docs/02-Architecture.md`,
- verification artifact linkage (tests + signed reports),
- risk linkage to `reports/security/risk-file-baseline-RC-2026.02.12.md`.

## Trace Matrix (Requirement -> Architecture -> Verification -> Risk)

| Design Input ID | Requirement IDs | Architecture / Service Boundary | Verification Evidence | Linked Risk IDs | Status |
|---|---|---|---|---|---|
| DCI-001 | `REQ-SCOPE-001`, `REQ-SCOPE-003`, `REQ-SCOPE-004` | `diccy` public facade + workstation profile boundaries (`docs/02`) | `docs/22-Claim-to-Evidence-Matrix.md`, `reports/clinical/CLI-042.md`, `reports/release/product-intent-addendum-RC-2026.02.12.md` | RSK-S07-001, RSK-S07-006 | Verified |
| DCI-002 | `REQ-CONF-002` | `dicom-io` admission + conformance envelope gating | `docs/03-DICOM-Conformance-Envelope.md`, `reports/security/conformance-negative-suite-RC-2026.02.12.md` | RSK-S07-002 | Verified |
| DCI-003 | `REQ-PIX-201` | CPU-oracle pixel boundary (`dicom-pixel`) | `docs/05-Pixel-Pipeline.md`, `reports/analytical/ANL-058.md` | RSK-S07-003 | Verified |
| DCI-004 | `REQ-SEC-405`, `REQ-SEC-420`, `REQ-SEC-423` | parser/decoder/security limits boundary (`dicom-core`, `dicom-io`, `dicom-pixel`) | `docs/09-Security-Threat-Model.md`, `reports/security/fuzz-campaign-report-RC-2026.02.11.md`, `reports/security/fuzz-wave1-RC-2026.02.12.md` | RSK-S07-002, RSK-S07-004 | Verified |
| DCI-005 | `REQ-AUTH-300`, `REQ-AUDIT-350` | authn/authz + audit boundaries (`dicom-auth`, `dicom-audit`, runtime servers) | `reports/security/authz-audit-regression-report-RC-2026.02.11.md`, `reports/analytical/ANL-040.md` | RSK-S07-004 | Verified |
| DCI-006 | `REQ-WEB-300`, `REQ-DIMSE-300`, `REQ-WL-301`, `REQ-MPPS-352` | external interoperability service boundaries (`dicom-web`, DIMSE, workflow) | `reports/interoperability/release-evidence-index.md`, `reports/interoperability/verification-matrix.md`, `reports/interoperability/negative-path-matrix.md` | RSK-S07-005 | Verified |
| DCI-007 | `REQ-REL-601`, `REQ-REL-602` | release controls and evidence governance boundary | `docs/14-Release-and-Versioning.md`, `reports/release/release-notes-RC-2026.02.11.md`, `reports/release/evidence-signature-RC-2026.02.12.json` | RSK-S07-007 | Verified |
| DCI-008 | `REQ-TEST-701` | traceability boundary across docs/code/test/report | `reports/traceability/traceability-snapshot-RC-2026.02.12.md`, `reports/analytical/gates/s06-traceability-report-RC-2026.02.12.log` | RSK-S07-008 | Verified |

## Design Output Inventory

| Design Output ID | Artifact | Decision |
|---|---|---|
| DCO-001 | `reports/release/product-intent-addendum-RC-2026.02.12.md` | Product claim scope separated from framework baseline |
| DCO-002 | `reports/security/risk-file-baseline-RC-2026.02.12.md` | Risk baseline defined with verified controls and residual-risk decisions |
| DCO-003 | `reports/security/cybersecurity-submission-bundle-RC-2026.02.12.md` | Cybersecurity package assembled with threat/SBOM/pen-test/fuzz evidence |
| DCO-004 | `reports/clinical/usability-summative-bundle-RC-2026.02.12.md` | Summative usability and residual-use-risk rationale published |
| DCO-005 | `reports/analytical/ANL-S07-CLOSE.md` | Sprint 09 closure gate signed |

## Review Outcome

- Requirement-to-architecture linkage completeness: PASS
- Verification linkage completeness: PASS
- Risk linkage completeness: PASS
- Open high-severity unlinked requirement rows: 0

## Sign-off

- Systems Engineering Lead: Signed
- QA/RA Lead: Signed
- Regulatory/Quality Lead: Signed
- Decision: Design-control trace package approved for RC-2026.02.12
- Signature ID: SIG-DCTRL-TRACE-20260212-S07
- Signed At (UTC): 2026-02-12T11:14:00Z
