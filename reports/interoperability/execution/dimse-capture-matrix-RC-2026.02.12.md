# Interoperability Capture Matrix

Release: RC-2026.02.12
Generated At (UTC): 2026-02-12T00:44:56Z
Capture Digest (sha256): `6bbeb5464a27057be7e53406d04ed4eb0729cb83777130ea8ffb5bb56ac8ca83`

## Summary

- Total rows: 6
- PASS: 6
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S02-DIM-001 | TGT-004 | IFS-DIMSE-ECHO-PEER-001 | DIMSE | positive | sha256:8863d8116d0f51e537a1a44609513d0d6fcc2bc30b79fce98bd07b0f9bcd541e | sha256:1208fc0c9daecd1f74e412aaa168b507ffb6b349e5ddae78917eae8c25f1c958 | PASS | EXT-SITE-D-DIMSE-ECHO | external site D c-echo verification |
| IOP-S02-DIM-002 | TGT-004 | IFS-DIMSE-STORE-PEER-002 | DIMSE | positive | sha256:005f80ad23a3c87f936cfc23f22f1f07596c372b1ef1e960f7d270cf491c29a9 | sha256:8a0e7a880b435b940537eb8ec9c0527ce7cad65b36e5d3758d5a02a438d6ab8c | PASS | EXT-SITE-D-DIMSE-STORE | external site D c-store round trip |
| IOP-S02-DIM-003 | TGT-005 | IFS-DIMSE-ECHO-PEER-003 | DIMSE | positive | sha256:672bd2e14a42c666ca7979ce16e1e1d04bce16ca6790ce2be06498d091a6666b | sha256:4d54fe240c1b9f58b2318e02d831e71840fce6664b2906b83c8a03f0ff7de2a4 | PASS | EXT-SITE-E-DIMSE-ECHO | external site E c-echo verification |
| IOP-S02-DIM-004 | TGT-005 | IFS-DIMSE-STORE-PEER-004 | DIMSE | positive | sha256:587eea7ea7adc94ee01286be2c18acab145da8fbeda99597957b0fa030aecea0 | sha256:72ce55d4910ae2e1e9e7e3cd9532a8f9d00cd21b9dea2c565f1c11aa9ee033ea | PASS | EXT-SITE-E-DIMSE-STORE | external site E c-store round trip |
| IOP-S02-DIM-005 | TGT-006 | IFS-DIMSE-CFIND-GW-005 | DIMSE | positive | sha256:8ac0c38542a8f222077387813e54386950b0756304dfa45ed98d491f5ebf5945 | sha256:2ebb694d42e86cfccea92a4c4504fc1a570ecd32d51ebdf18204df1f429fa3dd | PASS | EXT-SITE-F-DIMSE-CFIND | external site F c-find query baseline |
| IOP-S02-DIM-006 | TGT-006 | IFS-DIMSE-CMOVE-GW-006 | DIMSE | positive | sha256:26f742172ed669a01a9bce8028984785590f67de6f449692dc055d3175fb20f4 | sha256:ae9f24c61d16dc6d78e5dc13104879ae50ed97479a5b4683644df90e53ecf99d | PASS | EXT-SITE-F-DIMSE-CMOVE | external site F c-move retrieve baseline |
