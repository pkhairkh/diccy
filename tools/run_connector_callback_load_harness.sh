#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_LABEL="${1:-connector-callback-load}"

python3 "${ROOT_DIR}/tools/performance_harness.py" \
  --profile "${ROOT_DIR}/reports/performance/profiles/connector-callback-load-profile-RC-2026.02.24.json" \
  --output-md "${ROOT_DIR}/reports/performance/${RUN_LABEL}.md" \
  --output-json "${ROOT_DIR}/reports/performance/${RUN_LABEL}.json" \
  --run-label "${RUN_LABEL}" \
  --fail-on-threshold-breach
