#!/usr/bin/env python3
"""
Security startup fail-closed gate.

Validates that backend services fail startup when unknown service-scoped
environment variables are present.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import subprocess
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate backend startup fail-closed behavior.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--binary-dir",
        default="target/debug",
        help="Directory containing compiled backend binaries.",
    )
    parser.add_argument(
        "--output-json",
        default="reports/security/startup-fail-closed-gate.json",
        help="Output JSON report path.",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=6.0,
        help="Per-process timeout in seconds.",
    )
    return parser.parse_args()


def run_process(
    command: list[str],
    env: dict[str, str],
    timeout_seconds: float,
) -> tuple[int, str, bool]:
    try:
        completed = subprocess.run(
            command,
            env=env,
            text=True,
            capture_output=True,
            timeout=timeout_seconds,
            check=False,
        )
        output = (completed.stdout or "") + (completed.stderr or "")
        return completed.returncode, output, False
    except subprocess.TimeoutExpired as error:
        output = (error.stdout or "") + (error.stderr or "")
        return 124, output, True


def evaluate_case(
    *,
    case_id: str,
    command: list[str],
    env_updates: dict[str, str],
    timeout_seconds: float,
) -> dict[str, Any]:
    run_env = {
        "PATH": os.environ.get("PATH", ""),
        "HOME": os.environ.get("HOME", ""),
    }
    run_env.update(env_updates)

    exit_code, output, timed_out = run_process(command, run_env, timeout_seconds)
    lowered = output.lower()
    passed = (not timed_out) and exit_code != 0 and ("env-contract violation" in lowered)
    return {
        "case_id": case_id,
        "command": command,
        "env_updates": env_updates,
        "timed_out": timed_out,
        "exit_code": exit_code,
        "passed": passed,
        "output_excerpt": output.strip()[:2000],
    }


def build_cases(binaries: dict[str, pathlib.Path]) -> list[tuple[str, list[str], dict[str, str]]]:
    return [
        (
            "web-unknown-env-fails-startup",
            [str(binaries["web"])],
            {"DICOM_WEB_UNKNOWN_SECURITY_FLAG": "1"},
        ),
        (
            "workflow-unknown-env-fails-startup",
            [str(binaries["workflow"])],
            {"DICOM_WORKFLOW_UNKNOWN_SECURITY_FLAG": "1"},
        ),
        (
            "dimse-unknown-env-fails-startup",
            [str(binaries["dimse"])],
            {"DICOM_DIMSE_UNKNOWN_SECURITY_FLAG": "1"},
        ),
        (
            "web-test-only-env-rejected-in-production",
            [str(binaries["web"])],
            {"DICOM_WEB_TEST_STORAGE_BYTES": "4096"},
        ),
        (
            "workflow-test-only-env-rejected-in-production",
            [str(binaries["workflow"])],
            {"DICOM_WORKFLOW_TEST_RATE_LIMIT": "99"},
        ),
        (
            "dimse-test-only-env-rejected-in-production",
            [str(binaries["dimse"])],
            {"DICOM_DIMSE_TEST_MAX_BYTES": "2048"},
        ),
    ]


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    binary_dir = (repo_root / args.binary_dir).resolve()
    output_json = (repo_root / args.output_json).resolve()

    binaries = {
        "web": binary_dir / "dicom-web-server",
        "workflow": binary_dir / "dicom-workflow-server",
        "dimse": binary_dir / "dicom-dimse-service",
    }
    missing = [str(path) for path in binaries.values() if not path.exists()]
    if missing:
        print("Security startup fail-closed gate: FAIL")
        print("error: missing compiled binaries:")
        for item in missing:
            print(f" - {item}")
        print("build binaries first (cargo build -p dicom-web-server -p dicom-workflow-server -p dicom-dimse-service)")
        return 1

    cases = build_cases(binaries)

    results = [
        evaluate_case(
            case_id=case_id,
            command=command,
            env_updates=env_updates,
            timeout_seconds=args.timeout_seconds,
        )
        for (case_id, command, env_updates) in cases
    ]
    failures = [row for row in results if not row["passed"]]
    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not failures else "FAIL",
        "case_count": len(results),
        "failed_case_count": len(failures),
        "cases": results,
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print("Security startup fail-closed gate: PASS" if not failures else "Security startup fail-closed gate: FAIL")
    print(f"cases: {len(results)}")
    print(f"failed: {len(failures)}")
    print(f"report: {output_json}")
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
