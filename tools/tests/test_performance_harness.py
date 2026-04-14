from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "performance_harness.py"


def run_harness(
    *,
    profile_path: pathlib.Path,
    output_md: pathlib.Path,
    output_json: pathlib.Path,
    run_label: str = "test",
    fail_on_threshold_breach: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd = [
        "python3",
        str(SCRIPT),
        "--profile",
        str(profile_path),
        "--output-md",
        str(output_md),
        "--output-json",
        str(output_json),
        "--run-label",
        run_label,
    ]
    if fail_on_threshold_breach:
        cmd.append("--fail-on-threshold-breach")
    return subprocess.run(cmd, check=False, capture_output=True, text=True)


class PerformanceHarnessTests(unittest.TestCase):
    def test_generates_markdown_and_json_for_valid_profile(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            profile = root / "profile.json"
            output_md = root / "report.md"
            output_json = root / "report.json"
            profile.write_text(
                json.dumps(
                    {
                        "profile_id": "test-baseline",
                        "workloads": [
                            {
                                "id": "python-noop",
                                "category": "web-small",
                                "command": ["python3", "-c", "print('ok')"],
                                "iterations": 3,
                                "operations_per_iteration": 1,
                                "capture_rss": False,
                                "slo_latency_p95_ms": 500.0,
                                "slo_throughput_ops_s": 0.2,
                                "error_budget_pct": 0.0,
                            }
                        ],
                    }
                )
            )

            result = run_harness(profile_path=profile, output_md=output_md, output_json=output_json)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Performance harness: PASS", result.stdout)
            self.assertTrue(output_md.exists())
            self.assertTrue(output_json.exists())
            self.assertIn("python-noop", output_md.read_text())
            payload = json.loads(output_json.read_text())
            self.assertEqual(payload["run_label"], "test")
            self.assertEqual(len(payload["workloads"]), 1)
            self.assertEqual(len(payload["violations"]), 0)
            self.assertIn("p99_latency_ms", payload["workloads"][0])

    def test_threshold_breach_returns_nonzero_with_fail_flag(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            profile = root / "profile.json"
            output_md = root / "report.md"
            output_json = root / "report.json"
            profile.write_text(
                json.dumps(
                    {
                        "profile_id": "test-threshold",
                        "workloads": [
                            {
                                "id": "latency-breach",
                                "category": "dimse-medium",
                                "command": ["python3", "-c", "import time; time.sleep(0.02)"],
                                "iterations": 2,
                                "operations_per_iteration": 1,
                                "capture_rss": False,
                                "slo_latency_p95_ms": 1.0,
                                "slo_throughput_ops_s": 0.1,
                                "error_budget_pct": 0.0,
                            }
                        ],
                    }
                )
            )

            result = run_harness(
                profile_path=profile,
                output_md=output_md,
                output_json=output_json,
                fail_on_threshold_breach=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("Performance harness: FAIL", result.stdout)
            self.assertIn("latency-breach", result.stdout)
            payload = json.loads(output_json.read_text())
            self.assertGreater(len(payload["violations"]), 0)

    def test_gpu_budget_breach_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            profile = root / "profile.json"
            output_md = root / "report.md"
            output_json = root / "report.json"
            profile.write_text(
                json.dumps(
                    {
                        "profile_id": "test-gpu-budget",
                        "workloads": [
                            {
                                "id": "gpu-budget-breach",
                                "category": "gpu",
                                "command": ["python3", "-c", "print('ok')"],
                                "iterations": 1,
                                "operations_per_iteration": 1,
                                "capture_rss": False,
                                "error_budget_pct": 0.0,
                                "observed_gpu_cache_bytes": 1048577,
                                "gpu_cache_budget_bytes": 1048576,
                            }
                        ],
                    }
                )
            )

            result = run_harness(
                profile_path=profile,
                output_md=output_md,
                output_json=output_json,
                fail_on_threshold_breach=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("gpu-budget-breach", result.stdout)

    def test_rejects_invalid_profile_schema(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            profile = root / "profile.json"
            output_md = root / "report.md"
            output_json = root / "report.json"
            profile.write_text(json.dumps({"profile_id": "invalid", "workloads": []}))

            result = run_harness(
                profile_path=profile,
                output_md=output_md,
                output_json=output_json,
                fail_on_threshold_breach=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("profile must define non-empty workloads", result.stderr)
            self.assertFalse(output_md.exists())
            self.assertFalse(output_json.exists())


if __name__ == "__main__":
    unittest.main()
