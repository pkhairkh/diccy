#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys

from runtime_env_contract_lib import extract_runtime_env_contract


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail closed when docs/12 env-contract content diverges from runtime parser declarations."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root")
    parser.add_argument(
        "--report",
        default="reports/docs/docs-env-review-gate.json",
        help="Output report path",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.repo_root).resolve()
    report_path = root / args.report
    contracts = extract_runtime_env_contract(root)
    violations = []
    for contract in contracts:
        if contract["missing_in_docs"] or contract["documented_not_in_code"]:
            violations.append(
                {
                    "component": contract["component"],
                    "missing_in_docs": contract["missing_in_docs"],
                    "documented_not_in_code": contract["documented_not_in_code"],
                    "docs_file": contract["docs_file"],
                }
            )

    payload = {
        "status": "FAIL" if violations else "PASS",
        "violations": violations,
        "contract_count": len(contracts),
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if violations:
        print("Docs env review gate: FAIL")
        print(f"Violations: {len(violations)}")
        print(f"Report: {report_path.relative_to(root)}")
        return 1
    print("Docs env review gate: PASS")
    print(f"Report: {report_path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
