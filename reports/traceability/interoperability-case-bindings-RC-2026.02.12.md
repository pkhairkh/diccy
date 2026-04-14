# Interoperability Case Bindings

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed)
Owner: Traceability Lead

| Case ID | Protocol | Phase | Target | Interface | Trace Cluster | Bound REQ IDs | Source Summary |
|---|---|---|---|---|---|---|---|
| IOP-S02-DIM-001 | DIMSE | positive | TGT-004 | IFS-DIMSE-ECHO-PEER-001 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DIM-002 | DIMSE | positive | TGT-004 | IFS-DIMSE-STORE-PEER-002 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DIM-003 | DIMSE | positive | TGT-005 | IFS-DIMSE-ECHO-PEER-003 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DIM-004 | DIMSE | positive | TGT-005 | IFS-DIMSE-STORE-PEER-004 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DIM-005 | DIMSE | positive | TGT-006 | IFS-DIMSE-CFIND-GW-005 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DIM-006 | DIMSE | positive | TGT-006 | IFS-DIMSE-CMOVE-GW-006 | TRC-CLUSTER-S02-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-001 | DICOMweb | positive | TGT-004 | IFS-WEB-QIDO-PEER-001 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-002 | DICOMweb | positive | TGT-004 | IFS-WEB-STOW-WADO-PEER-002 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-003 | DICOMweb | positive | TGT-004 | IFS-WEB-WADO-METADATA-PEER-003 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-004 | DICOMweb | positive | TGT-005 | IFS-WEB-QIDO-PEER-004 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-005 | DICOMweb | positive | TGT-005 | IFS-WEB-STOW-WADO-PEER-005 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-DW-006 | DICOMweb | positive | TGT-005 | IFS-WEB-WADO-METADATA-PEER-006 | TRC-CLUSTER-S02-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-001 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-UNSUPPORTED-METHOD | TRC-CLUSTER-S02-002 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-002 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-MALFORMED-UID | TRC-CLUSTER-S02-002 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-003 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-TLS-REQUIRED | TRC-CLUSTER-S02-002 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-004 | DICOMweb | negative | TGT-005 | IFS-WEB-NEG-AUTH-TOKEN | TRC-CLUSTER-S02-002 | REQ-AUTH-300, REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-005 | DIMSE | negative | TGT-005 | IFS-DIMSE-NEG-BAD-COMMAND | TRC-CLUSTER-S02-002 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-006 | DIMSE | negative | TGT-005 | IFS-DIMSE-NEG-AE-TITLE | TRC-CLUSTER-S02-002 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-007 | DIMSE | negative | TGT-006 | IFS-DIMSE-NEG-TLS-POLICY | TRC-CLUSTER-S02-002 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-008 | MWL | negative | TGT-006 | IFS-WL-NEG-OVERSIZED-FILTER | TRC-CLUSTER-S02-002 | REQ-WL-302 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-009 | MPPS | negative | TGT-005 | IFS-MPPS-NEG-TERMINAL-REVERT | TRC-CLUSTER-S02-002 | REQ-MPPS-352 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-NEG-010 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-TRANSFER-SYNTAX | TRC-CLUSTER-S02-002 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-001 | MWL | positive | TGT-005 | IFS-WL-PEER-001 | TRC-CLUSTER-S02-001 | REQ-WL-301 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-002 | MPPS | positive | TGT-005 | IFS-MPPS-PEER-002 | TRC-CLUSTER-S02-001 | REQ-MPPS-351, REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-003 | MWL | negative | TGT-005 | IFS-WL-NEG-PEER-003 | TRC-CLUSTER-S02-002 | REQ-WL-302 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-004 | MPPS | negative | TGT-005 | IFS-MPPS-NEG-PEER-004 | TRC-CLUSTER-S02-002 | REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-005 | MWL | positive | TGT-006 | IFS-WL-PEER-005 | TRC-CLUSTER-S02-001 | REQ-WL-301 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S02-WF-006 | MPPS | negative | TGT-006 | IFS-MPPS-NEG-PEER-006 | TRC-CLUSTER-S02-002 | REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-RC-2026.02.12.json |
| IOP-S03-DIM-001 | DIMSE | positive | TGT-004 | IFS-DIMSE-ECHO-PEER-001 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DIM-002 | DIMSE | positive | TGT-004 | IFS-DIMSE-STORE-PEER-002 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DIM-003 | DIMSE | positive | TGT-005 | IFS-DIMSE-ECHO-PEER-003 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DIM-004 | DIMSE | positive | TGT-005 | IFS-DIMSE-STORE-PEER-004 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DIM-005 | DIMSE | positive | TGT-006 | IFS-DIMSE-CFIND-GW-005 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DIM-006 | DIMSE | positive | TGT-006 | IFS-DIMSE-CMOVE-GW-006 | TRC-CLUSTER-S03-001 | REQ-DIMSE-300, REQ-DIMSE-302 | reports/interoperability/execution/dimse-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-001 | DICOMweb | positive | TGT-004 | IFS-WEB-QIDO-PEER-001 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-002 | DICOMweb | positive | TGT-004 | IFS-WEB-STOW-WADO-PEER-002 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-003 | DICOMweb | positive | TGT-004 | IFS-WEB-WADO-METADATA-PEER-003 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-004 | DICOMweb | positive | TGT-005 | IFS-WEB-QIDO-PEER-004 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-005 | DICOMweb | positive | TGT-005 | IFS-WEB-STOW-WADO-PEER-005 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-DW-006 | DICOMweb | positive | TGT-005 | IFS-WEB-WADO-METADATA-PEER-006 | TRC-CLUSTER-S03-001 | REQ-WEB-300, REQ-WEB-304 | reports/interoperability/execution/dicomweb-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-001 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-UNSUPPORTED-METHOD | TRC-CLUSTER-S03-001 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-002 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-MALFORMED-UID | TRC-CLUSTER-S03-001 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-003 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-TLS-REQUIRED | TRC-CLUSTER-S03-001 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-004 | DICOMweb | negative | TGT-005 | IFS-WEB-NEG-AUTH-TOKEN | TRC-CLUSTER-S03-001 | REQ-AUTH-300, REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-005 | DIMSE | negative | TGT-005 | IFS-DIMSE-NEG-BAD-COMMAND | TRC-CLUSTER-S03-001 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-006 | DIMSE | negative | TGT-005 | IFS-DIMSE-NEG-AE-TITLE | TRC-CLUSTER-S03-001 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-007 | DIMSE | negative | TGT-006 | IFS-DIMSE-NEG-TLS-POLICY | TRC-CLUSTER-S03-001 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-008 | MWL | negative | TGT-006 | IFS-WL-NEG-OVERSIZED-FILTER | TRC-CLUSTER-S03-001 | REQ-WL-302 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-009 | MPPS | negative | TGT-005 | IFS-MPPS-NEG-TERMINAL-REVERT | TRC-CLUSTER-S03-001 | REQ-MPPS-352 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-010 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-TRANSFER-SYNTAX | TRC-CLUSTER-S03-001 | REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-011 | DIMSE | negative | TGT-006 | IFS-DIMSE-NEG-PDU-LIMIT | TRC-CLUSTER-S03-002 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-012 | DIMSE | negative | TGT-006 | IFS-DIMSE-NEG-PDV-LIMIT | TRC-CLUSTER-S03-002 | REQ-DIMSE-304, REQ-NET-308 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-013 | DICOMweb | negative | TGT-004 | IFS-WEB-NEG-ROUTE-CANONICALIZATION | TRC-CLUSTER-S03-002 | REQ-CONF-002, REQ-HTTP-300, REQ-HTTP-303 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-014 | DICOMweb | negative | TGT-005 | IFS-WEB-NEG-QUERY-BOUNDARY | TRC-CLUSTER-S03-002 | REQ-CONF-002, REQ-HTTP-300, REQ-HTTP-303 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-NEG-015 | DICOMweb | negative | TGT-005 | IFS-AUTH-NEG-POLICY-TRANSITION | TRC-CLUSTER-S03-002 | REQ-AUTH-300, REQ-CONF-002, REQ-HTTP-300 | reports/interoperability/execution/negative-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-001 | MWL | positive | TGT-005 | IFS-WL-PEER-001 | TRC-CLUSTER-S03-001 | REQ-WL-301 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-002 | MPPS | positive | TGT-005 | IFS-MPPS-PEER-002 | TRC-CLUSTER-S03-001 | REQ-MPPS-351, REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-003 | MWL | negative | TGT-005 | IFS-WL-NEG-PEER-003 | TRC-CLUSTER-S03-001 | REQ-WL-302 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-004 | MPPS | negative | TGT-005 | IFS-MPPS-NEG-PEER-004 | TRC-CLUSTER-S03-001 | REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-005 | MWL | positive | TGT-006 | IFS-WL-PEER-005 | TRC-CLUSTER-S03-001 | REQ-WL-301 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |
| IOP-S03-WF-006 | MPPS | negative | TGT-006 | IFS-MPPS-NEG-PEER-006 | TRC-CLUSTER-S03-001 | REQ-MPPS-352 | reports/interoperability/execution/workflow-capture-summary-wave2-RC-2026.02.12.json |

## Sign-off

- Traceability Lead: Signed
- QA/RA Lead: Signed
- Signature ID: SIG-TRC-CASE-BIND-20260212
- Signed At (UTC): 2026-02-12T02:09:00Z
