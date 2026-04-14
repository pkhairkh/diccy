from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "state_backup_manifest.py"


class StateBackupManifestTests(unittest.TestCase):
    def test_generates_manifest_and_restore_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            state_root = root / "state" / "workflow"
            state_root.mkdir(parents=True, exist_ok=True)
            (state_root / "worklist.snapshot").write_text("A\n", encoding="utf-8")
            (state_root / "mpps.snapshot").write_text("B\n", encoding="utf-8")

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--state-root",
                    "state/workflow",
                    "--release-id",
                    "RC-TEST-1",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

            manifest_path = root / "reports/release/state-backup-manifest-RC-TEST-1.json"
            report_path = root / "reports/release/state-restore-verification-RC-TEST-1.md"
            self.assertTrue(manifest_path.exists())
            self.assertTrue(report_path.exists())

            payload = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertTrue(payload["immutable"])
            self.assertEqual(payload["release_id"], "RC-TEST-1")
            self.assertEqual(payload["file_count"], 2)
            paths = [entry["relative_path"] for entry in payload["files"]]
            self.assertEqual(paths, sorted(paths))

            report = report_path.read_text(encoding="utf-8")
            self.assertIn("State Restore Verification - RC-TEST-1", report)
            self.assertIn("Manifest file hashes validated before restore", report)


if __name__ == "__main__":
    unittest.main()
