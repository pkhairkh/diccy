#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate public API/docs claim entries include implementation and test evidence paths."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--manifest",
        default="reports/traceability/public-api-claim-evidence-manifest.json",
        help="Manifest path.",
    )
    parser.add_argument(
        "--report",
        default="reports/traceability/public-api-claim-evidence-gate.json",
        help="Output report path.",
    )
    return parser.parse_args()


def is_test_path(path: str) -> bool:
    normalized = path.replace("\\", "/")
    return (
        "/tests/" in normalized
        or normalized.startswith("tools/tests/")
        or normalized.startswith("frontend/tests/")
        or pathlib.Path(normalized).name.startswith("test_")
    )


def validate_claim(repo_root: pathlib.Path, claim: dict[str, Any]) -> dict[str, Any]:
    claim_id = str(claim.get("claim_id", "")).strip()
    source_doc = str(claim.get("source_doc", "")).strip()
    impl_paths = claim.get("implementation_evidence_paths", [])
    test_paths = claim.get("test_evidence_paths", [])
    owner = str(claim.get("owner", "")).strip()

    violations: list[str] = []
    if not claim_id:
        violations.append("missing claim_id")
    if not source_doc:
        violations.append("missing source_doc")
    else:
        source_path = repo_root / source_doc
        if not source_path.exists():
            violations.append(f"missing source_doc path: {source_doc}")
        if not source_doc.startswith("docs/"):
            violations.append(f"source_doc must be under docs/: {source_doc}")

    if not isinstance(impl_paths, list) or not impl_paths:
        violations.append("implementation_evidence_paths must be a non-empty list")
        impl_paths = []
    if not isinstance(test_paths, list) or not test_paths:
        violations.append("test_evidence_paths must be a non-empty list")
        test_paths = []

    for path in impl_paths:
        if not isinstance(path, str) or not path:
            violations.append("implementation evidence path must be a non-empty string")
            continue
        if not (repo_root / path).exists():
            violations.append(f"missing implementation evidence path: {path}")

    for path in test_paths:
        if not isinstance(path, str) or not path:
            violations.append("test evidence path must be a non-empty string")
            continue
        if not is_test_path(path):
            violations.append(f"test evidence path is not test-like: {path}")
        if not (repo_root / path).exists():
            violations.append(f"missing test evidence path: {path}")

    return {
        "claim_id": claim_id,
        "owner": owner,
        "source_doc": source_doc,
        "implementation_evidence_paths": impl_paths,
        "test_evidence_paths": test_paths,
        "status": "PASS" if not violations else "FAIL",
        "violations": violations,
    }


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    manifest_path = (repo_root / args.manifest).resolve()
    report_path = (repo_root / args.report).resolve()

    if not manifest_path.exists():
        payload = {
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "status": "FAIL",
            "manifest": str(manifest_path.relative_to(repo_root)),
            "violations": [f"missing manifest: {manifest_path.relative_to(repo_root)}"],
            "claims": [],
        }
        write_json(report_path, payload)
        print("Public API claim evidence gate: FAIL")
        print(f"report: {report_path.relative_to(repo_root)}")
        return 1

    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    claims = payload.get("claims", [])
    if not isinstance(claims, list):
        claims = []

    claim_results = [validate_claim(repo_root, claim) for claim in claims if isinstance(claim, dict)]
    top_level_violations: list[str] = []
    if not claim_results:
        top_level_violations.append("manifest must contain at least one claim entry")

    failing_claims = [claim for claim in claim_results if claim["status"] != "PASS"]
    status = "PASS" if not top_level_violations and not failing_claims else "FAIL"
    report_payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": status,
        "manifest": str(manifest_path.relative_to(repo_root)),
        "claim_count": len(claim_results),
        "failing_claim_count": len(failing_claims),
        "violations": top_level_violations,
        "claims": claim_results,
    }
    write_json(report_path, report_payload)

    if status != "PASS":
        print("Public API claim evidence gate: FAIL")
        print(f"failing_claim_count: {len(failing_claims)}")
        print(f"report: {report_path.relative_to(repo_root)}")
        return 1

    print("Public API claim evidence gate: PASS")
    print(f"claim_count: {len(claim_results)}")
    print(f"report: {report_path.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
