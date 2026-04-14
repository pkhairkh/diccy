# Runtime and Profile Capability Matrix

Status: **Baseline capability matrix** (As of 2026-02-22)
Reference: `docs/33-Productization-Profiles-and-Playbooks.md`

| Capability | `framework-core` | `workstation` | `backend-services` | `backend-services-with-dimse` |
|---|---|---|---|---|
| CPU oracle pixel pipeline | Yes | Yes | Yes | Yes |
| Tri-planar MPR + slab controls | API only | Yes | Service-independent | Service-independent |
| SR create/update/retrieve workflow endpoints | API helpers only | Prototype UI + service calls | Yes | Yes |
| DICOMweb QIDO/WADO/STOW runtime | No | Targets backend | Yes | Yes |
| Worklist/MPPS runtime | No | Targets backend | Yes | Yes |
| DIMSE integration transport | No | Optional/targeted integrations | Planned optional | Yes |
| WebGPU presentation path | N/A | Gated optional path | N/A | N/A |
| DIMSE SOP support (default profile view) | N/A | Optional/targeted integrations | Deferred/disabled | Query+Store+Retrieve (feature profile) |
| Profile capability manifests | Yes (`tools/package_profiles.sh`) | Yes | Yes | Yes |

## DIMSE SOP claim-to-capability state matrix

`dicom-dimse-service` command availability is separated into:

- package/runtime availability (profile includes `dicom-dimse-service`), and
- compile-time feature gates (`dicom-dimse` / `dicom-dimse-service` crate features).

| DIMSE command | `framework-core` | `workstation` | `backend-services` | `backend-services-with-dimse` |
|---|---|---|---|---|
| C-ECHO | N/A (no DIMSE runtime) | N/A (no DIMSE runtime) | Omitted (no `dicom-dimse-service` artifact) | **Active** (runnable binary + active command contract) |
| C-STORE | N/A (no DIMSE runtime) | N/A (no DIMSE runtime) | Omitted (no `dicom-dimse-service` artifact) | **Active** (runnable binary + active command contract) |
| C-FIND | N/A (no DIMSE runtime) | N/A (no DIMSE runtime) | Omitted (no `dicom-dimse-service` artifact) | **Feature-gated disabled-by-default** (`dimse-c-find`) |
| C-MOVE | N/A (no DIMSE runtime) | N/A (no DIMSE runtime) | Omitted (no `dicom-dimse-service` artifact) | **Feature-gated disabled-by-default** (`dimse-c-move`) |
| C-GET | N/A (no DIMSE runtime) | N/A (no DIMSE runtime) | Omitted (no `dicom-dimse-service` artifact) | **Feature-gated disabled-by-default** (`dimse-c-get`) |

Claim source: `crates/dicom-dimse-service/src/lib.rs` and `crates/dicom-dimse/Cargo.toml` feature gates.

## Runtime-docs and package-claim compatibility matrix


The package claim tables in `docs/33` and `docs/12` must stay aligned with packaged artifacts under `dist/profiles/`.

| Profile | Runtime/docs claim source | Packaged artifact assertion |
|---|---|---|
| `framework-core` | `docs/33`: profile artifact row + `docs/12`: `dicom-visualizer` runnable artifact posture | `framework-core.<RELEASE_ID>.tar.gz` must include `capability.manifest.json` and `bin/dicom-visualizer` |
| `workstation` | `docs/33`: artifact-class section + `docs/12`: non-runnable viewer library posture | `workstation.<RELEASE_ID>.tar.gz` must include `capability.manifest.json`; no guaranteed `bin/viewer-wasm` or `bin/viewer-wgpu` in current posture |
| `backend-services` | `docs/33`: archive-layout row + `docs/12`: `dicom-web-server` and `dicom-workflow-server` sections | `backend-services.<RELEASE_ID>.tar.gz` must include `capability.manifest.json`, `bin/dicom-web-server`, and `bin/dicom-workflow-server`; must not include `bin/dicom-dimse-service` |
| `backend-services-with-dimse` | `docs/33`: archive-layout row + `docs/12`: `dicom-dimse-service` section | `backend-services-with-dimse.<RELEASE_ID>.tar.gz` must include `capability.manifest.json`, `bin/dicom-web-server`, `bin/dicom-workflow-server`, and `bin/dicom-dimse-service` |

## Runtime env-contract parity gate

Status: **Required for release and packaging sign-off**

Release and profile operations must keep the runtime contract contract in sync with parser definitions:

- `python3 tools/runtime_env_contract.py --repo-root . --check-docs --report reports/docs/runtime-env-contract.json`
- `python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json`
- `./tools/package_profiles.sh --release-id "<RELEASE_ID>"`

Parser source-of-truth:
- `crates/dicom-web-server/src/main.rs`
- `crates/dicom-workflow-server/src/main.rs`
- `crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs`

Observed failure behavior:
- any mismatch in documented vs parsed variables fails the release check and blocks artifact publication in this phase.

## Notes

- `workstation` profile defaults to CPU/canvas correctness path with gated WebGPU presentation.
- `backend-services` profile is fail-closed by default and requires explicit auth/transport policy settings.
