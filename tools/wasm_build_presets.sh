#!/usr/bin/env bash
set -euo pipefail

# Deterministic viewer-wasm preset builds:
# - cpu-only: default features only
# - webgpu: explicit webgpu-backend feature enabled

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT_DIR="${ROOT_DIR}/target/wasm-presets"
mkdir -p "${OUT_DIR}"

build_preset() {
  local name="$1"
  shift
  local dest="${OUT_DIR}/${name}"
  mkdir -p "${dest}"
  cargo build -p viewer-wasm --target wasm32-unknown-unknown "$@"
  cp "${ROOT_DIR}/target/wasm32-unknown-unknown/debug/viewer_wasm.wasm" "${dest}/viewer_wasm.wasm"
}

build_preset cpu-only --no-default-features
build_preset webgpu --no-default-features --features webgpu-backend

echo "Built presets in ${OUT_DIR}"
