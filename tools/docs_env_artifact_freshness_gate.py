#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import sys

from docs_drift_lint import (
    DEFERRED_WHITELIST,
    DENYLIST_FOR_IMPLEMENTED,
    lint as docs_drift_lint,
)
from runtime_env_contract_lib import (
    extract_runtime_env_contract,
    runtime_env_contract_has_drift,
)


def _json_dump(payload: object) -> str:
    return json.dumps(payload, indent=2, sort_keys=True) + "\n"


def runtime_env_contract_payload(repo_root: pathlib.Path) -> dict[str, object]:
    contracts = extract_runtime_env_contract(repo_root)
    drift = runtime_env_contract_has_drift(contracts)
    return {
        "repo_root": str(repo_root),
        "contract_count": len(contracts),
        "contracts": contracts,
        "status": "FAIL" if drift else "PASS",
    }


def docs_env_review_payload(repo_root: pathlib.Path) -> dict[str, object]:
    contracts = extract_runtime_env_contract(repo_root)
    violations: list[dict[str, object]] = []
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
    return {
        "status": "FAIL" if violations else "PASS",
        "violations": violations,
        "contract_count": len(contracts),
    }


def docs_drift_payload(repo_root: pathlib.Path) -> dict[str, object]:
    violations, scanned, contracts = docs_drift_lint(repo_root)
    return {
        "repo_root": str(repo_root),
        "scanned_markdown_files": [p.relative_to(repo_root).as_posix() for p in scanned],
        "violation_count": len(violations),
        "violations": violations,
        "denylist_for_implemented": list(DENYLIST_FOR_IMPLEMENTED),
        "deferred_whitelist": list(DEFERRED_WHITELIST),
        "runtime_env_contracts": contracts,
    }


def evaluate_freshness(
    repo_root: pathlib.Path,
    *,
    runtime_report_path: pathlib.Path,
    docs_drift_report_path: pathlib.Path,
    docs_env_review_report_path: pathlib.Path,
) -> dict[str, object]:
    runtime_expected = _json_dump(runtime_env_contract_payload(repo_root))
    docs_drift_expected = _json_dump(docs_drift_payload(repo_root))
    docs_env_expected = _json_dump(docs_env_review_payload(repo_root))

    checks = [
        {
            "path": str(runtime_report_path.relative_to(repo_root)),
            "expected": runtime_expected,
        },
        {
            "path": str(docs_drift_report_path.relative_to(repo_root)),
            "expected": docs_drift_expected,
        },
        {
            "path": str(docs_env_review_report_path.relative_to(repo_root)),
            "expected": docs_env_expected,
        },
    ]

    violations: list[dict[str, object]] = []
    for check in checks:
        artifact_path = repo_root / check["path"]
        actual = artifact_path.read_text(encoding="utf-8") if artifact_path.exists() else ""
        expected = check["expected"]
        actual_hash = hashlib.sha256(actual.encode("utf-8")).hexdigest() if actual else None
        expected_hash = hashlib.sha256(expected.encode("utf-8")).hexdigest()
        if actual != expected:
            violations.append(
                {
                    "kind": "stale_artifact",
                    "artifact": check["path"],
                    "actual_hash": actual_hash,
                    "expected_hash": expected_hash,
                }
            )

    runtime_payload = runtime_env_contract_payload(repo_root)
    docs_env_payload = docs_env_review_payload(repo_root)
    if runtime_payload["status"] != docs_env_payload["status"]:
        violations.append(
            {
                "kind": "status_mismatch",
                "runtime_env_contract_status": runtime_payload["status"],
                "docs_env_review_status": docs_env_payload["status"],
            }
        )

    return {
        "status": "FAIL" if violations else "PASS",
        "violations": violations,
        "checked_artifacts": [check["path"] for check in checks],
        "runtime_env_contract_status": runtime_payload["status"],
        "docs_env_review_status": docs_env_payload["status"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fail when docs/runtime contract report artifacts are stale or status-inconsistent."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root")
    parser.add_argument(
        "--runtime-report",
        default="reports/docs/runtime-env-contract-check.json",
        help="Runtime env contract check artifact path",
    )
    parser.add_argument(
        "--docs-drift-report",
        default="reports/docs/drift-report.json",
        help="Docs drift report artifact path",
    )
    parser.add_argument(
        "--docs-env-review-report",
        default="reports/docs/docs-env-review-gate.json",
        help="Docs env review gate artifact path",
    )
    parser.add_argument(
        "--report",
        default="reports/docs/docs-env-artifact-freshness-gate.json",
        help="Output report path",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.repo_root).resolve()
    payload = evaluate_freshness(
        root,
        runtime_report_path=root / args.runtime_report,
        docs_drift_report_path=root / args.docs_drift_report,
        docs_env_review_report_path=root / args.docs_env_review_report,
    )
    report_path = root / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(_json_dump(payload), encoding="utf-8")

    if payload["status"] == "FAIL":
        print("Docs env artifact freshness gate: FAIL")
        print(f"Violations: {len(payload['violations'])}")
        print(f"Report: {report_path.relative_to(root)}")
        return 1

    print("Docs env artifact freshness gate: PASS")
    print(f"Report: {report_path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
