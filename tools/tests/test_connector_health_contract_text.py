from __future__ import annotations

import pathlib
import unittest


MAIN_RS = pathlib.Path(__file__).resolve().parents[2] / "crates/dicom-workflow-server/src/main.rs"


class ConnectorHealthContractTextTests(unittest.TestCase):
    def test_connector_health_endpoint_and_error_payload_contract_present(self) -> None:
        text = MAIN_RS.read_text(encoding="utf-8", errors="replace")
        self.assertIn("interop::InteropRoute::ConnectorHealth", text)
        self.assertIn("handle_hl7_connector_health", text)
        self.assertIn("DVF.WORKFLOW.CONNECTOR_HEALTH.EMPTY", text)
        self.assertIn("render_interop_error_json", text)

    def test_connector_metadata_contract_includes_plugin_compatibility(self) -> None:
        text = MAIN_RS.read_text(encoding="utf-8", errors="replace")
        self.assertIn("connector_plugins", text)
        self.assertIn("compatible_min", text)
        self.assertIn("compatible_max", text)


if __name__ == "__main__":
    unittest.main()
