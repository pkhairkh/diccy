#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
hook_dir="${repo_root}/.git/hooks"
hook_path="${hook_dir}/pre-commit"

if [[ ! -d "${repo_root}/.git" ]]; then
  echo "No .git directory found; skipping pre-commit hook install."
  exit 0
fi

mkdir -p "${hook_dir}"
cat > "${hook_path}" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

changed="$(git diff --cached --name-only)"
if grep -Eq '^(README\.md|docs/)' <<< "${changed}"; then
  echo "Running docs drift lint for staged docs/README changes..."
  python3 tools/runtime_env_contract.py --check-docs --report reports/docs/runtime-env-contract.json
  python3 tools/docs_drift_lint.py --report reports/docs/drift-report.json
  python3 tools/runtime_env_contract.py \
    --components dicom-workflow-server,dicom-dimse-service \
    --check-docs \
    --report reports/docs/runtime-env-contract-workflow-dimse.json
fi
HOOK

chmod +x "${hook_path}"
echo "Installed optional pre-commit hook: ${hook_path}"
