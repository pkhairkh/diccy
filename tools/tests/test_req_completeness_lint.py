from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import req_completeness_lint


class ReqCompletenessLintTests(unittest.TestCase):
    def test_collect_normative_without_req_ignores_req_lines_and_code_blocks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            docs = pathlib.Path(tmp_dir) / "docs"
            docs.mkdir(parents=True)
            (docs / "sample.md").write_text(
                "\n".join(
                    [
                        "This behavior MUST be deterministic.",
                        "- **REQ-ABC-001:** parser **MUST** fail closed.",
                        "```text",
                        "code block MAY contain words but should be ignored",
                        "```",
                    ]
                )
                + "\n"
            )
            findings = req_completeness_lint.collect_normative_without_req(docs)
            self.assertEqual(len(findings), 1)
            self.assertIn("MUST be deterministic", findings[0]["text"])

    def test_fingerprint_is_stable(self) -> None:
        rel = pathlib.Path("docs/example.md")
        line = "This SHOULD be tracked."
        first = req_completeness_lint.fingerprint(rel, 12, line)
        second = req_completeness_lint.fingerprint(rel, 12, line)
        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
