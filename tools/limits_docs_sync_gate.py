#!/usr/bin/env python3
"""
Fail closed when limit-related code changes do not include docs/09 and docs/14 updates.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import subprocess
import sys
from typing import Any

LIMIT_CHANGE_RE = re.compile(
    r"\b(max_[a-z0-9_]+|numericbounds|limitexceeded|_MAX_|_RATE_LIMIT|UPLOAD_CAP|QUOTA|ttl_ms)\b",
    re.IGNORECASE,
)
DOC_09 = "docs/09-Security-Threat-Model.md"
DOC_14 = "docs/14-Release-and-Versioning.md"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Require docs/09 and docs/14 updates for limit-related code changes.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument("--base-ref", required=True, help="Base git ref/SHA.")
    parser.add_argument("--head-ref", required=True, help="Head git ref/SHA.")
    parser.add_argument(
        "--output-json",
        default="reports/security/limits-docs-sync-gate.json",
        help="Output report path.",
    )
    return parser.parse_args()


def run_git(repo_root: pathlib.Path, args: list[str]) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError((completed.stderr or completed.stdout).strip() or "git command failed")
    return completed.stdout


def extract_changed_files(repo_root: pathlib.Path, base_ref: str, head_ref: str) -> list[str]:
    output = run_git(repo_root, ["diff", "--name-only", base_ref, head_ref])
    return [line.strip() for line in output.splitlines() if line.strip()]


def extract_limit_touched_files(repo_root: pathlib.Path, base_ref: str, head_ref: str) -> list[str]:
    output = run_git(repo_root, ["diff", "--unified=0", base_ref, head_ref])
    current_path: str | None = None
    touched: set[str] = set()
    for raw_line in output.splitlines():
        if raw_line.startswith("diff --git "):
            parts = raw_line.split()
            current_path = parts[2][2:] if len(parts) >= 3 and parts[2].startswith("a/") else None
            continue
        if current_path is None:
            continue
        if raw_line.startswith(("+++", "---", "@@")):
            continue
        if not raw_line.startswith(("+", "-")):
            continue
        if LIMIT_CHANGE_RE.search(raw_line[1:]):
            touched.add(current_path)
    return sorted(touched)


def enforce_docs_sync(changed_files: list[str], limit_touched_files: list[str]) -> tuple[bool, str]:
    if not limit_touched_files:
        return True, "no limit-related code changes detected"
    changed = set(changed_files)
    has_09 = DOC_09 in changed
    has_14 = DOC_14 in changed
    if has_09 and has_14:
        return True, "limit-related code changes include docs/09 and docs/14 updates"
    missing = []
    if not has_09:
        missing.append(DOC_09)
    if not has_14:
        missing.append(DOC_14)
    return False, "missing required docs updates: " + ", ".join(missing)


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    output_json = (repo_root / args.output_json).resolve()

    try:
        changed_files = extract_changed_files(repo_root, args.base_ref, args.head_ref)
        limit_touched_files = extract_limit_touched_files(repo_root, args.base_ref, args.head_ref)
    except Exception as error:  # pragma: no cover
        print("Limits docs sync gate: FAIL")
        print(f"error: {error}")
        return 1

    passed, detail = enforce_docs_sync(changed_files, limit_touched_files)
    payload: dict[str, Any] = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "base_ref": args.base_ref,
        "head_ref": args.head_ref,
        "status": "PASS" if passed else "FAIL",
        "detail": detail,
        "changed_files": changed_files,
        "limit_touched_files": limit_touched_files,
        "required_docs": [DOC_09, DOC_14],
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print("Limits docs sync gate: PASS" if passed else "Limits docs sync gate: FAIL")
    print(f"detail: {detail}")
    print(f"report: {output_json}")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
