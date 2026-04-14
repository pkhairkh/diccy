#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

TTFI_REPORT="${1:-reports/performance/time-to-first-image.json}"
BROWSER_REPORT="${2:-reports/performance/web-renderer-harness.json}"
SUMMARY_REPORT="${3:-reports/performance/ux-benchmark-suite.json}"

mkdir -p "$(dirname "${TTFI_REPORT}")"
mkdir -p "$(dirname "${BROWSER_REPORT}")"
mkdir -p "$(dirname "${SUMMARY_REPORT}")"

./tools/time_to_first_image.sh "${TTFI_REPORT}"
./tools/browser_perf_renderer.sh --output "${BROWSER_REPORT}"

python3 - <<'PY' "${TTFI_REPORT}" "${BROWSER_REPORT}" "${SUMMARY_REPORT}"
import json
import pathlib
import sys

ttfi_path = pathlib.Path(sys.argv[1])
browser_path = pathlib.Path(sys.argv[2])
summary_path = pathlib.Path(sys.argv[3])

ttfi = json.loads(ttfi_path.read_text(encoding="utf-8"))
browser = json.loads(browser_path.read_text(encoding="utf-8"))

summary = {
    "suite": "ux-benchmark-suite",
    "time_to_first_image_ms": ttfi.get("elapsed_ms"),
    "interaction_latency_ms": browser.get("interaction_duration_ms"),
    "fallback_latency_ms": (
        browser.get("fallback", {}).get("last_fallback_latency_ms")
    ),
    "fallback_budget_ms": (
        browser.get("fallback", {}).get("fallback_latency_budget_ms")
    ),
    "artifacts": {
        "ttfi": str(ttfi_path),
        "browser": str(browser_path),
    },
}
summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
print(f"wrote UX benchmark summary: {summary_path}")
PY
