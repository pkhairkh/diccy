from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import release_artifact_integrity


class ReleaseArtifactIntegrityTests(unittest.TestCase):
    def test_collect_markdown_references_deduplicates(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            index_path = pathlib.Path(tmp_dir) / "index.md"
            index_path.write_text(
                "\n".join(
                    [
                        "- `reports/a.md`",
                        "- `reports/b.md` and `reports/a.md`",
                        "- unrelated `docs/spec.md`",
                    ]
                )
                + "\n"
            )
            refs = release_artifact_integrity.collect_markdown_references(index_path)
            self.assertEqual(refs, ["reports/a.md", "reports/b.md"])

    def test_evaluate_release_artifacts_flags_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            index = root / "reports" / "interoperability" / "release-evidence-index.md"
            index.parent.mkdir(parents=True)
            index.write_text("- `reports/existing.md`\n- `reports/missing.md`\n")
            (root / "reports" / "existing.md").write_text("ok\n")

            payload = release_artifact_integrity.evaluate_release_artifacts(
                repo_root=root,
                release_id="RC-TEST",
                index_path=index,
            )
            self.assertEqual(payload["overall_status"], "FAIL")
            self.assertGreater(payload["missing_required_count"], 0)
            self.assertEqual(payload["missing_index_reference_count"], 1)
            self.assertIn("reports/missing.md", payload["missing_index_references"])

    def test_evaluate_release_artifacts_passes_when_complete(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            for rel in release_artifact_integrity.required_artifacts(release_id):
                path = root / rel
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("ok\n")

            index = root / "reports" / "interoperability" / "release-evidence-index.md"
            index.write_text(
                "\n".join(
                    [
                        "- `reports/security/sbom-RC-TEST.json`",
                        "- `reports/traceability/release-evidence-trace-index-RC-TEST.md`",
                    ]
                )
                + "\n"
            )

            payload = release_artifact_integrity.evaluate_release_artifacts(
                repo_root=root,
                release_id=release_id,
                index_path=index,
            )
            self.assertEqual(payload["overall_status"], "PASS")
            self.assertEqual(payload["missing_required_count"], 0)
            self.assertEqual(payload["missing_index_reference_count"], 0)


if __name__ == "__main__":
    unittest.main()
