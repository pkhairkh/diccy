# Security Fuzz Campaign Report

Run Label: security-fuzz-RC-2026.02.11
Generated At (UTC): 2026-02-11T21:45:22Z
Report Digest (sha256): `d2de582aaf3ad1f2c2787f310e8be08026851eb62ea5ad7459ce388f136a2986`

## Summary

- Targets: 7
- Crash findings: 0
- Coverage warnings: 7

## Target Results

| Target | Return Code | Seeds | Runs | RSS (MB) | Crash | Coverage Warning | Log |
|---|---:|---:|---:|---:|---|---|---|
| dicom_io_p10 | 0 | N/A | 128 | 25 | NO | YES | `./reports/security/fuzz/logs/dicom_io_p10.log` |
| dimse_pdu | 0 | 2 | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dimse_pdu.log` |
| dimse_command | 0 | 2 | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dimse_command.log` |
| dicomweb_request | 0 | N/A | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dicomweb_request.log` |
| dicom_pixel_pipeline | 0 | N/A | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dicom_pixel_pipeline.log` |
| dicom_pixel_rle | 0 | N/A | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dicom_pixel_rle.log` |
| dicom_pixel_jpeg_baseline | 0 | 8 | 128 | 26 | NO | YES | `./reports/security/fuzz/logs/dicom_pixel_jpeg_baseline.log` |
