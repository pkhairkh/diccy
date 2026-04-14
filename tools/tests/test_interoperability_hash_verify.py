from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "interoperability_hash_verify.py"


def run_verify(
    *,
    summaries: list[pathlib.Path],
    output_md: pathlib.Path,
    output_json: pathlib.Path,
    fail_on_non_pass: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd = ["python3", str(SCRIPT)]
    for summary in summaries:
        cmd.extend(["--summary", str(summary)])
    cmd.extend(["--output-md", str(output_md), "--output-json", str(output_json)])
    if fail_on_non_pass:
        cmd.append("--fail-on-non-pass")
    return subprocess.run(cmd, check=False, capture_output=True, text=True)


def write_summary(path: pathlib.Path, rows: list[dict[str, str]]) -> None:
    payload = {
        "release_id": "RC-TEST-2026.02.11",
        "generated_at_utc": "2026-02-11T00:00:00Z",
        "total_rows": len(rows),
        "verdict_counts": {},
        "capture_digest_sha256": "x" * 64,
        "rows": rows,
    }
    path.write_text(json.dumps(payload))


class InteroperabilityHashVerifyTests(unittest.TestCase):
    def test_passes_with_valid_summaries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            summary_a = root / "a.json"
            summary_b = root / "b.json"
            output_md = root / "register.md"
            output_json = root / "register.json"

            write_summary(
                summary_a,
                [
                    {
                        "case_id": "CASE-001",
                        "target_system": "TGT-001",
                        "interface_id": "IFS-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                        "verdict": "PASS",
                    }
                ],
            )
            write_summary(
                summary_b,
                [
                    {
                        "case_id": "CASE-002",
                        "target_system": "TGT-002",
                        "interface_id": "IFS-002",
                        "protocol": "DIMSE",
                        "phase": "negative",
                        "request_hash": "sha256:" + "c" * 64,
                        "response_hash": "sha256:" + "d" * 64,
                        "verdict": "PASS",
                    }
                ],
            )

            result = run_verify(
                summaries=[summary_a, summary_b],
                output_md=output_md,
                output_json=output_json,
                fail_on_non_pass=True,
            )
            self.assertEqual(result.returncode, 0)
            self.assertIn("Interoperability hash verify: PASS", result.stdout)
            self.assertTrue(output_md.exists())
            self.assertTrue(output_json.exists())
            self.assertIn("CASE-001", output_md.read_text())

    def test_fails_on_conflicting_duplicate_case(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            summary_a = root / "a.json"
            summary_b = root / "b.json"
            output_md = root / "register.md"
            output_json = root / "register.json"

            write_summary(
                summary_a,
                [
                    {
                        "case_id": "CASE-001",
                        "target_system": "TGT-001",
                        "interface_id": "IFS-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                        "verdict": "PASS",
                    }
                ],
            )
            write_summary(
                summary_b,
                [
                    {
                        "case_id": "CASE-001",
                        "target_system": "TGT-001",
                        "interface_id": "IFS-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "c" * 64,
                        "response_hash": "sha256:" + "d" * 64,
                        "verdict": "PASS",
                    }
                ],
            )

            result = run_verify(
                summaries=[summary_a, summary_b],
                output_md=output_md,
                output_json=output_json,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("conflicting duplicate case definitions", result.stdout)

    def test_fails_on_non_pass_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            summary = root / "a.json"
            output_md = root / "register.md"
            output_json = root / "register.json"

            write_summary(
                summary,
                [
                    {
                        "case_id": "CASE-001",
                        "target_system": "TGT-001",
                        "interface_id": "IFS-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                        "verdict": "BLOCKED",
                    }
                ],
            )

            result = run_verify(
                summaries=[summary],
                output_md=output_md,
                output_json=output_json,
                fail_on_non_pass=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("non-pass verdict rows present", result.stdout)


if __name__ == "__main__":
    unittest.main()
