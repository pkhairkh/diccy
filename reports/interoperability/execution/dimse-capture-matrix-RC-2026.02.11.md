# Interoperability Capture Matrix

Release: RC-2026.02.11
Generated At (UTC): 2026-02-11T21:24:49Z
Capture Digest (sha256): `38baa51649aa927ed1be33f8c449828cf806eedbd538738c41e460c5ffd0d174`

## Summary

- Total rows: 6
- PASS: 6
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S11-DIM-001 | TGT-002 | IFS-DIMSE-ECHO-003 | DIMSE | positive | sha256:7777777777777777777777777777777777777777777777777777777777777777 | sha256:abababababababababababababababababababababababababababababababab | PASS | DIMSE-C-ECHO | local dimse echo |
| IOP-S11-DIM-002 | TGT-002 | IFS-DIMSE-STORE-004 | DIMSE | positive | sha256:8888888888888888888888888888888888888888888888888888888888888888 | sha256:bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc | PASS | DIMSE-C-STORE | local dimse store/query |
| IOP-S11-DIM-003 | TGT-004 | IFS-DIMSE-ECHO-PEER-001 | DIMSE | positive | sha256:9999999999999999999999999999999999999999999999999999999999999999 | sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd | PASS | ORTHANC-DIMSE-ECHO | peer profile replay echo |
| IOP-S11-DIM-004 | TGT-005 | IFS-DIMSE-STORE-PEER-002 | DIMSE | positive | sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | sha256:dededededededededededededededededededededededededededededededede | PASS | DCM4CHEE-DIMSE-STORE | enterprise peer profile replay store |
| IOP-S11-DIM-005 | TGT-006 | IFS-DIMSE-ECHO-MOD-003 | DIMSE | positive | sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | sha256:efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef | PASS | DCMTK-ECHO | modality gateway echo |
| IOP-S11-DIM-006 | TGT-006 | IFS-DIMSE-STORE-MOD-004 | DIMSE | positive | sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc | sha256:fafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafa | PASS | DCMTK-STORE | modality gateway store |
