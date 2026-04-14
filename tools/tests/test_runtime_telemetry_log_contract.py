from __future__ import annotations

import pathlib
import sys
import unittest


TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

from runtime_env_contract_lib import extract_runtime_env_contract  # noqa: E402


def telemetry_and_log_keys(items: list[str]) -> set[str]:
    return {
        key
        for key in items
        if key.endswith("_LOG_LEVEL") or "_TELEMETRY_" in key
    }


class RuntimeTelemetryLogContractTests(unittest.TestCase):
    def test_documented_telemetry_and_log_keys_match_runtime_contract(self) -> None:
        repo_root = pathlib.Path(__file__).resolve().parents[2]
        contracts = extract_runtime_env_contract(repo_root)
        by_component = {contract["component"]: contract for contract in contracts}

        for component in ("dicom-web-server", "dicom-workflow-server", "dicom-dimse-service"):
            contract = by_component[component]
            code_keys = telemetry_and_log_keys(contract["code_env_vars"])
            docs_keys = telemetry_and_log_keys(contract["docs_env_vars"])
            self.assertEqual(
                sorted(code_keys),
                sorted(docs_keys),
                msg=f"telemetry/log env contract drift for {component}",
            )

        workflow_code_keys = telemetry_and_log_keys(by_component["dicom-workflow-server"]["code_env_vars"])
        workflow_docs_keys = telemetry_and_log_keys(by_component["dicom-workflow-server"]["docs_env_vars"])
        self.assertNotIn(
            "DICOM_WORKFLOW_TELEMETRY_ENABLED",
            workflow_code_keys,
            "workflow telemetry key must be implemented explicitly before documentation",
        )
        self.assertNotIn(
            "DICOM_WORKFLOW_TELEMETRY_ENABLED",
            workflow_docs_keys,
            "workflow telemetry key must not be documented unless parser support exists",
        )


if __name__ == "__main__":
    unittest.main()
