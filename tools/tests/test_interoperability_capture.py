from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "interoperability_capture.py"


def run_capture(
    *,
    input_path: pathlib.Path,
    output_md: pathlib.Path,
    output_json: pathlib.Path,
    fail_on_non_pass: bool = False,
) -> subprocess.CompletedProcess[str]:
    cmd = [
        "python3",
        str(SCRIPT),
        "--input",
        str(input_path),
        "--output-md",
        str(output_md),
        "--output-json",
        str(output_json),
        "--release-id",
        "RC-TEST-2026.02.11",
    ]
    if fail_on_non_pass:
        cmd.append("--fail-on-non-pass")
    return subprocess.run(cmd, check=False, capture_output=True, text=True)


class InteroperabilityCaptureTests(unittest.TestCase):
    def test_generates_outputs_for_valid_rows(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            input_path = root / "input.jsonl"
            output_md = root / "out.md"
            output_json = root / "out.json"

            input_path.write_text(
                "\n".join(
                    [
                        json.dumps(
                            {
                                "case_id": "CASE-001",
                                "target_system": "TARGET-A",
                                "interface_id": "IFS-WEB-QIDO-001",
                                "protocol": "DICOMweb",
                                "phase": "positive",
                                "request_hash": "sha256:" + "a" * 64,
                                "response_hash": "sha256:" + "b" * 64,
                                "verdict": "PASS",
                                "evidence_ref": "TEST-CASE-001",
                            }
                        ),
                        json.dumps(
                            {
                                "case_id": "CASE-002",
                                "target_system": "TARGET-B",
                                "interface_id": "IFS-DIMSE-NEG-001",
                                "protocol": "DIMSE",
                                "phase": "negative",
                                "request_hash": "sha256:" + "c" * 64,
                                "response_hash": "sha256:" + "d" * 64,
                                "verdict": "PASS",
                                "note": "fail-closed verified",
                            }
                        ),
                    ]
                )
                + "\n"
            )

            result = run_capture(input_path=input_path, output_md=output_md, output_json=output_json)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Interoperability capture: PASS", result.stdout)
            self.assertTrue(output_md.exists())
            self.assertTrue(output_json.exists())
            self.assertIn("CASE-001", output_md.read_text())

            payload = json.loads(output_json.read_text())
            self.assertEqual(payload["total_rows"], 2)
            self.assertEqual(payload["verdict_counts"]["PASS"], 2)

    def test_fails_on_missing_required_field(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            input_path = root / "input.jsonl"
            output_md = root / "out.md"
            output_json = root / "out.json"

            input_path.write_text(
                json.dumps(
                    {
                        "case_id": "CASE-001",
                        "target_system": "TARGET-A",
                        "interface_id": "IFS-WEB-QIDO-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                    }
                )
                + "\n"
            )

            result = run_capture(input_path=input_path, output_md=output_md, output_json=output_json)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Interoperability capture: FAIL", result.stdout)
            self.assertIn("missing/invalid required field: verdict", result.stdout)

    def test_fail_on_non_pass_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            input_path = root / "input.jsonl"
            output_md = root / "out.md"
            output_json = root / "out.json"

            input_path.write_text(
                json.dumps(
                    {
                        "case_id": "CASE-001",
                        "target_system": "TARGET-A",
                        "interface_id": "IFS-WEB-QIDO-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                        "verdict": "BLOCKED",
                    }
                )
                + "\n"
            )

            result = run_capture(
                input_path=input_path,
                output_md=output_md,
                output_json=output_json,
                fail_on_non_pass=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("non-pass verdicts present", result.stdout)


if __name__ == "__main__":
    unittest.main()
