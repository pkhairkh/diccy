# Security Fuzz Target Matrix

Date: 2026-02-11
Release Candidate: RC-2026.02.11
Status: Complete (Signed)
Owner: Security Engineering

## Campaign profile

- Runner: `tools/security_fuzz_campaign.py`
- Mode: offline deterministic campaign
- Runs per target: 128
- Crash gate: fail on crash enabled
- Summary JSON: `reports/security/fuzz-campaign-summary-RC-2026.02.11.json`

## Target matrix

| Target | Surface class | Corpus source | Runs | Result |
|---|---|---|---:|---|
| `dicom_io_p10` | parser | none (empty-start) | 128 | PASS |
| `dimse_pdu` | network | `fuzz/corpus/dimse_pdu` | 128 | PASS |
| `dimse_command` | parser/network | `fuzz/corpus/dimse_command` | 128 | PASS |
| `dicomweb_request` | parser/web | none (empty-start) | 128 | PASS |
| `dicom_pixel_pipeline` | pixel | none (empty-start) | 128 | PASS |
| `dicom_pixel_rle` | pixel codec | none (empty-start) | 128 | PASS |
| `dicom_pixel_jpeg_baseline` | pixel codec | `fuzz/corpus/dicom_pixel_jpeg_baseline` | 128 | PASS |

## Signature

- Security Lead: Signed
- V&V Lead: Signed
- Signature ID: SIG-SEC-FUZZ-MATRIX-20260211
- Signed At (UTC): 2026-02-11T22:04:00Z
