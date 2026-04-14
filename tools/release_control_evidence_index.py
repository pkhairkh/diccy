#!/usr/bin/env python3
"""
Generate a release-control evidence index with owner + status attribution.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
from typing import Any


REQUIRED_PROFILES = (
    "framework-core",
    "workstation",
    "backend-services",
    "backend-services-with-dimse",
)

REQUIRED_ARTIFACTS = (
    ("release-notes", "reports/release/release-notes-{release_id}.md", "Release Engineering"),
    ("preflight-summary", "reports/release/release-preflight-summary-{release_id}.json", "Release Engineering"),
    ("artifact-integrity", "reports/release/release-artifact-integrity-{release_id}.json", "Build and Packaging"),
    ("release-controls-gate", "reports/release/release-controls-gate-{release_id}.json", "Quality Engineering"),
    ("traceability-manifest", "reports/traceability/release-traceability-manifest-{release_id}.json", "Quality Engineering"),
    ("security-gate-decision", "reports/security/security-gate-decision-{release_id}.md", "Security"),
    ("performance-gate-decision", "reports/performance/performance-scalability-gate-decision-{release_id}.md", "Performance"),
    ("profiles-metadata", "dist/profiles/profiles.metadata.json", "Build and Packaging"),
    ("signed-evidence-bundle", "dist/profiles/profiles.metadata.bundle.signed.json", "Security"),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build release-control evidence index.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--output-json",
        help="Output JSON path (default: reports/release/release-control-evidence-index-<release-id>.json).",
    )
    parser.add_argument(
        "--output-md",
        help="Output Markdown path (default: reports/release/release-control-evidence-index-<release-id>.md).",
    )
    return parser.parse_args()


def evaluate_json_check(path: pathlib.Path, artifact_id: str, release_id: str) -> tuple[str, str]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return ("FAIL", f"invalid JSON: {exc}")
    if not isinstance(payload, dict):
        return ("FAIL", "JSON payload must be an object")

    observed_release = payload.get("release_id")
    if isinstance(observed_release, str) and observed_release != release_id:
        return ("FAIL", f"release_id mismatch: expected={release_id} observed={observed_release}")

    if artifact_id == "preflight-summary":
        failed_gate_count = int(payload.get("failed_gate_count", -1))
        overall_status = str(payload.get("overall_status", ""))
        if failed_gate_count != 0 or overall_status != "PASS":
            return ("FAIL", f"preflight failed_gate_count={failed_gate_count} overall_status={overall_status}")
    if artifact_id == "release-controls-gate":
        failed_checks = int(payload.get("failed_check_count", -1))
        overall_status = str(payload.get("overall_status", ""))
        if failed_checks != 0 or overall_status != "PASS":
            return ("FAIL", f"release-controls failed_check_count={failed_checks} overall_status={overall_status}")
    return ("PASS", "artifact present and validated")


def evaluate_artifact(
    repo_root: pathlib.Path,
    artifact_id: str,
    rel_path_template: str,
    owner: str,
    release_id: str,
) -> dict[str, Any]:
    rel_path = rel_path_template.format(release_id=release_id)
    artifact_path = repo_root / rel_path
    if not artifact_path.exists():
        return {
            "artifact_id": artifact_id,
            "owner": owner,
            "path": rel_path,
            "status": "FAIL",
            "details": "artifact missing",
        }
    if artifact_path.suffix == ".json":
        status, details = evaluate_json_check(artifact_path, artifact_id, release_id)
    else:
        status, details = ("PASS", "artifact present")
    return {
        "artifact_id": artifact_id,
        "owner": owner,
        "path": rel_path,
        "status": status,
        "details": details,
    }


def build_index(repo_root: pathlib.Path, release_id: str) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    for artifact_id, rel_path_template, owner in REQUIRED_ARTIFACTS:
        checks.append(
            evaluate_artifact(
                repo_root=repo_root,
                artifact_id=artifact_id,
                rel_path_template=rel_path_template,
                owner=owner,
                release_id=release_id,
            )
        )

    for profile in REQUIRED_PROFILES:
        profile_path = f"dist/profiles/{profile}.{release_id}.tar.gz"
        checks.append(
            evaluate_artifact(
                repo_root=repo_root,
                artifact_id=f"profile-artifact:{profile}",
                rel_path_template=profile_path,
                owner="Build and Packaging",
                release_id=release_id,
            )
        )

    failures = [item for item in checks if item["status"] != "PASS"]
    return {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "overall_status": "PASS" if not failures else "FAIL",
        "failed_check_count": len(failures),
        "artifact_count": len(checks),
        "checks": checks,
    }


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Release Control Evidence Index",
        "",
        f"Release ID: `{payload['release_id']}`",
        f"Generated at (UTC): `{payload['generated_at_utc']}`",
        f"Overall status: **{payload['overall_status']}**",
        "",
        "| Artifact ID | Owner | Path | Status | Details |",
        "|---|---|---|---|---|",
    ]
    for check in payload["checks"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    check["artifact_id"],
                    check["owner"],
                    f"`{check['path']}`",
                    check["status"],
                    check["details"],
                ]
            )
            + " |"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    payload = build_index(repo_root=repo_root, release_id=args.release_id)

    output_json = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (repo_root / f"reports/release/release-control-evidence-index-{args.release_id}.json")
    )
    output_md = (
        pathlib.Path(args.output_md).resolve()
        if args.output_md
        else (repo_root / f"reports/release/release-control-evidence-index-{args.release_id}.md")
    )
    write_json(output_json, payload)
    write_markdown(output_md, payload)

    print("Release control evidence index: PASS" if payload["overall_status"] == "PASS" else "Release control evidence index: FAIL")
    print(f"release_id: {args.release_id}")
    print(f"failed_checks: {payload['failed_check_count']}")
    print(f"json_report: {output_json}")
    print(f"markdown_report: {output_md}")
    return 0 if payload["overall_status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
