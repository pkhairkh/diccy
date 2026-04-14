from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "docs_capability_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(SCRIPT), "--repo-root", str(repo_root)],
        check=False,
        capture_output=True,
        text=True,
    )


class DocsCapabilityLintTests(unittest.TestCase):
    def test_passes_with_status_stamp_and_reference(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)

            good_text = "As of 2026-02-21\nSee docs/31-Implementation-Status.md\n"
            (root / "README.md").write_text(good_text)
            (root / "docs/01-Vision-and-Scope.md").write_text(good_text)
            (root / "docs/06-Rendering-and-Interaction.md").write_text(good_text)
            (root / "docs/08-WASM-Target.md").write_text(good_text)

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Capability claim lint: PASS", result.stdout)

    def test_fails_for_prohibited_claim(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)

            (root / "README.md").write_text(
                "WASM via `wgpu`/WebGPU with WebGL fallback\nAs of 2026-02-21\n"
                "docs/31-Implementation-Status.md\n"
            )
            ok_text = "As of 2026-02-21\ndocs/31-Implementation-Status.md\n"
            (root / "docs/01-Vision-and-Scope.md").write_text(ok_text)
            (root / "docs/06-Rendering-and-Interaction.md").write_text(ok_text)
            (root / "docs/08-WASM-Target.md").write_text(ok_text)

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Capability claim lint: FAIL", result.stdout)
            self.assertIn("unsupported webgpu fallback claim", result.stdout)


if __name__ == "__main__":
    unittest.main()
