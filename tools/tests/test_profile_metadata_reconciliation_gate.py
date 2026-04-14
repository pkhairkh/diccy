from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "profile_metadata_reconciliation_gate.py"


class ProfileMetadataReconciliationGateTests(unittest.TestCase):
    def test_passes_when_matrix_docs_and_artifacts_align(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "tools").mkdir(parents=True, exist_ok=True)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "dist/profiles").mkdir(parents=True, exist_ok=True)

            (root / "tools/profile_matrix.json").write_text(
                json.dumps({"profiles": {"backend-services": {}, "backend-services-with-dimse": {}}}),
                encoding="utf-8",
            )
            (root / "docs/33-Productization-Profiles-and-Playbooks.md").write_text(
                "| Profile |\n|---|\n| `backend-services` |\n| `backend-services-with-dimse` |\n",
                encoding="utf-8",
            )
            (root / "dist/profiles/profiles.metadata.json").write_text(
                json.dumps(
                    {
                        "artifacts": [
                            {"profile": "backend-services"},
                            {"profile": "backend-services-with-dimse"},
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--require-artifacts",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_fails_when_docs_omit_profile(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "tools").mkdir(parents=True, exist_ok=True)
            (root / "docs").mkdir(parents=True, exist_ok=True)

            (root / "tools/profile_matrix.json").write_text(
                json.dumps({"profiles": {"backend-services": {}, "backend-services-with-dimse": {}}}),
                encoding="utf-8",
            )
            (root / "docs/33-Productization-Profiles-and-Playbooks.md").write_text(
                "| Profile |\n|---|\n| `backend-services` |\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("profiles missing in docs/33", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
