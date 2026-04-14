from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import evidence_realism_lint


class EvidenceRealismLintTests(unittest.TestCase):
    def test_detects_replay_style_hash_suffix_pattern(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            reports = root / "reports" / "release"
            reports.mkdir(parents=True)
            sample = reports / "synthetic-RC-TEST.md"
            sample.write_text(
                "\n".join(
                    [
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0001",
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0002",
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0003",
                    ]
                )
                + "\n"
            )
            findings = evidence_realism_lint.run_lint(root, "RC-TEST", None)
            self.assertTrue(any("replay-style sequential" in finding for finding in findings))

    def test_manifest_origin_validation_requires_all_evidence_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports").mkdir(parents=True)
            evidence = root / "reports" / "ok-RC-TEST.md"
            evidence.write_text("ok\n")
            manifest = root / "manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "CLM-1",
                                "evidence_paths": ["reports/ok-RC-TEST.md"],
                                "evidence_origin": {},
                            }
                        ],
                        "release_index_entries": [
                            {
                                "entry_id": "REL-1",
                            }
                        ],
                    }
                )
            )
            findings = evidence_realism_lint.run_lint(root, "RC-TEST", manifest)
            joined = "\n".join(findings)
            self.assertIn("missing/invalid origin", joined)
            self.assertIn("missing/invalid entry_origin", joined)

    def test_passes_with_non_pattern_hashes_and_origin_coverage(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            reports = root / "reports"
            reports.mkdir(parents=True)
            a = reports / "a-RC-TEST.md"
            b = reports / "b-RC-TEST.md"
            a.write_text("sha256:28f17217cd27f3b80849dbf2bb16acb813f9ce2f6f3e08ba7fdb2c16ea2c2d6a\n")
            b.write_text("sha256:c09d326f7e73cd6e63f74d4e0e229ba4ad8c019f82f5988e4dfd4e9ef0b5a16c\n")
            manifest = root / "manifest.json"
            manifest.write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "CLM-1",
                                "evidence_paths": ["reports/a-RC-TEST.md", "reports/b-RC-TEST.md"],
                                "evidence_origin": {
                                    "reports/a-RC-TEST.md": "de-identified",
                                    "reports/b-RC-TEST.md": "external-independent",
                                },
                            }
                        ],
                        "release_index_entries": [
                            {
                                "entry_id": "REL-1",
                                "entry_origin": "external-independent",
                            }
                        ],
                    }
                )
            )
            findings = evidence_realism_lint.run_lint(root, "RC-TEST", manifest)
            self.assertEqual(findings, [])


if __name__ == "__main__":
    unittest.main()
