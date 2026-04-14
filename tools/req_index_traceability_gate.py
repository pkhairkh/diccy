#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys


REQ_RE = re.compile(r"REQ-[A-Z]+-\d{3}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Gate new docs/16 REQ IDs on test-reference coverage.")
    parser.add_argument("--repo-root", default=".", help="Repository root")
    parser.add_argument(
        "--requirements-index",
        default="docs/16-Requirements-Index.md",
        help="Requirements index path",
    )
    parser.add_argument(
        "--baseline",
        default="reports/traceability/req-index-traceability-baseline.json",
        help="Baseline JSON path",
    )
    parser.add_argument(
        "--report",
        default="reports/traceability/req-index-traceability-gate.json",
        help="Output report path",
    )
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="Write baseline from current docs/16 REQ IDs",
    )
    return parser.parse_args()


def parse_req_ids(path: pathlib.Path) -> set[str]:
    text = path.read_text(encoding="utf-8", errors="replace")
    return set(REQ_RE.findall(text))


def collect_test_req_refs(repo_root: pathlib.Path) -> dict[str, list[str]]:
    refs: dict[str, list[str]] = {}

    def add_refs(file_path: pathlib.Path) -> None:
        rel = file_path.relative_to(repo_root).as_posix()
        for line_no, line in enumerate(file_path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1):
            if "REQ-" not in line:
                continue
            for req in REQ_RE.findall(line):
                refs.setdefault(req, []).append(f"{rel}:{line_no}")

    for path in sorted((repo_root / "crates").rglob("*.rs")):
        if "target" in path.parts or "tests" not in path.parts:
            continue
        add_refs(path)

    for path in sorted((repo_root / "tools").rglob("test_*.py")):
        add_refs(path)

    for path in sorted((repo_root / "frontend" / "tests").rglob("*.mjs")):
        add_refs(path)

    return {req: sorted(locations) for req, locations in sorted(refs.items())}


def write_json(path: pathlib.Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    requirements_index = (repo_root / args.requirements_index).resolve()
    baseline_path = (repo_root / args.baseline).resolve()
    report_path = (repo_root / args.report).resolve()

    current_req_ids = sorted(parse_req_ids(requirements_index))
    if args.write_baseline:
        baseline_payload = {
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "requirements_index": str(requirements_index.relative_to(repo_root)),
            "known_req_ids": current_req_ids,
        }
        write_json(baseline_path, baseline_payload)
        print("REQ index traceability gate: BASELINE_WRITTEN")
        print(f"REQ IDs recorded: {len(current_req_ids)}")
        return 0

    if not baseline_path.exists():
        print("REQ index traceability gate: FAIL")
        print(f"error: baseline missing: {baseline_path.relative_to(repo_root)}")
        return 1

    baseline_payload = json.loads(baseline_path.read_text(encoding="utf-8"))
    baseline_req_ids = {
        str(item) for item in baseline_payload.get("known_req_ids", []) if isinstance(item, str)
    }
    current_req_set = set(current_req_ids)
    new_req_ids = sorted(current_req_set - baseline_req_ids)

    test_refs = collect_test_req_refs(repo_root)
    missing_test_refs = sorted(req for req in new_req_ids if req not in test_refs)

    payload = {
        "status": "FAIL" if missing_test_refs else "PASS",
        "requirements_index": str(requirements_index.relative_to(repo_root)),
        "baseline": str(baseline_path.relative_to(repo_root)),
        "current_req_count": len(current_req_ids),
        "baseline_req_count": len(baseline_req_ids),
        "new_req_ids": new_req_ids,
        "new_req_count": len(new_req_ids),
        "missing_test_refs_for_new_req_ids": missing_test_refs,
        "missing_test_refs_count": len(missing_test_refs),
    }
    write_json(report_path, payload)

    if missing_test_refs:
        print("REQ index traceability gate: FAIL")
        print(f"New REQ IDs without test references: {len(missing_test_refs)}")
        print(f"Report: {report_path.relative_to(repo_root)}")
        return 1

    print("REQ index traceability gate: PASS")
    print(f"New REQ IDs checked: {len(new_req_ids)}")
    print(f"Report: {report_path.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
