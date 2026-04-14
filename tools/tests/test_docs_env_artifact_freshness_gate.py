from __future__ import annotations

import pathlib
import tempfile
import unittest

import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.append(str(ROOT))

from docs_env_artifact_freshness_gate import (  # noqa: E402
    _json_dump,
    docs_drift_payload,
    docs_env_review_payload,
    evaluate_freshness,
    runtime_env_contract_payload,
)


def seed_repo(root: pathlib.Path) -> None:
    (root / "docs").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-web-server/src").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-workflow-server/src").mkdir(parents=True, exist_ok=True)
    (root / "crates/dicom-dimse-service/src/bin").mkdir(parents=True, exist_ok=True)

    (root / "README.md").write_text("seed\n", encoding="utf-8")
    (root / "docs/31-Implementation-Status.md").write_text(
        "| Symbol | Status | Expected location |\n|---|---|---|\n",
        encoding="utf-8",
    )
    (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
        "\n".join(
            [
                "### `dicom-web-server` (env-contract)",
                "",
                "- `DICOM_WEB_BIND`",
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

    reports = root / "reports/docs"
    reports.mkdir(parents=True, exist_ok=True)
    (reports / "runtime-env-contract-check.json").write_text(
        _json_dump(runtime_env_contract_payload(root)),
        encoding="utf-8",
    )
    (reports / "drift-report.json").write_text(
        _json_dump(docs_drift_payload(root)),
        encoding="utf-8",
    )
    (reports / "docs-env-review-gate.json").write_text(
        _json_dump(docs_env_review_payload(root)),
        encoding="utf-8",
    )


class DocsEnvArtifactFreshnessGateTests(unittest.TestCase):
    def test_passes_for_fresh_and_consistent_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(root)
            payload = evaluate_freshness(
                root,
                runtime_report_path=root / "reports/docs/runtime-env-contract-check.json",
                docs_drift_report_path=root / "reports/docs/drift-report.json",
                docs_env_review_report_path=root / "reports/docs/docs-env-review-gate.json",
            )
            self.assertEqual(payload["status"], "PASS")
            self.assertEqual(payload["violations"], [])

    def test_fails_for_stale_report_artifact(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_repo(root)
            (root / "reports/docs/runtime-env-contract-check.json").write_text(
                "{}\n",
                encoding="utf-8",
            )
            payload = evaluate_freshness(
                root,
                runtime_report_path=root / "reports/docs/runtime-env-contract-check.json",
                docs_drift_report_path=root / "reports/docs/drift-report.json",
                docs_env_review_report_path=root / "reports/docs/docs-env-review-gate.json",
            )
            self.assertEqual(payload["status"], "FAIL")
            self.assertTrue(any(v["kind"] == "stale_artifact" for v in payload["violations"]))


if __name__ == "__main__":
    unittest.main()
