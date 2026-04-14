# External Validation Campaign Tracker Template

Template ID: EVT-TRACKER-001  
Status: Controlled Template (RC-2026.02.11)  
Owner: External Validation Program Office

## Usage

- One row per site + dataset batch + endpoint group.
- Do not overwrite closed rows; append a new row for any correction.
- Deviation disposition fields are mandatory before closure.

## Tracker Table

| Row ID | Site Profile ID | Site Code | Batch ID | Batch Hash | Intake Date (UTC) | Endpoint Group | Blinded Review Required (Y/N) | Blinded Review Status | Deviation Status | Deviation ID | Deviation Disposition Owner | Security Check Status | Analysis Status | Final Row Status | Reviewer Sign-off | Closed At (UTC) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| EVT-ROW-0001 | EVP-SITE-01 | SITE-A | BATCH-0001 | sha256:... | YYYY-MM-DD | EVP-PE-001 | Y | Pending | None | N/A | N/A | Pending | Pending | Open | Pending | N/A |

## Controlled Status Values

- Blinded Review Status: `Pending`, `In Review`, `Complete`, `Waived`.
- Deviation Status: `None`, `Open`, `Resolved`, `Escalated`.
- Security Check Status: `Pending`, `Pass`, `Quarantine`.
- Analysis Status: `Pending`, `In Progress`, `Complete`, `Excluded`.
- Final Row Status: `Open`, `Ready for Closure`, `Closed`.
