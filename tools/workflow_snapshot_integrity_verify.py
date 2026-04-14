#!/usr/bin/env python3
"""
Verify workflow snapshot export/import payload integrity (checksum + tenant isolation).
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify workflow snapshot export/import integrity.")
    parser.add_argument("--repo-root", default=".", help="Repository root.")
    parser.add_argument("--export-json", required=True, help="Export API payload JSON path.")
    parser.add_argument("--import-json", required=True, help="Import API payload JSON path.")
    parser.add_argument(
        "--output-json",
        default="reports/release/workflow-snapshot-integrity-verify.json",
        help="Output report path.",
    )
    return parser.parse_args()


def canonical_snapshot_text(snapshot: Any) -> str:
    if isinstance(snapshot, str):
        return snapshot
    return json.dumps(snapshot, sort_keys=True, separators=(",", ":"))


def compute_checksum(snapshot: Any) -> str:
    return hashlib.sha256(canonical_snapshot_text(snapshot).encode("utf-8")).hexdigest()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must be a JSON object")
    return payload


def verify_payload(payload: dict[str, Any], label: str) -> list[str]:
    findings: list[str] = []
    tenant = payload.get("tenant")
    snapshot = payload.get("snapshot")
    checksum = payload.get("checksum_sha256")
    if not isinstance(tenant, str) or not tenant.strip():
        findings.append(f"{label}: missing tenant")
    if snapshot is None:
        findings.append(f"{label}: missing snapshot")
    if not isinstance(checksum, str) or len(checksum) != 64:
        findings.append(f"{label}: invalid checksum_sha256 shape")
    if snapshot is not None and isinstance(checksum, str) and len(checksum) == 64:
        expected = compute_checksum(snapshot)
        if expected != checksum:
            findings.append(f"{label}: checksum mismatch")
    return findings


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    export_path = (repo_root / args.export_json).resolve()
    import_path = (repo_root / args.import_json).resolve()
    output_json = (repo_root / args.output_json).resolve()

    export_payload = load_json(export_path)
    import_payload = load_json(import_path)

    findings = []
    findings.extend(verify_payload(export_payload, "export"))
    findings.extend(verify_payload(import_payload, "import"))

    export_tenant = export_payload.get("tenant")
    import_tenant = import_payload.get("tenant")
    if isinstance(export_tenant, str) and isinstance(import_tenant, str):
        if export_tenant != import_tenant:
            findings.append("tenant isolation violation: import tenant differs from export tenant")

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not findings else "FAIL",
        "findings": findings,
        "export_path": str(export_path),
        "import_path": str(import_path),
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if findings:
        print("Workflow snapshot integrity verify: FAIL")
        for item in findings:
            print(f" - {item}")
        print(f"report: {output_json}")
        return 1

    print("Workflow snapshot integrity verify: PASS")
    print(f"report: {output_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
