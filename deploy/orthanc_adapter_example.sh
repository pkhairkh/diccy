#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Orthanc integration adapter example (reproducible)

Usage:
  ./deploy/orthanc_adapter_example.sh find-study [PATIENT_ID]
  ./deploy/orthanc_adapter_example.sh stow-file /path/to/file.dcm [STUDY_UID]

Environment:
  ORTHANC_BASE_URL   (default: http://127.0.0.1:8042)
  ORTHANC_USER       (optional)
  ORTHANC_PASS       (optional)
  DICOMWEB_BASE_URL  (default: http://127.0.0.1:8080)

Examples:
  ./deploy/orthanc_adapter_example.sh find-study
  ./deploy/orthanc_adapter_example.sh find-study PATIENT-42
  ./deploy/orthanc_adapter_example.sh stow-file ./sample.dcm
EOF
}

ORTHANC_BASE_URL="${ORTHANC_BASE_URL:-http://127.0.0.1:8042}"
DICOMWEB_BASE_URL="${DICOMWEB_BASE_URL:-http://127.0.0.1:8080}"

auth_flags=()
if [[ -n "${ORTHANC_USER:-}" || -n "${ORTHANC_PASS:-}" ]]; then
  auth_flags=(-u "${ORTHANC_USER:-}:${ORTHANC_PASS:-}")
fi

cmd="${1:-}"
case "${cmd}" in
  find-study)
    patient_id="${2:-}"
    if [[ -n "${patient_id}" ]]; then
      query_payload="$(cat <<EOF
{"Level":"Study","Query":{"PatientID":"${patient_id}"}}
EOF
)"
    else
      query_payload='{"Level":"Study","Query":{}}'
    fi
    echo "Querying Orthanc studies via /tools/find..."
    curl -fsSL "${auth_flags[@]}" \
      -H "Content-Type: application/json" \
      -d "${query_payload}" \
      "${ORTHANC_BASE_URL}/tools/find"
    echo
    ;;
  stow-file)
    dicom_file="${2:-}"
    study_uid="${3:-}"
    if [[ -z "${dicom_file}" ]]; then
      echo "missing DICOM file path"
      usage
      exit 2
    fi
    if [[ ! -f "${dicom_file}" ]]; then
      echo "file not found: ${dicom_file}"
      exit 2
    fi
    if [[ -n "${study_uid}" ]]; then
      target="${DICOMWEB_BASE_URL}/studies/${study_uid}"
    else
      target="${DICOMWEB_BASE_URL}/studies"
    fi
    echo "Forwarding DICOM file to DICOMweb STOW endpoint: ${target}"
    curl -fsSL \
      -X POST \
      -H "Content-Type: application/dicom" \
      --data-binary "@${dicom_file}" \
      "${target}"
    echo
    ;;
  *)
    usage
    exit 2
    ;;
esac
