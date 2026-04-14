from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "perf_budget_regression_gate.py"


class PerfBudgetRegressionGateTests(unittest.TestCase):
    def test_passes_when_within_qido_wado_stow_budgets(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "reports/performance").mkdir(parents=True, exist_ok=True)
            (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
                "route latency budgets default to qido `250/500`, wado `400/800`, stow `1200/2400`",
                encoding="utf-8",
            )
            payload = {
                "workloads": [
                    {"id": "qido-budget-probe", "budget_class": "qido", "p95_latency_ms": 100, "p99_latency_ms": 120},
                    {"id": "wado-budget-probe", "budget_class": "wado", "p95_latency_ms": 200, "p99_latency_ms": 250},
                    {"id": "stow-budget-probe", "budget_class": "stow", "p95_latency_ms": 900, "p99_latency_ms": 1000},
                ]
            }
            (root / "reports/performance/current.json").write_text(json.dumps(payload), encoding="utf-8")

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--current-json",
                    "reports/performance/current.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_fails_on_sustained_breach(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "reports/performance").mkdir(parents=True, exist_ok=True)
            (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
                "route latency budgets default to qido `250/500`, wado `400/800`, stow `1200/2400`",
                encoding="utf-8",
            )
            breach_payload = {
                "workloads": [
                    {"id": "qido-budget-probe", "budget_class": "qido", "p95_latency_ms": 300, "p99_latency_ms": 600}
                ]
            }
            (root / "reports/performance/current.json").write_text(
                json.dumps(breach_payload), encoding="utf-8"
            )
            (root / "reports/performance/previous.json").write_text(
                json.dumps(breach_payload), encoding="utf-8"
            )

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--current-json",
                    "reports/performance/current.json",
                    "--previous-json",
                    "reports/performance/previous.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertIn("qido-budget-probe", result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
