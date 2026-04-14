from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REGISTER = ROOT / "reports" / "clinical" / "localization-controls-register-RC-2026.02.14.md"


class HiLocalizationControlsTests(unittest.TestCase):
    def test_localization_register_exists_and_covers_required_controls(self) -> None:
        self.assertTrue(REGISTER.exists(), f"missing localization register: {REGISTER}")
        text = REGISTER.read_text(errors="ignore")
        text_lower = text.lower()

        for req in ["REQ-HI-233", "REQ-HI-234", "REQ-HI-235", "REQ-HI-236"]:
            self.assertIn(req, text)

        for keyword in [
            "controlled glossary",
            "approval",
            "locale-aware",
            "truncation",
            "ambiguity",
            "regression",
        ]:
            self.assertIn(keyword, text_lower)

        rows = re.findall(r"\| LOC-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 4)

    def test_localization_register_is_signed_and_gate_ready(self) -> None:
        text = REGISTER.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Decision: Localization controls accepted for release gate", text)
        self.assertIn("Signature ID:", text)
        self.assertNotIn("TBD", text)


if __name__ == "__main__":
    unittest.main()
