from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest
import json


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "claim_surface_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--repo-root",
            str(repo_root),
            "--root",
            "README.md",
            "--root",
            "docs",
            "--allow-path",
            "docs/15-Regulatory-and-Standards-Mapping.md",
        ],
        check=False,
        capture_output=True,
        text=True,
    )


class ClaimSurfaceLintTests(unittest.TestCase):
    def test_fails_closed_for_prohibited_phrase_outside_allowlist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "README.md").write_text("Product is FDA cleared for diagnostic use.\n")
            (root / "docs/15-Regulatory-and-Standards-Mapping.md").write_text("informative\n")

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Claim surface lint: FAIL", result.stdout)
            self.assertIn("README.md:1", result.stdout)

    def test_allows_regulatory_terms_inside_informative_allowlist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "README.md").write_text("RDVF framework scope only.\n")
            (root / "docs/15-Regulatory-and-Standards-Mapping.md").write_text(
                "Informative note references MDR class and diagnostic context.\n"
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Claim surface lint: PASS", result.stdout)

    def test_writes_machine_readable_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "README.md").write_text("RDVF framework scope only.\n")
            (root / "docs/15-Regulatory-and-Standards-Mapping.md").write_text("informative\n")
            report_path = root / "reports/docs/claim-surface-lint.json"
            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--root",
                    "README.md",
                    "--root",
                    "docs",
                    "--allow-path",
                    "docs/15-Regulatory-and-Standards-Mapping.md",
                    "--report",
                    str(report_path),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)
            payload = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["status"], "PASS")
            self.assertEqual(payload["violations_count"], 0)


if __name__ == "__main__":
    unittest.main()
