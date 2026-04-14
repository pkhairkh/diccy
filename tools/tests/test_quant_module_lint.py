from __future__ import annotations

import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "quant_module_lint.py"


def run_lint(repo_root: pathlib.Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(SCRIPT), "--repo-root", str(repo_root)],
        check=False,
        capture_output=True,
        text=True,
    )


def write_fixture(
    repo_root: pathlib.Path,
    *,
    module_rows: str,
    claim_rows: str,
    alignment_rows: str,
    evidence_ids: list[str],
) -> None:
    (repo_root / "docs").mkdir(parents=True, exist_ok=True)
    (repo_root / "reports" / "analytical").mkdir(parents=True, exist_ok=True)
    (repo_root / "reports" / "clinical").mkdir(parents=True, exist_ok=True)
    (repo_root / "reports" / "pmcf").mkdir(parents=True, exist_ok=True)

    for evidence_id in evidence_ids:
        if evidence_id.startswith("ANL-"):
            path = repo_root / "reports" / "analytical" / f"{evidence_id}.md"
        elif evidence_id.startswith("CLI-"):
            path = repo_root / "reports" / "clinical" / f"{evidence_id}.md"
        elif evidence_id.startswith("PMCF-"):
            path = repo_root / "reports" / "pmcf" / f"{evidence_id}.md"
        else:
            raise AssertionError(f"unsupported evidence id in fixture: {evidence_id}")
        path.write_text(f"# {evidence_id}\n")

    (repo_root / "docs/26-Scientific-Grade-Quantification-Validation-Program.md").write_text(
        "\n".join(
            [
                "# Fixture Doc 26",
                "",
                "| Module ID | Quantification Area | Analytical Protocol ID | Clinical Protocol ID | Uncertainty ID | Limits-of-Use ID | Evidence IDs | Status |",
                "|---|---|---|---|---|---|---|---|",
                module_rows,
                "",
            ]
        )
    )

    (repo_root / "docs/22-Claim-to-Evidence-Matrix.md").write_text(
        "\n".join(
            [
                "# Fixture Doc 22",
                "",
                "| Claim ID | Claim Text | Current Status |",
                "|---|---|---|",
                claim_rows,
                "",
                "| Module ID | Claim IDs | Evidence IDs | Alignment Status |",
                "|---|---|---|---|",
                alignment_rows,
                "",
            ]
        )
    )


class QuantModuleLintTests(unittest.TestCase):
    def test_passes_for_complete_active_and_deferred_modules(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            write_fixture(
                root,
                module_rows="\n".join(
                    [
                        "| QNT-001 | Geometry | ANP-001 | CLP-001 | UNC-001 | LIM-001 | ANL-001, CLI-001, PMCF-001 | Active (Signed RC-2026.02.11) |",
                        "| QNT-004 | AI-assisted | ANP-004 | CLP-004 | UNC-004 | LIM-004 | N/A (out-of-envelope baseline) | Deferred (out-of-envelope baseline) |",
                    ]
                ),
                claim_rows="| CLM-001 | Reproducible quantification claim | Approved (RC-2026.02.11) |",
                alignment_rows="\n".join(
                    [
                        "| QNT-001 | CLM-001 | ANL-001, CLI-001, PMCF-001 | Aligned (Signed RC-2026.02.11) |",
                        "| QNT-004 | N/A (Deferred) | N/A (out-of-envelope baseline) | Deferred |",
                    ]
                ),
                evidence_ids=["ANL-001", "CLI-001", "PMCF-001"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 0)
            self.assertIn("Quant module lint: PASS", result.stdout)
            self.assertIn("Violations: 0", result.stdout)

    def test_fails_when_active_module_is_missing_protocol_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            write_fixture(
                root,
                module_rows="| QNT-001 | Geometry | N/A | CLP-001 | UNC-001 | LIM-001 | ANL-001, CLI-001, PMCF-001 | Active (Signed RC-2026.02.11) |",
                claim_rows="| CLM-001 | Reproducible quantification claim | Approved (RC-2026.02.11) |",
                alignment_rows="| QNT-001 | CLM-001 | ANL-001, CLI-001, PMCF-001 | Aligned (Signed RC-2026.02.11) |",
                evidence_ids=["ANL-001", "CLI-001", "PMCF-001"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Quant module lint: FAIL", result.stdout)
            self.assertIn("QNT-001: missing required field: Analytical Protocol ID", result.stdout)

    def test_fails_when_deferred_module_maps_to_claims(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            write_fixture(
                root,
                module_rows="| QNT-004 | AI-assisted | ANP-004 | CLP-004 | UNC-004 | LIM-004 | N/A (out-of-envelope baseline) | Deferred (out-of-envelope baseline) |",
                claim_rows="| CLM-001 | Reproducible quantification claim | Approved (RC-2026.02.11) |",
                alignment_rows="| QNT-004 | CLM-001 | ANL-001 | Aligned (invalid deferred mapping) |",
                evidence_ids=["ANL-001"],
            )

            result = run_lint(root)
            self.assertEqual(result.returncode, 1)
            self.assertIn("Quant module lint: FAIL", result.stdout)
            self.assertIn("QNT-004: deferred module must not map to active claim IDs", result.stdout)


if __name__ == "__main__":
    unittest.main()
