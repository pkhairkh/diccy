from __future__ import annotations

import pathlib
import sys
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import limits_docs_sync_gate


class LimitsDocsSyncGateTests(unittest.TestCase):
    def test_enforce_docs_sync_passes_without_limit_changes(self) -> None:
        passed, detail = limits_docs_sync_gate.enforce_docs_sync(
            changed_files=["crates/dicom-web-server/src/main.rs"],
            limit_touched_files=[],
        )
        self.assertTrue(passed)
        self.assertIn("no limit-related", detail)

    def test_enforce_docs_sync_fails_when_required_docs_missing(self) -> None:
        passed, detail = limits_docs_sync_gate.enforce_docs_sync(
            changed_files=["crates/dicom-web-server/src/main.rs", "docs/09-Security-Threat-Model.md"],
            limit_touched_files=["crates/dicom-web-server/src/main.rs"],
        )
        self.assertFalse(passed)
        self.assertIn("docs/14-Release-and-Versioning.md", detail)

    def test_enforce_docs_sync_passes_when_both_docs_present(self) -> None:
        passed, detail = limits_docs_sync_gate.enforce_docs_sync(
            changed_files=[
                "crates/dicom-web-server/src/main.rs",
                "docs/09-Security-Threat-Model.md",
                "docs/14-Release-and-Versioning.md",
            ],
            limit_touched_files=["crates/dicom-web-server/src/main.rs"],
        )
        self.assertTrue(passed)
        self.assertIn("include docs/09 and docs/14", detail)


if __name__ == "__main__":
    unittest.main()
