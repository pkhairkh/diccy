#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys

from runtime_env_contract_lib import (
    extract_runtime_env_contract,
    runtime_env_contract_has_drift,
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Extract deterministic runtime env-var contracts and optionally verify docs alignment."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path")
    parser.add_argument(
        "--components",
        help="Comma-separated list of contract components to check (e.g. dicom-workflow-server,dicom-dimse-service)",
    )
    parser.add_argument(
        "--report",
        default="reports/docs/runtime-env-contract.json",
        help="Path for machine-readable report artifact",
    )
    parser.add_argument(
        "--check-docs",
        action="store_true",
        help="Fail non-zero when docs and runtime env-var contracts diverge",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.repo_root).resolve()
    report_path = root / args.report
    selected_components = (
        tuple(part.strip() for part in args.components.split(","))
        if args.components
        else None
    )

    contracts = extract_runtime_env_contract(root, components=selected_components)
    drift = runtime_env_contract_has_drift(contracts)
    payload = {
        "repo_root": str(root),
        "contract_count": len(contracts),
        "contracts": contracts,
        "status": "FAIL" if drift else "PASS",
    }

    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    summary_path = root / "reports/docs/runtime-env-contract-summary.md"
    summary_path.parent.mkdir(parents=True, exist_ok=True)
    summary_path.write_text(
        f"Contract drift status: {'FAIL' if drift else 'PASS'}\n",
        encoding="utf-8",
    )

    if args.check_docs and drift:
        print("Runtime env contract check: FAIL")
        for contract in contracts:
            component = contract["component"]
            missing = contract["missing_in_docs"]
            extra = contract["documented_not_in_code"]
            if missing:
                print(f" - {component}: missing in docs: {', '.join(missing)}")
            if extra:
                print(f" - {component}: documented but missing in code: {', '.join(extra)}")
        report_display = report_path if not report_path.is_relative_to(root) else report_path.relative_to(root)
        summary_display = summary_path if not summary_path.is_relative_to(root) else summary_path.relative_to(root)
        print(f"Report: {report_display}")
        print(f"Summary: {summary_display}")
        return 1

    print("Runtime env contract check: PASS")
    print(f"Contracts: {len(contracts)}")
    report_display = report_path if not report_path.is_relative_to(root) else report_path.relative_to(root)
    summary_display = summary_path if not summary_path.is_relative_to(root) else summary_path.relative_to(root)
    print(f"Report: {report_display}")
    print(f"Summary: {summary_display}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
