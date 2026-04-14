from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "req_normative_traceability_gate.py"


class ReqNormativeTraceabilityGateTests(unittest.TestCase):
    def _seed_repo(self, root: pathlib.Path) -> None:
        (root / "docs").mkdir(parents=True, exist_ok=True)
        (root / "crates/sample/tests").mkdir(parents=True, exist_ok=True)
        (root / "reports/traceability").mkdir(parents=True, exist_ok=True)

    def test_passes_when_new_normative_req_has_index_and_test_refs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_repo(root)
            (root / "docs/03-DICOM-Conformance-Envelope.md").write_text(
                "- **REQ-ABC-002:** Runtime **MUST** fail closed.\n",
                encoding="utf-8",
            )
            (root / "docs/16-Requirements-Index.md").write_text("- `REQ-ABC-002`\n", encoding="utf-8")
            (root / "crates/sample/tests/req_ref.rs").write_text("// REQ-ABC-002\n", encoding="utf-8")
            (root / "reports/traceability/req-normative-traceability-baseline.json").write_text(
                json.dumps({"known_normative_req_ids": ["REQ-ABC-001"]}) + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)

    def test_fails_when_new_normative_req_missing_index_entry(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_repo(root)
            (root / "docs/03-DICOM-Conformance-Envelope.md").write_text(
                "- **REQ-ABC-002:** Runtime **MUST** fail closed.\n",
                encoding="utf-8",
            )
            (root / "docs/16-Requirements-Index.md").write_text("- `REQ-ABC-001`\n", encoding="utf-8")
            (root / "crates/sample/tests/req_ref.rs").write_text("// REQ-ABC-002\n", encoding="utf-8")
            (root / "reports/traceability/req-normative-traceability-baseline.json").write_text(
                json.dumps({"known_normative_req_ids": []}) + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)
            payload = json.loads(
                (root / "reports/traceability/req-normative-traceability-gate.json").read_text(encoding="utf-8")
            )
            self.assertIn("REQ-ABC-002", payload["missing_index_entries_for_new_req_ids"])


if __name__ == "__main__":
    unittest.main()
