#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

FRONTEND_PORT="${DICCY_FRONTEND_PORT:-4173}"
WEB_BIND="${DICOM_WEB_BIND:-127.0.0.1:8080}"
WORKFLOW_BIND="${DICOM_WORKFLOW_BIND:-127.0.0.1:8082}"
DIMSE_BIND="${DICOM_DIMSE_BIND:-127.0.0.1:11112}"
DIMSE_HEALTH_BIND="${DICOM_DIMSE_HEALTH_BIND:-127.0.0.1:18112}"
PREVIEW_PROFILE="${STARTUP_PREVIEW_PROFILE:-backend-services}"
REQUIRED_DIRS="${STARTUP_PREVIEW_DIRS:-}"
STATE_LIMITS="${STARTUP_PREVIEW_STATE_LIMITS:-2097152}"

ok=true
needs_dimse_ports=false

case "${PREVIEW_PROFILE}" in
  core|backend-services)
    ;;
  dimse|backend-services-with-dimse)
    needs_dimse_ports=true
    ;;
  *)
    echo "FAIL: unsupported STARTUP_PREVIEW_PROFILE=${PREVIEW_PROFILE}"
    echo "Expected one of: backend-services, backend-services-with-dimse, core, dimse"
    exit 1
    ;;
esac

if [[ -z "${REQUIRED_DIRS}" ]]; then
  if [[ "${needs_dimse_ports}" == "true" ]]; then
    REQUIRED_DIRS="state,state/web,state/workflow,state/dimse"
  else
    REQUIRED_DIRS="state,state/web,state/workflow"
  fi
fi

check_port_free() {
  local bind="$1"
  local port="${bind##*:}"
  if command -v ss >/dev/null 2>&1; then
    if ss -ltn "sport = :${port}" | grep -q ":${port}"; then
      echo "FAIL: port already in use: ${bind}"
      ok=false
      return
    fi
  fi
  echo "PASS: port free check ${bind}"
}

check_dir() {
  local path="$1"
  if [[ ! -d "${path}" ]]; then
    mkdir -p "${path}"
  fi
  if [[ ! -w "${path}" ]]; then
    echo "FAIL: directory not writable ${path}"
    ok=false
    return
  fi
  echo "PASS: directory writable ${path}"
}

check_positive_int() {
  local label="$1"
  local value="$2"
  if ! [[ "${value}" =~ ^[0-9]+$ ]] || [[ "${value}" == "0" ]]; then
    echo "FAIL: ${label} must be a positive integer (${value})"
    ok=false
    return
  fi
  echo "PASS: ${label}=${value}"
}

echo "Startup preflight profile=${PREVIEW_PROFILE}"
check_port_free "${WEB_BIND}"
check_port_free "${WORKFLOW_BIND}"
if [[ "${needs_dimse_ports}" == "true" ]]; then
  check_port_free "${DIMSE_BIND}"
  check_port_free "${DIMSE_HEALTH_BIND}"
fi
IFS="," read -r -a dirs <<<"${REQUIRED_DIRS}"
for path in "${dirs[@]}"; do
  check_dir "${path}"
done

check_positive_int "frontend port" "${FRONTEND_PORT}"
check_positive_int "frontend cache size limit (bytes)" "${STATE_LIMITS}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "WARN: cargo not found in PATH"
fi

echo "Startup preflight artifacts:"
if [[ "${ok}" == "true" ]]; then
  echo "PASS: startup preflight complete"
  exit 0
else
  echo "FAIL: startup preflight blocked"
  exit 1
fi
