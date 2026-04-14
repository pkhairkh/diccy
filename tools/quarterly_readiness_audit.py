#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import subprocess
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Quarterly readiness audit for runtime contract parity, profile packaging posture, and deployment posture."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument("--release-id", required=True, help="Audit identifier.")
    parser.add_argument(
        "--output-json",
        help="Output JSON report path (default: reports/audit/quarterly-readiness-audit-<release-id>.json).",
    )
    parser.add_argument(
        "--output-md",
        help="Output markdown report path (default: reports/audit/quarterly-readiness-audit-<release-id>.md).",
    )
    return parser.parse_args()


def run_command(cmd: list[str], repo_root: pathlib.Path) -> dict[str, Any]:
    completed = subprocess.run(
        cmd,
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    return {
        "command": cmd,
        "status": "PASS" if completed.returncode == 0 else "FAIL",
        "exit_code": completed.returncode,
        "stdout": completed.stdout.strip(),
        "stderr": completed.stderr.strip(),
    }


def build_audit(repo_root: pathlib.Path, release_id: str) -> dict[str, Any]:
    runtime_report = f"reports/docs/runtime-env-contract-{release_id}.json"
    docs_review_report = f"reports/docs/docs-env-review-gate-{release_id}.json"
    profile_report = f"reports/release/profile-metadata-reconciliation-{release_id}.json"

    commands = [
        [
            "python3",
            "tools/runtime_env_contract.py",
            "--repo-root",
            str(repo_root),
            "--check-docs",
            "--report",
            runtime_report,
        ],
        [
            "python3",
            "tools/docs_env_review_gate.py",
            "--repo-root",
            str(repo_root),
            "--report",
            docs_review_report,
        ],
        [
            "python3",
            "tools/profile_metadata_reconciliation_gate.py",
            "--repo-root",
            str(repo_root),
            "--output-json",
            profile_report,
        ],
    ]
    command_results = [run_command(command, repo_root) for command in commands]

    required_paths = [
        "docs/12-API-Surface-and-Crate-Boundaries.md",
        "docs/33-Productization-Profiles-and-Playbooks.md",
        "docs/63-Environment-Promotion-Checklist.md",
        "reports/docs/startup-contract-snapshot-backend-services.md",
        "reports/docs/startup-contract-snapshot-backend-services-with-dimse.md",
        "tools/package_profiles.sh",
    ]
    file_checks: list[dict[str, Any]] = []
    for rel_path in required_paths:
        path = repo_root / rel_path
        file_checks.append(
            {
                "path": rel_path,
                "status": "PASS" if path.exists() else "FAIL",
            }
        )

    failing_commands = [item for item in command_results if item["status"] != "PASS"]
    missing_paths = [item for item in file_checks if item["status"] != "PASS"]
    return {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "overall_status": "PASS" if not failing_commands and not missing_paths else "FAIL",
        "failed_command_count": len(failing_commands),
        "missing_path_count": len(missing_paths),
        "commands": command_results,
        "required_path_checks": file_checks,
    }


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Quarterly Readiness Audit",
        "",
        f"Release ID: `{payload['release_id']}`",
        f"Generated at (UTC): `{payload['generated_at_utc']}`",
        f"Overall status: **{payload['overall_status']}**",
        "",
        "## Command checks",
        "",
        "| Command | Status | Exit |",
        "|---|---|---|",
    ]
    for result in payload["commands"]:
        command_string = " ".join(result["command"])
        lines.append(f"| `{command_string}` | {result['status']} | {result['exit_code']} |")
    lines.extend(
        [
            "",
            "## Required path checks",
            "",
            "| Path | Status |",
            "|---|---|",
        ]
    )
    for check in payload["required_path_checks"]:
        lines.append(f"| `{check['path']}` | {check['status']} |")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    payload = build_audit(repo_root=repo_root, release_id=args.release_id)

    output_json = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (repo_root / f"reports/audit/quarterly-readiness-audit-{args.release_id}.json")
    )
    output_md = (
        pathlib.Path(args.output_md).resolve()
        if args.output_md
        else (repo_root / f"reports/audit/quarterly-readiness-audit-{args.release_id}.md")
    )
    write_json(output_json, payload)
    write_markdown(output_md, payload)

    print("Quarterly readiness audit: PASS" if payload["overall_status"] == "PASS" else "Quarterly readiness audit: FAIL")
    print(f"release_id: {payload['release_id']}")
    print(f"failed_command_count: {payload['failed_command_count']}")
    print(f"missing_path_count: {payload['missing_path_count']}")
    print(f"report_json: {output_json}")
    print(f"report_md: {output_md}")
    return 0 if payload["overall_status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
