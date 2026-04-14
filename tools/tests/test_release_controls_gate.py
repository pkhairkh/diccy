from __future__ import annotations

import argparse
import json
import pathlib
import sys
import tarfile
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import release_controls_gate


def _write_profile_artifact(path: pathlib.Path, binaries: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "release_id": "RC-TEST",
        "binary": binaries,
        "binaries": [{"name": name} for name in binaries],
    }
    with tempfile.TemporaryDirectory() as tmp_dir:
        staging = pathlib.Path(tmp_dir)
        (staging / "capability.manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        with tarfile.open(path, "w:gz") as archive:
            archive.add(staging / "capability.manifest.json", arcname="capability.manifest.json")


class ReleaseControlsGateTests(unittest.TestCase):
    def _seed_release_materials(self, root: pathlib.Path, release_id: str) -> None:
        (root / "reports/release").mkdir(parents=True, exist_ok=True)
        (root / "reports/traceability").mkdir(parents=True, exist_ok=True)
        (root / "reports/security").mkdir(parents=True, exist_ok=True)
        (root / "reports/performance").mkdir(parents=True, exist_ok=True)
        (root / "dist/profiles").mkdir(parents=True, exist_ok=True)

        (root / "reports/release/release-notes-template.md").write_text(
            "\n".join(
                [
                    "# Template",
                    "## Release ID",
                    "## Conformance envelope delta",
                    "## Runtime env-contract delta",
                    "## Competitive gap movement",
                    "## Security and quality gates",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-notes-{release_id}.md").write_text(
            "\n".join(
                [
                    "# Release Notes",
                    "## Release ID",
                    f"- `{release_id}`",
                    "## Conformance envelope delta",
                    "- no envelope changes",
                    "## Runtime env-contract delta",
                    "- no runtime changes",
                    "## Competitive gap movement",
                    "- unchanged, next review scheduled",
                    "## Security and quality gates",
                    "- all gates passed",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-preflight-summary-{release_id}.json").write_text(
            json.dumps(
                {
                    "release_id": release_id,
                    "overall_status": "PASS",
                    "failed_gate_count": 0,
                    "gates": [{"gate_id": "sample", "status": "PASS"}],
                }
            )
            + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-artifact-integrity-{release_id}.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/traceability/release-traceability-manifest-{release_id}.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / "dist/profiles/profiles.metadata.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/security/security-gate-decision-{release_id}.md").write_text("pass\n", encoding="utf-8")
        (root / f"reports/performance/performance-scalability-gate-decision-{release_id}.md").write_text(
            "pass\n",
            encoding="utf-8",
        )
        for profile in release_controls_gate.REQUIRED_PROFILES:
            _write_profile_artifact(root / f"dist/profiles/{profile}.{release_id}.tar.gz", ["dicom-web-server"])

    def test_run_gate_passes_with_complete_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed_release_materials(root, release_id)
            args = argparse.Namespace(
                release_id=release_id,
                repo_root=str(root),
                release_notes_template="reports/release/release-notes-template.md",
                release_notes=None,
                preflight_json=None,
                artifact_integrity_json=None,
                traceability_manifest_json=None,
                security_gate_decision=None,
                performance_gate_decision=None,
                profiles_dir="dist/profiles",
                profiles_metadata="dist/profiles/profiles.metadata.json",
                signed_bundle="dist/profiles/profiles.metadata.bundle.signed.json",
                key_env="EVIDENCE_SIGNING_KEY",
                output_json=None,
                skip_signature_verify=True,
            )
            code, payload = release_controls_gate.run_gate(args)
            self.assertEqual(code, 0)
            self.assertEqual(payload["overall_status"], "PASS")

    def test_run_gate_fails_when_preflight_has_failed_gate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed_release_materials(root, release_id)
            (root / f"reports/release/release-preflight-summary-{release_id}.json").write_text(
                json.dumps(
                    {
                        "release_id": release_id,
                        "overall_status": "FAIL",
                        "failed_gate_count": 1,
                        "gates": [{"gate_id": "sample", "status": "FAIL"}],
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            args = argparse.Namespace(
                release_id=release_id,
                repo_root=str(root),
                release_notes_template="reports/release/release-notes-template.md",
                release_notes=None,
                preflight_json=None,
                artifact_integrity_json=None,
                traceability_manifest_json=None,
                security_gate_decision=None,
                performance_gate_decision=None,
                profiles_dir="dist/profiles",
                profiles_metadata="dist/profiles/profiles.metadata.json",
                signed_bundle="dist/profiles/profiles.metadata.bundle.signed.json",
                key_env="EVIDENCE_SIGNING_KEY",
                output_json=None,
                skip_signature_verify=True,
            )
            code, payload = release_controls_gate.run_gate(args)
            self.assertEqual(code, 1)
            self.assertEqual(payload["overall_status"], "FAIL")
            check_ids = {check["check_id"] for check in payload["checks"] if check["status"] == "FAIL"}
            self.assertIn("preflight-summary-gates-pass", check_ids)


if __name__ == "__main__":
    unittest.main()
