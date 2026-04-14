from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "docs_env_review_gate.py"


def seed_repo(root: pathlib.Path, *, docs_web: str, code_web: str) -> None:
    (root / "docs").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-web-server/src").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-workflow-server/src").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-dimse-service/src/bin").mkdir(parents=True, exist_ok=True)
    (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
        "\n".join(
            [
                "### `dicom-web-server` (env-contract)",
                "",
                docs_web,
                "",
                "### `dicom-workflow-server` (env-contract)",
                "",
                "- `DICOM_WORKFLOW_BIND`",
                "",
                "### `dicom-dimse-service` (env-contract)",
                "",
                "- `DICOM_DIMSE_BIND`",
                "",
            ]
        ),
        encoding="utf-8",
    )
    (root / "crates/dicom-web-server/src/main.rs").write_text(code_web + "\n", encoding="utf-8")
    (root / "crates/dicom-workflow-server/src/main.rs").write_text(
        'let _ = std::env::var("DICOM_WORKFLOW_BIND");\n',
        encoding="utf-8",
    )
    (root / "crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs").write_text(
        'let _ = std::env::var("DICOM_DIMSE_BIND");\n',
        encoding="utf-8",
    )


class DocsEnvReviewGateTests(unittest.TestCase):
    def test_passes_when_docs_match_code(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(
                root,
                docs_web="- `DICOM_WEB_BIND`",
                code_web='let _ = std::env::var("DICOM_WEB_BIND");',
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)
            payload = json.loads(
                (root / "reports/docs/docs-env-review-gate.json").read_text(encoding="utf-8")
            )
            self.assertEqual(payload["status"], "PASS")

    def test_fails_when_docs_drift_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(
                root,
                docs_web="- `DICOM_WEB_BIND`",
                code_web='let _ = std::env::var("DICOM_WEB_BIND");\nlet _ = std::env::var("DICOM_WEB_NEW_FLAG");',
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)
            payload = json.loads(
                (root / "reports/docs/docs-env-review-gate.json").read_text(encoding="utf-8")
            )
            self.assertEqual(payload["status"], "FAIL")
            self.assertGreaterEqual(len(payload["violations"]), 1)


if __name__ == "__main__":
    unittest.main()
