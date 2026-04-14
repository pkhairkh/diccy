from __future__ import annotations

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW_MAIN = ROOT / "crates/dicom-workflow-server/src/main.rs"
WEB_MAIN = ROOT / "crates/dicom-web-server/src/main.rs"
DIMSE_BIN = ROOT / "crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs"


class RustParserCoverageGuardTests(unittest.TestCase):
    def test_parser_modules_define_test_blocks(self) -> None:
        for path in (WORKFLOW_MAIN, WEB_MAIN, DIMSE_BIN):
            text = path.read_text(encoding="utf-8", errors="replace")
            self.assertIn("#[cfg(test)]", text, msg=f"missing unit test block in {path}")

    def test_parser_modules_reference_numeric_bounds(self) -> None:
        for path in (WORKFLOW_MAIN, WEB_MAIN, DIMSE_BIN):
            text = path.read_text(encoding="utf-8", errors="replace")
            self.assertIn("NumericBounds", text, msg=f"missing numeric bounds usage in {path}")


if __name__ == "__main__":
    unittest.main()
