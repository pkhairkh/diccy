#!/usr/bin/env python3
"""
Validate DIMSE profile release controls for packaged profile artifacts.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
import tarfile
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate DIMSE profile release controls.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--profiles-dir",
        default="dist/profiles",
        help="Directory containing packaged profile artifacts.",
    )
    parser.add_argument(
        "--profiles-metadata",
        default="dist/profiles/profiles.metadata.json",
        help="Profile metadata JSON path.",
    )
    parser.add_argument(
        "--release-notes",
        help="Release notes path. Default: reports/release/release-notes-<release-id>.md",
    )
    parser.add_argument(
        "--output-json",
        help="Output report path. Default: reports/release/dimse-profile-release-controls-<release-id>.json",
    )
    return parser.parse_args()


def add_result(results: list[dict[str, Any]], check_id: str, passed: bool, details: str) -> None:
    results.append(
        {
            "check_id": check_id,
            "status": "PASS" if passed else "FAIL",
            "details": details,
        }
    )


def read_capability_manifest(artifact_path: pathlib.Path) -> dict[str, Any]:
    with tarfile.open(artifact_path, "r:gz") as archive:
        names = set(archive.getnames())
        if "capability.manifest.json" not in names:
            raise ValueError(f"{artifact_path} missing capability.manifest.json")
        handle = archive.extractfile("capability.manifest.json")
        if handle is None:
            raise ValueError(f"{artifact_path} cannot extract capability.manifest.json")
        payload = json.loads(handle.read().decode("utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{artifact_path} capability.manifest.json is not an object")
    return payload


def manifest_binary_names(payload: dict[str, Any]) -> set[str]:
    names: set[str] = set()
    direct = payload.get("binary", [])
    if isinstance(direct, list):
        for item in direct:
            if isinstance(item, str):
                names.add(item)
    expanded = payload.get("binaries", [])
    if isinstance(expanded, list):
        for item in expanded:
            if isinstance(item, dict) and isinstance(item.get("name"), str):
                names.add(item["name"])
    return names


def run_gate(args: argparse.Namespace) -> tuple[int, dict[str, Any]]:
    repo_root = pathlib.Path(args.repo_root).resolve()
    profiles_dir = (repo_root / args.profiles_dir).resolve()
    release_id = args.release_id
    profiles_metadata = (repo_root / args.profiles_metadata).resolve()
    release_notes = (
        pathlib.Path(args.release_notes).resolve()
        if args.release_notes
        else (repo_root / f"reports/release/release-notes-{release_id}.md").resolve()
    )
    dimse_artifact = profiles_dir / f"backend-services-with-dimse.{release_id}.tar.gz"
    baseline_artifact = profiles_dir / f"backend-services.{release_id}.tar.gz"

    results: list[dict[str, Any]] = []

    add_result(
        results,
        "profiles-metadata-exists",
        profiles_metadata.exists(),
        f"path={profiles_metadata}",
    )
    if profiles_metadata.exists():
        metadata = json.loads(profiles_metadata.read_text(encoding="utf-8"))
        metadata_release_id = metadata.get("release_id") if isinstance(metadata, dict) else None
        add_result(
            results,
            "profiles-metadata-release-id",
            metadata_release_id == release_id,
            f"metadata release_id={metadata_release_id}",
        )

    add_result(
        results,
        "backend-services-artifact-exists",
        baseline_artifact.exists(),
        f"path={baseline_artifact}",
    )
    add_result(
        results,
        "backend-services-with-dimse-artifact-exists",
        dimse_artifact.exists(),
        f"path={dimse_artifact}",
    )

    if baseline_artifact.exists():
        try:
            baseline_manifest = read_capability_manifest(baseline_artifact)
            baseline_names = manifest_binary_names(baseline_manifest)
            add_result(
                results,
                "baseline-profile-excludes-dimse-binary",
                "dicom-dimse-service" not in baseline_names,
                f"baseline binaries={sorted(baseline_names)}",
            )
        except Exception as error:  # pragma: no cover
            add_result(results, "baseline-profile-excludes-dimse-binary", False, str(error))

    if dimse_artifact.exists():
        try:
            dimse_manifest = read_capability_manifest(dimse_artifact)
            dimse_names = manifest_binary_names(dimse_manifest)
            add_result(
                results,
                "dimse-profile-includes-dimse-binary",
                "dicom-dimse-service" in dimse_names,
                f"dimse binaries={sorted(dimse_names)}",
            )
        except Exception as error:  # pragma: no cover
            add_result(results, "dimse-profile-includes-dimse-binary", False, str(error))

    add_result(
        results,
        "release-notes-exists",
        release_notes.exists(),
        f"path={release_notes}",
    )
    if release_notes.exists():
        text = release_notes.read_text(encoding="utf-8", errors="ignore")
        add_result(
            results,
            "release-notes-reference-dimse-profile",
            ("backend-services-with-dimse" in text) or ("dicom-dimse-service" in text),
            "release notes should reference DIMSE profile posture",
        )

    failures = [result for result in results if result["status"] != "PASS"]
    payload: dict[str, Any] = {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "overall_status": "PASS" if not failures else "FAIL",
        "failed_check_count": len(failures),
        "checks": results,
    }
    return (0 if not failures else 1, payload)


def main() -> int:
    args = parse_args()
    code, payload = run_gate(args)
    repo_root = pathlib.Path(args.repo_root).resolve()
    output_path = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (repo_root / f"reports/release/dimse-profile-release-controls-{args.release_id}.json").resolve()
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print("DIMSE profile release controls: PASS" if code == 0 else "DIMSE profile release controls: FAIL")
    print(f"release_id: {payload['release_id']}")
    print(f"failed_checks: {payload['failed_check_count']}")
    print(f"report: {output_path}")
    return code


if __name__ == "__main__":
    sys.exit(main())
