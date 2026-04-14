from __future__ import annotations

import os
import json
import pathlib
import stat
import subprocess
import tarfile
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
PACKAGE_PROFILE_SCRIPT = TOOLS_DIR / "package_profiles.sh"

PROFILE_BINARIES = (
    "dicom-visualizer",
    "viewer-wasm",
    "viewer-wgpu",
    "dicom-web-server",
    "dicom-workflow-server",
    "dicom-dimse-service",
)


def run_package_profiles(
    *,
    dist_dir: pathlib.Path,
    release_id: str,
    extra_args: list[str],
    binary_dir: pathlib.Path,
    validate: bool = True,
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    command = [
        "bash",
        str(PACKAGE_PROFILE_SCRIPT),
        str(dist_dir),
        "--release-id",
        release_id,
        "--no-sign",
    ]
    if validate:
        command.append("--validate")
    command.extend(extra_args)
    env = os.environ.copy()
    env.pop("EVIDENCE_SIGNING_KEY", None)
    if extra_env is not None:
        env.update(extra_env)
    env["PACKAGE_RELEASE_DIR"] = str(binary_dir)
    return subprocess.run(
        command,
        cwd=str(TOOLS_DIR),
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def create_profile_binary(binary_path: pathlib.Path) -> None:
    binary_path.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
    mode = binary_path.stat().st_mode
    binary_path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def seed_profile_binaries(*, workspace: pathlib.Path) -> list[pathlib.Path]:
    binary_root = workspace / "target" / "release"
    binary_root.mkdir(parents=True, exist_ok=True)

    created_binaries: list[pathlib.Path] = []
    for binary in PROFILE_BINARIES:
        binary_path = binary_root / binary
        if binary_path.exists():
            if not os.access(binary_path, os.X_OK):
                raise AssertionError(f"{binary_path} is not executable")
            continue

        create_profile_binary(binary_path)
        created_binaries.append(binary_path)

    return created_binaries


def tar_names(path: pathlib.Path) -> set[str]:
    with tarfile.open(path, "r:gz") as tar:
        return set(tar.getnames())


class PackageProfilesSmokeTests(unittest.TestCase):
    def test_dimse_optional_profile_smoke_test(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            workspace = pathlib.Path(tmp_dir)
            dist_dir = workspace / "dist" / "profiles"
            release_id = "RC-PACKAGE-SMOKE"

            seeded_binaries = seed_profile_binaries(workspace=workspace)
            self.addCleanup(lambda: [path.unlink() for path in seeded_binaries if path.exists()])

            default_result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=[],
                binary_dir=workspace / "target" / "release",
            )
            self.assertEqual(default_result.returncode, 0, msg=default_result.stderr)

            baseline_artifact = dist_dir / f"backend-services.{release_id}.tar.gz"
            dimse_artifact = dist_dir / f"backend-services-with-dimse.{release_id}.tar.gz"
            self.assertTrue(baseline_artifact.is_file())
            self.assertFalse(
                "bin/dicom-dimse-service" in tar_names(baseline_artifact),
                "DIMSE service must be absent from baseline backend-services profile",
            )

            self.assertTrue((dist_dir / f"sbom.framework-core.{release_id}.json").is_file())
            self.assertTrue((dist_dir / f"sbom.workstation.{release_id}.json").is_file())
            self.assertTrue((dist_dir / f"sbom.backend-services.{release_id}.json").is_file())

            include_result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=["--include-dimse"],
                binary_dir=workspace / "target" / "release",
            )
            self.assertEqual(include_result.returncode, 0, msg=include_result.stderr)
            self.assertTrue(dimse_artifact.is_file())
            names = tar_names(dimse_artifact)
            self.assertIn("bin/dicom-dimse-service", names)
            self.assertTrue((dist_dir / f"sbom.backend-services-with-dimse.{release_id}.json").is_file())

    def test_unknown_profile_combinations_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            workspace = pathlib.Path(tmp_dir)
            dist_dir = workspace / "dist" / "profiles"
            release_id = "RC-PACKAGE-SMOKE"

            seeded_binaries = seed_profile_binaries(workspace=workspace)
            self.addCleanup(lambda: [path.unlink() for path in seeded_binaries if path.exists()])

            result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=["--profiles", "backend-services-with-dimse"],
                binary_dir=workspace / "target" / "release",
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("unknown profile combination", result.stdout + result.stderr)

    def test_require_dimse_guardrail_for_enterprise_profiles(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            workspace = pathlib.Path(tmp_dir)
            dist_dir = workspace / "dist" / "profiles"
            release_id = "RC-PACKAGE-SMOKE"

            seeded_binaries = seed_profile_binaries(workspace=workspace)
            self.assertGreater(len(seeded_binaries), 0)
            self.addCleanup(lambda: [path.unlink() for path in seeded_binaries if path.exists()])

            omit_result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=["--require-dimse", "--profiles", "backend-services"],
                binary_dir=workspace / "target" / "release",
            )
            self.assertEqual(omit_result.returncode, 1, msg=omit_result.stderr)
            self.assertIn("did not include DIMSE", omit_result.stdout + omit_result.stderr)

            include_result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=["--require-dimse", "--profiles", "backend-services,backend-services-with-dimse"],
                binary_dir=workspace / "target" / "release",
            )
            self.assertEqual(include_result.returncode, 0, msg=include_result.stderr)

    def test_profile_matrix_is_authoritative_for_defaults(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            workspace = pathlib.Path(tmp_dir)
            dist_dir = workspace / "dist" / "profiles"
            release_id = "RC-PACKAGE-MATRIX"

            seeded_binaries = seed_profile_binaries(workspace=workspace)
            self.assertGreater(len(seeded_binaries), 0)
            self.addCleanup(lambda: [path.unlink() for path in seeded_binaries if path.exists()])

            matrix = {
                "version": "x-test",
                "profiles": {
                    "legacy-workstation-sim": {
                        "default_profile": True,
                        "requires": [],
                        "required_for_documentation": False,
                        "binaries": ["dicom-visualizer"],
                        "features": ["core"],
                        "image_suffix": "legacy-workstation-sim",
                        "build_command": "cargo build -p rdvf",
                    }
                },
            }
            matrix_path = workspace / "profile_matrix.json"
            matrix_path.write_text(json.dumps(matrix, indent=2), encoding="utf-8")

            result = run_package_profiles(
                dist_dir=dist_dir,
                release_id=release_id,
                extra_args=[],
                binary_dir=workspace / "target" / "release",
                validate=False,
                extra_env={"PROFILE_MATRIX_FILE": str(matrix_path)},
            )
            self.assertEqual(result.returncode, 0, msg=result.stderr)

            packaged_artifact = dist_dir / f"legacy-workstation-sim.{release_id}.tar.gz"
            self.assertTrue(packaged_artifact.is_file(), msg="matrix-defined profile was not packaged")
