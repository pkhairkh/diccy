# Interoperability Target System Registry

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed for Sprint 10)
Owner: DICOM Interop Lead

## Purpose

Define the controlled target-system list, version baselines, and integration configuration profiles for interoperability campaign preparation.

## Registry

| Target ID | System | Version Baseline | Role in Campaign | Interface Surfaces | Baseline Configuration Profile | Availability for Sprint 10 Dry Run |
|---|---|---|---|---|---|---|
| TGT-001 | RDVF DICOMweb Runtime (`dicom-web-server`) | Workspace `RC-2026.02.11` build | Reference service | QIDO/WADO/STOW | TLS required, deny-by-default auth, durable WAL storage | Available |
| TGT-002 | RDVF DIMSE Runtime (`dicom-dimse-service`) | Workspace `RC-2026.02.11` build | Reference service | C-ECHO/C-STORE/C-FIND/C-MOVE/C-GET | TLS policy enforced, association throttle, storage-backed query/retrieve | Available |
| TGT-003 | RDVF Workflow Runtime (`dicom-workflow-server`) | Workspace `RC-2026.02.11` build | Reference workflow service | MWL/MPPS | TLS required, deny-by-default auth, durable snapshot persistence | Available |
| TGT-004 | Orthanc Peer Profile | 1.12.x baseline (site-managed) | External peer target | DICOMweb + DIMSE store/query | Site-managed profile with fail-closed unsupported route/method behavior required | Planned (Sprint 11) |
| TGT-005 | dcm4chee-arc Peer Profile | 5.3x baseline (site-managed) | External peer target | DICOMweb + DIMSE + workflow integration touchpoints | Site-managed profile with explicit TLS/auth posture and conformance disclosure required | Planned (Sprint 11) |
| TGT-006 | Modality Gateway Simulation (DCMTK profile) | 3.6.x baseline (site-managed) | External modality-facing target | DIMSE association/store/query | AETitle allowlist, bounded PDV/PDU limits, explicit transfer syntax negotiation | Planned (Sprint 11) |

## Configuration Baseline Rules

- Every target must declare AE Title / endpoint, TLS mode, auth mode, and conformance profile version.
- Any target with missing baseline parameters is excluded from execution set until resolved.
- Sprint 11 execution can include only targets with signed baseline profile records.

## Sign-off

- Reviewer: DICOM Interop Lead
- Reviewer: Security Lead
- Decision: Target registry approved for campaign preparation
- Signature ID: SIG-IOP-TARGETS-20260211
- Signed At (UTC): 2026-02-11T20:10:00Z
