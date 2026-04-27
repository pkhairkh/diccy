# Integration Cookbook

Status: **Baseline integration cookbook** (As of 2026-02-22)
Reference: `docs/40-Reference-Deployment-Topology.md`

## 1. Adjacent PACS/VNA interoperability model

DiCCY is DICOMweb-first and interoperates with adjacent PACS/VNA ecosystems through three explicit integration modes:

- **Direct DICOMweb mode** – workstation and modality paths use HTTP DICOMweb and rely on `dicom-web-server` for ingest/query/retrieve.
- **Bridge mode** – a local adapter translates legacy DIMSE flows into DICOMweb transactions.
- **Hybrid mode** – a native DIMSE entrypoint and `dicom-web-server` are both active when both transport contracts are required.

For every integration, treat DIMSE as *optional transport assistance* and not as a universal runtime requirement.

| Adjacent ecosystem | Primary path | Fallback path when legacy DIMSE is unavailable | Boundary owner |
|---|---|---|---|
| Orthanc | `dicomweb` (`/dicom-web/*`) as primary ingress/egress | Push study-level STOW/QIDO/WADO fallback to local edge queue and replay service | `dicom-web-server` + optional bridge adapter |
| dcm4chee | `dicomweb` for query/retrieve, `dimse` where installed | Use dcm4chee’s DICOMweb-first integration profile where available; otherwise queue C-Find intent and convert to QIDO-RS | Workflow adapter in deployment profile |
| OHIF/Weasis | DICOMweb for study/worklist browsing and retrieval | SR/task handoff via workflow endpoints and static viewer deep links | `dicom-workflow-server` + `viewer-wasm` |
| Clinic VNA with strict C-Move policy | Controlled DIMSE integration profile for legacy modalities | Route image pull operations via WADO-RS through policy queue | Security gateway + bridge shim |

Operational rule: if you cannot prove DIMSE availability and credential alignment in deployment, run in DICOMweb-only mode and treat DIMSE calls as optional capabilities with explicit fallback behavior.

## 2. DICOMweb-only deployment pattern

- Start `dicom-web-server` and `viewer-wasm` host.
- Route workstation query/retrieve and ingest through QIDO/WADO/STOW endpoints.
- Keep workflow endpoints disabled when SR/worklist/MPPS flows are out of scope.
- If a remote PACS expects DIMSE behavior, provide client-side or gateway fallback guidance:
  - Prefer QIDO search filters that match modality naming and accession constraints.
  - Convert C-Move intent to server-side pre-authenticated WADO fetch jobs when needed.
  - Fail closed with explicit operator diagnostics when requested SOP classes are unsupported.

## 3. DIMSE-only deployment pattern

- Start DIMSE service boundary (`dicom-dimse-service`) with storage/query integration.
- Use DIMSE C-STORE/C-FIND/C-MOVE clients for legacy modality integration.
- Keep browser host optional for visualization-only review paths.
- If DIMSE cannot negotiate required SOP classes, transition immediately to DICOMweb fallback policy rather than continuing with partial runtime.

## 4. Hybrid deployment pattern

- Run DICOMweb + DIMSE + workflow services concurrently.
- Use DICOMweb for workstation-facing APIs; DIMSE for legacy modality ingress.
- Use workflow endpoints for SR create/update/retrieve and MWL/MPPS lifecycle.
- Explicitly document and monitor negotiated DIMSE SOP class boundary:
  - Prefer DICOMweb for retrieval when both channels return equivalent data.
  - Use DIMSE only when modality or transport policy requires it.

## 5. Orthanc bridge profile example

For orthanc-adjacent deployments, preferred path is an adapter boundary with Orthanc as DIMSE terminus and DiCCY as DICOMweb target:

1. Keep Orthanc as local ingestion edge for legacy modality sessions.
2. Configure Orthanc export hook to call DiCCY STOW endpoint on accepted studies.
3. Maintain a replay queue for Orthanc callbacks that fail under transient ingress errors.
4. Publish study-level manifest metadata from Orthanc to workflow/SR via deterministic event log.

Example trigger path:

```bash
# Orthanc (edge) -> DiCCY (DICOMweb)
curl -X POST 'http://127.0.0.1:8080/studies' --data-binary '@study.zip' \
  -H 'Content-Type: application/dicom+zip'
```

Suggested environment boundary (example only):

```env
DICCY_DICOMWEB_BASE=http://127.0.0.1:8080
DICCY_WORKFLOW_BASE=http://127.0.0.1:8082
DICCY_INGEST_QUARANTINE_SECONDS=120
DICCY_BRIDGE_MODE=orthanc
```

Security constraints for Orthanc bridging:
- mTLS or authenticated reverse-proxy for the Orthanc callback endpoint.
- Signed callback payload or checksum validation at adapter boundary.
- Hard study and SOP-class allow-lists derived from clinical policy.

## 6. dcm4chee interoperability mapping guide

| dcm4chee workflow touchpoint | DiCCY mapping |
|---|---|
| C-FIND (Patient/Study query) | QIDO-RS `/studies` with equivalent query keys |
| C-MOVE (study retrieve requests) | Server-side precomputed WADO-RS pull with policy gate |
| STOW for new instances | `/studies` POST (`multipart/related`) ingestion path |
| MWL/MPPS interactions | Workflow server API endpoints and webhook bridge where available |

Recommended migration policy:
- Keep dcm4chee as source of truth for modality scheduling/state if already in production.
- Use DiCCY for deterministic pixel pipeline and SR lifecycle operations.
- Disable C-MOVE if retrieval ACL cannot be expressed in local policy contract; enforce DICOMweb pull model instead.
- Record every translation action in non-PHI audit events for deterministic reconciliation.

## 7. Connector fallback behavior contract

When a remote system exposes only DIMSE-style workflows:

- Validate transport capability at startup (DIMSE association and SOP-class matrix).
- If a required class is missing, mark integration as degraded and activate DICOMweb gateway mode.
- Expose explicit status on the operator panel and reject unsafe requests with typed reasons.
- Queue fallback jobs for retriable operations and surface retry windows.
- For workflow event-driven callbacks, treat delivery failures as retriable with bounded attempts and persist DLQ manifest entries for operator review under `/interop/hl7/failures`.
- Callback retry policy is operator-tunable and should be pinned per release:
  - `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS` (default `3`, recommended `2..=5`)
  - `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD` (default `2`, recommended `2..=4`)
  - `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS` (default `15000`, allowed `100..=3_600_000`)
  - `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS` (default `300000`, allowed `1_000..=86_400_000`)
  - Startup fails closed when bounds are invalid or `BASE_BACKOFF_MS > MAX_BACKOFF_MS`.

Fallback examples:
- If C-MOVE is denied, execute server-driven WADO polling with deterministic polling window and expiry.
- If DIMSE find times out, run QIDO search with equivalent patient/study filters and return deterministic sort order.
- If callback delivery fails, persist manifest records and retry with exponential backoff limits.

## 8. Failure and fallback behavior contract

- `dicomweb-only` mode is the safe default for all new sites.
- DIMSE components are optional and may be excluded if a partner only requires web APIs.
- Unsupported legacy calls must return fail-closed messages and documented remediation steps, never silent success.
- Any optional integration feature must have a documented fallback path and a testable status label in runbook tooling.

## 9. Security and fail-closed checklist

- Explicit auth mode selection (deny-all default unless deployment policy allows otherwise).
- Explicit transport policy declarations.
- Persistence paths writable and rotation-bounded before accepting traffic.
- Out-of-envelope payloads rejected with typed fail-closed errors.
- Integration errors must preserve enough context for deterministic incident triage without leaking PHI.

## 10. API compatibility profiles for evaluator onboarding

Use these onboarding profiles when validating external evaluator claims against common DICOM stacks:

### Profile A: Orthanc-centric clinic
- Core ingress: DICOMweb STOW/QIDO/WADO via `dicom-web-server`.
- Optional relay: Orthanc bridge adapter emits webhooks into `dicom-workflow-server`.
- Expected runtime checks:
  - DICOMweb endpoints return deterministic pagination and query order.
  - `/interop/subscriptions` stores callback registrations and rejects unsupported sink kinds.
  - Unknown connector aliases fail closed in `/interop/subscriptions`.

### Profile B: dcm4chee-centric deployment
- Core ingress: DICOMweb + controlled DIMSE if enabled.
- External interoperability:
  - Convert C-FIND into QIDO requests using canonical attribute names.
  - Route move-style retrieval as pull jobs with explicit retry windows.
- Expected runtime checks:
  - `interoperability-policy` flags for `qido`, `wado`, and `stow` can be toggled by environment.
  - If DIMSE not present, all retrieval flows continue via WADO-RS fallback.

### Profile C: PACS-agnostic evaluator mode
- Core ingress: DICOMweb only.
- Optional partner-specific callbacks via `/interop/subscriptions`.
- Expected runtime checks:
  - Endpoints available in `docs/12-API-Surface-and-Crate-Boundaries.md`.
  - No assumed DIMSE transport in startup scripts or runbooks.

## 11. Supported and unsupported interoperability capabilities

Current limitations are documented so evaluator teams can scope proof points:

- HL7 ingestion is supported in three transport modes:
  - Native MLLP listener (enable with `DICOM_WORKFLOW_HL7_MLLP_ENABLED=true`, bind with `DICOM_WORKFLOW_HL7_MLLP_BIND`).
  - File-drop polling (`DICOM_WORKFLOW_HL7_FILE_DROP_DIR` with done/error directories).
  - HTTPS bridge via `POST /interop/hl7` with workflow auth controls.
- No in-server DIMSE transaction proxy; DIMSE support is handled via optional external adapters.
- Modality-facing C-MOVE fallback is job-oriented pull by WADO/RS when DIMSE is unavailable.
- Cross-system reconciliation for external HIS/RIS remains best-effort; no distributed exactly-once guarantees yet.
- Reconciliation run API (`POST /interop/reconciliation/jobs/{job_id}/run`) is idempotent per tenant/job/key (`x-idempotency-key`) with restart-safe replay cache persistence.
- Native HL7 ACK behavior is transport-dependent and documented through integration examples in this cookbook and `docs/12-API-Surface-and-Crate-Boundaries.md`.
- MLLP ACK/NACK behavior is deterministic:
  - success: `MSA|AA|ACK`,
  - auth denied: `MSA|AR|ACK` + `ERR|DVF.WORKFLOW.SR.AUTH_DENIED`,
  - other fail-closed ingest errors: `MSA|AE|ACK` + `ERR|<DVF error code>`.

### Orthanc and dcm4chee interoperability profile

- Supported in this runtime:
  - Inbound HL7 over `/interop/hl7` with task/order/report semantics.
  - MLLP listener and file-drop ingress for institutions already shipping HL7 files/streams.
  - Outbound callback delivery to configured sinks and `DICOM_WORKFLOW_HL7_CONNECTOR_*` mappings.
- Operator runbooks:
  - tenant onboarding and rollout strategy: `docs/60-Tenant-Aware-Integration-Onboarding-Runbook.md`
  - certificate/secret rotation: `docs/61-Connector-Certificate-and-Secret-Rotation-Runbook.md`
- Explicitly unsupported or out-of-tree:
  - No built-in native Orthanc plugin bridge (deploy one and point it at `/interop/hl7`).
  - No built-in native dcm4chee proxy; treat dcm4chee as a transport/source endpoint only unless wrapped by an external connector.
  - No guaranteed bidirectional workflow reconciliation protocol (reconciliation jobs are provided, but not a HIS/RIS-native round-trip API).

## 12. OHIF and Weasis deployment templates

- **OHIF**:
  - Point OHIF DICOMweb endpoint to `http://<host>:8080`.
  - Route study metadata and image retrieval through QIDO/WADO.
  - Disable OHIF DICOMweb feature flags that assume native DIMSE.
- **Weasis**:
  - Use the same DICOMweb endpoint for study list and retrieve.
  - For workflow events, link Weasis tasks to `dicom-workflow-server` SR/task endpoints.
  - Use explicit fallback copy for failed C-Move-style handoffs to prevent opaque UI failures.

## 13. Modality C-Find/C-Move fallback contract

Where DIMSE is absent, enforce the following contract:

1. Probe DIMSE class negotiation before binding modality callbacks.
2. If class probing fails, force DICOMweb equivalents and return typed fallback status.
3. For C-FIND style queries:
   - execute deterministic QIDO queries with strict sort normalization.
4. For C-MOVE style requests:
   - enqueue WADO pull tasks with bounded retries.
   - publish retryable failures through `/interop/hl7` subscriptions so enterprise orchestrators can decide human escalation.

## 14. Migration pattern: OHIF/Orthanc plugin workflows vs DiCCY adapter model

This pattern maps common plugin-first integrations (OHIF extensions, Orthanc bridge plugins) to the DiCCY adapter boundary so teams can migrate incrementally without breaking existing ingress.

| Legacy plugin workflow | Typical plugin responsibility | DiCCY adapter-model equivalent | Migration cutover signal |
|---|---|---|---|
| OHIF extension posts workflow side-effects directly | Viewer-driven event mutation and ad-hoc callback wiring | Route side-effects through `dicom-workflow-server` (`/workflow/tasks`, `/sr/documents`, `/interop/subscriptions`) | Viewer no longer writes directly to third-party sinks |
| Orthanc plugin forwards DIMSE events and retries internally | Transport conversion + replay queue in plugin code | Keep Orthanc at edge; move replay + policy decisions to DiCCY adapter and `/interop/hl7` pipelines | Retry policy and audit visibility move from plugin logs to DiCCY audit/failure APIs |
| Mixed OHIF + Orthanc custom hooks | Custom cross-system state coupling | Separate concerns: OHIF uses DICOMweb + workflow APIs; Orthanc uses bridge adapter to DICOMweb/interop | Shared state transitions appear as deterministic DiCCY task/SR events |

Recommended sequence:

1. Preserve existing plugin behavior, but mirror the same events into DiCCY interop endpoints in shadow mode.
2. Move retry/circuit-breaker/auth policy from plugin code to DiCCY adapter endpoints and verify parity with existing operational metrics.
3. Cut viewer/plugin write paths over to DiCCY APIs, leaving plugin paths read-only for rollback.
4. Remove plugin-owned business logic once DiCCY audit, failure queues, and rollout controls are the source of truth.

Rollback guidance:

- Keep OHIF/Orthanc plugin handlers deployable during transition, but gate them behind feature flags.
- If parity checks fail, revert only the latest adapter cutover stage and keep transport ingress unchanged.
