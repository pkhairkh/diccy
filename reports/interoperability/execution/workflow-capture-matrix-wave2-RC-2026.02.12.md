# Interoperability Capture Matrix

Release: RC-2026.02.12
Generated At (UTC): 2026-02-12T00:53:38Z
Capture Digest (sha256): `d7a90e99ed601f888fd22c4749936b9a2ab68ddf0c3ec94cf49e3116dfac6aa6`

## Summary

- Total rows: 6
- PASS: 6
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S03-WF-001 | TGT-005 | IFS-WL-PEER-001 | MWL | positive | sha256:3cb80d89f1302df90e6e893f77ac33f449cd8fa7f459c49beb37f5fe97246502 | sha256:12e3da619a534dc6063a1539a3a8f3d65974341a070fa04dd0be3cf8df992621 | PASS | EXT-SITE-E-WL-POS | wave2 repeated window 01 | external site E mpps enabled mwl query ordering |
| IOP-S03-WF-002 | TGT-005 | IFS-MPPS-PEER-002 | MPPS | positive | sha256:f7c572d5237d113931c10b6921511661c584a166bccb058ecf24c69b12f14bb7 | sha256:335d615a05a013c8566e7891aeeb2beb618f27cc285e77ed7e51aaaee85399ed | PASS | EXT-SITE-E-MPPS-POS | wave2 repeated window 02 | external site E mpps in progress to completed |
| IOP-S03-WF-003 | TGT-005 | IFS-WL-NEG-PEER-003 | MWL | negative | sha256:d5b577fe9e13a3711a24fd9c60ccafa5919736597f9c4b3f62b1b136232aa360 | sha256:514dd546798d455b05489c82d834a0632831590b9d6def50ce5180c25852b805 | PASS | EXT-SITE-E-WL-NEG | wave2 repeated window 03 | external site E oversized mwl filter rejected |
| IOP-S03-WF-004 | TGT-005 | IFS-MPPS-NEG-PEER-004 | MPPS | negative | sha256:32bd207da8ed5c378cc1326f77849ea128877b73fc8901836da1f85eec1400dd | sha256:09c2fbce4918fd63989f9b6f08148937642f75cec31b45becd64270d4a34b3b7 | PASS | EXT-SITE-E-MPPS-NEG | wave2 repeated window 04 | external site E mpps terminal reversion rejected |
| IOP-S03-WF-005 | TGT-006 | IFS-WL-PEER-005 | MWL | positive | sha256:0292d12ed854f79a9b5646dbc7a7af75f29f256c967b85a2a652f9ca84a91d08 | sha256:d1ccbeb0b574aa2cee603ac3bf67cef1a0826c405824a2d6b6e74b5f8a6329b4 | PASS | EXT-SITE-F-WL-POS | wave2 repeated window 05 | external site F mwl response ordering check |
| IOP-S03-WF-006 | TGT-006 | IFS-MPPS-NEG-PEER-006 | MPPS | negative | sha256:c86086cbe8f4c2fecf2dbc82324d5b5a72224cb9b69db4ee5a616d1561cda7c3 | sha256:65b043ffb75fe1c7c5889306f5fa2c98241e0724aea475d3017f67919d561ff8 | PASS | EXT-SITE-F-MPPS-NEG | wave2 repeated window 06 | external site F mpps invalid status transition rejected |
