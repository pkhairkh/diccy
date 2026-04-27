# DiCCY

A Rust PACS/DICOM workstation framework. DiCCY provides a complete clinical
workstation stack — DICOM parsing, networking, storage, rendering,
segmentation, reporting, and workflow — built on deterministic,
fail-closed principles with first-class WebAssembly support.

## What DiCCY is

DiCCY is a **PACS workstation framework** implemented as a Cargo workspace of
~50 crates. It is designed for a **script-hosted web runtime first** with
optional native WGPU rendering. All inputs are treated as **untrusted**,
pixel outputs are **deterministic** on the CPU boundary, and the system
defines a strict conformance/workflow envelope for ingestion, storage,
query/retrieve, rendering, annotation, and reporting.

## What DiCCY is not

- An autonomous diagnosis engine.
- A treatment delivery or therapy control system.
- A permissive "best-effort" parser that silently guesses through malformed datasets.
- A replacement for enterprise IAM/EMR/billing systems; integrations are explicit boundaries.

## Intended purpose

> DiCCY is a Rust PACS workstation framework that ingests, stores, queries,
> retrieves, renders, annotates, and reports on DICOM studies for clinical
> workflow integration. Regulatory authorization and deployment claims are
> controlled by integrators and deployment programs.

This repository specifies a **framework**. Downstream products control
labeling, marketing, and deployment context. The default build posture is
**minimal-core and fail-closed**: optional modality/codec/workflow packs are
disabled unless explicitly enabled, and out-of-envelope data fails closed
with typed errors.

## Quickstart

```bash
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

### Multi-DICOM visualization

```bash
cargo run -p dicom-visualizer -- ./samples ./output --limit 512
```

Outputs preview PNGs, a manifest CSV, an HTML gallery, and an integrity
sidecar (SHA-256 hashes).

### WASM viewer (browser)

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
./tools/run_viewer_wasm_frontend.sh --port 4173
```

Open `http://127.0.0.1:4173`. For HTTPS, pass `--https`. The host app lives
in `crates/viewer-wasm/web` and defaults to WebGPU with CPU fallback.

## Product profiles

| Profile | Contents | Build |
|---|---|---|
| `framework-core` | Headless framework primitives and packs | `cargo build -p diccy` |
| `workstation` | WASM viewer + interaction workflows | `cargo build -p viewer-wasm -p viewer-wgpu` |
| `backend-services` | DICOMweb + durable workflow runtime | `cargo build -p dicom-web-server -p dicom-workflow-server` |

Packaged artifacts:

```bash
./tools/package_profiles.sh --release-id "<RELEASE_ID>"
```

See `docs/33-Productization-Profiles-and-Playbooks.md` for full details.

## Runtime feature matrix

| Feature class | Crates | Default profile |
|---|---|---|
| DICOMweb (WADO/STOW/QIDO) | `dicom-web`, `dicom-web-server` | `backend-services` |
| Workflow APIs (MWL/MPPS/SR) | `dicom-workflow-server`, `dicom-worklist`, `dicom-mpps` | `backend-services` |
| DIMSE networking | `dicom-dimse`, `dicom-net`, `dicom-dimse-service` | Optional (`--include-dimse`) |
| Visualization (native) | `viewer-wgpu` | `workstation` |
| Visualization (WASM) | `viewer-wasm` | `workstation` |
| CLI diagnostics | `dicom-visualizer` | `framework-core` |

## DIMSE integration

```bash
cargo run -p dicom-dimse-service -- --bind 127.0.0.1:11112
```

DIMSE is excluded from default `backend-services` packaging. Enable it
explicitly when a separate DIMSE endpoint is required.

## Local demo and benchmarks

```bash
./tools/local_demo_end_to_end.sh     # Ingest → query → render → SR writeback
./tools/time_to_first_image.sh       # Onboarding benchmark
./tools/fusion_perf_harness.sh       # Fusion baseline
```

## Documentation

| Topic | Document |
|---|---|
| Glossary | `docs/00-Glossary.md` |
| Vision & scope | `docs/01-Vision-and-Scope.md` |
| Architecture | `docs/02-Architecture.md` |
| Conformance envelope | `docs/03-DICOM-Conformance-Envelope.md` |
| I/O & series assembly | `docs/04-DICOM-IO-and-Series-Assembly.md` |
| Pixel pipeline | `docs/05-Pixel-Pipeline.md` |
| Rendering & interaction | `docs/06-Rendering-and-Interaction.md` |
| Volume & fusion | `docs/07-Volume-and-Fusion.md` |
| WASM target | `docs/08-WASM-Target.md` |
| Security threat model | `docs/09-Security-Threat-Model.md` |
| Testing & fuzzing | `docs/10-Testing-Corpus-Fuzzing-Evals.md` |
| Performance & caching | `docs/11-Performance-and-Caching.md` |
| API & crate boundaries | `docs/12-API-Surface-and-Crate-Boundaries.md` |
| Error model & telemetry | `docs/13-Error-Model-and-Telemetry.md` |
| Release & versioning | `docs/14-Release-and-Versioning.md` |
| Implementation status | `docs/31-Implementation-Status.md` |
| Productization profiles | `docs/33-Productization-Profiles-and-Playbooks.md` |
| SR workflow architecture | `docs/39-SR-Workflow-Architecture.md` |
| Deployment topology | `docs/40-Reference-Deployment-Topology.md` |
| Extension/plugin contract | `docs/41-Extension-Plugin-Contract.md` |

## Runtime contract verification

```bash
python3 tools/runtime_env_contract.py --repo-root . --check-docs \
  --report reports/docs/runtime-env-contract.json
python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json
```

## Contributing

See `CONTRIBUTING.md` and `SECURITY.md`.

## License

Apache License 2.0. See `LICENSE`.
