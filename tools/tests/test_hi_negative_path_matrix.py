from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
MATRIX = ROOT / "reports" / "release" / "human-interface-negative-path-matrix-RC-2026.02.14.md"


class HiNegativePathMatrixTests(unittest.TestCase):
    def test_negative_path_matrix_exists_and_maps_req_hi_243(self) -> None:
        self.assertTrue(MATRIX.exists(), f"missing negative-path matrix: {MATRIX}")
        text = MATRIX.read_text(errors="ignore")
        text_lower = text.lower()

        self.assertIn("REQ-HI-243", text)
        for keyword in [
            "negative-path",
            "high-risk workflow",
            "blocked",
            "fail closed",
            "coverage",
        ]:
            self.assertIn(keyword, text_lower)

        rows = re.findall(r"\| HINP-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 8)

    def test_negative_path_matrix_is_signed(self) -> None:
        text = MATRIX.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Signature ID:", text)


if __name__ == "__main__":
    unittest.main()
