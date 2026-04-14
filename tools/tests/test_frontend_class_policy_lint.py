from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "frontend_class_policy_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--repo-root",
            str(repo_root),
            "--frontend-root",
            "frontend/src",
            "--report",
            "reports/docs/frontend-class-policy-report.json",
            "--check",
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def seed_fixture(repo_root: pathlib.Path) -> None:
    src = repo_root / "frontend/src"
    (src / "components/components").mkdir(parents=True, exist_ok=True)
    (src / "components/primitives").mkdir(parents=True, exist_ok=True)
    (src / "components/panels").mkdir(parents=True, exist_ok=True)

    (src / "components/components/BadCard.vue").write_text(
        "<template><div class=\"px-2 py-1\" :style=\"{ color: 'red' }\">X</div></template>\n",
        encoding="utf-8",
    )
    (src / "components/primitives/PrimitiveBox.vue").write_text(
        "<template><div class=\"px-2 py-1\">ok in primitive</div></template>\n",
        encoding="utf-8",
    )
    (src / "components/panels/PanelA.vue").write_text(
        "<template><section class=\"mx-4 mt-2\">Panel</section></template>\n",
        encoding="utf-8",
    )


class FrontendClassPolicyLintTests(unittest.TestCase):
    def test_reports_grouped_violations_by_rule_and_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_fixture(root)

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("raw_multi_token_class", result.stdout)
            self.assertIn("style_attribute_non_primitive", result.stdout)
            self.assertIn("frontend/src/components/components/BadCard.vue", result.stdout)

            payload = json.loads(
                (root / "reports/docs/frontend-class-policy-report.json").read_text(encoding="utf-8")
            )
            self.assertEqual(payload["violation_file_count"], 2)
            self.assertIn("raw_multi_token_class", payload["violations_grouped_by_rule"])
            self.assertIn("style_attribute_non_primitive", payload["violations_grouped_by_rule"])


if __name__ == "__main__":
    unittest.main()
