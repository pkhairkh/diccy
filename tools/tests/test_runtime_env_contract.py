from __future__ import annotations

import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.append(str(ROOT))

from runtime_env_contract_lib import extract_runtime_env_contract


def _seed_contract_fixture(repo_root: Path) -> None:
    (repo_root / "docs").mkdir(parents=True, exist_ok=True)
    (repo_root / "crates/dicom-workflow-server/src").mkdir(parents=True, exist_ok=True)
    (repo_root / "crates/dicom-dimse-service/src/bin").mkdir(parents=True, exist_ok=True)

    (repo_root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
        textwrap.dedent(
            """
            ### `dicom-workflow-server` (env-contract)

            - `DICOM_WORKFLOW_BIND`

            ### `dicom-dimse-service` (env-contract)

            - `DICOM_DIMSE_BIND`
            """
        ).strip()
        + "\n",
        encoding="utf-8",
    )

    (repo_root / "crates/dicom-workflow-server/src/main.rs").write_text(
        'let _ = std::env::var("DICOM_WORKFLOW_BIND");\n',
        encoding="utf-8",
    )
    (repo_root / "crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs").write_text(
        'let _ = std::env::var("DICOM_DIMSE_BIND");\n',
        encoding="utf-8",
    )


def _run_contract(root: Path, components: str | None, args: list[str]) -> subprocess.CompletedProcess[str]:
    command = ["python3", str(Path(__file__).resolve().parents[1] / "runtime_env_contract.py")]
    command.extend(["--repo-root", str(root)])
    if components:
        command.extend(["--components", components])
    command.extend(["--report", "reports/runtime-env-contract-test.json"])
    command.extend(args)
    return subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
    )


class RuntimeEnvContractTests(unittest.TestCase):
    def test_extract_runtime_env_contract_filters_by_component(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _seed_contract_fixture(root)

            contracts = extract_runtime_env_contract(
                root,
                components=("dicom-workflow-server",),
            )

            self.assertEqual(len(contracts), 1)
            self.assertEqual(contracts[0]["component"], "dicom-workflow-server")

    def test_cli_contract_filter_and_drift_report(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            _seed_contract_fixture(root)

            result = _run_contract(root, "dicom-workflow-server,dicom-dimse-service", ["--check-docs"])
            self.assertEqual(result.returncode, 0)
            self.assertIn("Summary: reports/docs/runtime-env-contract-summary.md", result.stdout)

            report_path = root / "reports/runtime-env-contract-test.json"
            payload = report_path.read_text(encoding="utf-8")
            self.assertIn("dicom-workflow-server", payload)
            self.assertIn("dicom-dimse-service", payload)
            self.assertIn('"status": "PASS"', payload)

            summary = (root / "reports/docs/runtime-env-contract-summary.md").read_text(encoding="utf-8")
            self.assertIn("Contract drift status: PASS", summary)
