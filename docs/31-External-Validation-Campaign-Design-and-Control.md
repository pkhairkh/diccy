# External Validation Campaign Design and Control

Date: 2026-02-11  
Status: Complete (RC-2026.02.11 campaign-design baseline signed)  
Owner: External Validation Program Board (Clinical Evaluation + Biostatistics + Security + QA/RA)

## Purpose

Define the controlled design package for the external validation campaign, including:

- multi-site campaign charter,
- independent dataset policy,
- blinded review and adjudication procedure,
- statistical analysis plan linkage,
- secure data handling controls,
- execution tracker template,
- governance approval record,
- launch-readiness gate decision.

## Scope

Applies to the campaign-design phase (Sprint 08) for release candidate `RC-2026.02.11`.

Execution of site onboarding, data intake, and endpoint computation is handled in Sprint 09.

## 1. Multi-site Campaign Charter (S08-T01)

### 1.1 Campaign objective

Validate reproducibility and workflow performance claims under independent, site-owned operating conditions with controlled protocol execution.

### 1.2 Target site profile model

| Site Profile ID | Site Archetype | Modality Mix Requirement | Case Throughput Requirement | Required Local Roles |
|---|---|---|---|---|
| EVP-SITE-01 | Academic center | CT + MR + Ultrasound present | >= 150 eligible cases/quarter | Site PI, Data Steward, Radiology Workflow Lead |
| EVP-SITE-02 | Regional network hospital | CT + MR present | >= 100 eligible cases/quarter | Site PI, Data Steward |
| EVP-SITE-03 | Community hospital | CT or MR plus one secondary modality | >= 80 eligible cases/quarter | Site PI, Data Steward |

### 1.3 Inclusion criteria

- Site has independent governance and no shared development ownership with this repository.
- Site can provide de-identified datasets under signed data transfer controls.
- Site has named reviewers available for blinded endpoint review where required.
- Site can execute protocol checkpoints and provide signed deviation logs.

### 1.4 Governance roles

| Role | Responsibility | Decision authority |
|---|---|---|
| External Validation Program Director | Campaign schedule, site activation, milestone tracking | Launch package owner |
| Clinical Evaluation Lead | Clinical endpoint relevance and reviewer assignment | Clinical protocol approval |
| Biostatistics Lead | SAP ownership and endpoint analysis integrity | Statistical acceptance approval |
| Security Lead | Data handling controls and transfer policy enforcement | Security release veto |
| QA/RA Lead | Governance record control and release gating | Final launch gate co-approval |
| Site Principal Investigator | Site protocol execution and local oversight | Site cohort go/no-go |

## 2. Independent Dataset Selection Policy (S08-T02)

### 2.1 Selection criteria

- Independence: datasets originate from external institutions not used for internal development tuning.
- Representativeness: datasets cover supported operating contexts mapped to active claim IDs.
- Provenance control: every batch includes site source declaration, collection window, and content hash manifest.
- Cohort balance: each claim/use context has minimum cohort coverage across at least two site profiles.

### 2.2 Claim/use-context dataset mapping

| Claim ID | Use context | Required dataset properties | Minimum cohort rule |
|---|---|---|---|
| CLM-001 | Quantitative reproducibility | Repeat acquisitions and repeated measurement scenarios under supported configurations | >= 2 sites, >= 50 paired observations/site |
| CLM-002 | Interoperability fail-closed behavior | Positive + malformed/unsupported transaction examples from site interfaces | >= 2 sites, >= 40 transaction sequences/site |
| CLM-003 | Measurement provenance continuity | Cases with calibration/provenance metadata present and absent | >= 2 sites, >= 30 cases/site |
| CLM-004 | Workflow output stability | Cases processed on release-defined runtime environments | >= 2 sites, >= 40 workflow runs/site |

### 2.3 Exclusion rules

- Any dataset containing direct patient identifiers after de-identification check.
- Cases outside published conformance envelope where envelope-expansion authorization is absent.
- Datasets with incomplete provenance metadata required by selected endpoint protocol.
- Duplicate cases previously used as internal golden-corpus fixtures for target endpoint.

## 3. Blinded Review and Adjudication SOP (S08-T03)

### 3.1 Applicability

Blinded review is required for qualitative or reviewer-scored endpoints and optional for purely deterministic numeric endpoints.

### 3.2 Procedure

1. Assign two independent reviewers per blinded endpoint group.
2. Provide anonymized case packets with randomized ordering and no site labels.
3. Capture reviewer outcomes in isolated forms without cross-reviewer visibility.
4. Detect disagreements exceeding endpoint-specific tolerance threshold.
5. Route disagreements to adjudicator with full audit trail.
6. Record adjudication outcome and rationale.
7. Lock reviewed outcomes before statistical analysis.

### 3.3 Adjudication workflow

| Trigger | Adjudicator action | Resolution output |
|---|---|---|
| Reviewer disagreement on endpoint class | Review source packet and both reviewer notes | Final endpoint class + rationale ID |
| Missing reviewer rationale | Request completion from originating reviewer | Completed rationale or case exclusion record |
| Protocol deviation affecting review integrity | Escalate to Clinical Evaluation Lead + QA/RA Lead | Deviation disposition (accept/exclude/re-run) |

## 4. Statistical Analysis Plan Linkage (S08-T04)

The signed statistical analysis plan is published at:

- `reports/clinical/CLI-040.md`

This SAP controls:

- primary and secondary endpoint definitions,
- analysis populations,
- missing-data and protocol-deviation handling,
- acceptance boundaries and sign-off.

## 5. Data Security Handling SOP (S08-T05)

### 5.1 De-identification controls

- Apply DICOM de-identification profile before transfer.
- Remove or pseudonymize direct identifiers and institution-local IDs.
- Run pre-transfer validation scan and block transfer when residual identifiers are detected.
- Record de-identification execution log per dataset batch.

### 5.2 Transfer and storage controls

| Control area | Required control |
|---|---|
| Data in transit | Encrypted transfer channel only (TLS-secured API or key-based secure file channel) |
| Data at rest | Encrypted storage volume with controlled access list |
| Access control | Least-privilege role binding with named operator identity |
| Auditability | Immutable transfer + access log with timestamp and operator ID |

### 5.3 Incident and deviation handling

- Any de-identification or transfer control violation triggers immediate batch quarantine.
- Security Lead and QA/RA Lead review is required before any quarantined batch can be released.
- Repeated violations from a site profile trigger temporary site suspension pending corrective action.

## 6. Campaign Execution Tracker Template (S08-T06)

Controlled template file:

- `reports/clinical/external-validation-campaign-tracker-template.md`

Template control rules:

- one row per site + batch + endpoint group,
- immutable batch ID and hash fields,
- explicit deviation status and disposition owner,
- sign-off fields required before row closure.

## 7. Governance Review and Approval Record (S08-T07)

Review package (design phase):

- `docs/31-External-Validation-Campaign-Design-and-Control.md`
- `reports/clinical/CLI-040.md`
- `reports/clinical/external-validation-campaign-tracker-template.md`

Approval record:

| Reviewer role | Decision | Signature ID | Signed At (UTC) |
|---|---|---|---|
| External Validation Program Director | Approved | SIG-EVP-20260211-PROG | 2026-02-11T23:30:00Z |
| Clinical Evaluation Lead | Approved | SIG-EVP-20260211-CLIN | 2026-02-11T23:31:00Z |
| Biostatistics Lead | Approved | SIG-EVP-20260211-STAT | 2026-02-11T23:32:00Z |
| Security Lead | Approved | SIG-EVP-20260211-SEC | 2026-02-11T23:33:00Z |
| QA/RA Lead | Approved | SIG-EVP-20260211-QARA | 2026-02-11T23:34:00Z |

## 8. Campaign Readiness Gate Decision (S08-T08)

Gate checklist:

| Gate item | Status |
|---|---|
| Campaign charter finalized with site profiles and inclusion criteria | PASS |
| Independent dataset policy and exclusion rules finalized | PASS |
| Blinded review and adjudication workflow finalized | PASS |
| Signed SAP published (`CLI-040`) | PASS |
| Data security handling SOP finalized | PASS |
| Controlled tracker template published | PASS |
| Governance approvals captured | PASS |

Decision: **Approved** for Sprint 09 execution launch.

Launch gate signature:

- Signature ID: SIG-EVP-GATE-20260211-RDY
- Signed At (UTC): 2026-02-11T23:35:00Z

## Revision Log

| Date | Revision | Change | Owner |
|---|---|---|---|
| 2026-02-11 | v0.1 | Initial campaign design package with charter/policy/SOP/SAP linkage/tracker/governance/gate | External Validation Program Board |
