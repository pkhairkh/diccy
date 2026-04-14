# rust-dicom-viewer-framework

A **PACS workstation framework** in Rust, designed for **script-hosted web runtime first** with optional native/WGPU rendering integration (via library integration surfaces). Capability state references: `docs/31-Implementation-Status.md#major-subsystems` and `docs/31-Implementation-Status.md#native-vs-wasm-compatibility-matrix`. The framework treats all inputs as **untrusted**, provides deterministic CPU-boundary pixel outputs, and defines a conformance/workflow envelope for ingestion, storage, query/retrieve, reporting, and operational workflow.

## Non-goals

This project is intentionally **not**:

- An autonomous diagnosis engine.
- A treatment delivery or therapy control system.
- A permissive “best-effort” parser that silently guesses through malformed datasets.
- A replacement for enterprise IAM/EMR/billing systems; integrations are explicit boundaries.

## Intended purpose and claim surface

**Framework Intended Purpose:** RDVF is a Rust PACS workstation framework that ingests, stores, queries, retrieves, renders, annotates, and reports on DICOM studies for clinical workflow integration. Regulatory authorization and deployment claims are controlled by integrators and deployment programs.

- This repository specifies a **framework**. Downstream products control labeling, marketing, and deployment context.
- The default repository build posture is **minimal-core and fail-closed**:
  - optional modality/codec/workflow packs are disabled unless explicitly enabled,
  - out-of-envelope data fails closed with typed errors.
- The framework **bundles protocol and workflow capabilities by component**, not as an automatically bundled full clinical stack by default. See [Runtime feature vs. packaged artifact matrix](#runtime-feature-vs-packaged-artifact-matrix).

Technical mapping and primary references:
- `docs/15-Regulatory-and-Standards-Mapping.md` (informative)
- `docs/16-Requirements-Index.md` (REQ scheme)

## Runtime feature vs. packaged artifact matrix

Status: **Partially implemented** (As of 2026-02-24)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

| Feature class | In code (`crates`) | Packaged by default artifacts | Default launch behavior |
|---|---|---|---|
| DICOMweb query/retrieve/store | `dicom-dimse`, `dicom-net`, `dicom-web`, `dicom-web-server` | **Yes** via `dicom-web-server` | Launched from `backend-services` profile |
| Workflow APIs (MWL/MPPS/SR) | `dicom-workflow-server`, `dicom-worklist`, `dicom-mpps`, `dicom-workflow` | **Yes** via `dicom-workflow-server` | Launched from `backend-services` profile |
| Visualization runtime (native viewer) | `viewer-wgpu` | **Partially** via `workstation` profile | Hosted integration path; no dedicated `viewer-wgpu` script-native executable is documented as guaranteed in release profile artifacts |
| Visualization runtime (WASM host) | `viewer-wasm` | **Yes** via `workstation` profile | Launched via `./tools/run_viewer_wasm_frontend.sh` |
| DIMSE protocol logic | `dicom-dimse`, `dicom-net`, `dicom-dimse-service` | **Optional runtime binary** (`dicom-dimse-service`) | Added when DIMSE profile is explicitly enabled |
| PIXI/diagnostics tools | `dicom-visualizer` | **Yes** via `framework-core` profile (`dicom-visualizer`) | CLI tool |

## Capability status

Status: **Implemented/Deferred by capability** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#native-vs-wasm-compatibility-matrix`

Primary capability status and compatibility are tracked in:
- `docs/31-Implementation-Status.md#major-subsystems`
- `docs/31-Implementation-Status.md#native-vs-wasm-compatibility-matrix`

## Conformance envelope summary

Status: **Implemented/Deferred by subsystem** (As of 2026-02-22)
Reference: `docs/31-Implementation-Status.md#major-subsystems`

The framework implements and publishes a conformance envelope (see `docs/03-DICOM-Conformance-Envelope.md`) including:

- **Core build**: Tier 0 baseline with strict fail-closed behavior.
- **Workstation-capable profile**: DICOMweb, workflow APIs, deterministic storage/index/query paths, and report workflows under deterministic and fail-closed rules.

A dataset outside the envelope fails closed with a typed, actionable error and is not partially rendered in an undefined state.

Verification:
- Confirm `docs/03-DICOM-Conformance-Envelope.md` enumerates the Tier 0 SOP Classes and Transfer Syntaxes listed here.
- Confirm out-of-envelope inputs return typed errors via unit/integration coverage described in `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

## Quickstart (developer)

Assumptions:
- Rust toolchain installed (`rustup`).
- No network access required for core build/test once dependencies are already present in the local Cargo cache (or vendored).

Commands:

```bash
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

Multi-DICOM visualization (folder -> PNG previews + gallery):

```bash
cargo run -p dicom-visualizer -- ./behrooz ./behrooz_preview --limit 512
```

Outputs:
- `./behrooz_preview/*.png` preview frames (first frame per file)
- `./behrooz_preview/manifest.csv` decode/export outcomes
- `./behrooz_preview/index.html` gallery for quick review
- `./behrooz_preview/manifest.integrity.json` deterministic integrity metadata sidecar (SHA-256 hashes + export summary)
- Out-of-envelope/non-image SOPs are kept fail-closed and reported as error rows in `manifest.csv`.

Interactive WASM frontend host (browser + `viewer-wasm`):

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
./tools/run_viewer_wasm_frontend.sh --port 4173
```

For HTTPS (TLS) hosting:

```bash
./tools/run_viewer_wasm_frontend.sh --https --port 4173
```

Open `http://127.0.0.1:4173` (or `https://127.0.0.1:4173` with `--https`).

Default local endpoint topology and service binds are documented in `docs/40-Reference-Deployment-Topology.md`.

Notes:
- The host app lives in `crates/viewer-wasm/web` and binds directly to `viewer-wasm`.
- The host attempts the production WebGPU presentation path by default; if unavailable/failing it falls back to CPU.
- Input ingestion supports both single-file and folder selection; folder ingest opens the first decodable DICOM instance in deterministic filename order.
- By default, the host proxies backend calls through same-origin routes:
  - `/dicomweb` -> `http://127.0.0.1:8080`
  - `/workflow` -> `http://127.0.0.1:8082`
- For LAN-origin access (for example `http://10.x.x.x:4173`), browsers may disable WebGPU on insecure origins; use HTTPS to retain WebGPU.
- The network toggle in the UI is compile-time gated; enable it via:
  - `./tools/run_viewer_wasm_frontend.sh --features network`
  - `webgpu-backend` is enabled by default by `run_viewer_wasm_frontend.sh`.
- The SR workflow prototype panel defaults to `/workflow` and can be overridden to any reachable endpoint.

## Product profiles

Profile definitions and boundaries:
- `framework-core`: headless framework primitives and packs.
- `workstation`: script-hosted WASM viewer + interaction workflows.
- `backend-services`: DICOMweb + durable workflow runtime.

Reference: `docs/33-Productization-Profiles-and-Playbooks.md`

## Profile build commands and expected artifacts

```bash
# framework-core
cargo build -p rdvf

# workstation
cargo build -p viewer-wasm -p viewer-wgpu
# current profile documents do not promise standalone `viewer-*` runtime binaries in `dist/profiles/workstation.*`

# backend-services (DICOMweb + workflow; DIMSE excluded unless explicitly enabled)
cargo build -p dicom-web-server -p dicom-workflow-server
./tools/package_profiles.sh --release-id "<RELEASE_ID>"
```

Packaged profile artifacts (capability manifests):

```bash
./tools/package_profiles.sh
```

Outputs:
- `dist/profiles/framework-core.tar.gz`
- `dist/profiles/workstation.tar.gz`
- `dist/profiles/backend-services.<RELEASE_ID>.tar.gz`
- `dist/profiles/backend-services-with-dimse.<RELEASE_ID>.tar.gz` (with `--include-dimse`)

## DIMSE integration note

`dicom-dimse-service` is a runnable Rust service binary (`cargo run -p dicom-dimse-service`) that exposes the DIMSE runtime.
- Default `backend-services.<RELEASE_ID>.tar.gz` packaging does not include DIMSE.
- Enable `backend-services-with-dimse.<RELEASE_ID>.tar.gz` when a separate DIMSE endpoint is required.
- All non-default binaries require explicit profile inclusion or direct build commands.
- Runtime startup contract and profile mapping are documented in `docs/12-API-Surface-and-Crate-Boundaries.md` and `docs/33-Productization-Profiles-and-Playbooks.md`.

Example:

```bash
cargo run -p dicom-dimse-service -- --bind 127.0.0.1:11112
```

## Local demo and benchmarks

One-command local demo path (ingest/query/retrieve + MPR + SR + fusion):

```bash
./tools/local_demo_end_to_end.sh
```

Onboarding time-to-first-image benchmark:

```bash
./tools/time_to_first_image.sh
```

Fusion baseline performance harness:

```bash
./tools/fusion_perf_harness.sh
```

## Documentation map (source of truth)

- Glossary: `docs/00-Glossary.md`
- Vision & scope: `docs/01-Vision-and-Scope.md`
- Architecture: `docs/02-Architecture.md`
- Conformance envelope: `docs/03-DICOM-Conformance-Envelope.md`
- I/O + series assembly: `docs/04-DICOM-IO-and-Series-Assembly.md`
- Pixel pipeline: `docs/05-Pixel-Pipeline.md`
- Rendering & interaction: `docs/06-Rendering-and-Interaction.md`
- Volume & fusion: `docs/07-Volume-and-Fusion.md`
- WASM target: `docs/08-WASM-Target.md`
- Security threat model: `docs/09-Security-Threat-Model.md`
- Testing/corpus/fuzzing/evals: `docs/10-Testing-Corpus-Fuzzing-Evals.md`
- Performance & caching: `docs/11-Performance-and-Caching.md`
- API + crate boundaries: `docs/12-API-Surface-and-Crate-Boundaries.md`
- Error model & telemetry: `docs/13-Error-Model-and-Telemetry.md`
- Release & versioning: `docs/14-Release-and-Versioning.md`
- Implementation status: `docs/31-Implementation-Status.md`
- Documentation errata (2026-02-21): `docs/32-Documentation-Errata-RC-2026-02-21.md`
- Productization profiles: `docs/33-Productization-Profiles-and-Playbooks.md`
- SR workflow architecture: `docs/39-SR-Workflow-Architecture.md`
- Reference deployment topology: `docs/40-Reference-Deployment-Topology.md`
- Extension/plugin contract: `docs/41-Extension-Plugin-Contract.md`
- Frontend styling governance: `UI_WORKFLOW.md`, `UI_CONTRACTS.md`

## Runtime setup and contract verification

- Runtime environment contract source-of-truth for backend binaries is `docs/12-API-Surface-and-Crate-Boundaries.md`.
- Validate runtime docs parity with parser behavior before release/profile changes:
  - `python3 tools/runtime_env_contract.py --repo-root . --check-docs --report reports/docs/runtime-env-contract.json`
  - `python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json`
- Contract status page:
  - [Runtime Contract Status Matrix](docs/49-Runtime-Profile-Capability-Matrix.md)

## Contributing

See `CONTRIBUTING.md` and `SECURITY.md`.

## License

This repository is licensed under the Apache License 2.0. See `LICENSE`.
