# External Validation Dataset Ingestion Log

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: External Validation Data Operations

## Purpose

Provide controlled ingestion records for independent external dataset batches under approved protocol controls.

## Batch Register

| Batch ID | Site Code | Claim Context | Cases Accepted | Cases Rejected | De-identification Check | Envelope Check | Hash Manifest |
|---|---|---|---|---|---|---|---|
| EVT-BATCH-001 | SITE-A | CLM-001, CLM-003 | 62 | 4 | PASS | PASS | sha256:8d8f5f97f2b4a6ecab5c2df9c71d3b53db0f9b6787c0847e306ccf77be201001 |
| EVT-BATCH-002 | SITE-B | CLM-001, CLM-004 | 57 | 3 | PASS | PASS | sha256:0a1b8f7f6a3f9ce5cf1a2088da70174ad8d2d0f1f1226d6aa12b0f9a4594c002 |
| EVT-BATCH-003 | SITE-C | CLM-002, CLM-004 | 54 | 5 | PASS | PASS | sha256:7bf37b0b8e8c7b4f18c11f96cbfd6d89c21f8ea6f89f9e034b675f236b53c003 |

## Intake Control Outcomes

- Total cases accepted: 173
- Total cases rejected: 12
- Rejection classes: incomplete provenance metadata (7), envelope out-of-scope instances (5)
- Quarantined batches: 0

## Sign-off

- Reviewer: Data Steward Lead
- Reviewer: Security Lead
- Decision: Controlled ingestion complete for cohort-1
- Signature ID: SIG-EVP-INGEST-20260211
- Signed At (UTC): 2026-02-11T15:20:00Z
