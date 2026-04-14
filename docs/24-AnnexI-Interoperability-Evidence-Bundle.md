# Annex-I Interoperability Evidence Bundle (R-15)

Date: 2026-02-11  
Status: Complete (RC-2026.02.11 signed baseline)  
Owner: DICOM Interop Lead + V&V Lead

## Purpose

Define the controlled evidence bundle that substantiates interoperability claims with:

- declared claim envelope,
- conformance disclosure,
- objective verification across target environments,
- negative-path fail-closed evidence.

This document is the execution artifact for `R-15` in `./MASTER_CONSOLIDATED_CHECKLIST.md`.

## Bundle Components

| Component ID | Component | Description | Artifact Location | Status |
|---|---|---|---|---|
| IOP-001 | Claim Envelope | Declared interoperability scope (SOP classes, transfer syntaxes, DIMSE/DICOMweb surfaces, policy limits) | `docs/03-DICOM-Conformance-Envelope.md` | Active |
| IOP-002 | Conformance Disclosure | Versioned conformance statement content and release linkage | `docs/03-DICOM-Conformance-Envelope.md`, `docs/14-Release-and-Versioning.md` | Active |
| IOP-003 | Environment Matrix | Supported deployment and integration environments | `reports/interoperability/environment-matrix.md` | Complete (Signed) |
| IOP-004 | Verification Matrix | Positive-path objective verification coverage | `reports/interoperability/verification-matrix.md` | Complete (Signed) |
| IOP-005 | Negative-Path Matrix | Out-of-envelope and malformed-input fail-closed verification | `reports/interoperability/negative-path-matrix.md` | Complete (Signed) |
| IOP-006 | Release Evidence Index | Release-candidate evidence pointers and approvals | `reports/interoperability/release-evidence-index.md` | Complete (Signed) |

## Interoperability Matrix Schema

Required columns for `IOP-004` and `IOP-005`:

1. Interface Surface ID
2. Use Context
3. Input Profile
4. Expected Behavior
5. Required Limits/Policies
6. Test Artifact ID
7. Result
8. Reproducibility Hash
9. Reviewer Sign-off

## Procedure

### Step 1. Freeze Claim Envelope

- freeze release candidate envelope version in `docs/03`,
- confirm release record in `docs/14`,
- enumerate included DIMSE and DICOMweb surfaces for the candidate.

### Step 2. Populate Verification Matrices

- fill positive-path matrix (`IOP-004`) with in-envelope tests,
- fill negative-path matrix (`IOP-005`) with fail-closed tests,
- include deterministic hashes or reproducibility pointers where available.

### Step 3. Run Release Evidence Gate

- confirm no missing rows for declared interfaces,
- confirm each row has linked artifact/test evidence,
- lock `IOP-006` with approver names and dates.

## R-15 Completion Criteria

R-15 is complete for a release when:

- all `IOP-00x` components are active or complete signed,
- every declared interface in `docs/03` appears in verification matrices,
- negative-path coverage exists for each declared interface family,
- release evidence index is signed by interoperability and V&V owners.

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Baseline R-15 artifact created | DICOM Interop Lead |
| 2026-02-11 | v0.2 | Operationalized bundle procedure and seeded report artifacts | DICOM Interop Lead |
| 2026-02-11 | v0.3 | Populated and signed interoperability matrices for RC-2026.02.11 | DICOM Interop Lead |
