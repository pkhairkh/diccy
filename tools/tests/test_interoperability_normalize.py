from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "interoperability_normalize.py"


def run_normalize(*args: str) -> subprocess.CompletedProcess[str]:
    cmd = ["python3", str(SCRIPT), *args]
    return subprocess.run(cmd, check=False, capture_output=True, text=True)


class InteroperabilityNormalizeTests(unittest.TestCase):
    def test_normalizes_alias_fields_for_all_protocols(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            raw_json = root / "raw.json"
            out_jsonl = root / "out.jsonl"
            summary = root / "summary.json"

            raw_json.write_text(
                json.dumps(
                    [
                        {
                            "case": "CASE-DW-001",
                            "target": "TGT-004",
                            "interface": "IFS-WEB-QIDO-PEER-001",
                            "service": "dicomweb",
                            "path_type": "pos",
                            "req_sha256": "a" * 64,
                            "resp_sha256": "b" * 64,
                            "result": "pass",
                            "evidence": "REF-DW",
                        },
                        {
                            "case": "CASE-DIM-001",
                            "target": "TGT-005",
                            "interface": "IFS-DIMSE-ECHO-PEER-001",
                            "service": "DIMSE",
                            "path_type": "positive",
                            "req_sha256": "c" * 64,
                            "resp_sha256": "d" * 64,
                            "result": "PASS",
                        },
                        {
                            "case": "CASE-WL-001",
                            "target": "TGT-005",
                            "interface": "IFS-WL-PEER-001",
                            "service": "mwl",
                            "path_type": "negative",
                            "req_sha256": "e" * 64,
                            "resp_sha256": "f" * 64,
                            "result": "pass",
                        },
                        {
                            "case": "CASE-MPPS-001",
                            "target": "TGT-006",
                            "interface": "IFS-MPPS-PEER-001",
                            "service": "MPPS",
                            "path_type": "neg",
                            "req_sha256": "1" * 64,
                            "resp_sha256": "2" * 64,
                            "result": "PASS",
                        },
                    ]
                )
            )

            result = run_normalize(
                "--input",
                str(raw_json),
                "--output",
                str(out_jsonl),
                "--release-id",
                "RC-TEST",
                "--summary-json",
                str(summary),
            )

            self.assertEqual(result.returncode, 0)
            self.assertIn("Interoperability normalize: PASS", result.stdout)
            rows = [json.loads(line) for line in out_jsonl.read_text().splitlines() if line.strip()]
            self.assertEqual(len(rows), 4)
            protocols = {row["protocol"] for row in rows}
            self.assertEqual(protocols, {"DICOMweb", "DIMSE", "MWL", "MPPS"})
            self.assertTrue(all(row["request_hash"].startswith("sha256:") for row in rows))
            summary_payload = json.loads(summary.read_text())
            self.assertEqual(summary_payload["total_rows"], 4)

    def test_threshold_enforcement_fails_when_missing_volume(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            raw_jsonl = root / "raw.jsonl"
            out_jsonl = root / "out.jsonl"
            raw_jsonl.write_text(
                json.dumps(
                    {
                        "case_id": "CASE-DW-001",
                        "target_system": "TGT-004",
                        "interface_id": "IFS-WEB-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "sha256:" + "a" * 64,
                        "response_hash": "sha256:" + "b" * 64,
                        "verdict": "PASS",
                    }
                )
                + "\n"
            )

            result = run_normalize(
                "--input",
                str(raw_jsonl),
                "--output",
                str(out_jsonl),
                "--release-id",
                "RC-TEST",
                "--enforce-thresholds",
                "--threshold",
                "DICOMweb=1",
                "--threshold",
                "DIMSE=1",
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("threshold not met for DIMSE", result.stdout)

    def test_rejects_invalid_hash(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            raw_jsonl = root / "raw.jsonl"
            out_jsonl = root / "out.jsonl"
            raw_jsonl.write_text(
                json.dumps(
                    {
                        "case_id": "CASE-001",
                        "target_system": "TGT-004",
                        "interface_id": "IFS-001",
                        "protocol": "DICOMweb",
                        "phase": "positive",
                        "request_hash": "not-a-hash",
                        "response_hash": "sha256:" + "a" * 64,
                        "verdict": "PASS",
                    }
                )
                + "\n"
            )

            result = run_normalize(
                "--input",
                str(raw_jsonl),
                "--output",
                str(out_jsonl),
                "--release-id",
                "RC-TEST",
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("invalid request_hash", result.stdout)


if __name__ == "__main__":
    unittest.main()
