from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import post_release_monitor


class PostReleaseMonitorTests(unittest.TestCase):
    def test_build_jobs_is_stable(self) -> None:
        jobs = post_release_monitor.build_jobs("RC-TEST")
        self.assertEqual(len(jobs), 4)
        self.assertEqual(jobs[0]["job_id"], "DRIFT-REP-001")
        self.assertEqual(jobs[-1]["job_id"], "DRIFT-RPK-004")

    def test_evaluate_signals_pass(self) -> None:
        signals = post_release_monitor.evaluate_signals(
            reproducibility={"mismatch_count": 0},
            interoperability={"non_pass_cases": 0},
            traceability={"missing_references_count": 0, "invalid_links_count": 0},
            artifact_integrity={"missing_required_count": 0, "missing_index_reference_count": 0},
        )
        statuses = {signal["status"] for signal in signals}
        self.assertEqual(statuses, {"PASS"})

    def test_evaluate_signals_fail(self) -> None:
        signals = post_release_monitor.evaluate_signals(
            reproducibility={"mismatch_count": 1},
            interoperability={"non_pass_cases": 0},
            traceability={"missing_references_count": 2, "invalid_links_count": 0},
            artifact_integrity={"missing_required_count": 0, "missing_index_reference_count": 3},
        )
        failed = [signal for signal in signals if signal["status"] == "FAIL"]
        self.assertEqual(len(failed), 3)

    def test_markdown_writers(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            jobs_md = root / "jobs.md"
            cycle_md = root / "cycle.md"
            post_release_monitor.write_jobs_markdown(
                jobs_md,
                {
                    "release_id": "RC-TEST",
                    "generated_at_utc": "2026-02-11T00:00:00Z",
                    "jobs": post_release_monitor.build_jobs("RC-TEST"),
                },
            )
            post_release_monitor.write_cycle_markdown(
                cycle_md,
                {
                    "release_id": "RC-TEST",
                    "cycle_id": "cycle-1",
                    "generated_at_utc": "2026-02-11T00:00:00Z",
                    "overall_status": "PASS",
                    "signals": post_release_monitor.evaluate_signals(
                        reproducibility={"mismatch_count": 0},
                        interoperability={"non_pass_cases": 0},
                        traceability={"missing_references_count": 0, "invalid_links_count": 0},
                        artifact_integrity={"missing_required_count": 0, "missing_index_reference_count": 0},
                    ),
                },
            )
            self.assertIn("Drift Monitoring Job Definitions", jobs_md.read_text())
            self.assertIn("Post-Release Monitoring Cycle", cycle_md.read_text())


if __name__ == "__main__":
    unittest.main()
