# Interoperability Capture Matrix

Release: RC-2026.02.12
Generated At (UTC): 2026-02-12T00:53:38Z
Capture Digest (sha256): `d3d90d124260001a344f2ffb59703e4747d4b719ced1c67651f06a10bb537cbf`

## Summary

- Total rows: 15
- PASS: 15
- FAIL: 0
- BLOCKED: 0

## Rows

| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |
|---|---|---|---|---|---|---|---|---|---|
| IOP-S03-NEG-001 | TGT-004 | IFS-WEB-NEG-UNSUPPORTED-METHOD | DICOMweb | negative | sha256:0a7a8fabe19ecb6979959726c8a4d17285aee84435b64fd04dbde64e97dc4314 | sha256:e4e0f135a74928bb407301673640c173a9a2daf697edec8682606a539d70014e | PASS | EXT-SITE-D-WEB-METHOD-NEG | wave2 repeated negative window 01 | external site D unsupported method fail closed |
| IOP-S03-NEG-002 | TGT-004 | IFS-WEB-NEG-MALFORMED-UID | DICOMweb | negative | sha256:7a7009a81d3d743e8bac1df6b0342bc5f1f96b81726fdbbb7ba692e588d9ea38 | sha256:243fa73a1f7ee1397fa876c20a88c884c9930498bd1b846f5c0845606c2e4cd5 | PASS | EXT-SITE-D-WEB-UID-NEG | wave2 repeated negative window 02 | external site D malformed uid rejected |
| IOP-S03-NEG-003 | TGT-004 | IFS-WEB-NEG-TLS-REQUIRED | DICOMweb | negative | sha256:4269e8685a3cfed16b87db50b6b13ebc1ec1b7dc0584859ab5c705ebc6c7b8f0 | sha256:3ce90c55deee4b8e6486c8c7e568619f6bf33165236d896f3b2d7ec2b00edc2d | PASS | EXT-SITE-D-WEB-TLS-NEG | wave2 repeated negative window 03 | external site D tls downgrade denied |
| IOP-S03-NEG-004 | TGT-005 | IFS-WEB-NEG-AUTH-TOKEN | DICOMweb | negative | sha256:0b0a69cfd42ff77d279d584a59a75876d8ba0974ff449a53cb23e3134cd5318d | sha256:0dcf9034291614fb89f85c71e0f7fdc10314c596684411d9b1133f68cc4fcaaf | PASS | EXT-SITE-E-WEB-AUTH-NEG | wave2 repeated negative window 04 | external site E missing auth token denied |
| IOP-S03-NEG-005 | TGT-005 | IFS-DIMSE-NEG-BAD-COMMAND | DIMSE | negative | sha256:0bd01b40a98069fdefad915b51aabe25c28ce2653524be2eb25716bb73eaa0e4 | sha256:d8ffd131af059c1c32cfb053271f9ff3f393ecba8a7949d732547716f7a9ec39 | PASS | EXT-SITE-E-DIMSE-CMD-NEG | wave2 repeated negative window 05 | external site E unsupported dimse command rejected |
| IOP-S03-NEG-006 | TGT-005 | IFS-DIMSE-NEG-AE-TITLE | DIMSE | negative | sha256:7e9f76b03246054f0a63af54da47ae14fffed1112ec6a0596a07ecbe18cbc5b7 | sha256:b1f6c2f27d7f324808f14e0f4b53a381799882b38a7b2ae2c6eaffe5b2eedd9a | PASS | EXT-SITE-E-DIMSE-AE-NEG | wave2 repeated negative window 06 | external site E unknown ae title rejected |
| IOP-S03-NEG-007 | TGT-006 | IFS-DIMSE-NEG-TLS-POLICY | DIMSE | negative | sha256:cf568431a1408bef3f0dc3c0f0db674ee64219cb2bf0f2c9fae8de1307f1e654 | sha256:f8b3a89f95f3987756d44abacc35bedef77f37d774eb2e4457ee5ff050fd57b5 | PASS | EXT-SITE-F-DIMSE-TLS-NEG | wave2 repeated negative window 07 | external site F insecure association denied |
| IOP-S03-NEG-008 | TGT-006 | IFS-WL-NEG-OVERSIZED-FILTER | MWL | negative | sha256:edd4de0573085a1d2d0d8ca1116aed097bdf1d1ef36296d3c1741d231c045811 | sha256:6a1f50ade1eee00f8407dc99c7f5e3cfffbc43a80a85aec332a1fd13d24ca2ee | PASS | EXT-SITE-F-WL-LIMIT-NEG | wave2 repeated negative window 08 | external site F oversized mwl filter rejected |
| IOP-S03-NEG-009 | TGT-005 | IFS-MPPS-NEG-TERMINAL-REVERT | MPPS | negative | sha256:9953bbcee9945746b9fa6f1febc6e9785d775236b49e2b8a0079cb9e7798faf7 | sha256:2f09e2e49f7845147a7e4437ab2909b289fefbe1eb818d65ab7d7c97d903adac | PASS | EXT-SITE-E-MPPS-STATE-NEG | wave2 repeated negative window 09 | external site E terminal state revert rejected |
| IOP-S03-NEG-010 | TGT-004 | IFS-WEB-NEG-TRANSFER-SYNTAX | DICOMweb | negative | sha256:8df4c02ddcfbea174f81b235a4d0338819339a189be15f63b6df0d1669841a12 | sha256:c293c42adec6cc7ea5e9aa0fe25ad07d495a5068f4c5ba365d365faab33ad7c4 | PASS | EXT-SITE-D-WEB-TS-NEG | wave2 repeated negative window 10 | external site D unsupported transfer syntax rejected |
| IOP-S03-NEG-011 | TGT-006 | IFS-DIMSE-NEG-PDU-LIMIT | DIMSE | negative | sha256:ce02f2f36e4085581d3aa4252d3d51fdc8d4a393aa585ac48e83e4cf256b94c4 | sha256:53e208d477e6bbb573bf7ede0af3d189439ad8ec47964204e4182b3b95c7af06 | PASS | EXT-SITE-F-PDU-LIMIT | pdu size over limit rejected fail closed |
| IOP-S03-NEG-012 | TGT-006 | IFS-DIMSE-NEG-PDV-LIMIT | DIMSE | negative | sha256:665d766ab048fd4d716568f7ed9705a0d6216233265fb7733dc38ebef03edcb8 | sha256:17dbd70716283dec747d16f0537a4d98b1c0c4aa9a157492aadcf14fba2b83a3 | PASS | EXT-SITE-F-PDV-LIMIT | pdv length over limit rejected fail closed |
| IOP-S03-NEG-013 | TGT-004 | IFS-WEB-NEG-ROUTE-CANONICALIZATION | DICOMweb | negative | sha256:f5284e33503e0e890451fca00b822de264f81daa7c4c8c2e53b94dc2328669a5 | sha256:c1f9807b8090d1ed2dd7f7c429491c27f5878b38a1c16b4d91c91c1b245a6b4f | PASS | EXT-SITE-D-ROUTE-CANON | non-canonical route parsing rejected fail closed |
| IOP-S03-NEG-014 | TGT-005 | IFS-WEB-NEG-QUERY-BOUNDARY | DICOMweb | negative | sha256:7f9b83b751c5952cf8b2dd5ec4a3ccac394409ce9545b951b938ec1f425913e6 | sha256:4561f67e83eaef6052c088a2daca63821229634eab32f709a847611fa3dd2cd7 | PASS | EXT-SITE-E-QUERY-BOUNDARY | query parameter boundary overflow rejected fail closed |
| IOP-S03-NEG-015 | TGT-005 | IFS-AUTH-NEG-POLICY-TRANSITION | DICOMweb | negative | sha256:977291cbead1d9c536baac1d429785935ae71420189a5303ce332c5670297a0a | sha256:3faf5eee0804d9f6317f0917360ac04e584ee34d527447db8af1c1953d4b1f97 | PASS | EXT-SITE-E-AUTH-POLICY | token revoked during policy transition denied fail closed |
