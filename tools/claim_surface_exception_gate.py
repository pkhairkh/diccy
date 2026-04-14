#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
from typing import Any


ALLOWED_EXCEPTION_PATH = "docs/15-Regulatory-and-Standards-Mapping.md"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate claim-surface exception registry is fail-closed outside docs/15."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--exceptions",
        default="reports/security/claim-surface-exceptions.json",
        help="Exception registry JSON path.",
    )
    parser.add_argument(
        "--report",
        default="reports/security/claim-surface-exception-gate.json",
        help="Output report path.",
    )
    return parser.parse_args()


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    exceptions_path = (repo_root / args.exceptions).resolve()
    report_path = (repo_root / args.report).resolve()

    violations: list[str] = []
    exception_rows: list[dict[str, Any]] = []
    if not exceptions_path.exists():
        violations.append(f"missing exception registry: {exceptions_path.relative_to(repo_root)}")
    else:
        payload = json.loads(exceptions_path.read_text(encoding="utf-8"))
        exceptions = payload.get("exceptions", [])
        if not isinstance(exceptions, list):
            violations.append("registry key 'exceptions' must be a list")
            exceptions = []
        for row in exceptions:
            if not isinstance(row, dict):
                violations.append("exception entry must be an object")
                continue
            path = str(row.get("path", "")).strip()
            reason = str(row.get("reason", "")).strip()
            approved_by = str(row.get("approved_by", "")).strip()
            if not path:
                violations.append("exception entry missing path")
                continue
            if path != ALLOWED_EXCEPTION_PATH:
                violations.append(f"exception path outside allowlist: {path}")
            exception_rows.append(
                {
                    "path": path,
                    "reason": reason,
                    "approved_by": approved_by,
                    "status": "PASS" if path == ALLOWED_EXCEPTION_PATH else "FAIL",
                }
            )

    report_payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not violations else "FAIL",
        "allowed_exception_path": ALLOWED_EXCEPTION_PATH,
        "exception_count": len(exception_rows),
        "violations": violations,
        "exceptions": exception_rows,
    }
    write_json(report_path, report_payload)

    if violations:
        print("Claim-surface exception gate: FAIL")
        print(f"violations: {len(violations)}")
        print(f"report: {report_path.relative_to(repo_root)}")
        return 1

    print("Claim-surface exception gate: PASS")
    print(f"exception_count: {len(exception_rows)}")
    print(f"report: {report_path.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
