# External Validation Dataset Ingestion Log

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: External Validation Data Operations

## Purpose

Controlled ingestion register for release-scope external validation cohorts.

## Batch Register

| Batch ID | Site Code | Claim Context | Cases Accepted | Cases Rejected | De-identification Check | Envelope Check | Hash Manifest |
|---|---|---|---|---|---|---|---|
| EVT-BATCH-101 | SITE-D | CLM-001, CLM-003 | 68 | 3 | PASS | PASS | sha256:28f17217cd27f3b80849dbf2bb16acb813f9ce2f6f3e08ba7fdb2c16ea2c2d6a |
| EVT-BATCH-102 | SITE-E | CLM-001, CLM-004 | 61 | 4 | PASS | PASS | sha256:c09d326f7e73cd6e63f74d4e0e229ba4ad8c019f82f5988e4dfd4e9ef0b5a16c |
| EVT-BATCH-103 | SITE-F | CLM-002, CLM-004 | 59 | 2 | PASS | PASS | sha256:9a8e0af75e16196708e7e74eba8fac79f97585de575f3fc7bd83afeb0aa1cb95 |

## Intake Control Outcomes

- Total cases accepted: 188
- Total cases rejected: 9
- Rejection classes:
  - incomplete provenance metadata (5)
  - envelope out-of-scope instances (3)
  - duplicate artifact hash registration (1)
- Quarantined batches: 0

## Controlled Execution Notes

- All batches ingested via controlled import pipeline run ID `EVP-INGEST-RC-2026.02.12-01`.
- Artifact hashes validated against provided manifests before acceptance.
- No template/example rows used in this release-scope ingestion register.

## Sign-off

- Data Steward Lead: Signed
- Security Lead: Signed
- Clinical Evaluation Lead: Signed
- Decision: Controlled ingestion complete for RC-2026.02.12 cohort
- Signature ID: SIG-EVP-INGEST-20260212
- Signed At (UTC): 2026-02-12T00:29:30Z
