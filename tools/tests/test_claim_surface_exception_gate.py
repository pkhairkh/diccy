from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "claim_surface_exception_gate.py"


class ClaimSurfaceExceptionGateTests(unittest.TestCase):
    def test_passes_when_registry_entries_are_docs15_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports/security").mkdir(parents=True, exist_ok=True)
            (root / "reports/security/claim-surface-exceptions.json").write_text(
                json.dumps(
                    {
                        "exceptions": [
                            {
                                "path": "docs/15-Regulatory-and-Standards-Mapping.md",
                                "reason": "regulatory mapping reference",
                                "approved_by": "quality-owner",
                            }
                        ]
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)

    def test_fails_when_registry_contains_non_docs15_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports/security").mkdir(parents=True, exist_ok=True)
            (root / "reports/security/claim-surface-exceptions.json").write_text(
                json.dumps(
                    {
                        "exceptions": [
                            {
                                "path": "docs/01-Vision-and-Scope.md",
                                "reason": "invalid allowlist expansion",
                                "approved_by": "quality-owner",
                            }
                        ]
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main()
