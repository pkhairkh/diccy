# Productization Profiles and Playbooks

Last updated: 2026-02-22

## Product profiles

| Profile | Target runtime | Included capabilities | Excluded capabilities |
|---|---|---|---|
| `framework-core` | Rust library / headless | DICOM IO, CPU pixel oracle, deterministic cache/volume/MPR/fusion primitives, pack APIs | Web host UI and network service runtimes |
| `workstation` | Script-hosted WASM front-end + render libraries | Interactive viewport controls, tri-planar/slab validation panels, web diagnostics, SR workflow prototype UI | Multi-tenant backend operations and deployment orchestration |
| `backend-services` | Service deployment | DICOMweb and workflow APIs (MWL/MPPS/SR) with durable state/logging in `dist/profiles/backend-services.<RELEASE_ID>.tar.gz`; no `dicom-dimse-service` binary in that artifact | Legacy monolith workflows, full HIS/RIS/ADT/ORM suite |

## Runtime artifacts by profile

Status: **Partially implemented** (As of 2026-02-24)
Reference: `docs/12-API-Surface-and-Crate-Boundaries.md`, `tools/profile_matrix.json`

Artifact classes:

- `binary`: `bin/<binary-name>` executable artifact inside a profile artifact.
- `script-hosted`: browser-hosted front-end artifact delivered through scripts in `tools/*` and static assets.
- `library`: runtime crate without direct `bin/*` packaging.

| Profile | Runtime crates (library) | Packaged entrypoints |
|---|---|---|
| `framework-core` | `diccy`, `dicom-*` core crates, `viewer-*` core libraries | `dicom-visualizer` |
| `workstation` | `viewer-wasm`, `viewer-wgpu`, `viewer-core` | No dedicated `viewer-*` binaries in this version; run via `./tools/run_viewer_wasm_frontend.sh` and `crates/viewer-wasm/web` |
| `backend-services` | `dicom-web`, `dicom-workflow-server`, `dicom-net`, `dicom-dimse` | `dicom-web-server`, `dicom-workflow-server` |
| `backend-services` + optional DIMSE integration | plus `dicom-dimse-service` crate, transport security/auth configuration | `dicom-dimse-service` (requires `backend-services-with-dimse.<RELEASE_ID>.tar.gz` to include at runtime) |
| `backend-services-with-dimse` | `dicom-dimse-service`, `dicom-web`, `dicom-workflow-server` | `dicom-web-server`, `dicom-workflow-server`, `dicom-dimse-service` |

## Runtime posture and artifact markers

- `✅` = runnable artifact shipped when profile-enabled.
- `⚙️` = library/integration-only and not shipped as a direct service.
- `➖` = runtime omitted from defaults unless explicitly enabled.

| Crate/service | Runtime posture |
|---|---|
| `dicom-web-server` | ✅ runnable artifact (`backend-services` profile) |
| `dicom-workflow-server` | ✅ runnable artifact (`backend-services` profile) |
| `dicom-dimse-service` | ✅ runnable artifact (requires `backend-services-with-dimse.<RELEASE_ID>.tar.gz`; excluded from `backend-services.<RELEASE_ID>.tar.gz`) |
| `dicom-visualizer` | ✅ runnable artifact (`framework-core` profile) |
| `viewer-wasm` | ⚙️ library/integration-only (`viewer-wasm` host + script launch) |
| `viewer-wgpu` | ⚙️ library/integration-only (`viewer-wgpu` render integration target) |
| `dicom-core` / `dicom-io` / `dicom-net` / `dicom-dimse` | ⚙️ library/integration-only |

## Archive layout by profile

`tools/package_profiles.sh` emits deterministic profile artifacts under `dist/profiles/`.

| Profile | Archive name pattern | Runtime paths |
|---|---|---|
| `framework-core` | `framework-core.<RELEASE_ID>.tar.gz` | `capability.manifest.json`, `bin/dicom-visualizer` |
| `workstation` | `workstation.<RELEASE_ID>.tar.gz` | `capability.manifest.json` (no guaranteed `bin/viewer-*` entries in current artifact posture) |
| `backend-services` | `backend-services.<RELEASE_ID>.tar.gz` | `capability.manifest.json`, `bin/dicom-web-server`, `bin/dicom-workflow-server` |
| `backend-services-with-dimse` | `backend-services-with-dimse.<RELEASE_ID>.tar.gz` | `capability.manifest.json`, `bin/dicom-web-server`, `bin/dicom-workflow-server`, `bin/dicom-dimse-service` |

## Exact runtime binary archive paths

All profile artifacts use a fixed root path contract:

- `capability.manifest.json` at archive root.
- `bin/<binary-name>` for every shipped binary.

Service-level path mapping:

| Service | Archive paths (when included) |
|---|---|
| `dicom-web-server` | `bin/dicom-web-server` |
| `dicom-workflow-server` | `bin/dicom-workflow-server` |
| `dicom-dimse-service` | `bin/dicom-dimse-service` |
| `dicom-visualizer` | `bin/dicom-visualizer` |
| `viewer-wasm` | Script-hosted entrypoint via `tools/run_viewer_wasm_frontend.sh` |
| `viewer-wgpu` | Not a binary entrypoint in current profile output |

## Artifact-source contract map

| Claim type | Source of truth | Verification command |
|---|---|---|
| Artifact class and profile expectations | `docs/33-Productization-Profiles-and-Playbooks.md`, `tools/profile_matrix.json` | `python3 tools/runtime_env_contract.py --repo-root . --check-docs --profiles workstation backend-services backend-services-with-dimse` |
| Profile matrix/docs/artifact reconciliation | `tools/profile_matrix.json`, `docs/33`, `dist/profiles/profiles.metadata.json` | `python3 tools/profile_metadata_reconciliation_gate.py --repo-root . --require-artifacts` |
| Workstation launch path | `tools/run_viewer_wasm_frontend.sh` | `./tools/run_viewer_wasm_frontend.sh --help` |
| Archive layout and artifact shape | `tools/package_profiles.sh` (`profile_binaries`, `write_profile_artifact`) | `./tools/package_profiles.sh --release-id "<RELEASE_ID>"` |

Profiles expose no test-only `*_TEST_*` variables in packaged contracts:

- `DICOM_WEB_TEST_STORAGE_BYTES`, `DICOM_WEB_TEST_WORKERS`, `DICOM_WORKFLOW_TEST_RATE_LIMIT`, `DICOM_WORKFLOW_TEST_ROTATIONS`, `DICOM_DIMSE_TEST_MAX_BYTES`, and `DICOM_DIMSE_TEST_OPTIONAL_SECONDS` are intentionally test-only and excluded from shipped runtime profiles unless explicitly injected by local test harnesses.
- Production startup rejects these test-only variables (fail closed) through startup env-contract gates.
- Runtime parser sources:
  - `crates/dicom-web-server/src/main.rs`
  - `crates/dicom-workflow-server/src/main.rs`
  - `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`

## Legacy naming cleanup

- Remove stale `dicom-viewer-*` references in all future doc claims.
- Existing references to `bin/viewer-wasm` and `bin/viewer-wgpu` in older drafts are deprecated unless a native host binary strategy is added and documented in this same profile section.

## DIMSE runtime contract (playbook view)

When the DIMSE profile is enabled, the following environment contract is active:

| Environment variable | Default | Units / constraints | Purpose |
|---|---|---|---|
| `DICOM_DIMSE_BIND` | `127.0.0.1:11112` | host:port | DIMSE listener bind address |
| `DICOM_DIMSE_STORAGE_WAL` | `./state/dicom-dimse/storage.wal` | bytes path | WAL destination |
| `DICOM_DIMSE_STORAGE_WAL_MAX_BYTES` | `134217728` | bytes | Maximum WAL file size before rotation |
| `DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES` | `3` | count | Rotated WAL retention |
| `DICOM_DIMSE_MAX_INPUT_BYTES` | `134217728` | bytes | Raw input cap |
| `DICOM_DIMSE_MAX_DATASET_ELEMENTS` | `16384` | count | DICOM dataset element cap |
| `DICOM_DIMSE_MAX_SEQUENCE_DEPTH` | `32` | count | Nested sequence depth cap |
| `DICOM_DIMSE_MAX_STRING_BYTES` | `1048576` | bytes | String length cap |
| `DICOM_DIMSE_MAX_ELEMENT_VL_BYTES` | `262144` | bytes | Variable-length field cap |
| `DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE` | `4096` | count | Frame-cap for incoming datasets |
| `DICOM_DIMSE_MAX_PIXELS_PER_FRAME` | `1048576` | pixels | Pixel payload cap |
| `DICOM_DIMSE_MAX_DECOMPRESSED_BYTES` | `4194304` | bytes | Decompressed payload cap |
| `DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES` | `0` | bytes | GPU texture cap, CPU-only default |
| `DICOM_DIMSE_MAX_CACHE_BYTES` | `268435456` | bytes | Request cache cap |
| `DICOM_DIMSE_MAX_PDU_BYTES` | `16384` | bytes | Negotiated max PDU cap |
| `DICOM_DIMSE_MAX_PDV_BYTES` | `16384` | bytes | Negotiated max PDV cap |
| `DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS` | `64` | count | Negotiated presentation context cap |
| `DICOM_DIMSE_MAX_COMMAND_BYTES` | `8192` | bytes | Command PDV cap |
| `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS` | `64` | count | Active association cap |
| `DICOM_DIMSE_MAX_CONNECTIONS` | `64` | count | Active association cap (legacy alias) |
| `DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS` | `64` | count | Concurrent in-flight DIMSE operations |
| `DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT` | `4096` | count | Max responses emitted per query/retrieve operation |
| `DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES` | `0` | bytes | C-ECHO dataset cap |
| `DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES` | `134217728` | bytes | C-STORE dataset cap |
| `DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES` | `131072` | bytes | C-FIND dataset cap |
| `DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES` | `131072` | bytes | C-MOVE payload cap |
| `DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES` | `131072` | bytes | C-GET payload cap |
| `DICOM_DIMSE_CALLED_AE` | `MODALITY` | ASCII string | Required called AE title |
| `DICOM_DIMSE_ALLOWED_HOSTS` | `127.0.0.1` | CIDR/list | Source host allowlist |
| `DICOM_DIMSE_READ_TIMEOUT_SECS` | `20` | seconds (0.1–300) | Socket read timeout |
| `DICOM_DIMSE_WRITE_TIMEOUT_SECS` | `20` | seconds (0.1–300) | Socket write timeout |
| `DICOM_DIMSE_TLS_POLICY` | `require_tls` | enum | TLS enforcement policy |
| `DICOM_DIMSE_TRANSPORT_SECURITY` | `insecure` | enum | Explicit transport mode |
| `DICOM_DIMSE_AUTH_MODE` | `deny_all` | enum | Runtime authorization mode |
| `DICOM_DIMSE_HEALTH_BIND` | unset | host:port | Optional health/readiness endpoint bind |
| `DICOM_DIMSE_TLS_CERT_PATH` | unset | file path | TLS cert |
| `DICOM_DIMSE_TLS_KEY_PATH` | unset | file path | TLS key |
| `DICOM_DIMSE_TLS_CA_BUNDLE_PATH` | unset | file path | Optional CA bundle |
| `DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS` | `0` | seconds | Optional certificate rotation interval |

Startup guard expectations:
- startup validates bind address parse and WAL path writability before accepting traffic,
- fail closed when TLS policy/transport mismatch is configured,
- unsupported SOP-class and unsupported commands terminate associations with explicit typed rejections,
- health endpoints (`/healthz`, `/readyz`) are emitted only when `DICOM_DIMSE_HEALTH_BIND` is set.

## DIMSE startup preflight troubleshooting

Use this section to interpret startup failures for `dicom-dimse-service` before the process serves traffic.

Failure condition | Runtime check | Typical diagnostic | Resolution
---|---|---|---
Bind address parse failure | `parse_bind_arg` and TCP listener bind | `invalid bind address` | Set `DICOM_DIMSE_BIND` to `IP:port` (for example `127.0.0.1:11112`).
WAL parent path missing/unwritable | `preflight_storage` | `failed to initialize/prepare storage WAL` | Ensure parent directory exists or is writable and WAL target is a file path.
WAL rotation policy invalid | `DICOM_DIMSE_STORAGE_WAL_MAX_BYTES` or `DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES` invalid | `invalid input` | Correct values to unsigned integers (`> 0` for max bytes; rotated files `>= 1`).
Invalid timeout config | `DICOM_DIMSE_READ_TIMEOUT_SECS` / `DICOM_DIMSE_WRITE_TIMEOUT_SECS` parse error | `invalid input` | Provide integer seconds, not duration strings.
TLS material mismatch | `DICOM_DIMSE_TLS_POLICY`, `DICOM_DIMSE_TRANSPORT_SECURITY`, and TLS file paths | `TLS material is only valid when ...` or `must reference an existing file` | Use `DICOM_DIMSE_TRANSPORT_SECURITY=tls` only with certificate/key (and optional CA) file paths present, or clear TLS fields when transport is insecure.
Certificate rotation misuse | `DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS` with zero/unsupported mode | `must be greater than zero` | Set non-zero interval only with `tls` transport and valid cert material.
Authorization policy mismatch | `DICOM_DIMSE_AUTH_MODE` unsupported values | `unsupported DICOM_DIMSE_AUTH_MODE` | Use `deny_all` or `allow_all` only.
Association guard failures during runtime | Listener started; DIMSE negotiation stage | operation-level typed rejections | Set `DICOM_DIMSE_ROLE_*` and request `Called AE`, SOP-class support to supported capabilities.

Hard-fail pattern:

- Startup-preflight failures are fatal: process terminates before binding listeners.
- Recoverability is by correcting environment contract, clearing invalid overrides, and restarting.

## DIMSE runtime validation contract (TLS, AE title, hosts, bind)

- `DICOM_DIMSE_TLS_POLICY` is validated by `parse_tls_policy()` in `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`.
  - Unknown values fail startup with `unsupported DICOM_DIMSE_TLS_POLICY`.
  - Policy and transport are enforced in `DimseServerConfig` and association start.

- `DICOM_DIMSE_TRANSPORT_SECURITY` is validated by `parse_transport_security()` and `parse_tls_material()`.
  - Selecting `tls` requires `DICOM_DIMSE_TLS_CERT_PATH` and `DICOM_DIMSE_TLS_KEY_PATH`.
  - TLS material/env combinations are rejected when transport is not `tls` and on missing/invalid file paths.

- `DICOM_DIMSE_CALLED_AE` is parsed as optional runtime policy metadata and is attached to the server policy object.
  - The value is checked during association processing and contributes to auth/audit context.
  - Empty/whitespace values are treated as unset by parser normalization.

- `DICOM_DIMSE_ALLOWED_HOSTS` is validated by `parse_allowed_hosts()`.
  - The list is parsed as comma-separated IPv4/IPv6 literals and is rejected if any host token is invalid.

- `--bind` (or `DICOM_DIMSE_BIND`) is validated before startup by socket parse in `main()` and listener construction in `DicomServer`.
  - Invalid or non-bindable addresses fail at startup with `invalid bind address` or listener bind failure.

Runtime validation evidence is enforced before serving:

- Tests in `crates/dicom-dimse-service/src/lib.rs` for TLS policy and in `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs` for host/timeout helpers provide regression coverage.
- The startup preflight section above should be used first when errors mention TLS/auth/association contract fields.

## DIMSE operation audit lifecycle

DIMSE now emits a deterministic service-audit lifecycle for each operation:

- `phase: start` emitted as soon as an operation is identified and classified.
- `phase: response` emitted for each status response emitted by query/retrieve operations.
- `phase: completion` emitted on final success status after all response transmissions.
- `phase: failure` emitted for disabled commands, protocol/validation failures, explicit protocol denials, and transport failures while emitting responses.

Fields emitted for operation-audit records:

- `scope`, `operation`, `operation_id`, `message_id`, `phase`
- `status_code` where a DIMSE status is applicable
- `called_ae`, `calling_ae`, `sop_class_uid`, and optional `sop_instance_uid`
- `response_count` for multi-response operations
- `AuthzDecision` events still include decision/deny reason fields unchanged

## DIMSE association and transfer-syntax launch contract

The DIMSE service starts with a deterministic policy set that controls association behavior:

- Policy source: `workstation_default_association_policy()` in `crates/dicom-dimse-service/src/lib.rs`
- Runtime visibility: `dimse_association_contract()` in the same file.

Launch-time defaults used unless a policy override is applied:

- Bind interface: `DICOM_DIMSE_BIND` / `--bind` (default `127.0.0.1:11112`)
- Association reject policy: startup-fail on invalid bind or TLS/transport mismatch before listener start
- Transfer syntax policy:
  - `1.2.840.10008.1.2` (Implicit VR Little Endian)
  - `1.2.840.10008.1.2.1` (Explicit VR Little Endian)
- Abstract syntax base set exposed at startup:
  - `1.2.840.10008.1.1` (Verification)
  - `1.2.840.10008.5.1.4.1.1.2` (CT Image Storage)
  - `1.2.840.10008.5.1.4.1.1.4` (MR Image Storage)
  - `1.2.840.10008.5.1.4.1.1.7` (Secondary Capture)
  - `1.2.840.10008.5.1.4.1.1.7.2` (Multiframe SC – byte)
  - `1.2.840.10008.5.1.4.1.1.7.3` (Multiframe SC – word)
  - `1.2.840.10008.5.1.4.1.1.7.4` (Multiframe SC – true color)
  - `1.2.840.10008.5.1.4.1.1.128` (PET Image Storage)
  - `1.2.840.10008.5.1.4.1.1.1` (CR Image Storage)
  - `1.2.840.10008.5.1.4.1.1.1.1` (DX Presentation Image Storage)
  - `1.2.840.10008.5.1.4.1.2.2.1` (Study Root FIND) only when `dimse-c-find` is enabled
  - `1.2.840.10008.5.1.4.1.2.2.2` (Study Root MOVE) only when `dimse-c-move` is enabled
  - `1.2.840.10008.5.1.4.1.2.2.3` (Study Root GET) only when `dimse-c-get` is enabled

Negotiation behavior:

- Associations that propose unsupported abstract syntax or transfer syntax values are rejected by default policy handling.
- Unsupported role combinations produce typed association/operation rejections in the DIMSE state machine.

## Capability matrix by runtime

| Capability | Native | WASM | Backend services |
|---|---|---|---|
| CPU oracle pixel output | Yes | Yes | Yes |
| WebGPU presentation path | N/A | Feature-gated | N/A |
| MPR baseline API | Yes | CPU-only binding | Yes |
| SR authoring baseline API | Yes | API-boundary + prototype commit UI | Yes |
| Raw-mode parser fallback | Yes | Yes | Yes |
| DIMSE protocol (runtime service) | Runnable binary available (`dicom-dimse-service`) | Runnable binary available (`dicom-dimse-service`) | Optional profile-gated |

## Reference deployment

- Browser workstation (`viewer-wasm` web host) with deterministic runtime diagnostics and SR workflow prototype panel.
- Backend services: `dicom-web-server`, `dicom-workflow-server`.
- DIMSE: runnable service target available through `dicom-dimse-service`; enable only in `backend-services-with-dimse.<RELEASE_ID>.tar.gz` packaging.
- Optional Orthanc adapter via integration bridge script in `deploy/`.
- Default local endpoint topology is defined in `docs/40-Reference-Deployment-Topology.md`.

## Backend services launch checklists

### `backend-services` default launch checklist (packaged)

1. Build artifacts:
   - `cargo build -p dicom-web-server -p dicom-workflow-server`
   - `./tools/package_profiles.sh`
2. Start runtime services:
   - `cargo run -p dicom-web-server -- --bind 127.0.0.1:8080`
   - `cargo run -p dicom-workflow-server -- --bind 127.0.0.1:8082`
3. Validate health endpoints:
   - `curl -sS http://127.0.0.1:8080/healthz`
   - `curl -sS http://127.0.0.1:8082/healthz`
4. Validate package/profile claims:
   - `python3 tools/runtime_env_contract.py --repo-root . --check-docs`
   - `./tools/package_profiles.sh`

### DIMSE integration playbook (optional)

1. Enable the DIMSE profile target in your packaging/build profile.
2. Build and start the explicit service runtime:
   - `cargo build -p dicom-dimse-service`
   - `cargo run -p dicom-dimse-service -- --bind 127.0.0.1:11112`
3. Deploy the binary behind explicit policy and auth guardrails.
4. Point PACS/modality integrations to the new listener endpoint.
5. Publish updated deployment topology for mode with DIMSE runbooks.

### External orchestrator detection for DIMSE availability

- External orchestrators must detect DIMSE capability from profile artifacts before enabling DIMSE routes.
- Use artifact discovery + runtime probes together; do not assume DIMSE from image tags alone.

#### Profile artifact gate (install-time or boot-time)

Read `dist/profiles/<profile>.<RELEASE_ID>.tar.gz/capability.manifest.json` and check the `binaries` array:

```bash
ARTIFACT="dist/profiles/backend-services-with-dimse.${RELEASE_ID}.tar.gz"
if ! tar -tzf "${ARTIFACT}" | grep -q '^capability.manifest.json$'; then
  echo "missing manifest: ${ARTIFACT}"
  exit 1
fi

DIMSE_PROFILE_ENABLED="$(tar -xOf "${ARTIFACT}" capability.manifest.json | jq -e '.binaries | index("dicom-dimse-service")' >/dev/null && echo true || echo false)"

if [ "${DIMSE_PROFILE_ENABLED}" != "true" ]; then
  echo "DIMSE disabled: profile does not include dicom-dimse-service"
fi
```

Recommended packaging policy:
- `backend-services.<RELEASE_ID>.tar.gz` should never enable `DIMSE_PROFILE_ENABLED=true`.
- `backend-services-with-dimse.<RELEASE_ID>.tar.gz` must include `dicom-dimse-service` in both the manifest and `bin/dicom-dimse-service`.

#### Runtime probe contract (orchestrator runtime)

If `DIMSE_PROFILE_ENABLED=true`, configure orchestrator probes against the dedicated health bind:

```yaml
livenessProbe:
  httpGet:
    path: /healthz
    port: 18112
  initialDelaySeconds: 5
  periodSeconds: 10
  timeoutSeconds: 2
  failureThreshold: 6
readinessProbe:
  httpGet:
    path: /readyz
    port: 18112
  initialDelaySeconds: 3
  periodSeconds: 5
  timeoutSeconds: 2
  failureThreshold: 3
```

If `DIMSE_PROFILE_ENABLED=false`, do not route PACS/DICOMclient traffic to DIMSE and omit DIMSE probes to avoid false failure.

Operational checks for this mode:
- Start only when `DICOM_DIMSE_HEALTH_BIND` is set and reachable in the pod/container network.
- Route PACS traffic to port exposed by `DICOM_DIMSE_BIND` and verify that `/readyz` is HTTP 200 before declaring the service routable.
- Keep `/readyz` and `/healthz` scoped to the DIMSE runtime namespace/port (`DICOM_DIMSE_HEALTH_BIND`), not web/workflow ports.

#### Kubernetes/Compose manifest recommendations

- Gate DIMSE service generation in templates from the profile gate variable above.
- Keep a strict separation between manifest families:
  - `backend-services` profile family: no `dicom-dimse-service` stanza and no DIMSE probes.
  - `backend-services-with-dimse` profile family: include `dicom-dimse-service` + health/readiness probes.

### DIMSE partial protocol coverage and workflow/web fallback runbook

Use this runbook when peers expect DIMSE classes that are feature-gated, role-disabled, or policy-denied:

1. Establish service capability scope from packaging profile and command roles:
   - Confirm `dicom-dimse-service` appears in `capability.manifest.json` of the deployed artifact.
   - Confirm role policy in deployment:
     - `DICOM_DIMSE_ROLE_C_ECHO_ENABLED`
     - `DICOM_DIMSE_ROLE_C_STORE_ENABLED`
     - `DICOM_DIMSE_ROLE_C_FIND_ENABLED`
     - `DICOM_DIMSE_ROLE_C_MOVE_ENABLED`
     - `DICOM_DIMSE_ROLE_C_GET_ENABLED`
   - Confirm compile-time support in release profile (`dimse-c-find`, `dimse-c-move`, `dimse-c-get`).
2. Apply fallback policy by operation before enabling external modality routes:

| Requested DIMSE op | Runtime failure code (`0x0122`) indicates | Fallback path |
|---|---|---|
| C-ECHO | denied or unavailable | Keep DIMSE disabled until auth/policy is corrected; do not auto-switch transport. |
| C-STORE (supported SOP set) | denied or dataset policy violation | Return immediate failure to peer; require sender reconfiguration or explicit retry window. |
| C-FIND | command disabled or unimplemented in this build | Use `dicom-web-server` QIDO-RS pathways for equivalent study/query requests. |
| C-MOVE | command disabled/unavailable | Create WADO-RS pull jobs from workflow/web APIs using equivalent study-level filters. |
| C-GET | command disabled/unavailable | Use explicit web pull retrieval with policy checks and deterministic queue/state updates. |

3. Implement fallback in integration layer (not inside `dicom-dimse-service`):
   - Accept DIMSE denial as a deterministic signal and queue equivalent work in `dicom-workflow-server`.
   - Route retrieval and query requests to web APIs only after DIMSE denial is confirmed for the negotiated operation.
   - Keep modality transport retries idempotent while waiting for web-path retries.
4. Add operational guardrails:
   - Emit an explicit “CAPABILITY STATE” annotation in deployment notes and incident runbooks.
   - Document every partner integration assumption:
     - `C-FIND` fallback to QIDO,
     - `C-MOVE/C-GET` fallback to WADO pull,
     - unsupported store paths are non-fallback and require upstream reconfiguration.
5. Validate after topology changes:
   - Run `./tools/startup_preflight_check.sh` after startup policy/profile edits.
   - Validate workflow/web fallback paths with synthetic request paths from `docs/51-Integration-Cookbook.md`.
   - Verify DIMSE fallback behavior is still explicit in partner-facing topology docs.

### One-command bootstrap (profile-safe) playbook

1. Minimal DICOMweb + workflow + frontend profile:
   - `./tools/bootstrap_minimal_profile.sh`
2. DIMSE-enabled profile:
   - `./tools/bootstrap_dimse_profile.sh`
3. Preflight validation before first request:
   - `./tools/startup_preflight_check.sh`
4. Profile packaging and envelope-aware artifact capture:
   - `./tools/package_profiles.sh --include-dimse --validate`

## Deployment migration and playbook modes

### Migration note: environments currently assuming DIMSE is absent

- Legacy environments that deploy only `backend-services.<RELEASE_ID>.tar.gz` do not include
  `dicom-dimse-service` at runtime.
- To enable DIMSE, this is an explicit migration step:
  1. Replace or complement with `backend-services-with-dimse.<RELEASE_ID>.tar.gz`.
  2. Add `dicom-dimse-service` startup in the deployment manifest.
  3. Re-run startup preflight checks after rollout.
- For packaging workflows where DIMSE must not be omitted, run
  `./tools/package_profiles.sh --require-dimse` so omissions fail fast.

### Deployment mode playbook: no DIMSE

Use `./tools/bootstrap_minimal_profile.sh` for web/workflow-only operation.

- Health checks:
  - `curl -sS http://127.0.0.1:8080/healthz`
  - `curl -sS http://127.0.0.1:8082/healthz`
  - `curl -sS http://127.0.0.1:4173/`

### Deployment mode playbook: with DIMSE

Use `./tools/bootstrap_dimse_profile.sh` for web/workflow + DIMSE runtime integration.

- Health checks:
  - `curl -sS http://127.0.0.1:11112/healthz`
  - `curl -sS http://127.0.0.1:11112/readyz`
  - `curl -sS http://127.0.0.1:8080/healthz`
  - `curl -sS http://127.0.0.1:8082/healthz`
  - `curl -sS http://127.0.0.1:4173/`

## Extension contract

- Extensions are versioned by semver and must declare supported profile (`framework-core`, `workstation`, `backend-services`).
- Extension boundaries are data-only and must not bypass `Limits` or fail-closed invariants.
- Compatibility policy:
- Minor updates may add fields.
- Major updates may remove/rename fields.

## Onboarding quickstart (internal evaluators)

1. Run `cargo build` and `cargo test`.
2. Start backend services (`cargo run -p dicom-web-server` and `cargo run -p dicom-workflow-server`).
3. Start web host using `./tools/run_viewer_wasm_frontend.sh --port 4173`.
4. Open `http://127.0.0.1:4173` and confirm backend targets `http://127.0.0.1:8080` and `http://127.0.0.1:8082`.
5. Build preset artifacts using `./tools/wasm_build_presets.sh`.
6. Export runtime snapshot and confirm backend diagnostics block is present.
7. Record competitive position in `FEATURE_COMPARISON_COMPETITORS.md` and confirm remaining scope boundaries:
   - DICOMweb + workflow-first profile parity is baseline,
   - DIMSE is optional and separate from default `backend-services` packaging,
   - enterprise HIS/RIS connectors require explicit partner bridge work.

## Script and artifact references for profile claims

- Package manifest source: `tools/package_profiles.sh` (runtime mapping data in `tools/profile_matrix.json`).
- Per-profile SBOM artifacts: `sbom.<profile>.<RELEASE_ID>.json` generated under `dist/profiles`.
- Scripted runtime entrypoints:
  - `./tools/run_viewer_wasm_frontend.sh`
  - `./tools/local_demo_end_to_end.sh`
  - `./tools/package_profiles.sh`
- Validation scripts:
   - `python3 tools/runtime_env_contract.py`
   - `python3 tools/docs_capability_lint.py`

## Script and bootstrap source-of-truth

- Minimal bootstrap: `./tools/bootstrap_minimal_profile.sh`
- DIMSE bootstrap: `./tools/bootstrap_dimse_profile.sh`
- Compatibility entrypoint: `./tools/bootstrap_local_stack.sh`
- Startup preflight checks: `./tools/startup_preflight_check.sh`

## Contributor guide for profile additions

When adding a new profile, update:
- `tools/profile_matrix.json` (features, binaries, build commands, dependency flags, and default/doc claims)
- `docs/49-Runtime-Profile-Capability-Matrix.md`
- `deploy/docker-compose*.yml` and/or `deploy/k8s/*.yaml`
- `docs/40-Reference-Deployment-Topology.md`

Verification checklist:
- `python3 tools/package_profiles.sh --validate`
- `./tools/startup_preflight_check.sh`
- `./tools/release_preflight.py --release-id PROFILE-YYYYMMDD`

## Competitive positioning and scope boundaries

Baseline scope is intentionally focused on:
- Deterministic DICOMweb ingest/query/retrieve workflows,
- Deterministic workflow endpoints for MWL/MPPS/SR,
- Deterministic local visualization baselines.

Out-of-scope by default:
- Full HIS/RIS/enterprise orchestration,
- Native multi-tenant policy plane,
- Default PACS-like monolith semantics.

When a capability is out of scope, docs must state it explicitly and mark status as `Deferred` in status-bearing docs.

## Competitive review cadence template

- Frequency: monthly.
- Inputs: `COMPARE.md`, `FEATURE_COMPARISON_COMPETITORS.md`, open backlog deltas.
- Outputs:
- updated impact scores,
- changed profile boundaries,
- prioritized next-wave items.

## Differentiation statement

The product line prioritizes deterministic behavior, fail-closed parsing defaults, and clear CPU-oracle correctness boundaries across native and WASM targets.

## Source-of-truth rule

- Feature claims in this file are valid only when mirrored in:
  - `docs/12-API-Surface-and-Crate-Boundaries.md`
  - `docs/40-Reference-Deployment-Topology.md`
  - `tools/package_profiles.sh`

## Source-of-truth footer

- Runtime claim source: `docs/12-API-Surface-and-Crate-Boundaries.md` and `docs/40-Reference-Deployment-Topology.md`.
- Packaging claim source: `tools/package_profiles.sh` and `docs/14-Release-and-Versioning.md`.
- No profile claim is final unless covered by both a documented entry and a runnable/packaging artifact.
