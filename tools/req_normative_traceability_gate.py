#!/usr/bin/env python3
from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any


REQ_RE = re.compile(r"REQ-[A-Z]+-\d{3}")
NORMATIVE_RE = re.compile(r"\b(MUST|SHOULD|MAY)\b")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Gate new normative REQ IDs on docs/16 index coverage and automated test references."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument("--docs-dir", default="docs", help="Docs directory path.")
    parser.add_argument(
        "--requirements-index",
        default="docs/16-Requirements-Index.md",
        help="Requirements index path.",
    )
    parser.add_argument(
        "--baseline",
        default="reports/traceability/req-normative-traceability-baseline.json",
        help="Baseline JSON path for already-known normative REQ IDs.",
    )
    parser.add_argument(
        "--report",
        default="reports/traceability/req-normative-traceability-gate.json",
        help="Output report path.",
    )
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="Write baseline from current normative REQ IDs and exit.",
    )
    return parser.parse_args()


def collect_normative_req_ids(docs_dir: pathlib.Path) -> set[str]:
    req_ids: set[str] = set()
    for path in sorted(docs_dir.rglob("*.md")):
        in_code_block = False
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            stripped = line.strip()
            if stripped.startswith("```"):
                in_code_block = not in_code_block
                continue
            if in_code_block:
                continue
            if not NORMATIVE_RE.search(line):
                continue
            req_ids.update(REQ_RE.findall(line))
    return req_ids


def collect_index_req_ids(index_path: pathlib.Path) -> set[str]:
    text = index_path.read_text(encoding="utf-8", errors="replace")
    return set(REQ_RE.findall(text))


def collect_test_req_refs(repo_root: pathlib.Path) -> dict[str, list[str]]:
    refs: dict[str, list[str]] = {}

    def add_refs(path: pathlib.Path) -> None:
        rel = path.relative_to(repo_root).as_posix()
        for line_no, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1):
            if "REQ-" not in line:
                continue
            for req_id in REQ_RE.findall(line):
                refs.setdefault(req_id, []).append(f"{rel}:{line_no}")

    for path in sorted((repo_root / "crates").rglob("*.rs")):
        if "target" in path.parts:
            continue
        if "tests" not in path.parts:
            continue
        add_refs(path)

    for path in sorted((repo_root / "tools").rglob("test_*.py")):
        add_refs(path)

    frontend_tests = repo_root / "frontend" / "tests"
    if frontend_tests.exists():
        for path in sorted(frontend_tests.rglob("*.mjs")):
            add_refs(path)

    return {key: sorted(value) for key, value in sorted(refs.items())}


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    docs_dir = (repo_root / args.docs_dir).resolve()
    requirements_index = (repo_root / args.requirements_index).resolve()
    baseline_path = (repo_root / args.baseline).resolve()
    report_path = (repo_root / args.report).resolve()

    current_normative_req_ids = sorted(collect_normative_req_ids(docs_dir))
    if args.write_baseline:
        payload = {
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "known_normative_req_ids": current_normative_req_ids,
            "requirements_index": str(requirements_index.relative_to(repo_root)),
        }
        write_json(baseline_path, payload)
        print("REQ normative traceability gate: BASELINE_WRITTEN")
        print(f"Normative REQ IDs recorded: {len(current_normative_req_ids)}")
        return 0

    if not baseline_path.exists():
        print("REQ normative traceability gate: FAIL")
        print(f"error: baseline missing: {baseline_path.relative_to(repo_root)}")
        return 1

    baseline_payload = json.loads(baseline_path.read_text(encoding="utf-8"))
    baseline_req_ids = {
        item
        for item in baseline_payload.get("known_normative_req_ids", [])
        if isinstance(item, str)
    }

    current_set = set(current_normative_req_ids)
    new_normative_req_ids = sorted(current_set - baseline_req_ids)
    index_req_ids = collect_index_req_ids(requirements_index)
    test_refs = collect_test_req_refs(repo_root)

    missing_index_entries = sorted(req_id for req_id in new_normative_req_ids if req_id not in index_req_ids)
    missing_test_refs = sorted(req_id for req_id in new_normative_req_ids if req_id not in test_refs)

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not missing_index_entries and not missing_test_refs else "FAIL",
        "baseline": str(baseline_path.relative_to(repo_root)),
        "requirements_index": str(requirements_index.relative_to(repo_root)),
        "current_normative_req_count": len(current_normative_req_ids),
        "new_normative_req_ids": new_normative_req_ids,
        "new_normative_req_count": len(new_normative_req_ids),
        "missing_index_entries_for_new_req_ids": missing_index_entries,
        "missing_index_entries_count": len(missing_index_entries),
        "missing_test_refs_for_new_req_ids": missing_test_refs,
        "missing_test_refs_count": len(missing_test_refs),
    }
    write_json(report_path, payload)

    if missing_index_entries or missing_test_refs:
        print("REQ normative traceability gate: FAIL")
        print(f"missing index entries: {len(missing_index_entries)}")
        print(f"missing test refs: {len(missing_test_refs)}")
        print(f"report: {report_path.relative_to(repo_root)}")
        return 1

    print("REQ normative traceability gate: PASS")
    print(f"new normative REQ IDs: {len(new_normative_req_ids)}")
    print(f"report: {report_path.relative_to(repo_root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
