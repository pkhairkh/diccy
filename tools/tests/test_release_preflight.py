from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import release_preflight


class ReleasePreflightTests(unittest.TestCase):
    def test_build_gate_plan_contains_expected_sequence(self) -> None:
        release_id = "RC-TEST"
        plan = release_preflight.build_gate_plan(release_id)
        gate_ids = [gate["id"] for gate in plan]
        self.assertEqual(
            gate_ids,
            [
                "cargo-fmt",
                "cargo-build",
                "cargo-test",
                "cargo-test-workflow-main-tests",
                "cargo-clippy",
                "claim-surface",
                "backlog-open-p0",
                "blocker-register",
                "req-completeness",
                "traceability",
                "determinism-manifest",
                "cross-target-matrix",
                "artifact-integrity",
            ],
        )
        traceability_gate = next(gate for gate in plan if gate["id"] == "traceability")
        self.assertIn(f"--linkage-manifest", " ".join(traceability_gate["command"]))
        self.assertIn(f"{release_id}", " ".join(traceability_gate["command"]))

    def test_run_gate_passes_and_writes_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            gates_dir = root / "logs"
            gate = {
                "id": "smoke",
                "description": "smoke gate",
                "command": ["python3", "-c", "print('ok')"],
            }
            result = release_preflight.run_gate(
                repo_root=root,
                release_id="RC-TEST",
                gate=gate,
                gates_dir=gates_dir,
            )
            self.assertEqual(result["status"], "PASS")
            self.assertTrue((root / result["log_path"]).exists())

    def test_run_gate_failure_is_reported(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            gates_dir = root / "logs"
            gate = {
                "id": "fail",
                "description": "failing gate",
                "command": ["python3", "-c", "import sys; sys.exit(7)"],
            }
            result = release_preflight.run_gate(
                repo_root=root,
                release_id="RC-TEST",
                gate=gate,
                gates_dir=gates_dir,
            )
            self.assertEqual(result["status"], "FAIL")
            self.assertEqual(result["exit_code"], 7)


if __name__ == "__main__":
    unittest.main()
