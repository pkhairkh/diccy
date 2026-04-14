#!/usr/bin/env bash
set -euo pipefail

if ! command -v node >/dev/null 2>&1; then
  echo "node not found; skipping browser renderer performance harness."
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HARNESS="${ROOT_DIR}/tools/web_renderer_perf_harness.mjs"

if [[ ! -f "${HARNESS}" ]]; then
  echo "harness missing: ${HARNESS}"
  exit 1
fi

node "${HARNESS}" "$@"
