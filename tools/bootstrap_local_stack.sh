#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

if [[ "${1-}" == "-h" || "${1-}" == "--help" ]]; then
  echo "Usage: ${0##*/} [--dimse]"
  echo "  --dimse    start DIMSE-integrated profile"
  exit 0
fi

if [[ "${1-}" == "--dimse" ]]; then
  exec ./tools/bootstrap_dimse_profile.sh
fi

exec ./tools/bootstrap_minimal_profile.sh
