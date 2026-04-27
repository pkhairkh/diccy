#!/usr/bin/env bash
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  echo "node not found; skipping browser SR workflow integration spec."
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SPEC="${ROOT_DIR}/crates/viewer-wasm/web/sr.workflow.spec.mjs"

if [[ ! -f "${SPEC}" ]]; then
  echo "spec missing: ${SPEC}"
  exit 1
fi

if [[ "${DICCY_WORKFLOW_INTEGRATION:-0}" != "1" ]]; then
  echo "DICCY_WORKFLOW_INTEGRATION=1 not set; skipping SR workflow integration browser test."
  exit 0
fi

if ! node -e "require.resolve('@playwright/test')" >/dev/null 2>&1; then
  echo "@playwright/test not installed; skipping browser SR workflow integration spec."
  exit 0
fi

node --eval "import('@playwright/test').then(() => process.exit(0)).catch(() => process.exit(1))" >/dev/null 2>&1
npx playwright test "${SPEC}" "$@"
