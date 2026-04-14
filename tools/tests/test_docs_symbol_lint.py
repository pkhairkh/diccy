from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "docs_symbol_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(SCRIPT), "--repo-root", str(repo_root)],
        check=False,
        capture_output=True,
        text=True,
    )


class DocsSymbolLintTests(unittest.TestCase):
    def test_passes_when_implemented_symbols_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "crates/sample/src").mkdir(parents=True, exist_ok=True)

            (root / "docs/31-Implementation-Status.md").write_text(
                "| Symbol | Status | Expected location |\n"
                "|---|---|---|\n"
                "| `AlphaSymbol` | Implemented | `crates/sample/src/lib.rs` |\n"
                "| `BetaSymbol` | Deferred | `crates/sample/src/lib.rs` |\n"
            )
            (root / "crates/sample/src/lib.rs").write_text("pub struct AlphaSymbol;\n")

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Docs symbol lint: PASS", result.stdout)

    def test_fails_when_implemented_symbol_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "crates/sample/src").mkdir(parents=True, exist_ok=True)

            (root / "docs/31-Implementation-Status.md").write_text(
                "| Symbol | Status | Expected location |\n"
                "|---|---|---|\n"
                "| `MissingSymbol` | Implemented | `crates/sample/src/lib.rs` |\n"
            )
            (root / "crates/sample/src/lib.rs").write_text("pub struct SomethingElse;\n")

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Docs symbol lint: FAIL", result.stdout)
            self.assertIn("MissingSymbol", result.stdout)


if __name__ == "__main__":
    unittest.main()
