from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "tasks_md_guard.py"


class TasksMdGuardTests(unittest.TestCase):
    def test_fails_when_checked_entries_drop_without_override(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            before = root / "before.md"
            after = root / "after.md"
            before.write_text("- [x] done\n- [ ] open\n", encoding="utf-8")
            after.write_text("- [ ] open\n", encoding="utf-8")

            result = subprocess.run(
                ["python3", str(SCRIPT), "--before", str(before), "--after", str(after)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("TASKS flow guard: FAIL", result.stdout)

    def test_passes_when_checked_entries_drop_with_allow_reset(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            before = root / "before.md"
            after = root / "after.md"
            before.write_text("- [x] done\n- [ ] open\n", encoding="utf-8")
            after.write_text("- [ ] open\n", encoding="utf-8")

            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--before",
                    str(before),
                    "--after",
                    str(after),
                    "--allow-reset",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)
            self.assertIn("TASKS flow guard: PASS", result.stdout)


if __name__ == "__main__":
    unittest.main()
