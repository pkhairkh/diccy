from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import textwrap
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "profile_docs_parity_gate.py"


def run_gate(
    *,
    repo_root: pathlib.Path,
    profile_matrix_payload: dict,
) -> subprocess.CompletedProcess[str]:
    docs = repo_root / "docs"
    docs.mkdir()
    tools = repo_root / "tools"
    tools.mkdir()
    (docs / "03-DICOM-Conformance-Envelope.md").write_text(
        textwrap.dedent(
            """
            # DICOM conformance envelope
            ## Envelope version
            - `envelope_version`: `1.1`
            """
        ),
        encoding="utf-8",
    )
    (docs / "12-API-Surface-and-Crate-Boundaries.md").write_text(
        textwrap.dedent(
            """
            ### `dicom-web-server` (env-contract)
            - `DICOM_WEB_BIND`
            ### `dicom-workflow-server` (env-contract)
            - `DICOM_WORKFLOW_BIND`
            ### `dicom-dimse-service` (env-contract)
            - `DICOM_DIMSE_BIND`
            """
        ),
        encoding="utf-8",
    )
    (docs / "33-Productization-Profiles-and-Playbooks.md").write_text(
        textwrap.dedent(
            """
            ## Runtime artifacts by profile
            | Profile | Runtime crates (library) | Packaged entrypoints |
            |---|---|---|
            | `framework-core` | `rdvf` | `dicom-visualizer` |
            | `backend-services` | `dicom-web-server` | `dicom-web-server` |
            """
        ),
        encoding="utf-8",
    )
    (docs / "49-Runtime-Profile-Capability-Matrix.md").write_text(
        textwrap.dedent(
            """
            | Capability | `framework-core` | `backend-services` |
            |---|---|---|
            | CPU | Yes | Yes |
            """
        ),
        encoding="utf-8",
    )
    matrix = repo_root / "tools/profile_matrix.json"
    matrix.parent.mkdir(parents=True, exist_ok=True)
    matrix.write_text(json.dumps(profile_matrix_payload, indent=2), encoding="utf-8")

    return subprocess.run(
        [
            "python3",
            str(SCRIPT),
            "--docs-envelope",
            str(docs / "03-DICOM-Conformance-Envelope.md"),
            "--docs-service-contract",
            str(docs / "12-API-Surface-and-Crate-Boundaries.md"),
            "--docs-profile-playbook",
            str(docs / "33-Productization-Profiles-and-Playbooks.md"),
            "--docs-capability-matrix",
            str(docs / "49-Runtime-Profile-Capability-Matrix.md"),
            "--profile-matrix",
            str(matrix),
            "--report",
            str(repo_root / "reports/docs/profile-docs-parity.json"),
            "--fail-on-findings",
        ],
        capture_output=True,
        text=True,
        check=False,
    )


class ProfileDocsParityGateTests(unittest.TestCase):
    def test_gate_passes_when_profile_claims_align(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            result = run_gate(
                repo_root=root,
                profile_matrix_payload={
                    "version": "x",
                    "profiles": {
                        "framework-core": {"default_profile": True, "required_for_documentation": True},
                        "backend-services": {"default_profile": True, "required_for_documentation": True},
                    },
                },
            )
            self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_gate_fails_when_profile_docs_out_of_sync(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            result = run_gate(
                repo_root=root,
                profile_matrix_payload={
                    "version": "x",
                    "profiles": {
                        "framework-core": {"default_profile": True, "required_for_documentation": True},
                        "backend-services-with-dimse": {"default_profile": True, "required_for_documentation": True},
                    },
                },
            )
            self.assertEqual(result.returncode, 1, msg=result.stdout + result.stderr)
            payload = json.loads((root / "reports/docs/profile-docs-parity.json").read_text())
            self.assertEqual(payload["status"], "FAIL")
            self.assertIn("docs49 profile columns do not match profile matrix", "\n".join(payload["findings"]))


if __name__ == "__main__":
    unittest.main()
