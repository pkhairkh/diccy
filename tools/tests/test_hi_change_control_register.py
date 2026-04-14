from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REGISTER = (
    ROOT
    / "reports"
    / "release"
    / "human-interface-change-control-register-RC-2026.02.14.md"
)


class HiChangeControlRegisterTests(unittest.TestCase):
    def test_register_exists_with_req_hi_247_248_249_254(self) -> None:
        self.assertTrue(REGISTER.exists(), f"missing change-control register: {REGISTER}")
        text = REGISTER.read_text(errors="ignore")
        text_lower = text.lower()

        for req in ["REQ-HI-247", "REQ-HI-248", "REQ-HI-249", "REQ-HI-254"]:
            self.assertIn(req, text)

        for keyword in [
            "claim-surface lint",
            "controlled wording",
            "impact class",
            "envelope",
            "evidence review",
            "approver signature",
            "effective date",
        ]:
            self.assertIn(keyword, text_lower)

        rows = re.findall(r"\| HICR-\d{3} \|", text)
        self.assertGreaterEqual(len(rows), 4)

    def test_register_is_signed(self) -> None:
        text = REGISTER.read_text(errors="ignore")
        self.assertIn("Status: Complete (Signed)", text)
        self.assertIn("Signature ID:", text)


if __name__ == "__main__":
    unittest.main()
