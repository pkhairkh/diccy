# Release and versioning

## Goals

Normative: Every release **MUST** document conformance envelope changes, security changes, and verification evidence as defined in this document.

- Provide predictable compatibility for integrators.
- Make conformance envelope changes explicit.
- Support safe upgrades with deterministic behavior preserved.

---

## 1. Versioning scheme

### Workspace crate versions

- Crates **MUST** use Semantic Versioning (SemVer).
- The repository **SHOULD** use a unified version across core crates to reduce dependency complexity.
- Modality packs **MAY** use independent versions if they are clearly separated, but they **MUST** declare compatibility with the core version.

### Conformance envelope version

The project **MUST** version the conformance envelope independently of crate versioning.

Mechanism:
- Maintain an `envelope_version` identifier (e.g., `0.1`, `0.2`) in documentation and optionally in code.

Rules:
- Adding new supported SOP Classes or Transfer Syntaxes **MAY** increment envelope minor.
- Tightening limits or failure behavior **MUST** increment envelope minor (because outputs may change).
- Removing support or changing semantics **MUST** increment envelope major.

Envelope decision table (RC policy):

| Change type | Required envelope action | Required release evidence |
|---|---|---|
| Matrix row status changes from `Unsupported`/`Deferred` to `Supported` | Increment minor | Updated `docs/03` matrix row + conformance rejection/acceptance tests + signed conformance artifact |
| Matrix row status changes from `Supported` to `Deferred` or `Unsupported` | Increment major | Updated `docs/03` matrix row + migration note + signed GO/NO-GO decision |
| Default limit change affecting parse/decode/runtime admission | Increment minor | Updated `docs/09`/`docs/03` defaults + limit regression evidence |
| Error-kind or fail-closed behavior semantic change | Increment minor (or major if compatibility breaks) | Updated error-model references + negative-path evidence |
| Documentation-only clarification with no behavior delta | No envelope bump | Signed statement of no behavior delta in release evidence |

Release gating for envelope changes:

- Any release that changes matrix row status, limits, or fail-closed behavior **MUST** include:
  - updated matrix tables in `docs/03-DICOM-Conformance-Envelope.md`,
  - passing conformance rejection tests for out-of-envelope SOP/TS/command/method paths,
  - a signed release-specific conformance decision artifact under `reports/analytical/`.
- Releases **MUST NOT** reuse prior conformance signatures when envelope-relevant rows changed.

Transport/runtime expansion workflow (required):
- Any new production transport profile (including adding a default service/binary, such as DIMSE) requires:
  - bumping or confirming transport-specific section in `docs/03-DICOM-Conformance-Envelope.md`,
  - running `python3 tools/package_profiles.sh --include-dimse --validate`,
  - updating `docs/33-Productization-Profiles-and-Playbooks.md` and `docs/49-Runtime-Profile-Capability-Matrix.md`,
  - running `python3 tools/check_dimse_profile_release_controls.py --release-id <release-id>` when DIMSE profile posture changes,
  - attaching the release evidentiary delta under `docs/14` with signed `release_id` artifacts.

Verification:
- Golden corpus tests **MUST** be associated with a specific envelope version.
- Release notes **MUST** state envelope deltas.

HL7 callback retry release defaults (production profile):

| Environment key | Release default | Allowed bounds |
|---|---|---|
| `DICOM_WORKFLOW_HL7_CALLBACK_MAX_ATTEMPTS` | `3` | `1..=10` |
| `DICOM_WORKFLOW_HL7_CALLBACK_CIRCUIT_FAILURE_THRESHOLD` | `2` | `1..=10` |
| `DICOM_WORKFLOW_HL7_CALLBACK_BASE_BACKOFF_MS` | `15000` | `100..=3_600_000` |
| `DICOM_WORKFLOW_HL7_CALLBACK_MAX_BACKOFF_MS` | `300000` | `1_000..=86_400_000` |

Release notes must declare any default change for these keys and include interoperability regression evidence when values differ from baseline.

---

## 2. Stability guarantees

### Determinism

- Pixel pipeline determinism **MUST** not be broken by patch releases.
- If a bug fix changes outputs:
  - it **MUST** be documented,
  - golden hashes **MUST** be updated with justification,
  - and the envelope version **MUST** reflect the semantic change.

### Feature flags

- Unstable features **MUST** be explicitly labeled (e.g., `unstable-*`).
- Unstable features **MAY** change in minor releases; stable features must follow SemVer.

---

## 3. Release artifacts

Release artifacts are treated as part of the reproducibility and supply-chain surface.

Requirements:

- **REQ-REL-601:** Every tagged release **MUST** publish:
  - the crate version(s) and enabled feature set,
  - the `envelope_version` referenced by docs (and optionally a code constant),
  - a machine-readable SBOM for all shipped artifacts (core + packs),
  - reproducible build metadata (compiler version, target triples, build flags),
  - corpus manifest hashes (input hashes + expected output hash manifest identifiers) used for golden tests,
  - a release-scoped determinism manifest in `reports/analytical/` generated from `corpus/manifest.toml` with deterministic ordering and hash projection,
  - a signed cross-target reproducibility matrix (x86_64, arm64, wasm) with mismatch-budget verdict,
  - release-scoped gate artifacts (`release-preflight-summary`, `release-artifact-integrity`, gate logs) that all use the same active `release_id`,
  - state backup and restore verification artifacts (`state-backup-manifest-<release-id>.json`, `state-restore-verification-<release-id>.md`),
  - profile metadata reconciliation report linking `tools/profile_matrix.json`, `docs/33`, and emitted `dist/profiles/profiles.metadata.json`,
  - profile-specific performance gate reports for `backend-services` and `backend-services-with-dimse`,
  - and p95/p99 budget regression gate output tied to env-contract defaults,
  - and required release-control artifacts for the active `release_id` (security gate decision, performance gate decision, traceability gate decision, release notes, RC freeze record).
- **REQ-REL-602:** Release notes **MUST** include an explicit “conformance envelope delta” section listing any SOP Class/Transfer Syntax additions/removals/semantic changes.
- **REQ-REL-603:** If a release updates default limits or output-determining semantics, it **MUST** update golden hashes and bump `envelope_version` per Section 1.

SBOM format and generation (normative):
- The SBOM **MUST** be either SPDX (2.3+) or CycloneDX (1.4+) and **MUST** include transitive dependencies (REQ-REL-601).
- If SBOM tooling requires external installation, the release pipeline **MUST** pin tool versions and verify tool integrity (hash/signature) as part of the build (REQ-REL-601).

If web artifacts are published:
- they **MUST** be reproducible from tagged sources (REQ-REL-601),
- they **MUST** document the build command, toolchain versions, and browser capability assumptions (REQ-REL-601).

### 3.x Packaging-surface release controls (for this repo)

- Every tagged release must execute:
  - `python3 tools/package_profiles.sh --include-dimse --validate`
  - `tools/startup_preflight_check.sh`
  - `python3 tools/check_dimse_profile_release_controls.py --release-id <release-id>` (DIMSE packaging + release-note guard)
- `dist/profiles/profiles.metadata.json` must be retained per `release_id` and include:
  - `artifact_name`
  - `profile`
  - `binary`
  - `image`
  - `envelope_version`
- `release-notes` must include a row for each profile/pathway documentation change introduced by packaging claim changes.
- `release-notes` must include an explicit runtime env-contract delta section whenever:
  - environment variable keys in `docs/12-API-Surface-and-Crate-Boundaries.md`, `docs/33-Productization-Profiles-and-Playbooks.md`, or `tools/runtime_env_contract*.py` are added, removed, or renamed.
  - packaging profile inclusion/exclusion changes a runtime claim set.
  - The delta must list added keys, removed keys, and rollout impact for each affected profile.
- `release-notes` should be authored from `reports/release/release-notes-template.md`, including an explicit
  `Competitive gap movement` declaration (`moved` or `unchanged`) with evidence links.

### Evidence signing workflow (release control)

Release evidence bundles are cryptographically signed and verified using deterministic tooling:

- `tools/evidence_sign_verify.py sign`
- `tools/evidence_sign_verify.py verify`

Operational rules:

- Signing key material **MUST** be supplied via environment variable (default: `EVIDENCE_SIGNING_KEY`) and **MUST NOT** be committed to the repository (REQ-REL-601).
- The signed bundle **MUST** include release ID, key ID, artifact paths, artifact SHA256 digests, and generated timestamp (REQ-REL-601).
- Verification **MUST** re-hash artifacts from disk and **MUST** fail closed on any digest mismatch or signature mismatch (REQ-REL-601).
- Release control board sign-off **MUST** reference at least one successful verification log for the release evidence set (REQ-REL-601).

Example deterministic commands:

- `python3 tools/evidence_sign_verify.py sign --release-id RC-2026.02.12 --key-id release-key-01 --artifact reports/release/external-evidence-acceptance-criteria-v1.md --output reports/release/evidence-signature-RC-2026.02.12.json`
- `python3 tools/evidence_sign_verify.py verify --bundle reports/release/evidence-signature-RC-2026.02.12.json`

---

## 4. Security releases

- Security fixes **MUST** be backportable where feasible (REQ-REL-603).
- The project **MUST** document CVE/advisory links and affected `envelope_version` ranges in release notes (REQ-REL-603).
- If remediation is deferred (e.g., requires a breaking change, dependency ecosystem update, or major refactor), the project **MUST** document (REQ-REL-603):
  - the deferral rationale,
  - interim mitigations (limits/feature disablement),
  - and a concrete follow-up work item in `ROADMAP.md`.

---

## 5. Verification requirements for a release

Before tagging a release:
- Capability-claim verification gate **MUST** pass: status-tagged claims in `README.md`, `docs/01`, `docs/06`, and `docs/08` match `docs/31-Implementation-Status.md` and include date stamps (run `python3 tools/docs_capability_lint.py` and `python3 tools/docs_symbol_lint.py`) (REQ-REL-601).
- Safe command set **MUST** pass: `cargo build`, `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` (REQ-REL-601).
- Corpus tests **MUST** pass for the current envelope (REQ-REL-601).
- Fuzzing regressions **MUST** be clean (no known crashers) (REQ-REL-601).
- Security evidence refresh checklist item for the active `release_id`:
  - refresh penetration-test summary artifact (for example `reports/security/penetration-test-summary-<release-id>.md`),
  - refresh negative-security-suite artifact (for example `reports/security/conformance-negative-suite-<release-id>.md`),
  - record both artifact paths in the security gate decision for the same `release_id`.
- Docs suite **MUST** be internally consistent (links valid, terminology consistent) (REQ-REL-601).
- Runtime durability gate **MUST** pass (REQ-REL-601):
  - web runtime WAL preflight and restart continuity tests,
  - workflow runtime snapshot preflight and restart continuity tests.
- Determinism gate **MUST** pass (REQ-REL-601):
  - `dicom-pixel` golden corpus hash tests,
  - native repeated-run hash stability checks,
  - deterministic manifest verification (`tools/generate_determinism_manifest.py --verify-existing`),
  - cross-target matrix compare (`tools/reproducibility_compare.py`) with mismatch budget `0` for CPU-boundary output hashes.
- Migration readiness **MUST** be documented using the persistence migration runbook (`docs/30-Runtime-Persistence-Migration-Runbook.md`) before enabling durable defaults in production rollouts (REQ-REL-601).
- Release preflight **MUST** fail closed when any required release-scoped artifact is missing for the active `release_id` (REQ-REL-601).
- GO/NO-GO signatures **MUST** be blocked when the latest preflight or artifact-integrity outputs for the active `release_id` contain any failed gate (REQ-REL-601).
- Determinism release manifests **MUST** be generated from the current `corpus/manifest.toml` and verified with `--verify-existing`; any mismatch **MUST** block release closure (REQ-REL-601, REQ-REL-603).
- Release validation runners **MUST** provide all native build prerequisites required by enabled transitive codec dependencies (including `cmake` and C/C++ toolchains when applicable) and **MUST** record those versions in release build metadata (REQ-REL-601).
- Claim-parity audit gate **MUST** include a docs/runtime runtime-contract check that validates:
  - every service startup claim in `docs/40-Reference-Deployment-Topology.md` and `docs/12-API-Surface-and-Crate-Boundaries.md` has a concrete binary or binary-path contract,
  - every claimed runnable binary has a corresponding `main.rs`/crate entrypoint in workspace manifests,
  - every claim in `docs/33-Productization-Profiles-and-Playbooks.md` appears in `tools/package_profiles.sh`.
- Claims-to-runtime checklist:
  - `docs/33-Productization-Profiles-and-Playbooks.md` artifact matrix is reconciled with `tools/package_profiles.sh`,
  - `docs/40-Reference-Deployment-Topology.md` endpoint/bind contracts are reconciled with service startup manifests and environment contracts,
  - `tools/runtime_env_contract.py` contract output is reconciled with `docs/12-API-Surface-and-Crate-Boundaries.md` env-var tables,
  - and evidence artifacts include `reports/analytical/release-doc-parity.json`.
- Recommended release command sequence:
  - `python3 tools/runtime_env_contract.py --repo-root . --report reports/analytical/runtime-env-contract.json --check-docs`
  - `python3 tools/docs_capability_lint.py`
  - `python3 tools/docs_symbol_lint.py`
  - `python3 tools/claim_surface_lint.py`
  - `python3 tools/release_preflight.py --release-id <release-id>`

Enterprise governance check (release-specific):
- Historical signed RC artifacts are archival records only; they **MUST NOT** be used to close a different `release_id` (REQ-REL-601).
- Release control packages **MUST** be internally coherent on `release_id` across decisions, gate logs, traceability manifests, SBOM, and release notes (REQ-REL-601).

---

## 6. RC-2026.02.11 release control checklist and roles (approved)

### Release control board roles

| Role | Accountable scope | Approval artifact anchor |
|---|---|---|
| Release Manager | Final RC decision, envelope freeze confirmation, release package archive lock | `reports/release/rc-governance-gate-decision-RC-2026.02.11.md` |
| Security Lead | Runtime security defaults, fuzz/authz/dependency closure, SBOM policy closure | `reports/security/security-gate-decision-RC-2026.02.11.md` |
| Traceability Lead | REQ->test->report linkage completeness and signed snapshot closure | `reports/traceability/traceability-gate-decision-RC-2026.02.11.md` |
| Interoperability Lead | Signed interoperability matrix and negative-path campaign closure | `reports/interoperability/release-evidence-index.md` |
| Performance Lead | SLO/scalability gate closure and budgets acceptance | `reports/performance/performance-scalability-gate-decision-RC-2026.02.11.md` |
| Workflow Lead | Durable storage continuity, rollback drill, and restore drill acceptance | `reports/release/rollback-hotfix-drill-RC-2026.02.11.md`, `reports/release/workflow-restore-drill-RC-2026.02.11.md` |
| Regulatory/Quality Lead | GO/NO-GO board minutes integrity and evidence package sign-off | `reports/release/go-no-go-board-minutes-RC-2026.02.11.md` |

### RC checklist execution map

| Gate ID | Gate description | Deterministic execution/evidence path | Owner |
|---|---|---|---|
| REL-GATE-01 | Rust safe command set passes (`fmt/build/test/clippy`) | `python3 tools/release_preflight.py --release-id RC-2026.02.11` (gates `cargo-fmt`, `cargo-build`, `cargo-test`, `cargo-clippy`) | Engineering Lead |
| REL-GATE-02 | Claim surface and REQ completeness pass | Preflight gates `claim-surface`, `req-completeness` + logs in `reports/analytical/gates/` | Traceability Lead |
| REL-GATE-03 | Traceability linkage reports missing refs = 0 and invalid links = 0 | Preflight gate `traceability` output in `reports/traceability/traceability-snapshot-RC-2026.02.11.*` | Traceability Lead |
| REL-GATE-04 | Determinism manifest verification and cross-target matrix compare | Preflight gates `determinism-manifest`, `cross-target-matrix` | Reproducibility Lead |
| REL-GATE-05 | Artifact integrity check covers SBOM + evidence index references + hash register | Preflight gate `artifact-integrity` and `reports/release/release-artifact-integrity-RC-2026.02.11.*` | Security Lead |
| REL-GATE-06 | Mock board completed with closure actions and risk register | `reports/release/go-no-go-board-minutes-RC-2026.02.11.md` | Release Manager |
| REL-GATE-07 | Signed GO/NO-GO decision template locked | `reports/release/go-no-go-decision-template-v1.md` | Regulatory/Quality Lead |
| REL-GATE-08 | Rollback/hotfix drill executed and documented | `reports/release/rollback-hotfix-drill-RC-2026.02.11.md` | Workflow Lead |
| REL-GATE-08B | Workflow/WAL disaster-recovery restore drill executed and documented | `reports/release/workflow-restore-drill-RC-2026.02.11.md` | Workflow Lead |
| REL-GATE-09 | RC freeze package locks `envelope_version`, release notes, and signed evidence references | `reports/release/rc-freeze-record-RC-2026.02.11.md`, `reports/release/release-notes-RC-2026.02.11.md` | Release Manager |
| REL-GATE-10 | Final governance GO decision signed and archived | `reports/release/rc-governance-gate-decision-RC-2026.02.11.md` | Release Manager |

---

## 7. RC-2026.02.12 conformance envelope delta (S05)

This section is the release-note delta required by **REQ-REL-602** for Sprint 07 (S05).

Envelope version impact:
- `envelope_version` bumped from `1.0` to `1.1` due matrix-row status changes from `Deferred` to `Supported` in `docs/03-DICOM-Conformance-Envelope.md` (per Section 1 decision table).

SOP Class matrix delta (status `Deferred` -> `Supported`, workstation-completeness profile):
- Enhanced CT / Enhanced MR (`pack-enhanced`)
- Ultrasound / Ultrasound Multi-frame (`pack-us`)
- Nuclear Medicine (`pack-nm`)
- XA / XRF (`pack-xa`)
- Segmentation (`pack-seg`)
- RT Dose / RT Structure Set / RT Plan (`pack-rt`)
- Basic Text SR / Comprehensive SR (`pack-sr`)
- GSPS (`gsps`)

Explicit non-activation in RC-2026.02.12:
- Mammography SOP rows remain `Deferred` and fail closed as `UnsupportedSopClass` in the IO envelope.

Required release evidence anchors:
- Conformance baseline artifact: `reports/analytical/ANL-S05-CONFORMANCE.md`
- Conformance negative suite: `reports/security/conformance-negative-suite-RC-2026.02.12.md`
- Sprint closure artifact: `reports/analytical/ANL-S05-CLOSE.md`

---

## 8. RC-2026.02.12 productization transition controls (S07)

This section records release governance controls used for the regulatory/productization transition sprint.

### 8.1 Controlled productization package

Required S07 package artifacts:

- `reports/release/product-intent-addendum-RC-2026.02.12.md`
- `reports/release/design-control-trace-package-RC-2026.02.12.md`
- `reports/security/risk-file-baseline-RC-2026.02.12.md`
- `reports/security/cybersecurity-submission-bundle-RC-2026.02.12.md`
- `reports/clinical/usability-summative-bundle-RC-2026.02.12.md`
- `reports/interoperability/release-evidence-index.md` (external-evidence dependency lock)
- `reports/analytical/ANL-S07-CLOSE.md`

### 8.2 CAPA and PMCF governance linkage

Productization release governance must remain coupled to PMCF/CAPA controls:

- PMCF baseline: `reports/pmcf/PMCF-040.md`
- CAPA trigger policy: `reports/pmcf/capa-threshold-policy-RC-2026.02.11.md`
- Incident taxonomy: `reports/pmcf/incident-taxonomy-matrix-RC-2026.02.11.md`

Operational decision rule for S07 package:

- GO decisions require explicit confirmation that no CAPA-trigger threshold is breached for release evidence completeness, traceability linkage, or fail-closed security controls.
- Any S2/S3 threshold breach reopens governance review and invalidates closure artifacts until corrected evidence is signed.

### 8.3 Auditable gate execution map (S07)

| S07 Gate Item | Required Evidence | Owner |
|---|---|---|
| Product intended-use/claim separation is explicit | `reports/release/product-intent-addendum-RC-2026.02.12.md`, `docs/15-Regulatory-and-Standards-Mapping.md` | Regulatory/Quality Lead |
| Requirement->architecture->verification->risk trace package complete | `reports/release/design-control-trace-package-RC-2026.02.12.md` | Systems Engineering Lead |
| High-severity hazards have verified controls and residual decisions | `reports/security/risk-file-baseline-RC-2026.02.12.md` | Security Lead + QA/RA Risk Lead |
| Cybersecurity package includes SBOM/vulnerability/pen-test/secure-default proofs | `reports/security/cybersecurity-submission-bundle-RC-2026.02.12.md` | Security Lead |
| Usability summative evidence and residual-use-risk rationale are signed | `reports/clinical/usability-summative-bundle-RC-2026.02.12.md` | Clinical Usability Lead |
| Interoperability dossier uses external independent evidence only | `reports/interoperability/release-evidence-index.md` | Interoperability Lead |
| Sprint closure checklist complete and signed | `reports/analytical/ANL-S07-CLOSE.md` | Release Manager |

### 7.1 Documentation reconciliation migration note (RC-2026.02.12)

To align claims with implemented behavior:
- web rendering wording was corrected to CPU/canvas active path for WASM,
- `REQ-CONF-088` was narrowed to current SR extraction scope,
- transfer syntax support was split by processing layer (I/O recognition vs pixel decode),
- volume/MPR wording was marked deferred until implementation.

Migration guidance:
- Integrators that previously interpreted web GPU rendering, SR write APIs, or MPR as active baseline behavior must treat those capabilities as deferred until promoted in `docs/31-Implementation-Status.md` and release notes.

## 9. API evolution notes (2026-02-22)

This wave promotes additional baseline APIs while preserving backward compatibility.

Promoted baseline surfaces:
- patient-space-capable `VolumeGrid` metadata and mapping helpers,
- patient-space MPR request APIs and slab controls,
- PET/CT fusion baseline execution APIs,
- SR workflow service create/update/retrieve commit APIs.

Compatibility guarantees:
- Existing voxel-space `MprRequest` callers remain supported.
- `LegacyVolumeGrid` compatibility adapters remain available for schema migration.
- SR builder/update APIs remain unchanged; service commit workflow is additive.
- Profile packaging and capability manifests are additive tooling and do not alter runtime API contracts.

Integrator migration references:
- `docs/36-Migration-Guide-IO-SR-MPR.md`
- `docs/39-SR-Workflow-Architecture.md`
- `docs/43-Error-Code-Mapping-Workflows.md`
