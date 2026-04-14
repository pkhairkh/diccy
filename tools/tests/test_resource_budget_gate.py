from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "resource_budget_gate.py"


class ResourceBudgetGateTests(unittest.TestCase):
    def test_passes_when_usage_within_budget(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports/performance").mkdir(parents=True, exist_ok=True)
            (root / "reports/performance/report.json").write_text(
                json.dumps(
                    {
                        "workloads": [
                            {
                                "id": "qido-budget-probe",
                                "p95_latency_ms": 200.0,
                                "rss_growth_pct": 50.0,
                                "observed_gpu_cache_bytes": 1024.0,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (root / "reports/performance/budget.json").write_text(
                json.dumps(
                    {
                        "cpu_p95_latency_ms": {"qido-budget-probe": 250.0},
                        "memory_rss_growth_pct": {"default": 150.0},
                        "gpu_cache_bytes": {"default": 2048.0},
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
                    "--report-json",
                    "reports/performance/report.json",
                    "--budget-json",
                    "reports/performance/budget.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_fails_when_gpu_budget_exceeded(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports/performance").mkdir(parents=True, exist_ok=True)
            (root / "reports/performance/report.json").write_text(
                json.dumps(
                    {
                        "workloads": [
                            {
                                "id": "qido-budget-probe",
                                "p95_latency_ms": 200.0,
                                "rss_growth_pct": 50.0,
                                "observed_gpu_cache_bytes": 4096.0,
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (root / "reports/performance/budget.json").write_text(
                json.dumps(
                    {
                        "cpu_p95_latency_ms": {"qido-budget-probe": 250.0},
                        "memory_rss_growth_pct": {"default": 150.0},
                        "gpu_cache_bytes": {"default": 2048.0},
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
                    "--report-json",
                    "reports/performance/report.json",
                    "--budget-json",
                    "reports/performance/budget.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("gpu cache bytes", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
