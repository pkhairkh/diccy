from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import textwrap
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "docs_drift_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--repo-root",
            str(repo_root),
            "--report",
            "reports/docs/drift-report.json",
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def seed_minimal_markdown(repo_root: pathlib.Path) -> None:
    (repo_root / "docs").mkdir(parents=True, exist_ok=True)
    (repo_root / "README.md").write_text("No symbol contradiction here.\n", encoding="utf-8")
    (repo_root / "docs/31-Implementation-Status.md").write_text(
        "| Symbol | Status | Expected location |\n"
        "|---|---|---|\n"
        "| `AlphaSymbol` | Implemented | `crates/sample/src/lib.rs` |\n"
        "| `BetaSymbol` | Deferred | `crates/sample/src/lib.rs` |\n",
        encoding="utf-8",
    )


def seed_runtime_env_contract_fixture(
    repo_root: pathlib.Path,
    *,
    web_env: list[str],
    workflow_env: list[str],
    docs_web_env: list[str],
    docs_workflow_env: list[str],
) -> None:
    web_main = repo_root / "crates/dicom-web-server/src/main.rs"
    workflow_main = repo_root / "crates/dicom-workflow-server/src/main.rs"
    web_main.parent.mkdir(parents=True, exist_ok=True)
    workflow_main.parent.mkdir(parents=True, exist_ok=True)

    web_code = "\n".join([f'let _ = env::var("{name}");' for name in web_env]) + "\n"
    workflow_code = "\n".join([f'let _ = env::var("{name}");' for name in workflow_env]) + "\n"
    web_main.write_text(web_code, encoding="utf-8")
    workflow_main.write_text(workflow_code, encoding="utf-8")

    docs_web_lines = "\n".join([f"- `{name}` documented" for name in docs_web_env])
    docs_workflow_lines = "\n".join([f"- `{name}` documented" for name in docs_workflow_env])
    docs_12 = textwrap.dedent(
        f"""
        # API surface and crate boundaries

        ### `dicom-web-server` (env-contract)

        {docs_web_lines}

        ### `dicom-workflow-server` (env-contract)

        {docs_workflow_lines}
        """
    ).strip()
    (repo_root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
        docs_12 + "\n", encoding="utf-8"
    )


class DocsDriftLintTests(unittest.TestCase):
    def test_passes_for_deferred_symbol_with_whitelisted_phrase(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            (root / "docs/01-Vision-and-Scope.md").write_text(
                "`BetaSymbol` is deferred in the target architecture.\n",
                encoding="utf-8",
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Docs drift lint: PASS", result.stdout)
            payload = json.loads((root / "reports/docs/drift-report.json").read_text(encoding="utf-8"))
            self.assertEqual(payload["violation_count"], 0)

    def test_fails_when_implemented_symbol_is_claimed_deferred(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            (root / "docs/06-Rendering-and-Interaction.md").write_text(
                "`AlphaSymbol` remains deferred until implementation lands.\n",
                encoding="utf-8",
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Docs drift lint: FAIL", result.stdout)
            self.assertIn("AlphaSymbol", result.stdout)

            payload = json.loads((root / "reports/docs/drift-report.json").read_text(encoding="utf-8"))
            self.assertGreaterEqual(payload["violation_count"], 1)
            first = payload["violations"][0]
            self.assertEqual(first["symbol"], "AlphaSymbol")
            self.assertEqual(first["status"], "implemented")

    def test_fails_for_ambiguous_deployment_claim(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
                "# API surface and crate boundaries\n\n`dicom-dimse-service` is an optional service.\n",
                encoding="utf-8",
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Docs drift lint: FAIL", result.stdout)
            self.assertIn("deployment_posture_ambiguity", result.stdout)

    def test_allows_anchored_deployment_claim(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text(
                "# API surface and crate boundaries\n\n"
                "`dicom-dimse-service` is optional in `backend-services-with-dimse.<RELEASE_ID>.tar.gz`.\n",
                encoding="utf-8",
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Docs drift lint: PASS", result.stdout)

    def test_env_contract_fails_for_added_runtime_variable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            seed_runtime_env_contract_fixture(
                root,
                web_env=["DICOM_WEB_BIND", "DICOM_WEB_NEW_LIMIT"],
                workflow_env=["DICOM_WORKFLOW_BIND"],
                docs_web_env=["DICOM_WEB_BIND"],
                docs_workflow_env=["DICOM_WORKFLOW_BIND"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("env_var_missing_in_docs", result.stdout)
            self.assertIn("DICOM_WEB_NEW_LIMIT", result.stdout)
            self.assertIn("docs_only_env_contract_change", result.stdout)
            self.assertIn("guardrail: docs-only env-contract update detected", result.stdout)

    def test_env_contract_fails_for_removed_documented_variable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            seed_runtime_env_contract_fixture(
                root,
                web_env=["DICOM_WEB_BIND"],
                workflow_env=["DICOM_WORKFLOW_BIND"],
                docs_web_env=["DICOM_WEB_BIND", "DICOM_WEB_DEPRECATED"],
                docs_workflow_env=["DICOM_WORKFLOW_BIND"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("env_var_documented_not_in_code", result.stdout)
            self.assertIn("DICOM_WEB_DEPRECATED", result.stdout)

    def test_env_contract_fails_for_renamed_variable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            seed_minimal_markdown(root)
            seed_runtime_env_contract_fixture(
                root,
                web_env=["DICOM_WEB_BIND", "DICOM_WEB_ACCEPT_QUEUE_DEPTH"],
                workflow_env=["DICOM_WORKFLOW_BIND"],
                docs_web_env=["DICOM_WEB_BIND", "DICOM_WEB_QUEUE_DEPTH"],
                docs_workflow_env=["DICOM_WORKFLOW_BIND"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("DICOM_WEB_ACCEPT_QUEUE_DEPTH", result.stdout)


if __name__ == "__main__":
    unittest.main()
