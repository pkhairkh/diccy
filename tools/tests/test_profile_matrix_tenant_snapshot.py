from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "profile_matrix_tenant_snapshot.py"


class ProfileMatrixTenantSnapshotTests(unittest.TestCase):
    def test_generates_multi_tenant_snapshots(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "tools").mkdir(parents=True, exist_ok=True)
            (root / "tools/profile_matrix.json").write_text(
                json.dumps(
                    {
                        "profiles": {
                            "backend-services": {"features": ["workflow"], "binaries": ["dicom-workflow-server"]},
                            "backend-services-with-dimse": {"features": ["workflow", "dimse"], "binaries": ["dicom-dimse-service"]},
                        }
                    },
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--output",
                    "reports/docs/tenant-snapshots.json",
                    "--tenants",
                    "tenant-default,tenant-a",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

            payload = json.loads((root / "reports/docs/tenant-snapshots.json").read_text(encoding="utf-8"))
            self.assertEqual(payload["tenant_count"], 2)
            self.assertEqual(payload["profile_count"], 2)
            self.assertEqual(payload["snapshot_count"], 4)


if __name__ == "__main__":
    unittest.main()
