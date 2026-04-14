# Interoperability Capture Matrix

Release: RC-2026.02.11
Generated At (UTC): 2026-02-11T21:19:32Z
Capture Digest (sha256): `84e07085a86209b428c35a5967c8f671bf784903ecf9a904d8104558b9dd76dc`

## Summary

- Total rows: 12
- PASS: 12
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-TC-001 | TGT-001 | IFS-WEB-QIDO-001 | DICOMweb | positive | sha256:1111111111111111111111111111111111111111111111111111111111111111 | sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | PASS | WEB-QIDO-STUDIES | qido query baseline |
| IOP-TC-002 | TGT-001 | IFS-WEB-STOW-WADO-002 | DICOMweb | positive | sha256:2222222222222222222222222222222222222222222222222222222222222222 | sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | PASS | WEB-STOW-WADO | round trip baseline |
| IOP-TC-003 | TGT-001 | IFS-WEB-NEG-002 | DICOMweb | negative | sha256:3333333333333333333333333333333333333333333333333333333333333333 | sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | PASS | WEB-UNSUPPORTED-METHOD | fail-closed method reject |
| IOP-TC-004 | TGT-001 | IFS-WEB-NEG-001 | DICOMweb | negative | sha256:4444444444444444444444444444444444444444444444444444444444444444 | sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd | PASS | WEB-INVALID-UID | fail-closed uid reject |
| IOP-TC-005 | TGT-002 | IFS-DIMSE-ECHO-003 | DIMSE | positive | sha256:5555555555555555555555555555555555555555555555555555555555555555 | sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | PASS | DIMSE-C-ECHO | authorized echo |
| IOP-TC-006 | TGT-002 | IFS-DIMSE-STORE-004 | DIMSE | positive | sha256:6666666666666666666666666666666666666666666666666666666666666666 | sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff | PASS | DIMSE-C-STORE | store/query verify |
| IOP-TC-007 | TGT-002 | IFS-DIMSE-NEG-004 | DIMSE | negative | sha256:7777777777777777777777777777777777777777777777777777777777777777 | sha256:abababababababababababababababababababababababababababababababab | PASS | DIMSE-BAD-COMMAND | unsupported command fail-closed |
| IOP-TC-008 | TGT-002 | IFS-AUTH-TLS-NEG-005 | DIMSE | negative | sha256:8888888888888888888888888888888888888888888888888888888888888888 | sha256:bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc | PASS | TLS-POLICY | insecure transport denied |
| IOP-TC-009 | TGT-003 | IFS-WL-POS-001 | MWL | positive | sha256:9999999999999999999999999999999999999999999999999999999999999999 | sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd | PASS | WL-ORDERING | mwl deterministic ordering |
| IOP-TC-010 | TGT-003 | IFS-MPPS-POS-001 | MPPS | positive | sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | sha256:dededededededededededededededededededededededededededededededede | PASS | MPPS-TRANSITION | valid transition |
| IOP-TC-011 | TGT-003 | IFS-MPPS-NEG-001 | MPPS | negative | sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | sha256:efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef | PASS | MPPS-REVERSION-REJECT | terminal reversion rejected |
| IOP-TC-012 | TGT-003 | IFS-WL-NEG-001 | MWL | negative | sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | sha256:fafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafa | PASS | WL-LIMIT-REJECT | oversized filter fail-closed |
