from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REGISTER = ROOT / "reports" / "clinical" / "critical-task-usability-register-RC-2026.02.14.md"
REQ_HI_ANCHORS = [
    "REQ-HI-210",
    "REQ-HI-211",
    "REQ-HI-212",
    "REQ-HI-213",
    "REQ-HI-214",
    "REQ-HI-215",
    "REQ-HI-216",
    "REQ-HI-217",
    "REQ-HI-218",
    "REQ-HI-219",
    "REQ-HI-220",
    "REQ-HI-221",
    "REQ-HI-222",
    "REQ-HI-223",
    "REQ-HI-224",
]


class HiCriticalTaskRegisterTests(unittest.TestCase):
    def test_register_exists_and_covers_req_hi_210_to_224(self) -> None:
        self.assertTrue(REGISTER.exists(), f"missing critical-task register: {REGISTER}")
        text = REGISTER.read_text(errors="ignore")
        text_lower = text.lower()

        for req in REQ_HI_ANCHORS:
            self.assertIn(req, text)

        for keyword in [
            "critical task",
            "success criteria",
            "failure criteria",
            "formative",
            "summative",
            "taxonomy",
            "guided",
            "keyboard",
            "eta",
            "undo/redo",
            "contextual help",
            "onboarding",
            "simulation",
            "alert-fatigue",
            "capa",
            "pmcf",
        ]:
            self.assertIn(keyword, text_lower)

        rows = re.findall(r"\| CTU-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 15)

    def test_register_is_signed_and_gate_ready(self) -> None:
        text = REGISTER.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Signature ID:", text)
        self.assertNotIn("TBD", text)


if __name__ == "__main__":
    unittest.main()
