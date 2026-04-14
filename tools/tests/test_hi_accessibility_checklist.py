from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
CHECKLIST = ROOT / "reports" / "clinical" / "accessibility-baseline-checklist-RC-2026.02.14.md"


class HiAccessibilityChecklistTests(unittest.TestCase):
    def test_accessibility_checklist_exists_with_required_controls(self) -> None:
        self.assertTrue(CHECKLIST.exists(), f"missing checklist: {CHECKLIST}")
        text = CHECKLIST.read_text(errors="ignore")
        text_lower = text.lower()

        for req in [
            "REQ-HI-225",
            "REQ-HI-226",
            "REQ-HI-227",
            "REQ-HI-228",
            "REQ-HI-229",
            "REQ-HI-230",
            "REQ-HI-231",
            "REQ-HI-232",
        ]:
            self.assertIn(req, text)

        for keyword in [
            "keyboard-only",
            "focus",
            "programmatic",
            "contrast",
            "200%",
            "color-only",
        ]:
            self.assertIn(keyword, text_lower)

        control_rows = re.findall(r"\| ACC-\d{3} \|", text)
        self.assertGreaterEqual(len(control_rows), 8)

    def test_accessibility_checklist_is_signed_and_all_controls_passed(self) -> None:
        text = CHECKLIST.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Signature ID:", text)
        pass_cells = re.findall(r"\| PASS \|", text)
        self.assertGreaterEqual(len(pass_cells), 8)


if __name__ == "__main__":
    unittest.main()
