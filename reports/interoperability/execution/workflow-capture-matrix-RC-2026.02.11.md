# Interoperability Capture Matrix

Release: RC-2026.02.11
Generated At (UTC): 2026-02-11T21:24:49Z
Capture Digest (sha256): `aab50c7b9cfb6996fe2147f09a0c94b318122aa86d8c2a8ef072da063e26a46a`

## Summary

- Total rows: 4
- PASS: 4
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S11-WF-001 | TGT-003 | IFS-WL-POS-001 | MWL | positive | sha256:1212121212121212121212121212121212121212121212121212121212121212 | sha256:3434343434343434343434343434343434343434343434343434343434343434 | PASS | WL-ORDERING | local workflow mwl ordering |
| IOP-S11-WF-002 | TGT-003 | IFS-MPPS-POS-001 | MPPS | positive | sha256:5656565656565656565656565656565656565656565656565656565656565656 | sha256:7878787878787878787878787878787878787878787878787878787878787878 | PASS | MPPS-TRANSITION | local workflow valid transition |
| IOP-S11-WF-003 | TGT-005 | IFS-WL-PEER-001 | MWL | positive | sha256:9090909090909090909090909090909090909090909090909090909090909090 | sha256:abababababababababababababababababababababababababababababababac | PASS | DCM4CHEE-WL-PEER | peer workflow profile replay mwl |
| IOP-S11-WF-004 | TGT-005 | IFS-MPPS-PEER-002 | MPPS | positive | sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdce | sha256:efefefefefefefefefefefefefefefefefefefefefefefefefefefefefefeff0 | PASS | DCM4CHEE-MPPS-PEER | peer workflow profile replay mpps |
