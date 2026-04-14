# Interoperability Capture Matrix

Release: RC-2026.02.11
Generated At (UTC): 2026-02-11T21:24:49Z
Capture Digest (sha256): `337f96dfc1e83f426ba9eae0371ce2583c850b59fc5fd68979af2898bd6d7704`

## Summary

- Total rows: 6
- PASS: 6
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S11-DW-001 | TGT-001 | IFS-WEB-QIDO-001 | DICOMweb | positive | sha256:1111111111111111111111111111111111111111111111111111111111111111 | sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | PASS | WEB-QIDO-STUDIES | local runtime qido execution |
| IOP-S11-DW-002 | TGT-001 | IFS-WEB-STOW-WADO-002 | DICOMweb | positive | sha256:2222222222222222222222222222222222222222222222222222222222222222 | sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | PASS | WEB-STOW-WADO | local runtime round-trip |
| IOP-S11-DW-003 | TGT-004 | IFS-WEB-QIDO-PEER-001 | DICOMweb | positive | sha256:3333333333333333333333333333333333333333333333333333333333333333 | sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | PASS | ORTHANC-QIDO-PEER | peer profile replay qido |
| IOP-S11-DW-004 | TGT-004 | IFS-WEB-STOW-WADO-PEER-002 | DICOMweb | positive | sha256:4444444444444444444444444444444444444444444444444444444444444444 | sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd | PASS | ORTHANC-STOW-WADO-PEER | peer profile replay round-trip |
| IOP-S11-DW-005 | TGT-005 | IFS-WEB-QIDO-PEER-003 | DICOMweb | positive | sha256:5555555555555555555555555555555555555555555555555555555555555555 | sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | PASS | DCM4CHEE-QIDO-PEER | enterprise peer profile replay qido |
| IOP-S11-DW-006 | TGT-005 | IFS-WEB-STOW-WADO-PEER-004 | DICOMweb | positive | sha256:6666666666666666666666666666666666666666666666666666666666666666 | sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff | PASS | DCM4CHEE-STOW-WADO-PEER | enterprise peer profile replay round-trip |
