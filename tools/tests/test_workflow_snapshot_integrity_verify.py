from __future__ import annotations

import hashlib
import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "workflow_snapshot_integrity_verify.py"


def checksum(snapshot: dict[str, object]) -> str:
    raw = json.dumps(snapshot, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


class WorkflowSnapshotIntegrityVerifyTests(unittest.TestCase):
    def test_passes_when_export_import_checksum_and_tenant_match(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports").mkdir(parents=True, exist_ok=True)
            snapshot = {"tasks": ["T1"], "version": "2"}
            payload = {
                "tenant": "tenant-a",
                "snapshot": snapshot,
                "checksum_sha256": checksum(snapshot),
            }
            (root / "reports/export.json").write_text(json.dumps(payload), encoding="utf-8")
            (root / "reports/import.json").write_text(json.dumps(payload), encoding="utf-8")

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--export-json",
                    "reports/export.json",
                    "--import-json",
                    "reports/import.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_fails_when_tenant_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports").mkdir(parents=True, exist_ok=True)
            snapshot = {"tasks": ["T1"], "version": "2"}
            export_payload = {
                "tenant": "tenant-a",
                "snapshot": snapshot,
                "checksum_sha256": checksum(snapshot),
            }
            import_payload = {
                "tenant": "tenant-b",
                "snapshot": snapshot,
                "checksum_sha256": checksum(snapshot),
            }
            (root / "reports/export.json").write_text(json.dumps(export_payload), encoding="utf-8")
            (root / "reports/import.json").write_text(json.dumps(import_payload), encoding="utf-8")

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--export-json",
                    "reports/export.json",
                    "--import-json",
                    "reports/import.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("tenant isolation violation", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
