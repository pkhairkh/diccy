#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

WIDTH="${WIDTH:-64}"
HEIGHT="${HEIGHT:-64}"
DEPTH="${DEPTH:-32}"
ITERATIONS="${ITERATIONS:-12}"
BUDGET_MS="${BUDGET_MS:-2000}"
SCALE="${SCALE:-0.5}"

echo "Running deterministic PET/CT fusion perf harness..."
echo "dims=${WIDTH}x${HEIGHT}x${DEPTH} iterations=${ITERATIONS} budget_ms=${BUDGET_MS} scale=${SCALE}"

cargo run -p modality-pet --bin fusion_benchmark -- \
  --width="${WIDTH}" \
  --height="${HEIGHT}" \
  --depth="${DEPTH}" \
  --iterations="${ITERATIONS}" \
  --budget-ms="${BUDGET_MS}" \
  --scale="${SCALE}"
