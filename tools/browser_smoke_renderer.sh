#!/usr/bin/env bash
set -euo pipefail

# Deterministic smoke runner entrypoint for browser integration coverage.
# This script is intentionally conservative: it only runs when Node is
# available in the caller environment.

if ! command -v node >/dev/null 2>&1; then
  echo "node not found; skipping browser smoke tests."
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEST_FILE="${ROOT_DIR}/tools/web_renderer_integration_test.mjs"

if [[ ! -f "${TEST_FILE}" ]]; then
  echo "smoke spec missing: ${TEST_FILE}"
  exit 1
fi

node "${TEST_FILE}"
