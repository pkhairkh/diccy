from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "generate_soak_reliability_artifact.py"


class GenerateSoakReliabilityArtifactTests(unittest.TestCase):
    def test_generates_24h_soak_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports/performance/profiles").mkdir(parents=True, exist_ok=True)
            (root / "reports/performance/profiles/soak.json").write_text(
                json.dumps(
                    {
                        "profile_id": "soak-test",
                        "workloads": [{"id": "noop", "command": ["python3", "-c", "print('ok')"]}],
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
                    "--release-id",
                    "RC-SOAK-1",
                    "--profile",
                    "reports/performance/profiles/soak.json",
                    "--duration-hours",
                    "24",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

            json_path = root / "reports/performance/soak-reliability-RC-SOAK-1.json"
            md_path = root / "reports/performance/soak-reliability-RC-SOAK-1.md"
            self.assertTrue(json_path.exists())
            self.assertTrue(md_path.exists())
            payload = json.loads(json_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["duration_hours"], 24)
            self.assertEqual(payload["release_id"], "RC-SOAK-1")
            self.assertIn("scenario_sha256", payload)


if __name__ == "__main__":
    unittest.main()
