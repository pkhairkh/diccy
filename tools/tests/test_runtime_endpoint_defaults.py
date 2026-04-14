from __future__ import annotations

import pathlib
import re
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
DOCS_12 = ROOT / "docs/12-API-Surface-and-Crate-Boundaries.md"
DOCS_40 = ROOT / "docs/40-Reference-Deployment-Topology.md"
VIEWER_SCRIPT = ROOT / "tools/run_viewer_wasm_frontend.sh"
WEB_MAIN = ROOT / "crates/dicom-web-server/src/main.rs"
WORKFLOW_RUNTIME_CONFIG = ROOT / "crates/dicom-workflow-server/src/application/runtime_config.rs"


class RuntimeEndpointDefaultsTests(unittest.TestCase):
    def test_viewer_default_port_matches_docs(self) -> None:
        script_text = VIEWER_SCRIPT.read_text(encoding="utf-8", errors="replace")
        docs_40_text = DOCS_40.read_text(encoding="utf-8", errors="replace")

        script_match = re.search(r"^PORT=(\d+)$", script_text, flags=re.MULTILINE)
        self.assertIsNotNone(script_match, "missing PORT default in viewer script")
        script_port = script_match.group(1)

        docs_match = re.search(r"viewer-wasm` static host \| `127\.0\.0\.1:(\d+)`", docs_40_text)
        self.assertIsNotNone(docs_match, "missing viewer default in docs/40")
        docs_port = docs_match.group(1)

        self.assertEqual(script_port, docs_port)

    def test_service_bind_defaults_match_docs_and_runtime(self) -> None:
        docs_12_text = DOCS_12.read_text(encoding="utf-8", errors="replace")
        docs_40_text = DOCS_40.read_text(encoding="utf-8", errors="replace")
        web_text = WEB_MAIN.read_text(encoding="utf-8", errors="replace")
        workflow_runtime_text = WORKFLOW_RUNTIME_CONFIG.read_text(
            encoding="utf-8", errors="replace"
        )

        docs12_web = re.search(
            r"`DICOM_WEB_BIND`[^\n]*\(default `([0-9.:]+)`\)",
            docs_12_text,
        )
        docs12_workflow = re.search(
            r"`DICOM_WORKFLOW_BIND`[^\n]*\(default `([0-9.:]+)`\)",
            docs_12_text,
        )
        self.assertIsNotNone(docs12_web)
        self.assertIsNotNone(docs12_workflow)

        docs40_web = re.search(r"`dicom-web-server` \| `([0-9.:]+)`", docs_40_text)
        docs40_workflow = re.search(r"`dicom-workflow-server` \| `([0-9.:]+)`", docs_40_text)
        self.assertIsNotNone(docs40_web)
        self.assertIsNotNone(docs40_workflow)

        runtime_web = re.search(
            r'parse_string_non_empty\(\s*DICOM_WEB_SERVICE_NAME,\s*"DICOM_WEB_BIND",\s*"([0-9.:]+)"\s*,?\s*\)\?',
            web_text,
            flags=re.S,
        )
        runtime_workflow = re.search(
            r'parse_string_non_empty\(\s*WORKFLOW_SERVICE_NAME,\s*"DICOM_WORKFLOW_BIND",\s*"([0-9.:]+)"\s*,?\s*\)\?',
            workflow_runtime_text,
            flags=re.S,
        )
        self.assertIsNotNone(runtime_web)
        self.assertIsNotNone(runtime_workflow)

        self.assertEqual(docs12_web.group(1), docs40_web.group(1))
        self.assertEqual(docs12_workflow.group(1), docs40_workflow.group(1))
        self.assertEqual(docs12_web.group(1), runtime_web.group(1))
        self.assertEqual(docs12_workflow.group(1), runtime_workflow.group(1))


if __name__ == "__main__":
    unittest.main()
