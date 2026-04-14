#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

usage() {
  cat <<EOF
Usage: ${0##*/} [--no-build] [--log-dir DIR] [--web-port PORT] [--webserver-bind BIND] [--workflow-bind BIND] [--health-timeout SECONDS]

Start the minimal production-like DICOMweb + workflow + frontend profile in one command.

Environment defaults are conservative and path-safe:
- frontend port: 4173
- web server bind: 127.0.0.1:8080
- workflow bind: 127.0.0.1:8082
- state directory: ./state
- explicit health checks for `/healthz` and frontend root (default timeout: 35s)
EOF
}

if [[ "${1-}" == "-h" || "${1-}" == "--help" ]]; then
  usage
  exit 0
fi

BUILD_TARGETS="${BUILD_TARGETS:-1}"
LOG_DIR="${LOG_DIR:-reports/local-minimal-bootstrap}"
FRONTEND_PORT="${FRONTEND_PORT:-4173}"
WEB_BIND="${WEB_BIND:-127.0.0.1:8080}"
WORKFLOW_BIND="${WORKFLOW_BIND:-127.0.0.1:8082}"
HEALTH_CHECK_TIMEOUT="${HEALTH_CHECK_TIMEOUT:-35}"

while [[ $# -gt 0 ]]; do
  case "${1}" in
    --no-build)
      BUILD_TARGETS=0
      ;;
    --log-dir)
      shift
      LOG_DIR="${1}"
      ;;
    --web-port)
      shift
      FRONTEND_PORT="${1}"
      ;;
    --webserver-bind)
      shift
      WEB_BIND="${1}"
      ;;
    --workflow-bind)
      shift
      WORKFLOW_BIND="${1}"
      ;;
    --health-timeout)
      shift
      HEALTH_CHECK_TIMEOUT="${1}"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unsupported argument: ${1}" >&2
      usage
      exit 1
      ;;
  esac
  shift
done

if [[ "${BUILD_TARGETS}" == "1" ]]; then
  cargo build -p dicom-web-server -p dicom-workflow-server -p dicom-visualizer
fi

mkdir -p "${LOG_DIR}" state/web state/workflow

check_port_free() {
  local bind="$1"
  local port="${bind##*:}"
  if command -v ss >/dev/null 2>&1; then
    if ss -ltn "sport = :${port}" | grep -q ":${port}"; then
      echo "error: port ${port} already bound (${bind})"
      return 1
    fi
  fi
}

check_dir() {
  local path="$1"
  if [[ ! -d "${path}" ]]; then
    mkdir -p "${path}"
  fi
  if [[ ! -w "${path}" ]]; then
    echo "error: directory not writable: ${path}"
    return 1
  fi
}

wait_for_health() {
  local name="$1"
  local bind="$2"
  local endpoint="$3"
  local host_and_port="${bind}"
  local target
  target="http://${host_and_port}${endpoint}"
  echo "[health] waiting for ${name} at ${target}"
  if ! timeout "${HEALTH_CHECK_TIMEOUT}" bash -c "
    until curl -fsS '${target}' >/dev/null 2>&1; do
      sleep 1
    done
  "; then
    echo "error: ${name} did not become healthy within ${HEALTH_CHECK_TIMEOUT}s (${target})"
    return 1
  fi
  echo "[health] ${name} healthy"
}

check_port_free "${WEB_BIND}"
check_port_free "${WORKFLOW_BIND}"
check_dir "${ROOT_DIR}/state"

trap 'kill ${PID_WEB:-} ${PID_WORKFLOW:-} ${PID_FRONTEND:-} >/dev/null 2>&1 || true' EXIT INT TERM

export DICOM_WEB_BIND="${WEB_BIND}"
export DICOM_WORKFLOW_BIND="${WORKFLOW_BIND}"

echo "[1/3] Starting dicom-web-server on ${DICOM_WEB_BIND}"
cargo run -p dicom-web-server >"${LOG_DIR}/dicom-web-server.log" 2>&1 &
PID_WEB=$!

echo "[2/3] Starting dicom-workflow-server on ${DICOM_WORKFLOW_BIND}"
cargo run -p dicom-workflow-server >"${LOG_DIR}/dicom-workflow-server.log" 2>&1 &
PID_WORKFLOW=$!

echo "[3/3] Starting viewer-wasm frontend on http://127.0.0.1:${FRONTEND_PORT}"
./tools/run_viewer_wasm_frontend.sh --no-build --port "${FRONTEND_PORT}" >"${LOG_DIR}/viewer-wasm-frontend.log" 2>&1 &
PID_FRONTEND=$!

wait_for_health "dicom-web-server" "${DICOM_WEB_BIND}" "/healthz"
wait_for_health "dicom-workflow-server" "${DICOM_WORKFLOW_BIND}" "/healthz"
wait_for_health "viewer-wasm-host" "127.0.0.1:${FRONTEND_PORT}" "/"

echo "Minimal profile started."
echo "Viewer:   http://127.0.0.1:${FRONTEND_PORT}"
echo "DICOMweb: ${DICOM_WEB_BIND}"
echo "Workflow: ${DICOM_WORKFLOW_BIND}"
echo "Logs:    ${LOG_DIR}"
echo "Press Ctrl+C to stop all services."

wait "${PID_WEB}" "${PID_WORKFLOW}" "${PID_FRONTEND}"
