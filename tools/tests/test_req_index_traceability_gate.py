from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "req_index_traceability_gate.py"


def seed_repo(root: pathlib.Path, *, req_lines: list[str], test_lines: list[str], baseline_reqs: list[str]) -> None:
    (root / "docs").mkdir(parents=True, exist_ok=True)
    (root / "crates/sample/tests").mkdir(parents=True, exist_ok=True)
    (root / "reports/traceability").mkdir(parents=True, exist_ok=True)

    (root / "docs/16-Requirements-Index.md").write_text("\n".join(req_lines) + "\n", encoding="utf-8")
    (root / "crates/sample/tests/req_refs.rs").write_text("\n".join(test_lines) + "\n", encoding="utf-8")
    (root / "reports/traceability/req-index-traceability-baseline.json").write_text(
        json.dumps(
            {
                "known_req_ids": baseline_reqs,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


class ReqIndexTraceabilityGateTests(unittest.TestCase):
    def test_passes_when_new_req_ids_have_test_refs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(
                root,
                req_lines=["- `REQ-ABC-001`", "- `REQ-ABC-002`"],
                test_lines=["// REQ-ABC-002"],
                baseline_reqs=["REQ-ABC-001"],
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)

    def test_fails_when_new_req_ids_lack_test_refs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(
                root,
                req_lines=["- `REQ-ABC-001`", "- `REQ-ABC-002`"],
                test_lines=["// unrelated"],
                baseline_reqs=["REQ-ABC-001"],
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)
            payload = json.loads(
                (root / "reports/traceability/req-index-traceability-gate.json").read_text(encoding="utf-8")
            )
            self.assertEqual(payload["status"], "FAIL")
            self.assertIn("REQ-ABC-002", payload["missing_test_refs_for_new_req_ids"])


if __name__ == "__main__":
    unittest.main()
