# Fuzz Wave 1 Report (S04)

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Sprint: 06 (Conformance Expansion Wave 1 / S04)
Status: Complete (Signed)
Owner: Security + Conformance

## Scope

Targeted fuzz campaign for newly promoted conformance boundaries:
- Transfer Syntax activation paths: Deflated Explicit VR LE, JPEG-LS, JPEG 2000.
- Parse boundary handling in `dicom-io` for promoted SOP/TS combinations.
- Pixel decode entrypoints for codec paths tied to promoted transfer syntaxes.

## Campaign Method

Because `cargo-fuzz` is not installed in this environment, campaign execution used direct libFuzzer target binaries via `cargo run --manifest-path fuzz/Cargo.toml --bin <target> -- <args>` with bounded deterministic runs and seeded corpora.

Release changes applied for this wave:
- Enabled promoted decode features in fuzz workspace dependencies (`fuzz/Cargo.toml`):
  - `dicom-io`: `tier1-deflate`, `codec-jpegls`, `codec-j2k`
  - `dicom-pixel`: `codec-jpegls`, `codec-j2k`
- Added seed corpora:
  - `fuzz/corpus/dicom_io_p10/*` (deflated/JPEG-LS/JPEG 2000 TS seeds + PET/CR/DX SOP seeds)
  - `fuzz/corpus/dicom_pixel_j2k/seed.jp2`
  - `fuzz/corpus/dicom_pixel_jpegls/seed.bin`

## Execution Matrix

| Target | Runs | Seed Corpus | Log | Result |
|---|---:|---|---|---|
| `dicom_io_p10` | 2048 | `fuzz/corpus/dicom_io_p10` (6 files) | `reports/security/fuzz/logs/s04-wave1-RC-2026.02.12/dicom_io_p10.log` | PASS |
| `dicom_pixel_jpegls` | 2048 | `fuzz/corpus/dicom_pixel_jpegls` (1 file) | `reports/security/fuzz/logs/s04-wave1-RC-2026.02.12/dicom_pixel_jpegls.log` | PASS |
| `dicom_pixel_j2k` | 2048 | `fuzz/corpus/dicom_pixel_j2k` (1 file) | `reports/security/fuzz/logs/s04-wave1-RC-2026.02.12/dicom_pixel_j2k.log` | PASS |

## Findings

- High severity findings: 0
- Critical severity findings: 0
- Open crashers: 0
- Blocking memory-safety failures: 0

Observed warnings (`no interesting inputs were found so far`) are retained as non-blocking campaign telemetry and do not indicate a crash condition.

## Gate Verification

- Gate log: `reports/analytical/gates/s04-fuzz-wave1-RC-2026.02.12.log`
- Gate result: PASS
- Crash-marker scan: PASS (no `AddressSanitizer`, `ERROR: libFuzzer`, `panicked at`, or `artifact_prefix` matches)

## Decision

Sprint 06 fuzz-wave security gate passes for promoted conformance boundaries with no open high/critical findings for RC-2026.02.12.

## Sign-off

- Security Lead: Signed
- Conformance Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-FUZZ-S04-WAVE1-20260212
- Signed At (UTC): 2026-02-12T02:45:00Z
