# Interoperability Reproducibility Wave-2 Report

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Interoperability Verification Engineering

## Purpose

Compare independent wave-2 external execution against wave-1 baseline and quantify hash stability variance.

## Inputs

- Wave-1 capture summaries (`reports/interoperability/execution/*-capture-summary-RC-2026.02.12.json`)
- Wave-2 capture summaries (`reports/interoperability/execution/*-capture-summary-wave2-RC-2026.02.12.json`)
- Hash registers (`reports/interoperability/hash-verification-register-RC-2026.02.12.md`, `reports/interoperability/hash-verification-register-wave2-RC-2026.02.12.md`)

## Comparison Policy

- Operation identity key: `(target_system, interface_id, protocol, phase)`
- Stability metric: exact match on `request_hash` and `response_hash` across windows
- Allowed variance budget: `0.0%` (fail-closed)

## Protocol Stability Summary

| Protocol | Compared Operations | Hash Matches | Hash Mismatches | Variance % | Verdict |
|---|---|---|---|---|---|
| DICOMweb | 11 | 11 | 0 | 0.0000 | PASS |
| DIMSE | 9 | 9 | 0 | 0.0000 | PASS |
| MPPS | 4 | 4 | 0 | 0.0000 | PASS |
| MWL | 4 | 4 | 0 | 0.0000 | PASS |

## Wave-2 Extension Cases (not compared to wave-1)

- New operation keys in wave-2: 5
- `DICOMweb` `IFS-WEB-NEG-ROUTE-CANONICALIZATION` on `TGT-004` (negative)
- `DICOMweb` `IFS-AUTH-NEG-POLICY-TRANSITION` on `TGT-005` (negative)
- `DICOMweb` `IFS-WEB-NEG-QUERY-BOUNDARY` on `TGT-005` (negative)
- `DIMSE` `IFS-DIMSE-NEG-PDU-LIMIT` on `TGT-006` (negative)
- `DIMSE` `IFS-DIMSE-NEG-PDV-LIMIT` on `TGT-006` (negative)

## Overall Verdict

- Shared operations compared: 28
- Shared mismatches: 0
- Overall variance: 0.0000%
- Budget: 0.0000%
- Decision: PASS

## Sign-off

- Interop Lead: Signed
- Security Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-IOP-REPRO-WAVE2-20260212
- Signed At (UTC): 2026-02-12T01:52:00Z
