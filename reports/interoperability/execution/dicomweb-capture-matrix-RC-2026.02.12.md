# Interoperability Capture Matrix

Release: RC-2026.02.12
Generated At (UTC): 2026-02-12T00:44:56Z
Capture Digest (sha256): `5de0a608e6c25fed8c773b4f5107902bc60e4497cfc40ae9a500bb4f5bfae620`

## Summary

- Total rows: 6
- PASS: 6
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S02-DW-001 | TGT-004 | IFS-WEB-QIDO-PEER-001 | DICOMweb | positive | sha256:49b009d402a53e7abceac8fa51e70c19be7e2849e7333c675efffb465aeb6e89 | sha256:91e76f1d2cc88b390f87532b1836a2e86f651aab8cbb35ef3229241ef456c0c2 | PASS | EXT-SITE-D-QIDO | external site D qido studies witnessed run |
| IOP-S02-DW-002 | TGT-004 | IFS-WEB-STOW-WADO-PEER-002 | DICOMweb | positive | sha256:fa18edfd314432ab14b7075a92c4fa3e4436945be8c582acc1a1d361521c19c5 | sha256:3ee561e990f7695a3a4488de02a639f012c2007eda3697005dbfd59e528f3d04 | PASS | EXT-SITE-D-STOW-WADO | external site D stow then wado instance |
| IOP-S02-DW-003 | TGT-004 | IFS-WEB-WADO-METADATA-PEER-003 | DICOMweb | positive | sha256:a755628a321481d26480fbdb3b1462244111c3ccbe6c58ddfbd3c6c2b32cf0bc | sha256:eed3ebdfe57ddf6a5f26b5dd2157e37940924dad9293bd01e2ab8f54af7255a0 | PASS | EXT-SITE-D-WADO-META | external site D metadata retrieval check |
| IOP-S02-DW-004 | TGT-005 | IFS-WEB-QIDO-PEER-004 | DICOMweb | positive | sha256:f369649ca3dc7757db281848b21207fedd4579570becb3eb5fb035c8120ec73c | sha256:6e2fd8b6851bbff4d7b3bf9b84be7a6c075358fdfac352c7336231a14b8924cd | PASS | EXT-SITE-E-QIDO | external site E qido studies witnessed run |
| IOP-S02-DW-005 | TGT-005 | IFS-WEB-STOW-WADO-PEER-005 | DICOMweb | positive | sha256:97f94fc1e03a773d5257c0598bd906c9f344c3d76191ec7d491cdeee4fdddccf | sha256:c375aaa9a1ffb33ab8650813dc2032c9987a3217f60dc056a6c3b9240292d7d7 | PASS | EXT-SITE-E-STOW-WADO | external site E stow then wado instance |
| IOP-S02-DW-006 | TGT-005 | IFS-WEB-WADO-METADATA-PEER-006 | DICOMweb | positive | sha256:03c5f5bc6c97c2dd204c38ff3f9f6075cc74f39b545604d86aebee2eb959a3fa | sha256:44b9fb7ce8dd9830de5c52083f6306ae22f88edbc114ffd3988f89b66a0d1f80 | PASS | EXT-SITE-E-WADO-META | external site E metadata retrieval check |
