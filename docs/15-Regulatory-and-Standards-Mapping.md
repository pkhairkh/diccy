# Regulatory and standards mapping (technical)

This document is **informative**. It describes technical integration consequences when RDVF is embedded into systems that are marketed, labeled, or deployed for medical purposes. RDVF itself is a framework; regulatory authorization and deployment claims are controlled by integrators/downstream products.

RDVF design goal: provide a *workflow-complete PACS workstation* baseline while preventing accidental expansion into unsupported automation via explicit scope boundaries, deterministic pipelines, and verifiable limits.

---

## 0. Mapping control requirements (normative)

Requirements:

- **REQ-STD-101:** This document **MUST** remain informative and **MUST NOT** introduce regulatory authorization or diagnostic/therapeutic claims. The canonical intended-purpose text remains in `docs/01-Vision-and-Scope.md`.
- **REQ-STD-102:** This document **MUST** include explicit cross-links to the normative sources governing claim surface control (including conformance envelope and measurement gating), determinism, limits, traceability, telemetry redaction, and release/SBOM controls.
- **REQ-STD-103:** Each primary reference listed in this document **MUST** include a publication date and a stable URL.

Required cross-links (normative):

- `docs/03-DICOM-Conformance-Envelope.md` (envelope boundaries)
- `docs/01-Vision-and-Scope.md` (claim surface and intended purpose)
- `docs/06-Rendering-and-Interaction.md` (measurement gating)
- `docs/05-Pixel-Pipeline.md` (determinism and CPU oracle)
- `docs/08-WASM-Target.md` (WASM portability constraints)
- `docs/09-Security-Threat-Model.md` (limits and fuzzing posture)
- `docs/10-Testing-Corpus-Fuzzing-Evals.md` (traceability and verification)
- `docs/13-Error-Model-and-Telemetry.md` (telemetry redaction)
- `docs/14-Release-and-Versioning.md` (SBOM and release artifacts)

Verification:

- Documentation lint checks that the cross-links above are present and resolve to existing paths.
- Documentation lint scans for prohibited regulatory claim phrases (see `docs/01-Vision-and-Scope.md`).
- Documentation review confirms that each primary reference includes a date and URL.

---

## 1. Intended purpose and regulatory trigger surfaces (engineering view)

### 1.1 Intended purpose is an integration boundary

- RDVF defines a single canonical **Framework Intended Purpose** statement for PACS workstation and clinical workflow scope (see `docs/01-Vision-and-Scope.md`).
- Downstream deployment programs are responsible for the documentation, V&V, cybersecurity, and lifecycle evidence required for their regulated use context.

Key references (primary):
- FDA MDDS / image storage / communications guidance (2022-09-28): “transfer/store/convert formats/display” scope for low-risk policy posture.  
  https://www.fda.gov/regulatory-information/search-fda-guidance-documents/medical-device-data-systems-medical-image-storage-devices-and-medical-image-communications-devices
- EU MDR (Regulation (EU) 2017/745), Annex VIII Rule 11: software providing information used to take diagnostic/therapeutic decisions is classified at least class IIa.  
  https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A32017R0745
- MDCG 2019-11 rev.1 (June 2025): qualification/classification depends on intended purpose.  
  https://health.ec.europa.eu/document/download/b45335c5-1679-4c71-a91c-fc7a4d37f12b_en

### 1.2 Function-level trigger patterns

The following *technical capabilities* are common “trigger surfaces” for downstream medical-purpose claims. RDVF supports some of these as framework mechanics; therefore RDVF provides guardrails:

- quantitative measurements in physical units (mm) tied to patient-space geometry,
- advanced reconstruction outputs (MPR/volume rendering) used for decisions,
- automatic parameter selection (auto-window) that may be perceived as “analysis” if not carefully bounded and labeled,
- any inference/segmentation/classification (out of scope by default).

RDVF control strategy (technical):
- explicit feature gating for quantitative modes,
- deterministic CPU reference pipeline as source of truth,
- fail-closed policies for missing/invalid geometry,
- verifiable default limits and fuzzing.

---

## 2. US (FDA) mapping: “display-only” vs device software functions

### 2.1 Display-only posture and MDDS-adjacent constraints

FDA’s MDDS/image storage/image communications guidance describes a low-risk policy posture for software functions limited to transfer/storage/format conversion/display. RDVF extends beyond display-only posture into workstation workflow capabilities, so integrators should map deployment claims and controls accordingly.

RDVF technical controls continue to emphasize:

- explicit service policy controls for networking and workflow endpoints,
- deterministic, documented transforms and explicit failure behavior,
- fail-closed security limits, auditability, and traceability.

Primary references:
- MDDS/image storage/image communications guidance (PDF).  
  https://www.fda.gov/media/88572/download
- 21st Century Cures Act context (section 520(o)(1)(D) excerpt).  
  https://www.fda.gov/media/109622/download

### 2.2 Device software function documentation expectations (when applicable downstream)

If an integrator positions a product as a device software function, FDA software documentation expectations typically include architecture description, requirements, risk analysis, and V&V evidence.

Primary references:
- “Content of Premarket Submissions for Device Software Functions” (June 2023).  
  https://www.fda.gov/regulatory-information/search-fda-guidance-documents/content-premarket-submissions-device-software-functions
- “Off-The-Shelf Software Use in Medical Devices” (August 2023).  
  https://www.fda.gov/regulatory-information/search-fda-guidance-documents/shelf-software-use-medical-devices

### 2.3 Cybersecurity expectations (when applicable downstream)

RDVF’s threat model, limits, and fuzzing are designed to support a secure development posture consistent with modern medical-device cybersecurity expectations.

Primary reference:
- FDA “Cybersecurity in Medical Devices: Quality Management System Considerations and Content of Premarket Submissions” (issued 2026-02-03).  
  https://www.fda.gov/regulatory-information/search-fda-guidance-documents/cybersecurity-medical-devices-quality-management-system-considerations-and-content-premarket  
  PDF: https://www.fda.gov/media/119933/download

---

## 3. EU MDR / MDCG mapping: Rule 11, GSPR, and cybersecurity

### 3.1 Rule 11 classification trigger surface

EU MDR Annex VIII Rule 11 states (normative text in MDR) that software intended to provide information used for diagnostic/therapeutic decisions is class IIa or higher depending on impact. RDVF guardrails are intended to keep *default* framework claims away from that surface and make downstream shifts explicit via features/packs.

Primary reference:
- EU MDR (Regulation (EU) 2017/745), Annex VIII Rule 11.  
  https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A32017R0745

### 3.2 Annex I (GSPR) engineering implications

MDR Annex I contains general safety/performance requirements including software lifecycle, risk management, and IT security considerations. RDVF implements a technical substrate (limits, deterministic outputs, fuzzing, supply-chain controls) that downstream products can extend into a full lifecycle system.

Primary references:
- EU MDR (Annex I sections 17.x in the official text).  
  https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A32017R0745
- MDCG 2019-16 rev.1 (July 2020): cybersecurity guidance and “minimum IT security measures” framing.  
  https://health.ec.europa.eu/document/download/b23b362f-8a56-434c-922a-5b3ca4d0a7a1_en

---

## 4. Determinism research: WebGPU/WGSL and WebAssembly numerics

RDVF correctness oracle is the **CPU boundary output** (`Luma8`/`Rgba8`) produced by the pixel pipeline. GPU rendering is treated as a presentation layer and is constrained to avoid becoming a numeric source of truth.

### 4.1 WebGPU/WGSL floating-point indeterminacy

WebGPU specification text notes that floating-point NaN/Infinity values may be replaced by indeterminate values. This makes GPU-side numeric pipelines unsuitable as a strict byte-identical reference across heterogeneous drivers/backends.

Primary references:
- WebGPU specification (2026-01-29):  
  https://www.w3.org/TR/webgpu/
- WGSL specification:  
  https://www.w3.org/TR/WGSL/

RDVF design response:
- CPU-oracle and GPU constraints are defined by **REQ-PIX-201** and **REQ-GPU-210** in `docs/05-Pixel-Pipeline.md`.

### 4.2 WebAssembly NaN propagation nondeterminism and deterministic profile

WebAssembly allows nondeterministic selection of NaN payload bits in some cases; the spec describes a “deterministic profile” that canonicalizes NaNs.

Primary reference:
- WebAssembly core spec, Numerics section (2026-01-21):  
  https://webassembly.github.io/spec/core/exec/numerics.html

RDVF design response:
- Non-finite rejection and NaN/Infinity avoidance are defined by **REQ-PIX-220** (`docs/05-Pixel-Pipeline.md`) and **REQ-WASM-301** (`docs/08-WASM-Target.md`).

---

## 5. Research-driven guidance: web viewer variability and scalable volume rendering

### 5.1 Cross-browser performance variability

Studies comparing web-based DICOM viewers report performance variability across browsers and OS, motivating explicit performance matrices and budgets.

Reference:
- Pereira et al., “Web-Based DICOM Viewers: A Survey and a Performance Classification” (J Imaging Informatics in Medicine, 2024/2025).  
  https://pubmed.ncbi.nlm.nih.gov/39349783/

RDVF design response:
- Cross-browser performance matrices and budgets are specified in `docs/11-Performance-and-Caching.md` and `docs/08-WASM-Target.md`.

### 5.2 Scalable volume rendering: residency/LOD patterns

WebGPU-based volume rendering at scale often uses out-of-core residency and LOD/streaming approaches.

Reference:
- Herzberger et al., “Residency Octree: A Hybrid Approach for Scalable Web-Based Multi-Volume Rendering” (IEEE TVCG, 2024).  
  PDF: https://www.cg.tuwien.ac.at/research/publications/2024/herzberger-2024-roh/herzberger-2024-roh-paper.pdf

RDVF design response:
- RDVF volume features specify strict default limits and an optional LOD/residency extension path (see `docs/07-Volume-and-Fusion.md` and `docs/11-Performance-and-Caching.md`).

---

## 6. Threat research: DICOM as a hostile carrier

RDVF threat model assumes DICOM inputs are hostile and requires fuzzing, limits, and fail-closed behavior.

Reference:
- Mishra & Bagade, “MalDicom: A Memory Forensic Framework for Detecting Malicious Payload in DICOM Files” (arXiv 2023).  
  https://arxiv.org/abs/2312.00483

RDVF design response:
- Limits and fuzzing requirements are specified in `docs/09-Security-Threat-Model.md` and `docs/10-Testing-Corpus-Fuzzing-Evals.md`.

---

## 7. Practical integration guidance (technical)

If a downstream product needs compliance-grade traceability artifacts, treat RDVF docs as the starting “spec layer” and add:

- requirement-to-test traceability reports (REQ IDs → tests),
- risk management file structure (hazards, harms, mitigations → verification),
- corpus/fuzzing governance and evidence (see `docs/10-Testing-Corpus-Fuzzing-Evals.md`),
- telemetry redaction policy and evidence (see `docs/13-Error-Model-and-Telemetry.md`),
- SBOM + vulnerability management and disclosure mapping,
- environment minimums (display/hardware/network assumptions).

RDVF defines the minimal skeleton of these mechanics in:
- `docs/10-Testing-Corpus-Fuzzing-Evals.md` (traceability + evidence),
- `docs/14-Release-and-Versioning.md` (release artifacts, SBOM),
- `docs/09-Security-Threat-Model.md` (limits, threat surfaces),
- `docs/13-Error-Model-and-Telemetry.md` (telemetry redaction).

---

## 8. Productization transition boundary (RC-2026.02.12)

This section records the explicit separation between framework baseline scope and release-scoped product claim packaging.

Boundary records:

- Framework baseline intended purpose (canonical, unchanged):
  - `README.md`
  - `docs/01-Vision-and-Scope.md`
- Product profile addendum (release-scoped claim package):
  - `reports/release/product-intent-addendum-RC-2026.02.12.md`

Technical control consequence:

- Any product-profile claim must remain mapped to signed analytical + clinical + PMCF evidence and must not expand the conformance envelope without an explicit envelope/version gate.
- Any deployment-profile change that alters claim scope, conformance rows, or runtime security defaults requires a new signed governance decision artifact under `reports/release/`.

Verification anchors:

- `docs/22-Claim-to-Evidence-Matrix.md`
- `reports/release/design-control-trace-package-RC-2026.02.12.md`
- `reports/analytical/ANL-S07-CLOSE.md`
