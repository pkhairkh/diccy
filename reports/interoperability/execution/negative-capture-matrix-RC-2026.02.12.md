# Interoperability Capture Matrix

Release: RC-2026.02.12
Generated At (UTC): 2026-02-12T00:44:56Z
Capture Digest (sha256): `640cd0815778dfef3eb7755754ee20a5d3f3b360689dff3cacc57d7507d551bd`

## Summary

- Total rows: 10
- PASS: 10
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S02-NEG-001 | TGT-004 | IFS-WEB-NEG-UNSUPPORTED-METHOD | DICOMweb | negative | sha256:0a7a8fabe19ecb6979959726c8a4d17285aee84435b64fd04dbde64e97dc4314 | sha256:e4e0f135a74928bb407301673640c173a9a2daf697edec8682606a539d70014e | PASS | EXT-SITE-D-WEB-METHOD-NEG | external site D unsupported method fail closed |
| IOP-S02-NEG-002 | TGT-004 | IFS-WEB-NEG-MALFORMED-UID | DICOMweb | negative | sha256:7a7009a81d3d743e8bac1df6b0342bc5f1f96b81726fdbbb7ba692e588d9ea38 | sha256:243fa73a1f7ee1397fa876c20a88c884c9930498bd1b846f5c0845606c2e4cd5 | PASS | EXT-SITE-D-WEB-UID-NEG | external site D malformed uid rejected |
| IOP-S02-NEG-003 | TGT-004 | IFS-WEB-NEG-TLS-REQUIRED | DICOMweb | negative | sha256:4269e8685a3cfed16b87db50b6b13ebc1ec1b7dc0584859ab5c705ebc6c7b8f0 | sha256:3ce90c55deee4b8e6486c8c7e568619f6bf33165236d896f3b2d7ec2b00edc2d | PASS | EXT-SITE-D-WEB-TLS-NEG | external site D tls downgrade denied |
| IOP-S02-NEG-004 | TGT-005 | IFS-WEB-NEG-AUTH-TOKEN | DICOMweb | negative | sha256:0b0a69cfd42ff77d279d584a59a75876d8ba0974ff449a53cb23e3134cd5318d | sha256:0dcf9034291614fb89f85c71e0f7fdc10314c596684411d9b1133f68cc4fcaaf | PASS | EXT-SITE-E-WEB-AUTH-NEG | external site E missing auth token denied |
| IOP-S02-NEG-005 | TGT-005 | IFS-DIMSE-NEG-BAD-COMMAND | DIMSE | negative | sha256:0bd01b40a98069fdefad915b51aabe25c28ce2653524be2eb25716bb73eaa0e4 | sha256:d8ffd131af059c1c32cfb053271f9ff3f393ecba8a7949d732547716f7a9ec39 | PASS | EXT-SITE-E-DIMSE-CMD-NEG | external site E unsupported dimse command rejected |
| IOP-S02-NEG-006 | TGT-005 | IFS-DIMSE-NEG-AE-TITLE | DIMSE | negative | sha256:7e9f76b03246054f0a63af54da47ae14fffed1112ec6a0596a07ecbe18cbc5b7 | sha256:b1f6c2f27d7f324808f14e0f4b53a381799882b38a7b2ae2c6eaffe5b2eedd9a | PASS | EXT-SITE-E-DIMSE-AE-NEG | external site E unknown ae title rejected |
| IOP-S02-NEG-007 | TGT-006 | IFS-DIMSE-NEG-TLS-POLICY | DIMSE | negative | sha256:cf568431a1408bef3f0dc3c0f0db674ee64219cb2bf0f2c9fae8de1307f1e654 | sha256:f8b3a89f95f3987756d44abacc35bedef77f37d774eb2e4457ee5ff050fd57b5 | PASS | EXT-SITE-F-DIMSE-TLS-NEG | external site F insecure association denied |
| IOP-S02-NEG-008 | TGT-006 | IFS-WL-NEG-OVERSIZED-FILTER | MWL | negative | sha256:edd4de0573085a1d2d0d8ca1116aed097bdf1d1ef36296d3c1741d231c045811 | sha256:6a1f50ade1eee00f8407dc99c7f5e3cfffbc43a80a85aec332a1fd13d24ca2ee | PASS | EXT-SITE-F-WL-LIMIT-NEG | external site F oversized mwl filter rejected |
| IOP-S02-NEG-009 | TGT-005 | IFS-MPPS-NEG-TERMINAL-REVERT | MPPS | negative | sha256:9953bbcee9945746b9fa6f1febc6e9785d775236b49e2b8a0079cb9e7798faf7 | sha256:2f09e2e49f7845147a7e4437ab2909b289fefbe1eb818d65ab7d7c97d903adac | PASS | EXT-SITE-E-MPPS-STATE-NEG | external site E terminal state revert rejected |
| IOP-S02-NEG-010 | TGT-004 | IFS-WEB-NEG-TRANSFER-SYNTAX | DICOMweb | negative | sha256:8df4c02ddcfbea174f81b235a4d0338819339a189be15f63b6df0d1669841a12 | sha256:c293c42adec6cc7ea5e9aa0fe25ad07d495a5068f4c5ba365d365faab33ad7c4 | PASS | EXT-SITE-D-WEB-TS-NEG | external site D unsupported transfer syntax rejected |
