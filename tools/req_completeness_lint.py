#!/usr/bin/env python3
"""
REQ completeness lint for docs normative statements.

The lint enforces REQ identifiers for newly introduced normative statements
(`MUST`, `SHOULD`, `MAY`) by comparing against a checked-in baseline allowlist.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
from typing import Any


REQ_RE = re.compile(r"REQ-[A-Z]+-\d{3}")
NORMATIVE_RE = re.compile(r"\b(MUST|SHOULD|MAY)\b")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lint docs for REQ completeness on normative statements.")
    parser.add_argument("--docs-dir", default="docs", help="Docs directory to scan.")
    parser.add_argument(
        "--baseline",
        default="reports/traceability/req-completeness-baseline.json",
        help="Baseline allowlist JSON path.",
    )
    parser.add_argument("--output-json", help="Optional JSON result path.")
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="Write/update the baseline file from current docs.",
    )
    return parser.parse_args()


def fingerprint(rel_path: pathlib.Path, line_no: int, line: str) -> str:
    material = f"{rel_path}:{line_no}:{line.strip()}".encode("utf-8")
    return hashlib.sha256(material).hexdigest()


def collect_normative_without_req(docs_dir: pathlib.Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for path in sorted(docs_dir.rglob("*.md")):
        rel = path.relative_to(docs_dir.parent)
        in_code_block = False
        for line_no, line in enumerate(path.read_text(errors="ignore").splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("```"):
                in_code_block = not in_code_block
                continue
            if in_code_block:
                continue
            if not NORMATIVE_RE.search(line):
                continue
            if REQ_RE.search(line):
                continue
            findings.append(
                {
                    "fingerprint": fingerprint(rel, line_no, line),
                    "path": str(rel),
                    "line": line_no,
                    "text": stripped,
                }
            )
    return findings


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def run() -> int:
    args = parse_args()
    docs_dir = pathlib.Path(args.docs_dir).resolve()
    baseline_path = pathlib.Path(args.baseline).resolve()
    if not docs_dir.exists():
        print("REQ completeness lint: FAIL")
        print(f"error: docs dir not found: {docs_dir}")
        return 1

    observed = collect_normative_without_req(docs_dir)
    observed_by_fp = {item["fingerprint"]: item for item in observed}

    if args.write_baseline:
        payload = {
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "docs_dir": str(docs_dir),
            "allowlist": sorted(observed, key=lambda item: (item["path"], item["line"])),
        }
        write_json(baseline_path, payload)
        print("REQ completeness lint: BASELINE_WRITTEN")
        print(f"Allowlisted normative-without-REQ lines: {len(observed)}")
        return 0

    if not baseline_path.exists():
        print("REQ completeness lint: FAIL")
        print(f"error: baseline not found: {baseline_path}")
        return 1
    baseline_payload = json.loads(baseline_path.read_text())
    baseline_items = baseline_payload.get("allowlist", [])
    baseline_fps = {
        str(item.get("fingerprint", ""))
        for item in baseline_items
        if isinstance(item, dict) and item.get("fingerprint")
    }

    violations = [item for fp, item in observed_by_fp.items() if fp not in baseline_fps]
    result_payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "docs_dir": str(docs_dir),
        "baseline_path": str(baseline_path),
        "observed_normative_without_req_count": len(observed),
        "baseline_allowlist_count": len(baseline_fps),
        "violations_count": len(violations),
        "violations": sorted(violations, key=lambda item: (item["path"], item["line"])),
    }
    if args.output_json:
        write_json(pathlib.Path(args.output_json).resolve(), result_payload)

    if violations:
        print("REQ completeness lint: FAIL")
        print(f"Violations: {len(violations)}")
        for item in violations:
            print(f"{item['path']}:{item['line']} {item['text']}")
        return 1

    print("REQ completeness lint: PASS")
    print(f"Observed normative-without-REQ lines: {len(observed)}")
    print(f"Baseline allowlist entries: {len(baseline_fps)}")
    print("Violations: 0")
    return 0


if __name__ == "__main__":
    sys.exit(run())
