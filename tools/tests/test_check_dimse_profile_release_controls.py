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

import check_dimse_profile_release_controls


def _write_profile_artifact(path: pathlib.Path, release_id: str, binaries: list[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    manifest = {
        "release_id": release_id,
        "binary": binaries,
        "binaries": [{"name": name} for name in binaries],
    }
    with tempfile.TemporaryDirectory() as tmp_dir:
        staging = pathlib.Path(tmp_dir)
        (staging / "capability.manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
        with tarfile.open(path, "w:gz") as archive:
            archive.add(staging / "capability.manifest.json", arcname="capability.manifest.json")


class DimseProfileReleaseControlsTests(unittest.TestCase):
    def _seed(self, root: pathlib.Path, release_id: str, include_dimse_binary: bool = True) -> None:
        (root / "reports/release").mkdir(parents=True, exist_ok=True)
        (root / "dist/profiles").mkdir(parents=True, exist_ok=True)

        baseline_binaries = ["dicom-web-server", "dicom-workflow-server"]
        dimse_binaries = ["dicom-web-server", "dicom-workflow-server"]
        if include_dimse_binary:
            dimse_binaries.append("dicom-dimse-service")

        _write_profile_artifact(
            root / f"dist/profiles/backend-services.{release_id}.tar.gz",
            release_id,
            baseline_binaries,
        )
        _write_profile_artifact(
            root / f"dist/profiles/backend-services-with-dimse.{release_id}.tar.gz",
            release_id,
            dimse_binaries,
        )

        (root / "dist/profiles/profiles.metadata.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-notes-{release_id}.md").write_text(
            "DIMSE rollout includes backend-services-with-dimse and dicom-dimse-service\n",
            encoding="utf-8",
        )

    def test_run_gate_passes_when_dimse_controls_are_satisfied(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed(root, release_id, include_dimse_binary=True)
            args = argparse.Namespace(
                release_id=release_id,
                repo_root=str(root),
                profiles_dir="dist/profiles",
                profiles_metadata="dist/profiles/profiles.metadata.json",
                release_notes=None,
                output_json=None,
            )
            code, payload = check_dimse_profile_release_controls.run_gate(args)
            self.assertEqual(code, 0)
            self.assertEqual(payload["overall_status"], "PASS")

    def test_run_gate_fails_when_dimse_binary_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed(root, release_id, include_dimse_binary=False)
            args = argparse.Namespace(
                release_id=release_id,
                repo_root=str(root),
                profiles_dir="dist/profiles",
                profiles_metadata="dist/profiles/profiles.metadata.json",
                release_notes=None,
                output_json=None,
            )
            code, payload = check_dimse_profile_release_controls.run_gate(args)
            self.assertEqual(code, 1)
            self.assertEqual(payload["overall_status"], "FAIL")
            failing = {entry["check_id"] for entry in payload["checks"] if entry["status"] == "FAIL"}
            self.assertIn("dimse-profile-includes-dimse-binary", failing)


if __name__ == "__main__":
    unittest.main()
