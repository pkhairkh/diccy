# Interoperability Verification Matrix

Date: 2026-02-12
Release Candidate: RC-2026.02.12
Status: Complete (Signed for Sprint 04 execution)
Owner: V&V Lead

## Schema

- Interface Surface ID
- Use Context
- Input Profile
- Expected Behavior
- Required Limits/Policies
- Test Artifact ID
- Result
- Reproducibility Hash
- Reviewer Sign-off

## Rows

| Interface Surface ID | Use Context | Input Profile | Expected Behavior | Required Limits/Policies | Test Artifact ID | Result | Reproducibility Hash | Reviewer Sign-off |
|---|---|---|---|---|---|---|---|---|
| IFS-WEB-QIDO-PEER-001 | TGT-004 external DICOMweb study search | peer profile `GET /studies` query | Deterministic external query routing and ordered response | `ARS-IOP-004`, `REQ-WEB-300` | `IOP-S02-DW-001` (`reports/interoperability/execution/dicomweb-capture-matrix-RC-2026.02.12.md`) | PASS | `d7893a03c53b75d069dc38a5e684f7e4af5598dfece4b8591db6bb48134afe6e` | Signed (V&V Lead) |
| IFS-WEB-STOW-WADO-PEER-002 | TGT-004 external DICOMweb store/retrieve | peer profile `POST /studies` + `GET /instances` | Deterministic external store then exact byte retrieval | `ARS-IOP-004`, `REQ-WEB-304` | `IOP-S02-DW-002` (`reports/interoperability/execution/dicomweb-capture-matrix-RC-2026.02.12.md`) | PASS | `87fcbd44c36c81a971eb75fa0ea11dd628943aab5b56f6effcd642f153d56ade` | Signed (V&V Lead) |
| IFS-WEB-QIDO-PEER-004 | TGT-005 external enterprise DICOMweb query | enterprise peer profile QIDO request | Profile-conformant deterministic external query handling | `ARS-IOP-005`, `REQ-WEB-300` | `IOP-S02-DW-004` (`reports/interoperability/execution/dicomweb-capture-matrix-RC-2026.02.12.md`) | PASS | `73ca2519afc6ad3b20ac459adbfcffafd8d00131903fda9339bc99763f8353ac` | Signed (Interop Lead) |
| IFS-WEB-STOW-WADO-PEER-005 | TGT-005 external enterprise DICOMweb store/retrieve | enterprise peer profile STOW/WADO exchange | Profile-conformant deterministic external store/retrieve | `ARS-IOP-005`, `REQ-WEB-304` | `IOP-S02-DW-005` (`reports/interoperability/execution/dicomweb-capture-matrix-RC-2026.02.12.md`) | PASS | `4777f20d124afc32603424248fa0d07fc07a447bf5507e5497476832b3dc8a95` | Signed (Interop Lead) |
| IFS-DIMSE-ECHO-PEER-001 | TGT-004 external DIMSE liveness | peer profile `C-ECHO` association | Successful response on authorized external path | `ARS-IOP-004`, `REQ-DIMSE-300` | `IOP-S02-DIM-001` (`reports/interoperability/execution/dimse-capture-matrix-RC-2026.02.12.md`) | PASS | `8927356182a549b97bd29f78732110a0b36ba964ced425c15e657888df33eef7` | Signed (Interop Lead) |
| IFS-DIMSE-STORE-PEER-004 | TGT-005 external DIMSE ingest/query | peer profile `C-STORE` + indexed retrieval checks | Deterministic external storage ingest and query availability | `ARS-IOP-005`, `REQ-STOR-303` | `IOP-S02-DIM-004` (`reports/interoperability/execution/dimse-capture-matrix-RC-2026.02.12.md`) | PASS | `28e55467f89ba0b652134e94dbf13eea4005d194ef84df535b9dc17ad1c1b333` | Signed (Interop Lead) |
| IFS-DIMSE-CFIND-GW-005 | TGT-006 external modality gateway query | gateway profile `C-FIND` baseline | Deterministic query semantics under gateway policy | `ARS-IOP-006`, `REQ-DIMSE-302` | `IOP-S02-DIM-005` (`reports/interoperability/execution/dimse-capture-matrix-RC-2026.02.12.md`) | PASS | `60142614925d48a2ad14bf8e36c06cbdbd46bd945b9ecd321a62ce27bb754ba6` | Signed (Interop Lead) |
| IFS-DIMSE-CMOVE-GW-006 | TGT-006 external modality gateway retrieve | gateway profile `C-MOVE` baseline | Deterministic external retrieve path behavior | `ARS-IOP-006`, `REQ-DIMSE-302` | `IOP-S02-DIM-006` (`reports/interoperability/execution/dimse-capture-matrix-RC-2026.02.12.md`) | PASS | `cdbe3be7b45fe277c6327774e65c6283b265045d7c09ff8b0a6ad44a445b6bd6` | Signed (Interop Lead) |
| IFS-WL-PEER-001 | TGT-005 external workflow query | peer workflow MWL query | Deterministic ordering and response semantics | `ARS-IOP-005`, `REQ-WL-301` | `IOP-S02-WF-001` (`reports/interoperability/execution/workflow-capture-matrix-RC-2026.02.12.md`) | PASS | `b94c59edf57a8621ec6b70694c89ad7ed0da05df677a5bb22a7c1572e17b73cc` | Signed (Workflow Lead) |
| IFS-MPPS-PEER-002 | TGT-005 external workflow lifecycle | peer workflow MPPS transition | Deterministic valid `IN PROGRESS -> COMPLETED` transition accepted | `ARS-IOP-005`, `REQ-MPPS-352` | `IOP-S02-WF-002` (`reports/interoperability/execution/workflow-capture-matrix-RC-2026.02.12.md`) | PASS | `024cc38d280c1c344b8f0f7f569fcbd15f7e7d0243c6222156d3ed8b9c4772c4` | Signed (Workflow Lead) |

## Sign-off

- Reviewer: V&V Lead
- Reviewer: Interop Lead
- Decision: Approved for Sprint 04 execution wave 1 baseline
- Signature ID: SIG-IOP-VER-20260212-S02
- Signed At (UTC): 2026-02-12T01:26:00Z
