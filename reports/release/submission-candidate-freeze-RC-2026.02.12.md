# Submission Candidate Freeze Record

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Release Manager

## Freeze Inputs

- `reports/interoperability/release-evidence-index.md`
- `reports/traceability/release-evidence-trace-index-RC-2026.02.12.md`
- `reports/traceability/release-traceability-manifest-RC-2026.02.12.json`
- `reports/traceability/traceability-snapshot-RC-2026.02.12.md`
- `reports/release/mock-audit-report-RC-2026.02.12.md`
- `reports/release/external-readiness-review-RC-2026.02.12.md`
- `reports/security/runtime-secure-defaults-final-RC-2026.02.12.md`
- `reports/performance/durability-continuity-final-RC-2026.02.12.md`

## Hash Lock and Signature Bundle

Bundle: `reports/release/submission-candidate-signature-RC-2026.02.12.json`

- `release_id`: `RC-2026.02.12`
- `key_id`: `rc-s08-key-01`
- `signature_algorithm`: `hmac-sha256`
- `signature`: `f094e45cfab1a046afbaa575dc527637f7bde536c9db196bad8b87d9a16e03c0`
- `artifact_count`: `14`

### Locked Artifact Digests (sha256)

| Artifact | sha256 |
|---|---|
| `docs/03-DICOM-Conformance-Envelope.md` | `06576a2720da6630aaeb6b7ba11f6e4ced66e8e6d424a94eb93aa2e1ce837c81` |
| `docs/14-Release-and-Versioning.md` | `8e406402091494782c4512391fb22e2eb8be48c3e7a276e3a2eb7304e713b343` |
| `docs/15-Regulatory-and-Standards-Mapping.md` | `4d56b4e98a240bc378f0c4841c8f419ccb765aa7b0de6e4a6f0df2aab2d1e4e4` |
| `docs/22-Claim-to-Evidence-Matrix.md` | `923eef428e5a4b8fe90cdb61cd2ac1934425de3cdfbd9159a8fb93d03ea1b275` |
| `docs/26-Scientific-Grade-Quantification-Validation-Program.md` | `dce8f8a14858cb8ce4348a1f54ffbd5e1b1b44fb16581504b0f329224c61c77e` |
| `reports/interoperability/release-evidence-index.md` | `7c76113448d2378df998d1868248972502ac20b71f9346b901b99a5902f02555` |
| `reports/performance/durability-continuity-final-RC-2026.02.12.md` | `8f04d9a6d353a6ab2a1a69c7f2f7e22d34f78dba427c0b0d0a4fa884e2363f64` |
| `reports/release/external-readiness-review-RC-2026.02.12.md` | `4e140d92955394a82b2680231aafaae3cc36ec38751bc7ecf4c9692f958c8dbd` |
| `reports/release/mock-audit-report-RC-2026.02.12.md` | `82f48787fe54d220a4629da24d94d86ac9879bdf5172fd693c7a87a8c23c02f6` |
| `reports/security/runtime-secure-defaults-final-RC-2026.02.12.md` | `17303c39026a34110e27d2f15a667fc0e4cd012ce49ab424c9d09e814db11e63` |
| `reports/traceability/release-evidence-trace-index-RC-2026.02.12.md` | `47b0f3b477b43d808cf6c43888f37926c977238e5133847254c029014cce124a` |
| `reports/traceability/release-traceability-manifest-RC-2026.02.12.json` | `1e71d9935f04f04ffacb5c136a54747dc2c2fc21a827e9352419de204b838a23` |
| `reports/traceability/traceability-snapshot-RC-2026.02.12.json` | `fc328c3a8bf2466cd6993a7afdd8a08feade59f09a21894af55233d0f9a75664` |
| `reports/traceability/traceability-snapshot-RC-2026.02.12.md` | `d4842fa930cc88b81974d521b6b5312869a9335b4d5326da78ae0da4e8ede581` |

## Signature Verification

- Sign/verify log: `reports/analytical/gates/s08-evidence-sign-verify-RC-2026.02.12.log`
- Verification result: PASS

## Freeze Decision

Submission-candidate evidence is frozen and hash-locked for RC-2026.02.12. Any post-freeze artifact mutation requires a superseding freeze artifact and GO/NO-GO re-approval.

## Sign-off

- Release Manager: Signed
- Security Lead: Signed
- Traceability Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-SUBMIT-FREEZE-S08-20260212
- Signed At (UTC): 2026-02-12T13:29:00Z
