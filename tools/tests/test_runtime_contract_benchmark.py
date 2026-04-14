from __future__ import annotations

import pathlib
import subprocess
import tempfile
import textwrap
import time
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "runtime_env_contract.py"


class RuntimeContractBenchmarkTests(unittest.TestCase):
    def test_runtime_env_contract_check_stays_within_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "docs").mkdir(parents=True, exist_ok=True)
            (root / "crates/dicom-web-server/src").mkdir(parents=True, exist_ok=True)
            (root / "crates/dicom-workflow-server/src").mkdir(parents=True, exist_ok=True)
            (root / "crates/dicom-dimse-service/src/bin").mkdir(parents=True, exist_ok=True)

            (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
                textwrap.dedent(
                    """
                    ### `dicom-web-server` (env-contract)
                    - `DICOM_WEB_BIND`

                    ### `dicom-workflow-server` (env-contract)
                    - `DICOM_WORKFLOW_BIND`

                    ### `dicom-dimse-service` (env-contract)
                    - `DICOM_DIMSE_BIND`
                    """
                ).strip()
                + "\n",
                encoding="utf-8",
            )
            (root / "crates/dicom-web-server/src/main.rs").write_text(
                'let _ = std::env::var("DICOM_WEB_BIND");\n',
                encoding="utf-8",
            )
            (root / "crates/dicom-workflow-server/src/main.rs").write_text(
                'let _ = std::env::var("DICOM_WORKFLOW_BIND");\n',
                encoding="utf-8",
            )
            (root / "crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs").write_text(
                'let _ = std::env::var("DICOM_DIMSE_BIND");\n',
                encoding="utf-8",
            )

            start = time.perf_counter()
            result = subprocess.run(
                [
                    "python3",
                    str(SCRIPT),
                    "--repo-root",
                    str(root),
                    "--check-docs",
                    "--report",
                    "reports/bench/runtime-env-contract.json",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            elapsed_ms = (time.perf_counter() - start) * 1000.0

            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
            self.assertLess(
                elapsed_ms,
                1500.0,
                msg=f"runtime_env_contract.py exceeded benchmark threshold: {elapsed_ms:.2f}ms",
            )


if __name__ == "__main__":
    unittest.main()
