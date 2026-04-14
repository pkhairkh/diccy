from __future__ import annotations

import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
IO_SRC = ROOT / "crates/dicom-io/src/lib.rs"
PIXEL_RASTER_TEST = ROOT / "crates/dicom-pixel/tests/raster.rs"
WORKFLOW_MAIN = ROOT / "crates/dicom-workflow-server/src/main.rs"


class HostileInputLimitsGateTests(unittest.TestCase):
    def test_parser_boundary_limit_test_has_explicit_error_assertion(self) -> None:
        text = IO_SRC.read_text(encoding="utf-8", errors="replace")
        self.assertIn("fn read_dataset_respects_max_input_bytes()", text)
        self.assertIn("ErrorKind::LimitExceeded", text)

    def test_decoder_boundary_limit_test_has_explicit_error_assertion(self) -> None:
        text = PIXEL_RASTER_TEST.read_text(encoding="utf-8", errors="replace")
        self.assertIn("fn raster_enforces_input_limit()", text)
        self.assertIn("ErrorKind::LimitExceeded", text)

    def test_runtime_admission_limit_test_has_explicit_error_assertion(self) -> None:
        text = WORKFLOW_MAIN.read_text(encoding="utf-8", errors="replace")
        self.assertIn("fn parse_pairs_enforces_workflow_pair_cap()", text)
        self.assertIn("max_workflow_pair_count", text)


if __name__ == "__main__":
    unittest.main()
