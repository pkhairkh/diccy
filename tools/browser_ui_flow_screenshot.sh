#!/usr/bin/env bash
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  echo "node not found; skipping browser UI flow screenshot automation."
  exit 0
fi

if ! node --eval "import('playwright').then(() => process.exit(0)).catch(() => process.exit(1))" >/dev/null 2>&1; then
  echo "playwright dependency not found; skipping browser UI flow screenshot automation."
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="${ROOT_DIR}/tools/web_ui_flow_screenshot_diff.mjs"

if [[ ! -f "${SCRIPT}" ]]; then
  echo "visual flow script missing: ${SCRIPT}"
  exit 1
fi

node "${SCRIPT}"
