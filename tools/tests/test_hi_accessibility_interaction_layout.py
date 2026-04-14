from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REGISTER = (
    ROOT
    / "reports"
    / "clinical"
    / "accessibility-interaction-layout-register-RC-2026.02.14.md"
)


class HiAccessibilityInteractionLayoutTests(unittest.TestCase):
    def test_register_exists_with_pointer_touch_pen_and_multimonitor_controls(self) -> None:
        self.assertTrue(REGISTER.exists(), f"missing interaction/layout register: {REGISTER}")
        text = REGISTER.read_text(errors="ignore")
        text_lower = text.lower()

        for req in ["REQ-HI-237", "REQ-HI-238", "REQ-HI-239"]:
            self.assertIn(req, text)

        for keyword in [
            "pointer",
            "touch",
            "pen",
            "multi-monitor",
            "identity banner",
            "accessibility profile",
            "audit",
        ]:
            self.assertIn(keyword, text_lower)

        rows = re.findall(r"\| AIL-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 3)

    def test_register_is_signed(self) -> None:
        text = REGISTER.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Signature ID:", text)


if __name__ == "__main__":
    unittest.main()
