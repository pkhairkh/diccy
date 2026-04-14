# viewer-wasm web host

This directory contains a static browser host for `viewer-wasm`.

Run from repository root:

```bash
cargo install wasm-bindgen-cli
./tools/run_viewer_wasm_frontend.sh --port 4173
```

HTTPS (TLS) launch (recommended for WebGPU on non-localhost origins):

```bash
./tools/run_viewer_wasm_frontend.sh --https --port 4173
```

Optional network-enabled build:

```bash
./tools/run_viewer_wasm_frontend.sh --features network
```

Then open `http://127.0.0.1:4173` (or `https://127.0.0.1:4173` with `--https`).

Host runtime defaults:
- Production WebGPU path is enabled by default and will run whenever browser/device support is available.
- If WebGPU is unavailable or fails, presentation falls back to CPU deterministically.
- The launcher script compiles `viewer-wasm` with `webgpu-backend` enabled by default.
- The host exposes same-origin proxy routes by default:
  - `/dicomweb` -> `http://127.0.0.1:8080`
  - `/workflow` -> `http://127.0.0.1:8082`
  This avoids mixed-content blocks when the frontend is served via HTTPS.
- Ingestion supports both single-file selection and folder selection; folder ingest opens the first decodable DICOM instance in deterministic filename order.
- WebGPU in browsers typically requires a secure origin. For LAN access (for example `http://10.x.x.x:4173`), use HTTPS to keep WebGPU available.

Deterministic preset builds:

```bash
./tools/wasm_build_presets.sh
```

Browser smoke tests (Chromium + Firefox, Playwright required):

```bash
RDVF_WASM_SMOKE_URL=http://127.0.0.1:4173 ./tools/browser_smoke_renderer.sh
```

Browser visual regression for key flow screenshots (MPR baseline, MPR slab baseline, fusion baseline):

```bash
RDVF_WASM_SMOKE_URL=http://127.0.0.1:4173 ./tools/browser_ui_flow_screenshot.sh
```

Record baseline images first (first run, or after intentionally changing render outputs):

```bash
RDVF_UI_FLOW_SCREENSHOT_RECORD=1 RDVF_WASM_SMOKE_URL=http://127.0.0.1:4173 ./tools/browser_ui_flow_screenshot.sh
```

Browser performance harness (large-study interaction profile):

```bash
RDVF_WASM_SMOKE_URL=http://127.0.0.1:4173 ./tools/browser_perf_renderer.sh --output reports/performance/web-renderer-harness.json
python3 tools/webgpu_dashboard_report.py --inputs 'reports/performance/web-renderer-harness*.json' --output reports/performance/webgpu-dashboard.md
```
