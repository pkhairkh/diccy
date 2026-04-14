# API surface and crate boundaries

## Purpose

Normative:
- **REQ-API-201:** Public APIs **MUST** not cross crate boundaries in ways that violate the portability and security split defined here.

Verification:
- API review **MUST** confirm public types do not depend on platform-specific crates when they are declared portable.

This document specifies:

- the Cargo workspace layout and crate responsibilities,
- public types and traits that form the stable API surface,
- feature gating strategy for envelope expansion.

---

## Runtime artifact surface (normative source of truth)

- `dicom-web-server` and `dicom-workflow-server` are the only backend-service binaries included in the default backend artifact `dist/profiles/backend-services.<RELEASE_ID>.tar.gz`.
- `dicom-dimse-service` is a runnable DIMSE service binary and is not included in `backend-services.<RELEASE_ID>.tar.gz` unless explicitly included in `backend-services-with-dimse.<RELEASE_ID>.tar.gz` (via `./tools/package_profiles.sh --include-dimse`).
- `viewer-wasm` and `viewer-wgpu` are platform render integration crates for workstation workflows; current profile artifacts do not document guaranteed runnable binaries for these crates.
- `dicom-visualizer` is packaged for utility/workstation tooling with explicit artifact naming.

### Runtime posture note for viewer crates

Workstation visualization artifacts are in one of two classes:

- `viewer-wasm` — library+browser host integration path.
- `viewer-wgpu` — native/WASM render integration path.
- No explicit `viewer-wasm`/`viewer-wgpu` binaries are part of the current default `workstation` artifact contract; launch relies on front-end scripts and documented startup paths.

## Runtime environment contract (canonical per service)

Use this table when validating deployment scripts, runbook commands, and CI assertions:

- Global envelope gate: `DICOM_ENVELOPE_VERSION` is validated at startup by each backend runtime and unsupported values fail closed.
- Support diagnostics command: `--print-env-contract` prints a normalized runtime env-contract snapshot including effective envelope version.

| Service | Environment variable contract | Default / notes |
|---|---|---|
| `dicom-web-server` | `DICOM_WEB_ACCEPT_QUEUE_DEPTH`, `DICOM_WEB_AUTH_MODE`, `DICOM_WEB_BIND`, `DICOM_WEB_ENABLE_QIDO`, `DICOM_WEB_ENABLE_STOW`, `DICOM_WEB_ENABLE_WADO`, `DICOM_WEB_LOG_LEVEL`, `DICOM_WEB_QIDO_P95_LATENCY_MS`, `DICOM_WEB_QIDO_P99_LATENCY_MS`, `DICOM_WEB_STORAGE_WAL`, `DICOM_WEB_STORAGE_WAL_MAX_BYTES`, `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES`, `DICOM_WEB_STOW_P95_LATENCY_MS`, `DICOM_WEB_STOW_P99_LATENCY_MS`, `DICOM_WEB_TELEMETRY_ENABLED`, `DICOM_WEB_TELEMETRY_SAFE_SUBSET`, `DICOM_WEB_TEST_STORAGE_BYTES`, `DICOM_WEB_TEST_WORKERS`, `DICOM_WEB_TLS_POLICY`, `DICOM_WEB_TRANSPORT_SECURITY`, `DICOM_WEB_WADO_P95_LATENCY_MS`, `DICOM_WEB_WADO_P99_LATENCY_MS`, `DICOM_WEB_WORKERS` | `DICOM_WEB_BIND=127.0.0.1:8080`; WAL path `./state/dicom-web/storage.wal`; `DICOM_WEB_LOG_LEVEL=info`; TLS policy defaults to `require_tls`; feature toggles default to `true`; route latency budgets default to qido `250/500`, wado `400/800`, stow `1200/2400`; telemetry default `off` (`DICOM_WEB_TELEMETRY_ENABLED=false`, `DICOM_WEB_TELEMETRY_SAFE_SUBSET=true`); `DICOM_WEB_TEST_*` values are test-only |
| `dicom-workflow-server` | `DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD`, `DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT`, `DICOM_WORKFLOW_AUDIT_MAX_BYTES`, `DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_AUDIT_PATH`, `DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_AUTH_TOKEN`, `DICOM_WORKFLOW_AUTH_TOKEN_PATH`, `DICOM_WORKFLOW_BIND`, `DICOM_WORKFLOW_DENYLIST_PATHS`, `DICOM_WORKFLOW_FHIR_INGEST_ENABLED`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_FILE_DROP_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS`, `DICOM_WORKFLOW_HL7_MLLP_BIND`, `DICOM_WORKFLOW_HL7_MLLP_ENABLED`, `DICOM_WORKFLOW_LOG_LEVEL`, `DICOM_WORKFLOW_MPPS_STATE_PATH`, `DICOM_WORKFLOW_MUTATION_RATE_LIMIT`, `DICOM_WORKFLOW_QUERY_RATE_LIMIT`, `DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS`, `DICOM_WORKFLOW_SECRET_DIR`, `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES`, `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_SR_AUDIT_PATH`, `DICOM_WORKFLOW_SR_STATE_PATH`, `DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TEST_RATE_LIMIT`, `DICOM_WORKFLOW_TEST_ROTATIONS`, `DICOM_WORKFLOW_TLS_CERT_PATH`, `DICOM_WORKFLOW_TLS_KEY_PATH`, `DICOM_WORKFLOW_TRANSPORT_SECURITY`, `DICOM_WORKFLOW_UPLOAD_CAP_BYTES`, `DICOM_WORKFLOW_WEBHOOK_AUTH_SHARED_SECRET`, `DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY`, `DICOM_WORKFLOW_WORKLIST_STATE_PATH` | `DICOM_WORKFLOW_BIND=127.0.0.1:8082`; state paths under `./state/workflow/` by default; auth defaults to `deny_all`; `DICOM_WORKFLOW_HL7_MLLP_ENABLED=false` unless set; file-drop transport is enabled only when all three `DICOM_WORKFLOW_HL7_FILE_DROP_*` paths are configured; default poll interval is `3000ms`; `DICOM_WORKFLOW_LOG_LEVEL=info`; wildcard connector aliases are represented with `_STAR` suffix; `DICOM_WORKFLOW_TEST_*` values are test-only |
| `dicom-dimse-service` (optional profile-gated runtime binary) | `DICOM_DIMSE_ALLOWED_HOSTS`, `DICOM_DIMSE_AUTH_MODE`, `DICOM_DIMSE_BIND`, `DICOM_DIMSE_CALLED_AE`, `DICOM_DIMSE_HEALTH_BIND`, `DICOM_DIMSE_LOG_LEVEL`, `DICOM_DIMSE_MAX_CACHE_BYTES`, `DICOM_DIMSE_MAX_COMMAND_BYTES`, `DICOM_DIMSE_MAX_CONNECTIONS`, `DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_DATASET_ELEMENTS`, `DICOM_DIMSE_MAX_DECOMPRESSED_BYTES`, `DICOM_DIMSE_MAX_ELEMENT_VL_BYTES`, `DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE`, `DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES`, `DICOM_DIMSE_MAX_INPUT_BYTES`, `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS`, `DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS`, `DICOM_DIMSE_MAX_PDU_BYTES`, `DICOM_DIMSE_MAX_PDV_BYTES`, `DICOM_DIMSE_MAX_PIXELS_PER_FRAME`, `DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS`, `DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT`, `DICOM_DIMSE_MAX_SEQUENCE_DEPTH`, `DICOM_DIMSE_MAX_STRING_BYTES`, `DICOM_DIMSE_READ_TIMEOUT_SECS`, `DICOM_DIMSE_ROLE_C_ECHO_ENABLED`, `DICOM_DIMSE_ROLE_C_FIND_ENABLED`, `DICOM_DIMSE_ROLE_C_GET_ENABLED`, `DICOM_DIMSE_ROLE_C_MOVE_ENABLED`, `DICOM_DIMSE_ROLE_C_STORE_ENABLED`, `DICOM_DIMSE_STORAGE_WAL`, `DICOM_DIMSE_STORAGE_WAL_MAX_BYTES`, `DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES`, `DICOM_DIMSE_TELEMETRY_ENABLED`, `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET`, `DICOM_DIMSE_TEST_MAX_BYTES`, `DICOM_DIMSE_TEST_OPTIONAL_SECONDS`, `DICOM_DIMSE_TLS_CA_BUNDLE_PATH`, `DICOM_DIMSE_TLS_CERT_PATH`, `DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS`, `DICOM_DIMSE_TLS_KEY_PATH`, `DICOM_DIMSE_TLS_POLICY`, `DICOM_DIMSE_TRANSPORT_SECURITY`, `DICOM_DIMSE_WRITE_TIMEOUT_SECS` | `DICOM_DIMSE_BIND=127.0.0.1:11112`; fail-closed defaults (`require_tls`, `deny_all`, local WAL under `./state/dicom-dimse/storage.wal`); `DICOM_DIMSE_READ_TIMEOUT_SECS` and `DICOM_DIMSE_WRITE_TIMEOUT_SECS` default to `20`; `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS`/`DICOM_DIMSE_MAX_CONNECTIONS` defaults to `64`; `DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS` defaults to `64`; `DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT` defaults to `4096`; `DICOM_DIMSE_LOG_LEVEL=info`; telemetry default `off` (`DICOM_DIMSE_TELEMETRY_ENABLED=false`, `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET=true`); `DICOM_DIMSE_TEST_*` values are test-only; health checks on `/healthz` and `/readyz` when `DICOM_DIMSE_HEALTH_BIND` is set |
| `dicom-visualizer` | `RDVF_MANIFEST_INTEGRITY`, `RDVF_MANIFEST_HTML`, `RDVF_LIMIT_*` (project-local, when set via CLI/environment bridges in scripts) | `manifest.integrity.json` is mandatory in default `dicom-visualizer` output bundle; CLI limit args control non-DICOM raster behavior and input caps |

### `dicom-web-server` (env-contract)

- `DICOM_WEB_ACCEPT_QUEUE_DEPTH`, `DICOM_WEB_AUTH_MODE`, `DICOM_WEB_BIND`, `DICOM_WEB_ENABLE_QIDO`, `DICOM_WEB_ENABLE_STOW`, `DICOM_WEB_ENABLE_WADO`, `DICOM_WEB_LOG_LEVEL`, `DICOM_WEB_QIDO_P95_LATENCY_MS`, `DICOM_WEB_QIDO_P99_LATENCY_MS`, `DICOM_WEB_STORAGE_WAL`, `DICOM_WEB_STORAGE_WAL_MAX_BYTES`, `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES`, `DICOM_WEB_STOW_P95_LATENCY_MS`, `DICOM_WEB_STOW_P99_LATENCY_MS`, `DICOM_WEB_TELEMETRY_ENABLED`, `DICOM_WEB_TELEMETRY_SAFE_SUBSET`, `DICOM_WEB_TEST_STORAGE_BYTES`, `DICOM_WEB_TEST_WORKERS`, `DICOM_WEB_TLS_POLICY`, `DICOM_WEB_TRANSPORT_SECURITY`, `DICOM_WEB_WADO_P95_LATENCY_MS`, `DICOM_WEB_WADO_P99_LATENCY_MS`, `DICOM_WEB_WORKERS`

### `dicom-workflow-server` (env-contract)

- `DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD`, `DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT`, `DICOM_WORKFLOW_AUDIT_MAX_BYTES`, `DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_AUDIT_PATH`, `DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_AUTH_TOKEN`, `DICOM_WORKFLOW_AUTH_TOKEN_PATH`, `DICOM_WORKFLOW_BIND`, `DICOM_WORKFLOW_DENYLIST_PATHS`, `DICOM_WORKFLOW_FHIR_INGEST_ENABLED`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_FILE_DROP_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS`, `DICOM_WORKFLOW_HL7_MLLP_BIND`, `DICOM_WORKFLOW_HL7_MLLP_ENABLED`, `DICOM_WORKFLOW_LOG_LEVEL`, `DICOM_WORKFLOW_MPPS_STATE_PATH`, `DICOM_WORKFLOW_MUTATION_RATE_LIMIT`, `DICOM_WORKFLOW_QUERY_RATE_LIMIT`, `DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS`, `DICOM_WORKFLOW_SECRET_DIR`, `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES`, `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_SR_AUDIT_PATH`, `DICOM_WORKFLOW_SR_STATE_PATH`, `DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TEST_RATE_LIMIT`, `DICOM_WORKFLOW_TEST_ROTATIONS`, `DICOM_WORKFLOW_TLS_CERT_PATH`, `DICOM_WORKFLOW_TLS_KEY_PATH`, `DICOM_WORKFLOW_TRANSPORT_SECURITY`, `DICOM_WORKFLOW_UPLOAD_CAP_BYTES`, `DICOM_WORKFLOW_WEBHOOK_AUTH_SHARED_SECRET`, `DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY`, `DICOM_WORKFLOW_WORKLIST_STATE_PATH`

### `dicom-dimse-service` (env-contract)

- `DICOM_DIMSE_ALLOWED_HOSTS`
- `DICOM_DIMSE_AUTH_MODE`
- `DICOM_DIMSE_BIND`
- `DICOM_DIMSE_CALLED_AE`
- `DICOM_DIMSE_HEALTH_BIND`
- `DICOM_DIMSE_LOG_LEVEL`
- `DICOM_DIMSE_MAX_CACHE_BYTES`
- `DICOM_DIMSE_MAX_COMMAND_BYTES`
- `DICOM_DIMSE_MAX_CONNECTIONS`
- `DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES`
- `DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES`
- `DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES`
- `DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES`
- `DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES`
- `DICOM_DIMSE_MAX_DATASET_ELEMENTS`
- `DICOM_DIMSE_MAX_DECOMPRESSED_BYTES`
- `DICOM_DIMSE_MAX_ELEMENT_VL_BYTES`
- `DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE`
- `DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES`
- `DICOM_DIMSE_MAX_INPUT_BYTES`
- `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS`
- `DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS`
- `DICOM_DIMSE_MAX_PDU_BYTES`
- `DICOM_DIMSE_MAX_PDV_BYTES`
- `DICOM_DIMSE_MAX_PIXELS_PER_FRAME`
- `DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS`
- `DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT`
- `DICOM_DIMSE_MAX_SEQUENCE_DEPTH`
- `DICOM_DIMSE_MAX_STRING_BYTES`
- `DICOM_DIMSE_READ_TIMEOUT_SECS`
- `DICOM_DIMSE_ROLE_C_ECHO_ENABLED`
- `DICOM_DIMSE_ROLE_C_FIND_ENABLED`
- `DICOM_DIMSE_ROLE_C_GET_ENABLED`
- `DICOM_DIMSE_ROLE_C_MOVE_ENABLED`
- `DICOM_DIMSE_ROLE_C_STORE_ENABLED`
- `DICOM_DIMSE_STORAGE_WAL`
- `DICOM_DIMSE_STORAGE_WAL_MAX_BYTES`
- `DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES`
- `DICOM_DIMSE_TELEMETRY_ENABLED`
- `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET`
- `DICOM_DIMSE_TEST_MAX_BYTES`
- `DICOM_DIMSE_TEST_OPTIONAL_SECONDS`
- `DICOM_DIMSE_TLS_CA_BUNDLE_PATH`
- `DICOM_DIMSE_TLS_CERT_PATH`
- `DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS`
- `DICOM_DIMSE_TLS_KEY_PATH`
- `DICOM_DIMSE_TLS_POLICY`
- `DICOM_DIMSE_TRANSPORT_SECURITY`
- `DICOM_DIMSE_WRITE_TIMEOUT_SECS`

### Production-vs-test contract split

| Service | Production variables (runtime) | Test-only variables |
|---|---|---|
| `dicom-web-server` | `DICOM_WEB_ACCEPT_QUEUE_DEPTH`, `DICOM_WEB_AUTH_MODE`, `DICOM_WEB_BIND`, `DICOM_WEB_ENABLE_QIDO`, `DICOM_WEB_ENABLE_STOW`, `DICOM_WEB_ENABLE_WADO`, `DICOM_WEB_LOG_LEVEL`, `DICOM_WEB_QIDO_P95_LATENCY_MS`, `DICOM_WEB_QIDO_P99_LATENCY_MS`, `DICOM_WEB_STORAGE_WAL`, `DICOM_WEB_STORAGE_WAL_MAX_BYTES`, `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES`, `DICOM_WEB_STOW_P95_LATENCY_MS`, `DICOM_WEB_STOW_P99_LATENCY_MS`, `DICOM_WEB_TELEMETRY_ENABLED`, `DICOM_WEB_TELEMETRY_SAFE_SUBSET`, `DICOM_WEB_TLS_POLICY`, `DICOM_WEB_TRANSPORT_SECURITY`, `DICOM_WEB_WADO_P95_LATENCY_MS`, `DICOM_WEB_WADO_P99_LATENCY_MS`, `DICOM_WEB_WORKERS` | `DICOM_WEB_TEST_STORAGE_BYTES`, `DICOM_WEB_TEST_WORKERS` |
| `dicom-workflow-server` | `DICOM_WORKFLOW_ANOMALY_ALERT_THRESHOLD`, `DICOM_WORKFLOW_AUDIT_EXPORT_LIMIT`, `DICOM_WORKFLOW_AUDIT_MAX_BYTES`, `DICOM_WORKFLOW_AUDIT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_AUDIT_PATH`, `DICOM_WORKFLOW_AUTH_MODE`, `DICOM_WORKFLOW_AUTH_TOKEN`, `DICOM_WORKFLOW_AUTH_TOKEN_PATH`, `DICOM_WORKFLOW_BIND`, `DICOM_WORKFLOW_DENYLIST_PATHS`, `DICOM_WORKFLOW_FHIR_INGEST_ENABLED`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A1_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_AX_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_A_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_EAST_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_CORP_STAR`, `DICOM_WORKFLOW_HL7_CONNECTOR_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_VERSION_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MIN_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CONNECTOR_COMPAT_MAX_ENTERPRISE_HIS`, `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS`, `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS`, `DICOM_WORKFLOW_HL7_FILE_DROP_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_DONE_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_ERROR_DIR`, `DICOM_WORKFLOW_HL7_FILE_DROP_POLL_INTERVAL_MS`, `DICOM_WORKFLOW_HL7_MLLP_BIND`, `DICOM_WORKFLOW_HL7_MLLP_ENABLED`, `DICOM_WORKFLOW_LOG_LEVEL`, `DICOM_WORKFLOW_MPPS_STATE_PATH`, `DICOM_WORKFLOW_MUTATION_RATE_LIMIT`, `DICOM_WORKFLOW_QUERY_RATE_LIMIT`, `DICOM_WORKFLOW_RATE_LIMIT_WINDOW_MS`, `DICOM_WORKFLOW_SECRET_DIR`, `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES`, `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES`, `DICOM_WORKFLOW_SR_AUDIT_PATH`, `DICOM_WORKFLOW_SR_STATE_PATH`, `DICOM_WORKFLOW_TENANT_SUBSCRIPTION_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TENANT_TASK_QUOTA_TENANT_A`, `DICOM_WORKFLOW_TLS_CERT_PATH`, `DICOM_WORKFLOW_TLS_KEY_PATH`, `DICOM_WORKFLOW_TRANSPORT_SECURITY`, `DICOM_WORKFLOW_UPLOAD_CAP_BYTES`, `DICOM_WORKFLOW_WEBHOOK_AUTH_SHARED_SECRET`, `DICOM_WORKFLOW_WEBHOOK_AUTH_STRATEGY`, `DICOM_WORKFLOW_WORKLIST_STATE_PATH` | `DICOM_WORKFLOW_TEST_RATE_LIMIT`, `DICOM_WORKFLOW_TEST_ROTATIONS` |
| `dicom-dimse-service` (optional profile-gated runtime binary) | `DICOM_DIMSE_ALLOWED_HOSTS`, `DICOM_DIMSE_AUTH_MODE`, `DICOM_DIMSE_BIND`, `DICOM_DIMSE_CALLED_AE`, `DICOM_DIMSE_HEALTH_BIND`, `DICOM_DIMSE_LOG_LEVEL`, `DICOM_DIMSE_MAX_CACHE_BYTES`, `DICOM_DIMSE_MAX_COMMAND_BYTES`, `DICOM_DIMSE_MAX_CONNECTIONS`, `DICOM_DIMSE_MAX_C_ECHO_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_FIND_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_GET_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_MOVE_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_C_STORE_DATA_SET_BYTES`, `DICOM_DIMSE_MAX_DATASET_ELEMENTS`, `DICOM_DIMSE_MAX_DECOMPRESSED_BYTES`, `DICOM_DIMSE_MAX_ELEMENT_VL_BYTES`, `DICOM_DIMSE_MAX_FRAMES_PER_INSTANCE`, `DICOM_DIMSE_MAX_GPU_TEXTURE_BYTES`, `DICOM_DIMSE_MAX_INPUT_BYTES`, `DICOM_DIMSE_MAX_IN_FLIGHT_ASSOCIATIONS`, `DICOM_DIMSE_MAX_IN_FLIGHT_OPERATIONS`, `DICOM_DIMSE_MAX_PDU_BYTES`, `DICOM_DIMSE_MAX_PDV_BYTES`, `DICOM_DIMSE_MAX_PIXELS_PER_FRAME`, `DICOM_DIMSE_MAX_PRESENTATION_CONTEXTS`, `DICOM_DIMSE_MAX_QUERY_RESPONSE_COUNT`, `DICOM_DIMSE_MAX_SEQUENCE_DEPTH`, `DICOM_DIMSE_MAX_STRING_BYTES`, `DICOM_DIMSE_READ_TIMEOUT_SECS`, `DICOM_DIMSE_ROLE_C_ECHO_ENABLED`, `DICOM_DIMSE_ROLE_C_FIND_ENABLED`, `DICOM_DIMSE_ROLE_C_GET_ENABLED`, `DICOM_DIMSE_ROLE_C_MOVE_ENABLED`, `DICOM_DIMSE_ROLE_C_STORE_ENABLED`, `DICOM_DIMSE_STORAGE_WAL`, `DICOM_DIMSE_STORAGE_WAL_MAX_BYTES`, `DICOM_DIMSE_STORAGE_WAL_MAX_ROTATED_FILES`, `DICOM_DIMSE_TELEMETRY_ENABLED`, `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET`, `DICOM_DIMSE_TLS_CA_BUNDLE_PATH`, `DICOM_DIMSE_TLS_CERT_PATH`, `DICOM_DIMSE_TLS_CERT_ROTATION_INTERVAL_SECS`, `DICOM_DIMSE_TLS_KEY_PATH`, `DICOM_DIMSE_TLS_POLICY`, `DICOM_DIMSE_TRANSPORT_SECURITY`, `DICOM_DIMSE_WRITE_TIMEOUT_SECS` | `DICOM_DIMSE_TEST_MAX_BYTES`, `DICOM_DIMSE_TEST_OPTIONAL_SECONDS` |

### Workflow connector and rollout variable scope (conditional by profile)

- Connector aliases are **optional** and profile-gated by configuration completeness.
- `DICOM_WORKFLOW_HL7_CONNECTOR_*` and `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_*` variables only alter runtime behavior when a matching connector strategy is configured at startup.
- `DICOM_WORKFLOW_HL7_CONNECTOR_FEATURE_<ALIAS>` defaults to `true` when unset.
- `DICOM_WORKFLOW_HL7_CONNECTOR_ROLLOUT_<ALIAS>` defaults to `100` when unset.
- Wildcard variants (for example `*_CORP_STAR`, `*_CORP_A_STAR`, `*_CORP_AX_STAR`) are normalized at startup and should be documented whenever used by deployment profiles.

### Parser mapping and source of truth

- `dicom-web-server`: parser lives in `crates/dicom-web-server/src/main.rs`
  - `WebObservability::from_env("DICOM_WEB_", "dicom-web-server")` maps `DICOM_WEB_LOG_LEVEL`, `DICOM_WEB_TELEMETRY_ENABLED`, and `DICOM_WEB_TELEMETRY_SAFE_SUBSET`.
  - `InteropRuntimePolicy::from_env()` maps `DICOM_WEB_ENABLE_QIDO`, `DICOM_WEB_ENABLE_WADO`, and `DICOM_WEB_ENABLE_STOW`.
  - `RoutePerformanceBudgets::from_env()` maps `DICOM_WEB_QIDO_P95_LATENCY_MS`, `DICOM_WEB_QIDO_P99_LATENCY_MS`, `DICOM_WEB_WADO_P95_LATENCY_MS`, `DICOM_WEB_WADO_P99_LATENCY_MS`, `DICOM_WEB_STOW_P95_LATENCY_MS`, `DICOM_WEB_STOW_P99_LATENCY_MS`.
  - `parse_env_*` and transport/policy helpers map `DICOM_WEB_*` storage, queue, bind, auth, transport and test-only knobs.
- `dicom-workflow-server`: parser lives in `crates/dicom-workflow-server/src/main.rs`
  - `WorkflowObservability::from_env("DICOM_WORKFLOW_", "dicom-workflow-server")` maps `DICOM_WORKFLOW_LOG_LEVEL`.
  - `parse_hl7_transport_config()` and `parse_hl7_connector_config()` map all `DICOM_WORKFLOW_HL7_*` keys, including connector aliases and enterprise HIS entries.
  - `parse_limits()`, `parse_audit_rate_limits()`, and `parse_auth_config()` map numeric and auth-related limits and transport constraints.
  - `parse_env_*` helper tests map `DICOM_WORKFLOW_TEST_RATE_LIMIT` and `DICOM_WORKFLOW_TEST_ROTATIONS` for test harnesses.
- `dicom-dimse-service`: parser lives in `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`
  - `DimseObservability::from_env("DICOM_DIMSE_", "dicom-dimse-service")` maps `DICOM_DIMSE_LOG_LEVEL`, `DICOM_DIMSE_TELEMETRY_ENABLED`, and `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET`.
  - `parse_limits()`, `parse_network_limits()`, `parse_dimse_limits()`, and role parsers map the remaining `DICOM_DIMSE_*` production variables.
  - `parse_env_*` helper tests map `DICOM_DIMSE_TEST_MAX_BYTES` and `DICOM_DIMSE_TEST_OPTIONAL_SECONDS`.

Notes:
- `dicom-dimse-service` is a concrete runtime binary crate and appears in profiles only when DIMSE profile tokens are enabled.
- Environment variables are defined for the binary target; absent vars must default to strict, deterministic, fail-closed values.
- Add a corresponding entry in `tools/runtime_env_contract.py` when this binary is enabled in packaging workflows.
- This contract table is an input to `tools/runtime_env_contract.py`; update it before any packaging profile that toggles DIMSE env behavior.
- Link runnable artifact scripts:
  - `tools/package_profiles.sh`
  - `tools/run_viewer_wasm_frontend.sh`
  - `tools/local_demo_end_to_end.sh`

## Non-default binary inclusion rule

- Non-default binaries must be profile-gated and cannot be inferred from source docs alone.
- For explicit profile packaging, `dicom-dimse-service` is emitted only when `backend-services-with-dimse.<RELEASE_ID>.tar.gz` is requested.
- Manual direct runtime launch remains valid (`cargo run -p dicom-dimse-service`) but must be tracked as out-of-bundle unless paired with explicit `--include-dimse` packaging or equivalent manifest contract.

## Traceability anchors

- Runtime artifact source: [`tools/package_profiles.sh`](tools/package_profiles.sh) (`profile_binaries`, `profile_features`, `write_profile_artifact`).
- Runtime env contract extraction: [`tools/runtime_env_contract_lib.py`](tools/runtime_env_contract_lib.py) (`RUNTIME_ENV_CONTRACTS` entries and extraction helpers).
- Runtime artifact contract source-of-truth: [`docs/33-Productization-Profiles-and-Playbooks.md`](docs/33-Productization-Profiles-and-Playbooks.md).

Launch contract:

```bash
# default bind and AE policy
cargo run -p dicom-dimse-service -- --bind 127.0.0.1:11112

# explicit secure transport and AE constraints
DICOM_DIMSE_BIND=127.0.0.1:11112 \
  DICOM_DIMSE_CALLED_AE=MODALITY \
  DICOM_DIMSE_TLS_POLICY=require_tls \
  DICOM_DIMSE_TRANSPORT_SECURITY=tls \
  DICOM_DIMSE_READ_TIMEOUT_SECS=20 \
  DICOM_DIMSE_WRITE_TIMEOUT_SECS=20 \
  DICOM_DIMSE_ALLOWED_HOSTS=127.0.0.1,192.168.0.10 \
  DICOM_DIMSE_HEALTH_BIND=127.0.0.1:18112 \
  DICOM_DIMSE_AUTH_MODE=deny_all \
  cargo run -p dicom-dimse-service
```


## 1. Workspace crates (normative)

### `rdvf`

Responsibility:
- Public API facade for portable core types and feature capability queries.

Public API (illustrative, normative shape):
- `capabilities() -> Capabilities`
- `Config { limits, capabilities }`
- re-exports of `dicom-core` types (e.g., `Tag`, `Dataset`, `Error`)
- re-exports of portable pipeline/viewer types (e.g., `PixelPipeline`, `ViewerModel`)

Requirements:
- **REQ-API-205:** `rdvf` **MUST** be portable by default and **MUST NOT** depend on platform-specific crates unless behind explicit features.
- **REQ-API-206:** `rdvf` **MUST** expose a capability query and **MUST** forward feature flags to underlying crates so the query reflects build-time features.
- **REQ-API-207:** `rdvf::Config` **MUST** expose `Limits` and `Capabilities` together as the top-level configuration surface.
  - `Capabilities::raster_io` indicates optional non-DICOM raster decode support.

Verification:
- `cargo build -p rdvf`.
- `cargo test -p rdvf --all-features` to assert capability query behavior.
- Unit tests **MUST** assert `rdvf::Config::default()` uses `Limits::default()` and detected capabilities.

### `dicom-core`

Responsibility:
- Tag/VR definitions, dataset representation, element decoding primitives, and typed accessors.

Public API (illustrative, normative shape):
- `Tag` (u32 or (u16,u16) wrapper)
- `Vr` enum
- `Value` enum (scalars, strings, sequences, raw bytes)
- `Element { tag, vr, value }`
- `Dataset` with:
  - `fn get(tag) -> Option<&Element>`
  - typed getters like `get_str`, `get_i32`, `get_f64`, `get_uid`
  - `fn len() -> usize`
  - `fn is_empty() -> bool`
  - `fn elements() -> &[Element]`

Requirements:
- **REQ-API-202:** `dicom-core` **MUST** be WASM-compatible and **MUST NOT** use OS APIs (e.g., `std::fs`, `std::process`, native threading).
- **REQ-API-203:** `dicom-core` **SHOULD** be `no_std`-compatible; if it is not, the blocking dependency **MUST** be documented in crate-level docs.

Verification:
- `cargo build --target wasm32-unknown-unknown -p dicom-core`.
- Doc check **MUST** note any `no_std` blockers when applicable.

### `dicom-io`

Responsibility:
- P10 parsing, filesystem/directory scanning (native), streaming reads, and limit enforcement at input boundary.

Public API:
- `DicomSource` trait (bytes provider)
- `P10Reader`:
  - `fn read_meta(&mut self) -> Result<FileMeta>`
  - `fn read_dataset(&mut self) -> Result<Dataset>`
- `parse_dataset_bytes` (decode dataset bytes with explicit transfer syntax)

Requirements:
- **REQ-API-210:** All read operations **MUST** honor configured limits and return `LimitExceeded` errors on violations.

Verification:
- Unit tests **MUST** assert limit violations return `LimitExceeded` with correct `limit_name`.

### `dicom-net`

Responsibility:
- UL PDU parsing, association negotiation, and association state machine sequencing (byte-level; no socket I/O).

Public API:
- `NetworkLimits`
- `parse_pdu`, `parse_pdu_stream`
- `AssociationRequest`, `AssociationAccept`, `AssociationPolicy`
- `AssociationStateMachine`, `AssociationState`, `AssociationRole`, `AssociationEvent`, `AssociationPduType`
- `accept_association`

Verification:
- Unit tests cover PDU parsing, association parsing, and limit enforcement.

### `dicom-dimse`

Responsibility:
- DIMSE command parsing for Verification (C-ECHO), Storage (C-STORE), and Query/Retrieve commands (C-FIND/C-MOVE/C-GET).

Public API:
- `DimseLimits`
- `DimseMessage`
- `parse_command_set`, `parse_command_pdv`
- `build_c_echo_request`, `build_c_echo_response`
- `build_c_store_request`, `build_c_store_response`
- `build_c_find_request`, `build_c_find_response`
- `build_c_move_request`, `build_c_move_response`
- `build_c_get_request`, `build_c_get_response`

Verification:
- Unit tests cover C-ECHO/C-STORE/C-FIND/C-MOVE/C-GET request parsing and unsupported SOP rejection.

### `dicom-dimse-service`

Responsibility:
- Networked DIMSE SCU/SCP harness, association lifecycle, and PDV assembly.
- Packaged deployment note: this crate is available as a runnable service binary and is not included by default; it is explicit profile-driven via profile scripts.


Public API:
- `DimseServer`, `DimseClient`
- `DimseServerConfig`, `DimseClientConfig`
- `TransportSecurity`, `TlsPolicy`
- `DimseService` trait for C-ECHO/C-STORE handlers plus Query/Retrieve response-sequence handlers
- `StorageBackedDimseService`
- Query/Retrieve handlers and client methods: `on_c_find` / `c_find`, `on_c_move` / `c_move`, `on_c_get` / `c_get`.
- `query_matches_from_dimse` helper for DIMSE identifier queries.
- Environment configuration:
  - `DICOM_DIMSE_LOG_LEVEL` controls runtime log verbosity (`off`, `error`, `warn`, `info`, `debug`, `trace`; default `info`).
  - `DICOM_DIMSE_TELEMETRY_ENABLED` controls whether telemetry emission is enabled (`true`/`false`; default `false`).
  - `DICOM_DIMSE_TELEMETRY_SAFE_SUBSET` controls redacted telemetry output (`true` default; set `false` only for explicit local debugging).
- Runtime defaults:
  - `DimseServerConfig::default()` is fail-closed: `TlsPolicy::RequireTls`, `TransportSecurity::Insecure`, and `DimseAuthConfig::deny_all()`.
  - Runtime operators must explicitly opt in to insecure transport for local/test-only harnesses.

Verification:
- Unit tests cover C-ECHO round-trip, storage-backed C-STORE persistence behavior, limit enforcement, deterministic C-FIND/C-MOVE/C-GET pending/final response sequencing, and default TLS/auth fail-closed behavior.

### `dicom-web`

Responsibility:
- DICOMweb request parsing/routing plus in-process service runtime execution for QIDO-RS/WADO-RS/STOW-RS over storage/query/auth/audit integrations; no socket/network I/O.

Public API:
- `HttpMethod`
- `TransportSecurity`, `TlsPolicy`, `ThrottleDecision`, `WebPolicy`
- `DicomWebServiceConfig`, `DicomWebService`, `DicomWebResponse`
- `QueryParam`, `Header`, `WebRequest`
- `DicomWebContentType`, `DicomWebRequest`
- `parse_http_request`, `parse_dicomweb_request`
- `DicomWebService::handle_http`, `DicomWebService::handle_request`
- `DicomWebService::route_request`, `DicomWebService::execute_routed`
- `qido_query_matches` (QIDO-RS matching helper)

Verification:
- Unit tests cover method parsing, URI/query limits, TLS policy enforcement, throttling enforcement, UID validation, content-type parsing, and body limit enforcement.
- Integration-style tests cover QIDO/WADO/STOW runtime execution with auth allow/deny and audit emission.
- Route-level policy and limit error contract (runtime behavior):
  - `limit_exceeded` (`LimitExceeded`) => HTTP 413 payload errors when request/query/path/body fields exceed `Limits` bounds.
  - `auth_denied` => HTTP 403 when auth policy or transport policy rejects a request (`DICOM_WEB_AUTH_MODE`, `DICOM_WEB_TLS_POLICY`).
  - `validation_or_decode` => HTTP 400 for malformed route/path parsing, unsupported SOP/TS, and schema decode failures.
  - `not_found` => HTTP 404 when requested study/series/instance/context is unavailable for the active operation.
  - `integrity` => HTTP 409 for conflict outcomes (e.g., duplicate idempotency contexts or optimistic update mismatch when surfaced).
  - `io_or_internal` => HTTP 500 for storage/query/auth pipeline failures after input validation.

### `dicom-web-server`

Responsibility:
- Packaged TCP/HTTP DICOMweb runtime that hosts `dicom-web::DicomWebService` with durable WAL-backed storage for QIDO-RS/WADO-RS/STOW-RS request handling.
- Runtime defaults are fail-closed: TLS required, deny-by-default authorization, and deterministic durable storage initialization.

Public API:
- Binary entrypoint: `dicom-web-server`.
- Environment configuration:
  - `DICOM_WEB_BIND` for listener bind address (default `127.0.0.1:8080`).
  - `DICOM_WEB_ENABLE_QIDO` enables/disables QIDO-RS read routes when `true`/`false` (default `true`).
  - `DICOM_WEB_ENABLE_WADO` enables/disables WADO-RS read routes when `true`/`false` (default `true`).
  - `DICOM_WEB_ENABLE_STOW` enables/disables STOW-RS ingest routes when `true`/`false` (default `true`).
  - `DICOM_WEB_STORAGE_WAL` for durable WAL path (default `./state/dicom-web/storage.wal`).
  - `DICOM_WEB_STORAGE_WAL_MAX_BYTES` for WAL size threshold before rotation (default `134217728`).
  - `DICOM_WEB_STORAGE_WAL_MAX_ROTATED_FILES` for retained rotated WAL files (default `3`).
- `DICOM_WEB_WORKERS` for deterministic worker pool sizing (default: available parallelism, minimum `1`).
- `DICOM_WEB_ACCEPT_QUEUE_DEPTH` for accept-queue depth (default: `workers * 8`, minimum `1`).
- `DICOM_WEB_TLS_POLICY` (`require_tls` default; `allow_insecure` explicit override).
- `DICOM_WEB_AUTH_MODE` (`deny_all` default; `allow_all` explicit override).
- `DICOM_WEB_TRANSPORT_SECURITY` (`insecure` default; `tls` explicit secure-boundary declaration).
- `DICOM_WEB_LOG_LEVEL` controls runtime log verbosity (`off`, `error`, `warn`, `info`, `debug`, `trace`; default `info`).
- `DICOM_WEB_TELEMETRY_ENABLED` controls whether telemetry emission is enabled (`true`/`false`; default `false`).
- `DICOM_WEB_TELEMETRY_SAFE_SUBSET` controls redacted telemetry output (`true` default; set `false` only for explicit local debugging).
- Route latency budgets (rolling sample windows, values in milliseconds):
  - `DICOM_WEB_QIDO_P95_LATENCY_MS` (default `250`) and `DICOM_WEB_QIDO_P99_LATENCY_MS` (default `500`).
  - `DICOM_WEB_WADO_P95_LATENCY_MS` (default `400`) and `DICOM_WEB_WADO_P99_LATENCY_MS` (default `800`).
  - `DICOM_WEB_STOW_P95_LATENCY_MS` (default `1200`) and `DICOM_WEB_STOW_P99_LATENCY_MS` (default `2400`).
  - Route windows retain the most recent `256` samples to compute rolling p95/p99.
- Startup preflight validates that WAL paths are writable and fail-fast on invalid file targets.
  - Oversized WAL files rotate before startup to enforce bounded disk growth.
  - Deployment baseline template: `deploy/dicom-web-server.env.example`.
  - Backpressure behavior is bounded and deterministic: when the accept queue is full, runtime logs queue-state guidance and blocks on enqueue rather than dropping accepted sockets.

Verification:
- Workspace build checks compile the server runtime with the same fail-closed request parsing and service execution paths used by `dicom-web`.
- Runtime tests cover WAL preflight path validation, bounded rotation behavior, and restart recovery continuity.

- Workflow/web route contract error mapping and policy behavior:
  - `auth_denied` => HTTP 403 when deny-all/token auth, missing transport policy, or request-policy mismatch is configured.
  - `DVF.WORKFLOW.SR.AUTH_DENIED` => HTTP 403 for SR role/provenance policy failures (`writer`, `admin`, or `operator` role required for mutations; `x-sr-role` and `x-auth-role` are both accepted for compatibility).
  - `DVF.WORKFLOW.SR.NOT_FOUND`, `DVF.WORKFLOW.MPPS.NOT_FOUND` => HTTP 404 for missing workflow document/MPPS lookup.
  - `DVF.WORKFLOW.SR.VERSION_CONFLICT`, `DVF.WORKFLOW.SR.ALREADY_EXISTS`, `DVF.WORKFLOW.SR.IDEMPOTENCY_CONFLICT`, `DVF.WORKFLOW.SR.INVALID_TRANSITION`, `DVF.WORKFLOW.MPPS.IDEMPOTENCY_CONFLICT` => HTTP 409.
- Optional `x-request-id` request headers are accepted on all workflow routes; workflow writes persist a request-id hash in `sr.audit.log` to support tracing across request ingestion and audit storage.
- `LimitExceeded` (`max_string_bytes`, `max_workflow_pair_count`) => HTTP 413 for oversized path/query/header/form values, query/body pair count, and derived pagination windows.
- `DecodeError`/validation-related variants (`MissingRequiredTag`, `InvalidTagValue`, unsupported transfer/syntax) => HTTP 400.
- `IntegrityError` => HTTP 409 for internal conflict/integrity violations.
- `IoError`/`InternalError` => HTTP 500.
- `GET /workflow/audit?verify=true|1` returns additional signature verification output (`verification.ok`, `verification.failures`) without changing endpoint semantics.

### `dicom-visualizer`

Responsibility:
- Packaged multi-file visualization CLI that decodes DICOM Part 10 inputs and exports deterministic first-frame PNG previews for folder-level review.
- Produces an auditable output bundle (`manifest.csv` + optional `index.html` + `manifest.integrity.json`) without changing source datasets.

Public API:
- Binary entrypoint: `dicom-visualizer`.
- CLI usage:
  - `dicom-visualizer <input_dir> <output_dir> [--limit N] [--no-recursive] [--no-index]`
- Outputs:
  - `*.png` preview files (first frame per decoded instance),
  - `manifest.csv` export status rows with structured fail-closed error details (code/tag/stage),
  - `index.html` gallery unless `--no-index` is set,
  - `manifest.integrity.json` deterministic integrity sidecar.
    - Includes `sha256` hashes and non-secret metadata (`application_version`, `build_id`, `envelope_version`).
    - `--no-index` disables only gallery generation; integrity sidecar output remains mandatory.

Verification:
- Unit tests cover frame conversion edge cases and deterministic output naming.
- Manual smoke runs over representative folders verify manifest success/error reporting and gallery generation.

### `dicom-workflow-server`

Responsibility:
- Packaged TCP/HTTP workflow runtime that hosts durable MWL/MPPS/SR services backed by `dicom-worklist`, `dicom-mpps`, and `SrWorkflowStore` snapshot/audit persistence.
- Runtime defaults are fail-closed: deny-all authorization and insecure-transport rejection unless transport security is explicitly declared.

Public API:
- Binary entrypoint: `dicom-workflow-server`.
- Environment configuration:
  - `DICOM_WORKFLOW_BIND` for listener bind address (default `127.0.0.1:8082`).
  - `DICOM_WORKFLOW_WORKLIST_STATE_PATH` for durable MWL snapshot path (default `./state/workflow/worklist.snapshot`).
  - `DICOM_WORKFLOW_MPPS_STATE_PATH` for durable MPPS snapshot path (default `./state/workflow/mpps.snapshot`).
  - `DICOM_WORKFLOW_SR_STATE_PATH` for durable SR snapshot path (default `./state/workflow/sr.snapshot`).
  - `DICOM_WORKFLOW_SR_AUDIT_PATH` for durable SR audit log path (default `./state/workflow/sr.audit.log`).
  - `DICOM_WORKFLOW_SNAPSHOT_MAX_BYTES` for snapshot size threshold before rotation (default `33554432`).
  - `DICOM_WORKFLOW_SNAPSHOT_MAX_ROTATED_FILES` for retained rotated snapshot files (default `2`).
  - `DICOM_WORKFLOW_AUTH_MODE` (`deny_all` default; `allow_all` or `token` explicit override).
  - `DICOM_WORKFLOW_AUTH_TOKEN` required when `DICOM_WORKFLOW_AUTH_MODE=token`.
  - `DICOM_WORKFLOW_LOG_LEVEL` controls runtime log verbosity (`off`, `error`, `warn`, `info`, `debug`, `trace`; default `info`).
  - `DICOM_WORKFLOW_TRANSPORT_SECURITY` (`insecure` default; `tls` required to accept requests by default policy).
  - Startup preflight validates persistence paths are writable and fail-fast on invalid file targets.
  - Oversized snapshots rotate before startup to enforce bounded disk growth.
  - Deployment baseline template: `deploy/dicom-workflow-server.env.example`.
<!-- endpoint-matrix:auto:start -->
# Endpoint matrix (code-generated)

Generated: 2026-02-25T16:46:51Z (UTC) for RC-2026.02.25.

## dicom-web routes

| Method | Path | Operation | Required feature | State | Content-Type |
|---|---|---|---|---|---|
| GET | /instances | QIDO all instances | qido | implemented | N/A |
| HEAD | /instances | QIDO all instances | qido | implemented | N/A |
| GET | /series | QIDO all series | qido | implemented | N/A |
| HEAD | /series | QIDO all series | qido | implemented | N/A |
| GET | /studies | QIDO studies | qido | implemented | N/A |
| HEAD | /studies | QIDO studies | qido | implemented | N/A |
| POST | /studies | STOW studies | stow | implemented | application/dicom, application/dicom+xml, application/dicom+json, or multipart/related |
| DELETE | /studies/{StudyUID} | Delete study | delete | blocked | N/A |
| GET | /studies/{StudyUID} | WADO study retrieve | wado | implemented | multipart/related; type="application/dicom" |
| HEAD | /studies/{StudyUID} | WADO study retrieve | wado | implemented | multipart/related; type="application/dicom" |
| POST | /studies/{StudyUID} | STOW scoped by study UID | stow | implemented | application/dicom, application/dicom+xml, application/dicom+json, or multipart/related |
| GET | /studies/{StudyUID}/instances | QIDO instances by study | qido | implemented | N/A |
| HEAD | /studies/{StudyUID}/instances | QIDO instances by study | qido | implemented | N/A |
| GET | /studies/{StudyUID}/metadata | WADO study metadata | wado | implemented | application/dicom+json |
| GET | /studies/{StudyUID}/series | QIDO series by study | qido | implemented | N/A |
| HEAD | /studies/{StudyUID}/series | QIDO series by study | qido | implemented | N/A |
| DELETE | /studies/{StudyUID}/series/{SeriesUID} | Delete series | delete | blocked | N/A |
| GET | /studies/{StudyUID}/series/{SeriesUID} | WADO series retrieve | wado | implemented | multipart/related; type="application/dicom" |
| HEAD | /studies/{StudyUID}/series/{SeriesUID} | WADO series retrieve | wado | implemented | multipart/related; type="application/dicom" |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances | QIDO instances by study/series | qido | implemented | N/A |
| HEAD | /studies/{StudyUID}/series/{SeriesUID}/instances | QIDO instances by study/series | qido | implemented | N/A |
| DELETE | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID} | Delete instance | delete | blocked | N/A |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID} | WADO instance retrieve | wado | implemented | application/dicom |
| HEAD | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID} | WADO instance retrieve | wado | implemented | application/dicom |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata | WADO bulkdata retrieve | wado | implemented | application/octet-stream |
| HEAD | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/bulkdata | WADO bulkdata retrieve | wado | implemented | application/octet-stream |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber} | WADO frame retrieve | wado | implemented | application/octet-stream |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/frames/{FrameNumber}/rendered | WADO rendered frame retrieve | wado | implemented | image/png or image/jpeg |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/metadata | WADO instance metadata | wado | implemented | application/dicom+json |
| GET | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered | WADO rendered instance retrieve | wado | implemented | image/png or image/jpeg |
| HEAD | /studies/{StudyUID}/series/{SeriesUID}/instances/{InstanceUID}/rendered | WADO rendered instance retrieve | wado | implemented | image/png or image/jpeg |
| GET | /studies/{StudyUID}/series/{SeriesUID}/metadata | WADO series metadata | wado | implemented | application/dicom+json |
| GET | /wado | WADO-URI compatibility retrieve | wado | implemented | application/dicom |
| HEAD | /wado | WADO-URI compatibility retrieve | wado | implemented | application/dicom |

## dicom-workflow-server routes

| Method | Path | Operation | Requires writer role | Requires idempotency-key | Content-Type |
|---|---|---|---:|---:|---|
| GET|HEAD | /healthz | server health check | no | no | N/A |
| GET|HEAD | /interop/connectors/capabilities | discover connector capabilities | yes | no | N/A |
| GET|HEAD | /interop/connectors/features | read hl7 connector feature flags and rollout | yes | no | N/A |
| GET|HEAD | /interop/connectors/health | read connector health checks | yes | no | N/A |
| GET|HEAD | /interop/connectors/rollout | list connector rollout percentages | yes | no | N/A |
| POST | /interop/connectors/rollout | update connector rollout percentage | yes | no | application/x-www-form-urlencoded |
| GET|HEAD | /interop/connectors/status | read hl7 connector status dashboard | yes | no | N/A |
| POST | /interop/fhir | ingest fhir event | yes | no | application/x-www-form-urlencoded |
| POST | /interop/hl7 | ingest interoperability event | yes | no | application/x-www-form-urlencoded |
| GET|HEAD | /interop/hl7/failures | list hl7 failures | yes | no | N/A |
| GET|HEAD | /interop/hl7/ups-correlation | list hl7 ups correlations | yes | no | N/A |
| POST | /interop/hl7/ups-correlation | upsert hl7 ups correlation | yes | no | application/x-www-form-urlencoded |
| GET|HEAD | /interop/ian | list ian events | yes | no | N/A |
| POST | /interop/ian | ingest ian event | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /interop/reconciliation/jobs | list study reconciliation jobs | yes | no | N/A |
| POST | /interop/reconciliation/jobs | create study reconciliation job | yes | no | application/x-www-form-urlencoded |
| POST | /interop/reconciliation/jobs/{job_id}/run | run study reconciliation job | yes | yes | N/A |
| GET|HEAD | /interop/storage-commitment/status | list storage commitment status | yes | no | N/A |
| POST | /interop/storage-commitment/status | ingest storage commitment status | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /interop/storage-commitment/status/{task_id} | read storage commitment transaction status | yes | no | N/A |
| GET|HEAD | /interop/subscriptions | list hl7 subscriptions | yes | no | N/A |
| POST | /interop/subscriptions | create hl7 subscription | yes | no | application/x-www-form-urlencoded |
| GET|HEAD | /mpps/updates | query mpps update snapshots | no | no | N/A |
| POST | /mpps/updates | ingest mpps update | yes | no | N/A |
| GET|HEAD | /mpps/updates/{sop_instance_uid} | get latest mpps update by sop uid | no | no | N/A |
| GET|HEAD | /mpps/updates/{sop_instance_uid}/status | read mpps update status | no | no | N/A |
| POST | /mpps/updates/{sop_instance_uid}/status | update mpps status | yes | no | N/A |
| GET|HEAD | /readyz | server readiness check | no | no | N/A |
| GET|HEAD | /sr/documents | list sr documents | no | no | N/A |
| POST | /sr/documents | create sr document | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /sr/documents/{sop_instance_uid} | get sr document | no | no | N/A |
| POST | /sr/documents/{sop_instance_uid}/cancel | cancel sr document | yes | yes | N/A |
| POST | /sr/documents/{sop_instance_uid}/commit | commit sr document | yes | yes | N/A |
| POST | /sr/documents/{sop_instance_uid}/finalize | finalize sr document | yes | yes | N/A |
| GET|HEAD | /sr/documents/{sop_instance_uid}/history | read sr lifecycle history | no | no | N/A |
| POST | /sr/documents/{sop_instance_uid}/review | review sr document | yes | yes | N/A |
| POST | /sr/documents/{sop_instance_uid}/updates | append sr update | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /workflow/audit | read workflow audit trail | yes | no | N/A |
| GET|HEAD | /workflow/metrics | read tenant-aware workflow metrics | yes | no | N/A |
| GET|HEAD | /workflow/policy/quotas | read tenant quota policy overrides | yes | no | N/A |
| GET|HEAD | /workflow/policy/quotas/snapshot | export tenant quota policy snapshot | yes | no | N/A |
| GET|HEAD | /workflow/tasks | list tasks | no | no | N/A |
| POST | /workflow/tasks | create task | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /workflow/tasks/{task_id} | get task by id | no | no | N/A |
| POST | /workflow/tasks/{task_id}/cancel | cancel task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/commit | commit task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/complete | complete task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/pause | pause task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/resume | resume task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/review | review task | yes | no | N/A |
| POST | /workflow/tasks/{task_id}/start | start task | yes | no | N/A |
| GET|HEAD | /workflow/workitems | search ups workitems | no | no | N/A |
| POST | /workflow/workitems | create ups workitem | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /workflow/workitems/{task_id} | retrieve ups workitem | no | no | N/A |
| POST | /workflow/workitems/{task_id} | update ups workitem | yes | yes | application/x-www-form-urlencoded |
| POST | /workflow/workitems/{task_id}/cancel | cancel ups workitem | yes | yes | N/A |
| GET|HEAD | /workflow/workitems/{task_id}/state | read ups workitem state | no | no | N/A |
| POST | /workflow/workitems/{task_id}/state | change ups workitem state | yes | yes | application/x-www-form-urlencoded |
| GET|HEAD | /worklist/items | query worklist | no | no | N/A |
| POST | /worklist/items | upsert worklist item | yes | no | N/A |

<!-- endpoint-matrix:auto:end -->


Source-of-truth note:
- The workflow route matrix is generated from `tools/export_endpoint_matrix.py` examples and must match handler-level registration in `crates/dicom-workflow-server/src/main.rs`.

- Route policy and limit error semantics:
  - Transport policy (`DICOM_WORKFLOW_TRANSPORT_SECURITY`):
    - `insecure` defaults to rejecting requests not satisfying secure-mode checks with 403 `auth_denied`.
    - `tls` enables accepted policy for secured transport.
  - Auth mode (`DICOM_WORKFLOW_AUTH_MODE`):
    - `deny_all` rejects non-policy requests with 403 `auth_denied`.
    - `token` requires `DICOM_WORKFLOW_AUTH_TOKEN`.
  - Tenant context (`x-tenant`, `x-workflow-tenant`, `x-tenant-id`, `x-workflow-tenant-id`):
    - Missing tenant headers default to `tenant-default`.
    - Workflow queries and mutation handlers enforce tenant ownership, returning `DVF.WORKFLOW.SR.AUTH_DENIED` when caller tenant and resource tenant differ.
  - Mutation policy:
    - Workflow mutation routes and `/workflow/audit` require principal/role policy (`writer`, `admin`, or `operator`); mutation routes also require idempotency headers; failures return `DVF.WORKFLOW.SR.AUTH_DENIED`.
  - Input limits:
    - Oversized request line/query/form values fail closed as `LimitExceeded` and return HTTP 413.
    - Missing required SR query/body fields return 400 decode-like errors.

Verification:
- Unit tests cover form decoding, ASCII enforcement, and fail-closed authorization defaults.
- Workspace build/test checks compile and execute the runtime with the same deterministic store semantics as `dicom-worklist` and `dicom-mpps`.
- Runtime tests cover snapshot preflight path validation, bounded rotation behavior, and restart recovery continuity for MWL/MPPS/SR state.
- Integration tests cover deterministic SR create/update/retrieve route behavior, idempotency handling, and version-conflict fail-closed semantics.

Reference:
- `docs/39-SR-Workflow-Architecture.md`

### `dicom-index`

Responsibility:
- Deterministic metadata indexing for Study/Series/Instance ordering; no persistence or network I/O.

Public API:
- `IndexedInstance`, `InsertOutcome`, `Index`
- `Index::location_for_sop`
- `extract_indexed_instance`

Verification:
- Unit tests cover required UID extraction, deterministic ordering, UID conflict handling, and index limit enforcement.

### `dicom-storage`

Responsibility:
- Deterministic ingestion with canonical hashing, deduplication, and write-ahead log replay, including durable WAL mode via `Storage::open`; no network I/O.

Public API:
- `Storage`, `WriteAheadLog`, `WalEntry`, `IngestOutcome`
- `Storage::open`, `Storage::datasets`, `Storage::bytes_for_hash`, `Storage::instance_bytes`

Verification:
- Unit tests cover deduplication, UID conflict failures, write-ahead log replay, concurrent ingest/replay consistency, and cache/size limit enforcement.

### `dicom-query`

Responsibility:
- Deterministic query matching for Query/Retrieve identifier datasets with supported UID and text filters; no network I/O.

Public API:
- `QueryLevel`, `QueryKey`, `Query`, `QueryMatch`
- `query`
- `query_from_identifier`

Verification:
- Unit tests cover supported UID/text key matching, deterministic ordering, invalid UID rejection, and result limit enforcement.

### `dicom-worklist`

Responsibility:
- Modality Worklist dataset validation plus deterministic persisted upsert/query workflow behavior; no network I/O.

Public API:
- `WorklistItem`
- `WorklistQuery`
- `WorklistStore`
- `UpsertOutcome`
- `AuditCallback`
- `validate_worklist_item`
- `build_worklist_response`
- `WorklistStore::open`, `WorklistStore::open_with_audit`

Verification:
- Unit tests cover required tag validation, deterministic ordering, persisted query/upsert filtering, audit event emission, and limit enforcement.

### `dicom-mpps`

Responsibility:
- MPPS update validation plus deterministic persisted lifecycle ingestion semantics; no network I/O.

Public API:
- `MppsStatus`
- `MppsUpdate`
- `MppsStore`
- `MppsService`
- `MppsServiceConfig`
- `IngestOutcome`
- `AuditCallback`
- `validate_mpps_update`
- `MppsStore::open`, `MppsService::with_persistence`

Verification:
- Unit tests cover status validation, transition enforcement, persisted service behavior, audit event emission, and store limit enforcement.

### `dicom-auth`

Responsibility:
- Authn/authz policy hooks used by service layers; no network I/O.

Public API:
- `AuthScope`, `AuthAction`
- `AuthSubject`, `AuthResource`, `AuthRequest`
- `AuthDecision`, `AuthDenyReason`
- `Authorizer`, `AllowAll`, `DenyAll`

Requirements:
- **REQ-AUTH-300:** Authorization **MUST** be fail-closed unless explicitly allowed.
- **REQ-AUTH-302:** Authorization denial **MUST** return a structured error mapped per `docs/13`.

Verification:
- Unit tests cover allow/deny decisions and structured error mapping.

### `dicom-audit`

Responsibility:
- Deterministic audit logging with redaction and retention limits.

Public API:
- `AuditConfig`
- `AuditEventKind`, `AuditEvent`, `AuditField`, `AuditValue`
- `AuditRedactor`
- `AuditLog`

Requirements:
- **REQ-AUDIT-350:** Audit events **MUST** redact sensitive values before storage or emission.
- **REQ-AUDIT-351:** Audit logs **MUST** enforce retention/rotation limits deterministically.

Verification:
- Unit tests cover redaction and retention/limit enforcement.

### `dicom-series`

Requirements:
- **REQ-API-220:** `dicom-series` **MUST** not perform pixel decoding.
- **REQ-API-221:** `dicom-series` **MUST** expose deterministic ordering per `docs/04-DICOM-IO-and-Series-Assembly.md`.

Verification:
- Unit tests **MUST** ensure no `dicom-pixel` dependencies in `dicom-series`.
- Ordering tests **MUST** reference `docs/04-DICOM-IO-and-Series-Assembly.md` rules.

Responsibility:
- Study/Series grouping, deterministic ordering, geometry validation, frame indexing.

Public API:
- `InstanceHeader` (minimal extracted header)
- `Study`, `Series` structures
- `FrameKey { series_uid, instance_uid, frame_index }`
- `Series::frames() -> impl Iterator<Item=FrameRef>`

Notes:
- Ordering guidance and stability rules are defined in `docs/04-DICOM-IO-and-Series-Assembly.md` (see **REQ-API-221**).

### `dicom-pixel`

Responsibility:
- Transfer syntax decoding and the pixel pipeline.

Public API:
- `PixelPipelineConfig`
- `PixelDecodeInput`
- `DisplayTransform`:
  - `fn apply(&self, frame: &mut DisplayFrame) -> Result<()>`
- `PixelPipeline`:
  - `fn decode_frame(&self, dataset: &Dataset, transfer_syntax_uid: &str, frame_index: u32) -> Result<DisplayFrame>`
  - `fn decode_frame_with_transform(&self, dataset: &Dataset, transfer_syntax_uid: &str, frame_index: u32, transform: &impl DisplayTransform) -> Result<DisplayFrame>`
  - `fn decode_input(&self, input: PixelDecodeInput<'_>) -> Result<DisplayFrame>`
  - `fn decode_input_with_transform(&self, input: PixelDecodeInput<'_>, transform: &impl DisplayTransform) -> Result<DisplayFrame>`
- `DisplayFrame`:
  - `width`, `height`, `format`, `bytes`
- `WindowLevel`:
  - explicit WC/WW or VOI LUT selection or auto-window.
- `RasterFormat` + `decode_raster_bytes(...)` (feature `raster-io`, non-DICOM inputs)
  - Supported raster formats: BMP, PNG, JPEG, TIFF.

Requirements:
- **REQ-API-230:** CPU-boundary output **MUST** be deterministic (see `docs/05-Pixel-Pipeline.md`).

Verification:
- Golden-corpus tests **MUST** compare CPU boundary output hashes for deterministic stability.

### `viewer-core`

Requirements:
- **REQ-API-240:** `viewer-core` **MUST** be GPU-agnostic and **MUST NOT** depend on `wgpu` types.

Verification:
- Dependency audit **MUST** confirm `viewer-core` has no `wgpu` types in its public API.

Responsibility:
- Viewport model, interaction state machine, tools/measurements, scene representation independent of GPU backend.

Public API:
- `Viewport2D`, `InteractionState`, `ToolState`
- `Measurement` types with calibration/provenance + uncertainty metadata
- `ViewerModel` controlling series/frame selection and VOI state.
  - explicit measurement controls:
    - `set_measurement_mode(...)`
    - `set_pixel_spacing_calibration(...)`
    - `set_measurement_uncertainty(...)`

### `viewer-wgpu`

Responsibility:
- GPU rendering implementation (textures, shaders, render passes) using `wgpu`.

Public API:
- `Renderer` trait/object:
  - `fn render(&mut self, frame: &DisplayFrame, viewport: &Viewport2D, ...) -> Result<()>`
- `GpuCapabilities` query results.
- deterministic lifecycle controls:
  - `configure_surface(...)`
  - `resize_surface(...)`
  - `lifecycle_state(...)`

Requirements:
- **REQ-API-250:** Renderer **MUST NOT** parse DICOM directly.
- **REQ-GPU-210:** Renderer **MUST** treat GPU as presentation-only and **MUST** accept only CPU-oracle quantized frame formats (`Luma8`, `Rgba8`) at the render entry boundary.

Verification:
- Static dependency audit **MUST** confirm `viewer-wgpu` does not depend on DICOM parsing crates.
- Unit tests **MUST** reject non-quantized frame formats (e.g., `Luma16`) and mismatched byte-length/format combinations before render submission.

### `viewer-wasm`

Requirements:
- **REQ-API-260:** `viewer-wasm` **MUST** be the only crate that depends directly on browser bindings for viewer integration.

Verification:
- Workspace dependency audit **MUST** confirm browser bindings (`web_sys`, `js_sys`, `wasm-bindgen`) are confined to `viewer-wasm`.

Responsibility:
- Browser glue and bindings:
  - input adapters,
  - event wiring,
  - surface initialization.
  - local host harness assets under `crates/viewer-wasm/web`.

Host harness:
- `tools/run_viewer_wasm_frontend.sh` builds `viewer-wasm` via `wasm-pack` and serves `crates/viewer-wasm/web` as a local interactive entrypoint.
- The host harness binds directly to exported `viewer-wasm` APIs (`WasmViewer`, `escapeMetadataHtml`) without introducing a separate JS framework dependency.

---

## 2. Feature flags and envelope gating

### Principles (normative)

Requirements:
- **REQ-FEAT-301:** The default repository build **MUST** be minimal-core and fail-closed (`default = []`), while the workstation conformance profile **MUST** be enabled explicitly via features.
- **REQ-FEAT-302:** Optional extensions beyond the workstation profile **MUST** require explicit Cargo features.
- **REQ-FEAT-303:** Public APIs **MUST** expose feature availability explicitly (e.g., via capability query) to avoid runtime surprises.

Verification:
- Build matrix **MUST** include the default minimal-core profile, the explicit workstation profile, the workstation-completeness profile, and extension feature sets beyond the active profile (REQ-FEAT-301, REQ-FEAT-302, REQ-API-280).
- API tests **MUST** assert capability queries reflect enabled features (REQ-FEAT-303).

### Suggested feature taxonomy

- `tier1-deflate`
- `codec-jpegls`
- `codec-j2k`
- `raster-io`
- `modality-ct`
- `modality-pet`
- `modality-mg`
- `modality-xr`
- `gsps` (optional)
- `pack-enhanced` (enhanced CT/MR multi-frame)
- `pack-us` (ultrasound)
- `pack-nm` (nuclear medicine)
- `pack-xa` (x-ray angiography/fluoro)
- `pack-seg` (segmentation objects)
- `pack-rt` (RT dose + structure set + plan references)
- `pack-sr` (structured reports)
- `dimse-c-find`
- `dimse-c-move`
- `dimse-c-get`
- `qido`
- `wado`
- `stow`

### RC-2026.02.12 activation map (normative, release-scoped)

- Workstation baseline active: `tier1-deflate`, `codec-jpegls`, `codec-j2k`, `modality-pet`, `modality-xr`.
- Workstation-completeness baseline active: `pack-enhanced`, `pack-us`, `pack-nm`, `pack-xa`, `pack-seg`, `pack-rt`, `pack-sr`, `gsps`.
- Explicitly non-activated in IO envelope for RC-2026.02.12: `modality-mg` SOP rows remain deferred and **MUST** fail closed in `dicom-io` (REQ-FEAT-302).

Requirements:
- **REQ-FEAT-304:** Each feature **MUST**:
  - update `docs/03-DICOM-Conformance-Envelope.md` envelope tier tables,
  - add corpus samples and/or synthetic tests,
  - add fuzz targets for new codec boundaries.

Verification:
- PR checklist **MUST** confirm doc updates, corpus changes, and fuzz target additions for each feature (REQ-FEAT-304).

---

## 3. Public error types (link)

Requirements:
- **REQ-API-270:** All crates **MUST** use the shared error model specified in `docs/13-Error-Model-and-Telemetry.md`.

Verification:
- Unit tests **MUST** assert error kind and code mappings use the shared model across crates (REQ-API-270).

---

## 4. Verification requirements

- **REQ-API-280:** A workspace-level build **MUST** succeed for:
  - default workstation features,
  - `--all-features` (when optional extensions are present).
- **REQ-API-281:** WASM build **MUST** succeed for portable crates.

Verification:
- `cargo build` (default), `cargo build --all-features`, and `cargo build --target wasm32-unknown-unknown` for portable crates.

---

## 5. Stable API baseline (Volume/MPR/Fusion)

Status: **Implemented baseline** (As of 2026-02-22)

Stable public baseline APIs in this release wave:
- `viewer-core::VolumeGrid` with optional patient-space metadata and mapping helpers.
- `viewer-core::MprRequest` and `viewer-core::PatientMprRequest` for voxel-space and patient-space MPR request surfaces.
- `modality-pet::PetCtFusionInput` / `modality-pet::PetCtFusionResult` for deterministic PET-over-CT baseline execution.

Stable error-code surfaces:
- `viewer-core::VolumeError::code()`
- `viewer-core::MprError::code()`
- `viewer-wasm::backend::BackendErrorCode::as_code()`

Compatibility rules:
- additive fields are preferred,
- breaking signature changes require migration guidance,
- serializer-affecting volume schema changes require adapter coverage and compatibility tests.

## Source-of-truth footer

- Runtime and API contracts in this document are the source-of-truth for service env-vars and binary ownership.
- Any new claim in this file requires synchronized updates to corresponding launch scripts and packaging outputs before merge.
