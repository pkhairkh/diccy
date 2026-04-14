#!/usr/bin/env bash
set -eo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

SCRIPT_NAME="$(basename "${BASH_SOURCE[0]}")"
export DIST_DIR
export RELEASE_ID

PACKAGE_RELEASE_DIR="${PACKAGE_RELEASE_DIR:-target/release}"
PACKAGE_DEBUG_DIR="${PACKAGE_DEBUG_DIR:-target/debug}"

usage() {
  cat <<EOF
Usage:
  ${SCRIPT_NAME} [dist_dir] [--release-id ID] [--include-dimse] [--require-dimse] [--validate] [--no-sign]

Build and package production profiles with deterministic metadata.

Options:
  --release-id ID        release identifier for artifact naming (default: PROFILE-YYYYMMDD)
  --include-dimse        include backend-services-with-dimse profile
  --require-dimse        fail when packaging a backend-services profile without DIMSE profile inclusion
  --profiles LIST        comma-separated explicit profile set (e.g. framework-core,workstation,backend-services)
  --validate             validate packaged profile claims against profile docs
  --no-sign              disable evidence signing even when EVIDENCE_SIGNING_KEY is set
  -h, --help            show this help
EOF
}

if [[ "${1-}" == "--help" || "${1-}" == "-h" ]]; then
  usage
  exit 0
fi

DIST_DIR="${1:-dist/profiles}"
shift || true

RELEASE_ID="PROFILE-$(date -u +%Y%m%d)"
INCLUDE_DIMSE=0
VALIDATE_CLAIMS=0
ENABLE_SIGNING=1
REQUIRE_DIMSE=0
REQUESTED_PROFILES=()

PROFILE_MATRIX_FILE="${PROFILE_MATRIX_FILE:-${ROOT_DIR}/tools/profile_matrix.json}"

append_profile() {
  local profile="$1"
  local existing
  for existing in "${PROFILES[@]-}"; do
    if [[ "${existing}" == "${profile}" ]]; then
      return 0
    fi
  done
  PROFILES+=("${profile}")
}

normalize_profile_token() {
  local profile="$1"
  profile="${profile//[[:space:]]/}"
  echo "${profile}"
}

profile_matrix_profiles() {
  python3 - "$PROFILE_MATRIX_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

for profile in data.get("profiles", {}):
    print(profile)
PY
}

profile_matrix_scalar() {
  local profile="$1"
  local field="$2"
  python3 - "$PROFILE_MATRIX_FILE" "$profile" "$field" <<'PY'
import json
import sys

path = sys.argv[1]
profile = sys.argv[2]
field = sys.argv[3]

with open(path, encoding="utf-8") as handle:
    data = json.load(handle)

entry = data.get("profiles", {}).get(profile)
if not isinstance(entry, dict):
    raise SystemExit(1)

value = entry.get(field)
if isinstance(value, bool):
    print("true" if value else "false")
elif value is None:
    print("")
elif isinstance(value, list):
    raise SystemExit(2)
else:
    print(value)
PY
}

profile_matrix_list() {
  local profile="$1"
  local field="$2"
  python3 - "$PROFILE_MATRIX_FILE" "$profile" "$field" <<'PY'
import json
import sys

path = sys.argv[1]
profile = sys.argv[2]
field = sys.argv[3]

with open(path, encoding="utf-8") as handle:
    data = json.load(handle)

entry = data.get("profiles", {}).get(profile)
if not isinstance(entry, dict):
    raise SystemExit(1)

value = entry.get(field, [])
if not isinstance(value, list):
    raise SystemExit(2)

for item in value:
    if isinstance(item, str):
        print(item)
PY
}

profile_features() {
  local profile="$1"
  python3 - "$PROFILE_MATRIX_FILE" "$profile" <<'PY'
import json
import sys

path = sys.argv[1]
profile = sys.argv[2]

with open(path, encoding="utf-8") as handle:
    data = json.load(handle)

entry = data.get("profiles", {}).get(profile)
if not isinstance(entry, dict):
    raise SystemExit(1)

features = entry.get("features", [])
if not isinstance(features, list):
    raise SystemExit(2)

print(
    "["
    + ",".join([f"\"{feature}\"" for feature in features])
    + "]"
)
PY
}

profile_binaries() {
  local profile="$1"
  profile_matrix_list "${profile}" "binaries"
}

profile_has_binaries() {
  local profile="$1"
  local -a binaries=()
  local binary

  while IFS= read -r binary; do
    [[ -z "${binary}" ]] && continue
    binaries+=("${binary}")
  done < <(profile_binaries "${profile}")

  if (( ${#binaries[@]} > 0 )); then
    return 0
  fi
  return 1
}

profile_image() {
  local profile="$1"
  local image_suffix
  image_suffix="$(profile_matrix_scalar "${profile}" "image_suffix")"
  printf 'ghcr.io/dicom-rd/workflow/runtime-profile:%s-%s' "${image_suffix}" "${RELEASE_ID}"
}

profile_build_cmd() {
  local profile="$1"
  profile_matrix_scalar "${profile}" "build_command"
}

profile_requires_array() {
  local profile="$1"
  profile_matrix_list "${profile}" "requires"
}

profile_default_profiles() {
  python3 - "$PROFILE_MATRIX_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

for profile, entry in data.get("profiles", {}).items():
    if not isinstance(entry, dict):
        continue
    if entry.get("default_profile", False):
        print(profile)
PY
}

profile_required_documentation_profiles() {
  python3 - "$PROFILE_MATRIX_FILE" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

for profile, entry in data.get("profiles", {}).items():
    if not isinstance(entry, dict):
        continue
    if entry.get("required_for_documentation", False):
        print(profile)
PY
}

array_contains() {
  local needle="$1"
  local candidate
  shift
  for candidate in "$@"; do
    if [[ "${candidate}" == "${needle}" ]]; then
      return 0
    fi
  done
  return 1
}

validate_requested_profiles() {
  local -a normalized=()
  local profile
  local required_profile
  local -a known_profiles=()

  while IFS= read -r profile; do
    known_profiles+=("${profile}")
  done < <(profile_matrix_profiles)
  if [[ "${#known_profiles[@]}" -eq 0 ]]; then
    echo "error: no profiles declared in ${PROFILE_MATRIX_FILE}"
    return 1
  fi

  for profile in "$@"; do
    profile="$(normalize_profile_token "${profile}")"
    if [[ -z "${profile}" ]]; then
      continue
    fi

    if ! printf '%s\n' "${known_profiles[@]}" | rg -q --fixed-strings "${profile}"; then
      echo "error: unknown profile '${profile}' in --profiles"
      echo "known profiles: ${known_profiles[*]}"
      return 1
    fi

    if ! printf '%s\n' "${normalized[@]}" | rg -q --fixed-strings "${profile}"; then
      normalized+=("${profile}")
    fi
  done

  if [[ "${#normalized[@]}" -eq 0 ]]; then
    echo "error: --profiles requires at least one valid profile"
    return 1
  fi

  for profile in "${normalized[@]}"; do
    while IFS= read -r required_profile; do
      if [[ -z "${required_profile}" ]]; then
        continue
      fi
      if ! array_contains "${required_profile}" "${normalized[@]}"; then
        echo "error: unknown profile combination: ${profile} requires ${required_profile}"
        return 1
      fi
    done < <(profile_requires_array "${profile}")
  done
  REQUESTED_PROFILES=("${normalized[@]}")
  return 0
}

while [[ $# -gt 0 ]]; do
  case "${1}" in
    --release-id)
      shift
      RELEASE_ID="${1}"
      ;;
    --include-dimse)
      INCLUDE_DIMSE=1
      ;;
    --require-dimse)
      REQUIRE_DIMSE=1
      ;;
    --profiles)
      shift
      if [[ -z "${1-}" ]]; then
        echo "error: --profiles requires a comma-separated list"
        usage
        exit 1
      fi
      IFS=',' read -r -a REQUESTED_PROFILES <<< "${1}"
      ;;
    --validate)
      VALIDATE_CLAIMS=1
      ;;
    --no-sign)
      ENABLE_SIGNING=0
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

if [[ "${#REQUESTED_PROFILES[@]}" -gt 0 ]]; then
  validate_requested_profiles "${REQUESTED_PROFILES[@]}" || exit 1
fi

mkdir -p "${DIST_DIR}"

ENVELOPE_VERSION="$(sed -n 's/^- `envelope_version`: `\([^`]*\)`/\1/p' docs/03-DICOM-Conformance-Envelope.md | head -n1)"
if [[ -z "${ENVELOPE_VERSION}" ]]; then
  ENVELOPE_VERSION="envelope-unknown"
fi

now_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
PROFILE_CLAIMS_TABLE_FILE="docs/33-Productization-Profiles-and-Playbooks.md"
PROFILE_CAPABILITY_MATRIX_FILE="docs/49-Runtime-Profile-Capability-Matrix.md"

declare -a REQUIRED_DOC_PROFILES=()
while IFS= read -r profile; do
  REQUIRED_DOC_PROFILES+=("${profile}")
done < <(profile_required_documentation_profiles)
declare -a METADATA_ARTIFACTS=()
declare -a PROFILES=()
while IFS= read -r profile; do
  PROFILES+=("${profile}")
done < <(profile_default_profiles)

if [[ "${#REQUESTED_PROFILES[@]}" -gt 0 ]]; then
  PROFILES=()
  for profile in "${REQUESTED_PROFILES[@]}"; do
    append_profile "${profile}" PROFILES
  done
fi
if [[ "${INCLUDE_DIMSE}" -eq 1 ]]; then
  PROFILES+=("backend-services-with-dimse")
fi
if (( ${#PROFILES[@]} == 0 )); then
  echo "error: no active profiles were selected"
  exit 1
fi

resolve_binary_path() {
  local binary="$1"
  local release_path
  local debug_path

  if [[ "${PACKAGE_RELEASE_DIR}" == /* ]]; then
    release_path="${PACKAGE_RELEASE_DIR}/${binary}"
  else
    release_path="${ROOT_DIR}/${PACKAGE_RELEASE_DIR}/${binary}"
  fi

  if [[ "${PACKAGE_DEBUG_DIR}" == /* ]]; then
    debug_path="${PACKAGE_DEBUG_DIR}/${binary}"
  else
    debug_path="${ROOT_DIR}/${PACKAGE_DEBUG_DIR}/${binary}"
  fi

  if [[ -x "${release_path}" ]]; then
    echo "${release_path}"
  elif [[ -x "${debug_path}" ]]; then
    echo "${debug_path}"
  else
    echo ""
  fi
}

collect_binary_info() {
  local binary="$1"
  local path="$2"
  local size sha
  size="$(wc -c "${path}" | awk '{print $1}')"
  sha="$(sha256sum "${path}" | awk '{print $1}')"
  cat <<JSON
    {
      "name": "${binary}",
      "path": "bin/${binary}",
      "size_bytes": ${size},
      "sha256": "${sha}"
    }
JSON
}

extract_doc_profile_entrypoints() {
  local profile="$1"
  if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 missing; cannot validate profile entrypoint claims in docs"
    return 1
  fi

  python3 - "$PROFILE_CLAIMS_TABLE_FILE" "$profile" <<'PY'
import sys

path = sys.argv[1]
profile = sys.argv[2]
lines = open(path, encoding="utf-8").read().splitlines()

in_runtime_artifacts = False
for line in lines:
    if line.startswith("## Runtime artifacts by profile"):
        in_runtime_artifacts = True
        continue
    if in_runtime_artifacts:
        if line.startswith("## ") and not line.startswith("## Runtime artifacts by profile"):
            break
        if not line.startswith("|"):
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) < 5 or not parts[1].startswith("`") or not parts[1].endswith("`"):
            continue
        normalized_profile = parts[1].strip(" `")
        if normalized_profile != profile:
            continue
        entrypoints = [item.strip().strip("`") for item in parts[3].split(",") if item.strip()]
        for entrypoint in entrypoints:
            if entrypoint:
                print(entrypoint)
        sys.exit(0)
        break
sys.exit(1)
PY
}

extract_doc_profile_archive_layout() {
  local profile="$1"
  if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 missing; cannot validate archive-layout claim table in docs"
    return 1
  fi

  python3 - "$PROFILE_CLAIMS_TABLE_FILE" "$profile" <<'PY'
import sys

path = sys.argv[1]
profile = sys.argv[2]
lines = open(path, encoding="utf-8").read().splitlines()

in_archive_layout = False
for line in lines:
    if line.startswith("## Archive layout by profile"):
        in_archive_layout = True
        continue
    if in_archive_layout:
        if line.startswith("## ") and not line.startswith("## Archive layout by profile"):
            break
        if not line.startswith("|"):
            continue
        parts = [part.strip() for part in line.split("|")]
        if len(parts) < 5 or not parts[1].startswith("`") or not parts[1].endswith("`"):
            continue
        normalized_profile = parts[1].strip(" `")
        if normalized_profile != profile:
            continue
        runtime_paths = [item.strip() for item in parts[3].split(",") if item.strip()]
        for path in runtime_paths:
            if path:
                print(path)
        sys.exit(0)
        break
sys.exit(1)
PY
}

validate_profile_claims() {
  local profile="$1"
  local profile_tar="$2"
  local expected_binary
  local manifest_binary
  local expected_manifest
  local manifest_json
  local -a expected_binaries=()
  local -a doc_binaries=()
  local -a doc_archive_paths=()
  local -a manifest_binaries=()

  if ! rg -q --fixed-strings "| \`${profile}\` " "${PROFILE_CLAIMS_TABLE_FILE}"; then
    echo "error: profile claim for '${profile}' missing in ${PROFILE_CLAIMS_TABLE_FILE}"
    return 1
  fi
  if ! rg -q --fixed-strings "${profile}" "${PROFILE_CAPABILITY_MATRIX_FILE}"; then
    echo "error: profile claim for '${profile}' missing in ${PROFILE_CAPABILITY_MATRIX_FILE}"
    return 1
  fi

  while IFS= read -r expected_binary; do
    expected_binaries+=("${expected_binary}")
  done < <(profile_binaries "${profile}")

  while IFS= read -r expected_binary; do
    doc_binaries+=("${expected_binary}")
  done < <(extract_doc_profile_entrypoints "${profile}")

  while IFS= read -r expected_binary; do
    doc_archive_paths+=("${expected_binary}")
  done < <(extract_doc_profile_archive_layout "${profile}")

  if [[ "${#expected_binaries[@]}" -gt 0 && "${#doc_binaries[@]}" -eq 0 ]]; then
    echo "error: profile '${profile}' missing entrypoint claim row in ${PROFILE_CLAIMS_TABLE_FILE}"
    return 1
  fi
  if [[ "${#doc_archive_paths[@]}" -eq 0 ]]; then
    echo "error: profile '${profile}' missing archive layout claim row in ${PROFILE_CLAIMS_TABLE_FILE}"
    return 1
  fi

  if [[ "${#expected_binaries[@]}" -gt 0 ]]; then
    for expected_binary in "${expected_binaries[@]}"; do
      if ! printf '%s\n' "${doc_binaries[@]}" | rg -x --fixed-strings "${expected_binary}"; then
        echo "error: docs/${PROFILE_CLAIMS_TABLE_FILE} missing '${expected_binary}' for profile '${profile}'"
        return 1
      fi
    done

    for doc_binary in "${doc_binaries[@]}"; do
      if ! printf '%s\n' "${expected_binaries[@]}" | rg -x --fixed-strings "${doc_binary}"; then
        echo "error: docs/${PROFILE_CLAIMS_TABLE_FILE} include unexpected binary '${doc_binary}' for profile '${profile}'"
        return 1
      fi
    done

    for expected_binary in "${expected_binaries[@]}"; do
      if ! tar -tzf "${profile_tar}" "bin/${expected_binary}" >/dev/null; then
        echo "error: ${profile_tar} missing expected payload 'bin/${expected_binary}'"
        return 1
      fi
    done

    if ! printf '%s\n' "${doc_archive_paths[@]}" | rg -q --fixed-strings "bin/"; then
      echo "error: docs archive-layout row for '${profile}' does not include any bin/ entries"
      return 1
    fi
  fi

  if ! command -v python3 >/dev/null 2>&1; then
    echo "error: python3 missing; cannot parse ${profile_tar} capability manifest"
    return 1
  fi

  expected_manifest="$(printf '%s\n' "${expected_binaries[@]}")"
  manifest_json="$(tar -xOf "${profile_tar}" capability.manifest.json)"
  while IFS= read -r manifest_binary; do
    manifest_binaries+=("${manifest_binary}")
  done < <(MANIFEST_JSON="${manifest_json}" python3 - <<'PY'
import json
import os

manifest = json.loads(os.environ["MANIFEST_JSON"])
for binary in manifest.get("binary", []):
  print(binary)
PY
)

  for manifest_binary in "${manifest_binaries[@]}"; do
    if ! printf '%s\n' "${expected_binaries[@]}" | rg -x --fixed-strings "${manifest_binary}"; then
      echo "error: capability manifest for '${profile}' contains unexpected binary '${manifest_binary}'"
      return 1
    fi
  done

  for expected_binary in "${expected_binaries[@]}"; do
    if ! printf '%s\n' "${manifest_binaries[@]}" | rg -x --fixed-strings "${expected_binary}"; then
      echo "error: capability manifest for '${profile}' missing binary '${expected_binary}'"
      return 1
    fi
  done

  if [[ "${#expected_binaries[@]}" -gt 0 && ( -z "${expected_manifest}" || -z "${manifest_binaries[*]}" ) ]]; then
    echo "error: unexpected empty binary manifest for '${profile}'"
    return 1
  fi

  return 0
}

check_required_doc_claims() {
  for required_profile in "${REQUIRED_DOC_PROFILES[@]}"; do
    if ! rg -q --fixed-strings "| \`${required_profile}\` " "${PROFILE_CLAIMS_TABLE_FILE}"; then
      echo "error: required profile '${required_profile}' missing in ${PROFILE_CLAIMS_TABLE_FILE}"
      return 1
    fi
  done
}

export_dependency_manifest() {
  local dependency_manifest="${DIST_DIR}/profile-dependencies.${RELEASE_ID}.json"
  if command -v cargo >/dev/null 2>&1; then
    cargo metadata --format-version 1 --no-deps > "${dependency_manifest}"
    return 0
  fi
  python3 - <<'PY'
import json
import datetime
import os
import pathlib

manifest = {
    "generated_at_utc": datetime.datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
    "warning": "cargo metadata unavailable at packaging time",
}
dist_dir = pathlib.Path(os.environ["DIST_DIR"])
release_id = os.environ["RELEASE_ID"]
path = dist_dir / f"profile-dependencies.{release_id}.json"
path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
PY
}

write_profile_artifact() {
  local profile="$1"
  local profile_dir="${DIST_DIR}/${profile}"
  local profile_tar="${DIST_DIR}/${profile}.${RELEASE_ID}.tar.gz"
  local staging="${profile_dir}/staging"
  local capabilities
  local binaries_json=""
  local binary_names_json="[]"
  local image
  local build_cmd
  local tar_sha
  local tar_size
  local size_budget
  local binary_name
  local binary_escaped
  local rows=()
  local names=()
  local sbom_rows=()
  local sbom_path
  local sbom_json

  mkdir -p "${staging}/bin"
  rm -rf "${staging:?}/bin/"*

  capabilities="$(profile_features "${profile}")"
  image="$(profile_image "${profile}")"
  build_cmd="$(profile_build_cmd "${profile}")"

  while IFS= read -r binary; do
    [[ -z "${binary}" ]] && continue
    local binary_path
    local binary_sha
    local binary_size
    binary_path="$(resolve_binary_path "${binary}")"
    if [[ -z "${binary_path}" ]]; then
      echo "error: binary '${binary}' not present for profile ${profile}; run cargo build for profile targets first"
      exit 1
    fi
    cp "${binary_path}" "${staging}/bin/${binary}"
    rows+=("$(collect_binary_info "${binary}" "${staging}/bin/${binary}")")
    names+=("${binary}")
    binary_size="$(wc -c "${staging}/bin/${binary}" | awk '{print $1}')"
    binary_sha="$(sha256sum "${staging}/bin/${binary}" | awk '{print $1}')"
    binary_escaped="$(printf '%s' "${binary}" | sed 's/\\/\\\\/g; s/\"/\\\"/g')"
    sbom_rows+=(
      "{\"name\":\"${binary_escaped}\",\"sha256\":\"${binary_sha}\",\"size_bytes\":${binary_size},\"path\":\"bin/${binary_escaped}\"}"
    )
  done < <(profile_binaries "${profile}")

  for item in "${rows[@]}"; do
    if [[ -z "${binaries_json:-}" ]]; then
      binaries_json="${item}"
    else
      binaries_json="${binaries_json},${item}"
    fi
  done

  binary_names_json="["
  for binary_name in "${names[@]}"; do
    if [[ -n "${binary_name}" ]]; then
      if [[ "${binary_names_json}" != "[" ]]; then
        binary_names_json+=","
      fi
      binary_escaped="$(printf '%s' "${binary_name}" | sed 's/\\/\\\\/g; s/\"/\\\"/g')"
      binary_names_json+="\"${binary_escaped}\""
    fi
  done
  binary_names_json+="]"

  sbom_json="["
  for binary_name in "${sbom_rows[@]}"; do
    if [[ "${sbom_json}" != "[" ]]; then
      sbom_json+=","
    fi
    sbom_json+="${binary_name}"
  done
  sbom_json+="]"

  sbom_path="${DIST_DIR}/sbom.${profile}.${RELEASE_ID}.json"
  cat >"${sbom_path}" <<SBOM
{
  "artifact": "${profile}.${RELEASE_ID}.tar.gz",
  "profile": "${profile}",
  "release_id": "${RELEASE_ID}",
  "envelope_version": "${ENVELOPE_VERSION}",
  "generated_at_utc": "${now_utc}",
  "dependency_manifest": "profile-dependencies.${RELEASE_ID}.json",
  "image": "${image}",
  "binaries": ${sbom_json}
}
SBOM

  cat >"${staging}/capability.manifest.json" <<MANIFEST
{
  "artifact_name": "${profile}.${RELEASE_ID}.tar.gz",
  "profile": "${profile}",
  "release_id": "${RELEASE_ID}",
  "envelope_version": "${ENVELOPE_VERSION}",
  "image": "${image}",
  "binary": ${binary_names_json},
  "build_command": "${build_cmd}",
  "enabled_features": ${capabilities},
  "binaries": [
${binaries_json}
  ],
  "generated_at_utc": "${now_utc}"
}
MANIFEST

  tar -czf "${profile_tar}" -C "${staging}" capability.manifest.json bin
  tar_sha="$(sha256sum "${profile_tar}" | awk '{print $1}')"
  tar_size="$(wc -c "${profile_tar}" | awk '{print $1}')"
  size_budget="$(profile_matrix_scalar "${profile}" "size_budget_bytes")"
  if [[ -n "${size_budget}" ]]; then
    if ! [[ "${size_budget}" =~ ^[0-9]+$ ]]; then
      echo "error: size_budget_bytes for profile '${profile}' must be a non-negative integer"
      exit 1
    fi
    if (( tar_size > size_budget )); then
      echo "error: artifact ${profile}.${RELEASE_ID}.tar.gz (${tar_size} bytes) exceeds size budget (${size_budget} bytes)"
      exit 1
    fi
  fi

  METADATA_ARTIFACTS+=(
    "{\"profile\":\"${profile}\",\"artifact_name\":\"${profile}.${RELEASE_ID}.tar.gz\",\"artifact_path\":\"${profile}.${RELEASE_ID}.tar.gz\",\"binary\":${binary_names_json},\"image\":\"${image}\",\"release_id\":\"${RELEASE_ID}\",\"envelope_version\":\"${ENVELOPE_VERSION}\",\"sha256\":\"${tar_sha}\",\"size_bytes\":${tar_size},\"sbom_path\":\"$(basename "${sbom_path}")\"}"
  )
}

if ! check_required_doc_claims; then
  exit 1
fi

if [[ "${VALIDATE_CLAIMS}" -eq 1 ]]; then
  python3 tools/runtime_env_contract.py \
    --repo-root "${ROOT_DIR}" \
    --check-docs \
    --report "${DIST_DIR}/runtime-env-contract-validate.${RELEASE_ID}.json"

  if [[ "$?" -ne 0 ]]; then
    echo "error: runtime env-contract parity check failed; cannot continue with --validate"
    exit 1
  fi

  python3 tools/profile_docs_parity_gate.py \
    --docs-envelope "docs/03-DICOM-Conformance-Envelope.md" \
    --docs-service-contract "docs/12-API-Surface-and-Crate-Boundaries.md" \
    --docs-profile-playbook "docs/33-Productization-Profiles-and-Playbooks.md" \
    --docs-capability-matrix "docs/49-Runtime-Profile-Capability-Matrix.md" \
    --profile-matrix "${PROFILE_MATRIX_FILE}" \
    --report "${DIST_DIR}/docs-parity.${RELEASE_ID}.json" \
    --fail-on-findings

  if [[ "$?" -ne 0 ]]; then
    echo "error: docs-parity check failed; cannot continue with --validate"
    exit 1
  fi
fi

if [[ "${REQUIRE_DIMSE}" -eq 1 ]]; then
  if [[ "${INCLUDE_DIMSE}" -eq 0 ]]; then
    has_dimse_profile=0
    for profile in "${PROFILES[@]}"; do
      if [[ "${profile}" == "backend-services-with-dimse" ]]; then
        has_dimse_profile=1
        break
      fi
    done

    if [[ "${has_dimse_profile}" -eq 0 ]]; then
      echo "error: --require-dimse was set but selected profiles did not include DIMSE (backend-services-with-dimse)"
      echo "Use --include-dimse or --profiles backend-services-with-dimse to avoid silent DIMSE omission."
      exit 1
    fi
  fi
fi

for profile in "${PROFILES[@]}"; do
  write_profile_artifact "${profile}"
  if [[ "${VALIDATE_CLAIMS}" -eq 1 ]]; then
    validate_profile_claims "${profile}" "${DIST_DIR}/${profile}.${RELEASE_ID}.tar.gz"
  fi
done

export_dependency_manifest
dependency_manifest_path="${DIST_DIR}/profile-dependencies.${RELEASE_ID}.json"
dependency_sha="$(sha256sum "${dependency_manifest_path}" | awk '{print $1}')"
dependency_size="$(wc -c "${dependency_manifest_path}" | awk '{print $1}')"

METADATA_JSON="${DIST_DIR}/profiles.metadata.json"
{
  echo "{"
  echo "  \"release_id\": \"${RELEASE_ID}\","
  echo "  \"envelope_version\": \"${ENVELOPE_VERSION}\","
  echo "  \"generated_at_utc\": \"${now_utc}\","
  echo "  \"dependency_manifest\": {"
  echo "    \"path\": \"$(basename "${dependency_manifest_path}")\","
  echo "    \"sha256\": \"${dependency_sha}\","
  echo "    \"size_bytes\": ${dependency_size}"
  echo "  },"
  echo "  \"artifacts\": ["
  first=1
  for artifact in "${METADATA_ARTIFACTS[@]}"; do
    if [[ "${first}" -eq 0 ]]; then
      echo ","
    fi
    echo "    ${artifact}"
    first=0
  done
  echo ""
  echo "  ]"
  echo "}"
} > "${METADATA_JSON}"

if [[ "${ENABLE_SIGNING}" -eq 1 && -n "${EVIDENCE_SIGNING_KEY-}" ]]; then
  python3 tools/evidence_sign_verify.py sign \
    --release-id "${RELEASE_ID}" \
    --key-id "profiles-hmac-v1" \
    --key-env EVIDENCE_SIGNING_KEY \
    --repo-root "${ROOT_DIR}" \
    --artifact "$(realpath --relative-to="${ROOT_DIR}" "${METADATA_JSON}")" \
    --artifact "$(realpath --relative-to="${ROOT_DIR}" "${dependency_manifest_path}")" \
    --output "${DIST_DIR}/profiles.metadata.bundle.signed.json"
  echo "Signed profile bundle: ${DIST_DIR}/profiles.metadata.bundle.signed.json"
else
  echo "EVIDENCE_SIGNING_KEY not set; profile metadata signature skipped."
fi

if [[ "${VALIDATE_CLAIMS}" -eq 1 ]]; then
  for artifact in "${DIST_DIR}"/*.tar.gz; do
    [[ -e "${artifact}" ]] || continue
    profile=""
    profile="$(basename "${artifact}")"
    profile="${profile%%.*}"
    tar -tzf "${artifact}" | grep -q "^capability.manifest.json$" || {
      echo "error: ${artifact} missing capability.manifest.json"
      exit 1
    }
    if profile_has_binaries "${profile}"; then
      tar -tzf "${artifact}" | grep -q "^bin/" || {
        echo "error: ${artifact} missing binary payload under bin/"
        exit 1
      }
    fi
  done
fi

for profile in "${PROFILES[@]}"; do
  echo "Packaged profile: ${profile}.${RELEASE_ID}.tar.gz"
done
ls -1 "${DIST_DIR}"/*.tar.gz
ls -1 "${DIST_DIR}"/*.json
