#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUTPUT_PATH="${1:-reports/performance/time-to-first-image.json}"
mkdir -p "$(dirname "${OUTPUT_PATH}")"

echo "Running time-to-first-image benchmark (onboarding baseline)..."
start_epoch_ms="$(date +%s%3N)"

cargo test -p viewer-wasm ingest_boundary_validates_p10_before_state_mutation -- --exact >/tmp/diccy_ttfi.log 2>&1

end_epoch_ms="$(date +%s%3N)"
elapsed_ms="$((end_epoch_ms - start_epoch_ms))"

cat >"${OUTPUT_PATH}" <<EOF
{
  "benchmark": "time_to_first_image_onboarding",
  "started_epoch_ms": ${start_epoch_ms},
  "ended_epoch_ms": ${end_epoch_ms},
  "elapsed_ms": ${elapsed_ms},
  "command": "cargo test -p viewer-wasm ingest_boundary_validates_p10_before_state_mutation -- --exact"
}
EOF

echo "TTFI benchmark complete: ${elapsed_ms} ms"
echo "Report written to ${OUTPUT_PATH}"
