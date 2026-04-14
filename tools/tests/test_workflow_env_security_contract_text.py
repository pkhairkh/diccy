from __future__ import annotations

import pathlib
import unittest


MAIN_RS = pathlib.Path(__file__).resolve().parents[2] / "crates/dicom-workflow-server/src/main.rs"


class WorkflowEnvSecurityContractTextTests(unittest.TestCase):
    def test_plugin_path_validation_blocks_parent_traversal(self) -> None:
        text = MAIN_RS.read_text(encoding="utf-8", errors="replace")
        self.assertIn("must not contain parent-directory traversal segments", text)
        self.assertIn("DICOM_WORKFLOW_HL7_CONNECTOR_PLUGIN_", text)

    def test_hl7_payload_limit_checks_present(self) -> None:
        text = MAIN_RS.read_text(encoding="utf-8", errors="replace")
        self.assertIn("max_input_bytes", text)
        self.assertIn("HL7 MLLP frame exceeded max_input_bytes", text)


if __name__ == "__main__":
    unittest.main()
