from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
GATE = ROOT / "reports" / "release" / "human-interface-readiness-gate-RC-2026.02.14.md"


class HiReleaseGateMatrixTests(unittest.TestCase):
    def test_release_gate_matrix_exists_with_hi_release_controls(self) -> None:
        self.assertTrue(GATE.exists(), f"missing release gate matrix: {GATE}")
        text = GATE.read_text(errors="ignore")

        for req in [
            "REQ-HI-240",
            "REQ-HI-241",
            "REQ-HI-242",
            "REQ-HI-246",
            "REQ-HI-250",
            "REQ-HI-251",
            "REQ-HI-252",
            "REQ-HI-253",
        ]:
            self.assertIn(req, text)

        rows = re.findall(r"\| HIG-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 8)

    def test_release_gate_matrix_links_pmcf_capa_and_go_decision(self) -> None:
        text = GATE.read_text(errors="ignore")
        self.assertIn("PMCF", text)
        self.assertIn("CAPA", text)
        self.assertIn("Gate outcome: GO", text)
        self.assertIn("Signature ID:", text)
        self.assertNotIn("Pending", text)


if __name__ == "__main__":
    unittest.main()
