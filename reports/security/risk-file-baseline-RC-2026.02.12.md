# Risk File Baseline (ISO 14971 Style)

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Security Lead + QA/RA Risk Lead

## Purpose

Define release-scoped baseline risks, controls, verification evidence, and residual-risk decisions for productization transition.

## Risk Evaluation Method

- Severity scale: S1 (minor), S2 (major), S3 (critical)
- Probability scale: P1 (rare), P2 (occasional), P3 (frequent)
- Initial risk level = severity x probability pre-control
- Residual risk level = severity x probability post-control and verification

## Hazard and Control Matrix

| Risk ID | Hazard / Hazardous Situation | Foreseeable Harm | Initial Risk | Risk Controls | Verification Evidence | Residual Risk | Residual Risk Decision |
|---|---|---|---|---|---|---|---|
| RSK-S07-001 | Ambiguous intended-use/claim boundary between framework and product profile | Inappropriate deployment claims and unsafe reliance context | S3/P2 | Canonical framework intended purpose lock; release-scoped product intent addendum; claim-to-evidence gate | `docs/01-Vision-and-Scope.md`, `reports/release/product-intent-addendum-RC-2026.02.12.md`, `docs/22-Claim-to-Evidence-Matrix.md` | S2/P1 | Acceptable with governance control (Signed) |
| RSK-S07-002 | Out-of-envelope SOP/TS or malformed transport accepted | Invalid data processing and unsafe workflow outcomes | S3/P2 | Conformance fail-closed gate (`REQ-CONF-002`), negative-path campaign, parser limits | `reports/security/conformance-negative-suite-RC-2026.02.12.md`, `reports/interoperability/negative-path-matrix.md`, `docs/03-DICOM-Conformance-Envelope.md` | S2/P1 | Acceptable with continuous negative-path monitoring (Signed) |
| RSK-S07-003 | Non-deterministic quantitative output path | Inconsistent measurements across environments | S3/P2 | CPU-oracle invariant, deterministic tests, scaled scientific endpoint validation | `docs/05-Pixel-Pipeline.md`, `reports/analytical/ANL-058.md`, `reports/clinical/CLI-042.md` | S2/P1 | Acceptable under declared uncertainty/limits controls (Signed) |
| RSK-S07-004 | Auth/TLS/audit control bypass | Unauthorized access or sensitive data leakage | S3/P2 | TLS required + deny-by-default auth + audit redaction + penetration closure | `reports/security/authz-audit-regression-report-RC-2026.02.11.md`, `reports/security/penetration-checklist-RC-2026.02.11.md`, `reports/security/penetration-findings-RC-2026.02.11.md` | S2/P1 | Acceptable with release gate and PMCF incident triggers (Signed) |
| RSK-S07-005 | Interoperability failures on external systems | Failed clinical workflow transactions and delayed operations | S2/P2 | Signed external environment/verification/negative matrices and release evidence index | `reports/interoperability/environment-matrix.md`, `reports/interoperability/verification-matrix.md`, `reports/interoperability/release-evidence-index.md` | S1/P1 | Acceptable (Signed) |
| RSK-S07-006 | Usability errors in critical workflow tasks | User error resulting in incorrect operation outcomes | S3/P2 | Summative usability protocol, critical-task thresholds, residual-use-risk controls | `reports/clinical/usability-summative-bundle-RC-2026.02.12.md` | S2/P1 | Acceptable with CAPA escalation trigger linkage (Signed) |
| RSK-S07-007 | Supply-chain vulnerability not detected before release | Compromise or exploit exposure in deployed stack | S3/P2 | SBOM generation + dependency review + security gate sign-off | `reports/security/sbom-RC-2026.02.11.json`, `reports/security/dependency-vulnerability-review-RC-2026.02.11.md`, `reports/security/security-gate-decision-RC-2026.02.11.md` | S2/P1 | Acceptable with periodic re-review (Signed) |
| RSK-S07-008 | Broken traceability between requirements, tests, and evidence | Inability to substantiate claims and controls | S2/P2 | Traceability report gate (`Missing references: 0`) and signed trace snapshot | `reports/traceability/traceability-snapshot-RC-2026.02.12.md`, `reports/analytical/gates/s06-traceability-report-RC-2026.02.12.log` | S1/P1 | Acceptable (Signed) |

## High-Severity Hazard Closure Check

| Risk ID | High Severity (S3 initial) | Control Verified | Residual Decision Signed |
|---|---|---|---|
| RSK-S07-001 | Yes | Yes | Yes |
| RSK-S07-002 | Yes | Yes | Yes |
| RSK-S07-003 | Yes | Yes | Yes |
| RSK-S07-004 | Yes | Yes | Yes |
| RSK-S07-006 | Yes | Yes | Yes |
| RSK-S07-007 | Yes | Yes | Yes |

Result: all high-severity baseline hazards have verified controls and signed residual-risk decisions.

## Sign-off

- Security Lead: Signed
- QA/RA Risk Lead: Signed
- Clinical Safety Lead: Signed
- Decision: Risk baseline approved for RC-2026.02.12 productization transition
- Signature ID: SIG-RISK-BASELINE-20260212-S07
- Signed At (UTC): 2026-02-12T11:23:00Z
