from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import traceability_report


class TraceabilityReportTests(unittest.TestCase):
    def test_collects_must_reqs_and_detects_missing_test_refs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            docs = root / "docs"
            reports = root / "reports"
            tests_dir = root / "crates" / "diccy" / "tests"
            docs.mkdir(parents=True)
            reports.mkdir(parents=True)
            tests_dir.mkdir(parents=True)

            (docs / "spec.md").write_text(
                "\n".join(
                    [
                        "- **REQ-ABC-001:** parser **MUST** reject malformed input.",
                        "- **REQ-ABC-002:** decoder **MUST** fail closed on limits.",
                    ]
                )
                + "\n"
            )
            (tests_dir / "trace.rs").write_text(
                "\n".join(
                    [
                        "#[test]",
                        "fn sample() {",
                        "    // REQ-ABC-001",
                        "    assert!(true);",
                        "}",
                    ]
                )
                + "\n"
            )
            (reports / "evidence.md").write_text("Coverage for REQ-ABC-001.\n")

            must_reqs = traceability_report.collect_must_reqs(root)
            self.assertEqual(sorted(must_reqs.keys()), ["REQ-ABC-001", "REQ-ABC-002"])

            rust_refs, test_refs = traceability_report.collect_rust_and_test_refs(root)
            self.assertIn("REQ-ABC-001", rust_refs)
            self.assertIn("REQ-ABC-001", test_refs)
            self.assertNotIn("REQ-ABC-002", test_refs)

            missing = sorted(req for req in must_reqs if req not in test_refs)
            self.assertEqual(missing, ["REQ-ABC-002"])

    def test_manifest_validation_flags_missing_paths_and_unknown_req(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            docs = root / "docs"
            reports = root / "reports"
            docs.mkdir(parents=True)
            reports.mkdir(parents=True)
            (docs / "base.md").write_text("- **REQ-AAA-001:** test **MUST** pass.\n")
            (reports / "ok.md").write_text("Evidence REQ-AAA-001\n")

            manifest = root / "manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "CLM-001",
                                "source_doc": "docs/base.md",
                                "req_ids": ["REQ-AAA-001", "REQ-AAA-999"],
                                "evidence_paths": ["reports/ok.md", "reports/missing.md"],
                                "gate_logs": [],
                            }
                        ],
                        "release_index_entries": [
                            {
                                "entry_id": "REL-001",
                                "index_path": "reports/missing-index.md",
                                "cluster_ids": ["CLM-001"],
                                "gate_logs": ["reports/missing.log"],
                            }
                        ],
                    }
                )
            )

            invalid, claims, release_entries = traceability_report.validate_manifest(
                root,
                manifest,
                all_reqs={"REQ-AAA-001"},
                test_refs={"REQ-AAA-001": ["x"]},
                report_refs={"REQ-AAA-001": ["y"]},
            )
            self.assertTrue(invalid)
            self.assertEqual(len(claims), 1)
            self.assertEqual(len(release_entries), 1)
            joined = "\n".join(invalid)
            self.assertIn("unknown REQ ID REQ-AAA-999", joined)
            self.assertIn("missing evidence path reports/missing.md", joined)
            self.assertIn("missing index_path file reports/missing-index.md", joined)

    def test_compute_delta(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            baseline = root / "baseline.json"
            baseline.write_text(
                json.dumps(
                    {
                        "must_reqs_count": 10,
                        "missing_references_count": 2,
                        "invalid_links_count": 1,
                    }
                )
            )
            current = {
                "must_reqs_count": 12,
                "missing_references_count": 0,
                "invalid_links_count": 1,
            }
            delta = traceability_report.compute_delta(current, baseline)
            self.assertIsNotNone(delta)
            self.assertEqual(delta["must_reqs"], 2)
            self.assertEqual(delta["missing_references"], -2)
            self.assertEqual(delta["invalid_links"], 0)


if __name__ == "__main__":
    unittest.main()
