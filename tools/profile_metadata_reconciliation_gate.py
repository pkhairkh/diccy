#!/usr/bin/env python3
"""
Reconcile profile metadata across profile matrix, docs, and emitted profile artifacts.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any


PROFILE_TOKEN_RE = re.compile(r"`([a-z0-9][a-z0-9-]*)`")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Profile metadata reconciliation gate.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--profile-matrix",
        default="tools/profile_matrix.json",
        help="Path to profile matrix JSON.",
    )
    parser.add_argument(
        "--docs-profile-playbook",
        default="docs/33-Productization-Profiles-and-Playbooks.md",
        help="Path to docs profile playbook markdown.",
    )
    parser.add_argument(
        "--artifact-metadata",
        default="dist/profiles/profiles.metadata.json",
        help="Path to emitted profile metadata JSON.",
    )
    parser.add_argument(
        "--require-artifacts",
        action="store_true",
        help="Fail when artifact metadata is missing.",
    )
    parser.add_argument(
        "--output-json",
        default="reports/release/profile-metadata-reconciliation.json",
        help="Output JSON report path.",
    )
    return parser.parse_args()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must be a JSON object")
    return payload


def parse_docs_profiles(path: pathlib.Path) -> set[str]:
    text = path.read_text(encoding="utf-8")
    return {match.group(1) for match in PROFILE_TOKEN_RE.finditer(text)}


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    matrix_path = (repo_root / args.profile_matrix).resolve()
    docs_path = (repo_root / args.docs_profile_playbook).resolve()
    artifact_path = (repo_root / args.artifact_metadata).resolve()
    output_json = (repo_root / args.output_json).resolve()

    findings: list[str] = []
    matrix_payload = load_json(matrix_path)
    profiles_obj = matrix_payload.get("profiles")
    if not isinstance(profiles_obj, dict):
        findings.append("profile matrix missing 'profiles' object")
        matrix_profiles: set[str] = set()
    else:
        matrix_profiles = set(profiles_obj.keys())

    docs_profiles = parse_docs_profiles(docs_path)
    missing_in_docs = sorted(profile for profile in matrix_profiles if profile not in docs_profiles)
    if missing_in_docs:
        findings.append("profiles missing in docs/33: " + ", ".join(missing_in_docs))

    artifact_profiles: set[str] = set()
    if artifact_path.exists():
        artifact_payload = load_json(artifact_path)
        artifacts = artifact_payload.get("artifacts", [])
        if not isinstance(artifacts, list):
            findings.append("artifact metadata 'artifacts' must be a list")
        else:
            for entry in artifacts:
                if not isinstance(entry, dict):
                    findings.append("artifact metadata entry must be an object")
                    continue
                profile = entry.get("profile")
                if not isinstance(profile, str) or not profile:
                    findings.append("artifact metadata entry missing profile")
                    continue
                artifact_profiles.add(profile)
        unknown_artifact_profiles = sorted(
            profile for profile in artifact_profiles if profile not in matrix_profiles
        )
        if unknown_artifact_profiles:
            findings.append(
                "artifact metadata contains unknown profiles: "
                + ", ".join(unknown_artifact_profiles)
            )
    elif args.require_artifacts:
        findings.append(f"missing artifact metadata: {artifact_path}")

    artifact_missing_in_docs = sorted(
        profile for profile in artifact_profiles if profile not in docs_profiles
    )
    if artifact_missing_in_docs:
        findings.append(
            "artifact profiles missing in docs/33: " + ", ".join(artifact_missing_in_docs)
        )

    output_json.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not findings else "FAIL",
        "matrix_profile_count": len(matrix_profiles),
        "docs_profile_token_count": len(docs_profiles),
        "artifact_profile_count": len(artifact_profiles),
        "findings": findings,
    }
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if findings:
        print("Profile metadata reconciliation gate: FAIL")
        for item in findings:
            print(f" - {item}")
        print(f"report: {output_json}")
        return 1

    print("Profile metadata reconciliation gate: PASS")
    print(f"report: {output_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
