# Product Intended-Use and Claims Addendum

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Regulatory/Quality Lead + Product Safety Lead

## Purpose

Define a release-controlled product intended-use and claims package that is explicitly separated from the framework baseline intended purpose.

## Controlled Boundary Statement

Framework baseline (source-controlled, repository-wide):

- The canonical framework intended purpose remains the verbatim statement in:
  - `README.md`
  - `docs/01-Vision-and-Scope.md`

Product profile addendum (this document):

- This addendum defines a deployment-specific claim package for RC-2026.02.12 governance and evidence review.
- This addendum does not alter framework scope requirements or conformance envelope controls.

## Product Claim Package (RC-2026.02.12)

| Product Claim ID | Controlled Wording | Required Evidence Bundle | Claim Status |
|---|---|---|---|
| PCLM-001 | Quantitative outputs are released only for in-envelope configurations with declared uncertainty and limits-of-use controls. | `docs/22-Claim-to-Evidence-Matrix.md`, `docs/26-Scientific-Grade-Quantification-Validation-Program.md`, `reports/analytical/ANL-058.md`, `reports/clinical/CLI-042.md` | Active (Signed) |
| PCLM-002 | Interoperability transactions are released only for signed external-target matrices and fail-closed negative-path controls. | `reports/interoperability/release-evidence-index.md`, `reports/interoperability/verification-matrix.md`, `reports/interoperability/negative-path-matrix.md` | Active (Signed) |
| PCLM-003 | Security posture is released only with secure runtime defaults, signed SBOM review, and closed penetration/fuzz findings. | `reports/security/security-gate-decision-RC-2026.02.11.md`, `reports/security/dependency-vulnerability-review-RC-2026.02.11.md`, `reports/security/penetration-findings-RC-2026.02.11.md` | Active (Signed) |
| PCLM-004 | Workflow/storage reliability is released only with durable persistence continuity evidence and PMCF/CAPA escalation controls. | `reports/analytical/ANL-053.md`, `reports/pmcf/PMCF-040.md`, `reports/pmcf/capa-threshold-policy-RC-2026.02.11.md` | Active (Signed) |

## Explicit Exclusions (Product Profile)

The following remain out-of-scope for the RC-2026.02.12 product profile and require separate gated authorization before activation:

- any autonomous diagnosis or therapeutic recommendation logic,
- any out-of-envelope SOP/Transfer Syntax activation,
- any security policy relaxation that bypasses TLS-required and deny-by-default controls,
- any claim lacking linked analytical + clinical + PMCF evidence.

## Ambiguity Control Checklist

- [x] Framework intended purpose remains unchanged and canonical in `README.md` + `docs/01-Vision-and-Scope.md`.
- [x] Product claims are release-scoped and evidence-bound in this addendum.
- [x] Out-of-scope conditions are explicit.
- [x] Claim bundle references signed artifacts only.

## Sign-off

- Regulatory/Quality Lead: Signed
- Product Safety Lead: Signed
- Clinical Evaluation Lead: Signed
- Decision: Approved product intended-use/claims addendum for RC-2026.02.12
- Signature ID: SIG-PROD-INTENT-20260212-S07
- Signed At (UTC): 2026-02-12T11:05:00Z
