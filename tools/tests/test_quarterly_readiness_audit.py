from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "quarterly_readiness_audit.py"


class QuarterlyReadinessAuditTests(unittest.TestCase):
    def _seed_repo(self, root: pathlib.Path, *, failing_runtime_contract: bool = False) -> None:
        (root / "docs").mkdir(parents=True, exist_ok=True)
        (root / "reports/docs").mkdir(parents=True, exist_ok=True)
        (root / "tools").mkdir(parents=True, exist_ok=True)
        (root / "reports/release").mkdir(parents=True, exist_ok=True)

        (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text("# d\n", encoding="utf-8")
        (root / "docs/33-Productization-Profiles-and-Playbooks.md").write_text("# d\n", encoding="utf-8")
        (root / "docs/63-Environment-Promotion-Checklist.md").write_text("# d\n", encoding="utf-8")
        (root / "reports/docs/startup-contract-snapshot-backend-services.md").write_text("# d\n", encoding="utf-8")
        (root / "reports/docs/startup-contract-snapshot-backend-services-with-dimse.md").write_text("# d\n", encoding="utf-8")
        (root / "tools/package_profiles.sh").write_text("#!/bin/bash\nexit 0\n", encoding="utf-8")

        runtime_exit = "1" if failing_runtime_contract else "0"
        (root / "tools/runtime_env_contract.py").write_text(
            f"#!/usr/bin/env python3\nimport sys\nsys.exit({runtime_exit})\n",
            encoding="utf-8",
        )
        (root / "tools/docs_env_review_gate.py").write_text(
            "#!/usr/bin/env python3\nimport sys\nsys.exit(0)\n",
            encoding="utf-8",
        )
        (root / "tools/profile_metadata_reconciliation_gate.py").write_text(
            "#!/usr/bin/env python3\nimport sys\nsys.exit(0)\n",
            encoding="utf-8",
        )

    def test_passes_when_commands_and_required_paths_pass(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_repo(root, failing_runtime_contract=False)
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root), "--release-id", "AUDIT-OK"],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)

    def test_fails_when_runtime_contract_command_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_repo(root, failing_runtime_contract=True)
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root), "--release-id", "AUDIT-FAIL"],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main()
