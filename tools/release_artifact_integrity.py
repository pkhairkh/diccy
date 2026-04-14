#!/usr/bin/env python3
"""
Release artifact integrity validator.

Validates that a release package contains required artifacts, computes digests,
and verifies that release evidence index references resolve to real files.
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

REPORT_PATH_RE = re.compile(r"`(reports/[A-Za-z0-9._/\-]+)`")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate release artifact integrity.")
    parser.add_argument("--release-id", required=True, help="Release identifier (for example RC-2026.02.11).")
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root containing docs/, reports/, and tools/.",
    )
    parser.add_argument(
        "--index-path",
        default="reports/interoperability/release-evidence-index.md",
        help="Markdown index used to collect referenced evidence paths.",
    )
    parser.add_argument("--output-md", required=True, help="Output markdown integrity report path.")
    parser.add_argument("--output-json", required=True, help="Output JSON integrity report path.")
    parser.add_argument(
        "--fail-on-errors",
        action="store_true",
        help="Exit non-zero when required artifacts or index references are missing.",
    )
    return parser.parse_args()


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 64)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def collect_markdown_references(index_path: pathlib.Path) -> list[str]:
    refs: set[str] = set()
    for line in index_path.read_text(errors="ignore").splitlines():
        for match in REPORT_PATH_RE.finditer(line):
            refs.add(match.group(1))
    return sorted(refs)


def required_artifacts(release_id: str) -> list[str]:
    return [
        f"reports/security/sbom-{release_id}.json",
        f"reports/security/dependency-vulnerability-review-{release_id}.md",
        f"reports/interoperability/hash-verification-register-{release_id}.md",
        "reports/interoperability/release-evidence-index.md",
        f"reports/traceability/release-evidence-trace-index-{release_id}.md",
        f"reports/traceability/traceability-gate-decision-{release_id}.md",
        f"reports/performance/performance-scalability-gate-decision-{release_id}.md",
        f"reports/security/security-gate-decision-{release_id}.md",
        f"reports/release/release-notes-{release_id}.md",
        f"reports/release/rc-freeze-record-{release_id}.md",
    ]


def evaluate_release_artifacts(
    *,
    repo_root: pathlib.Path,
    release_id: str,
    index_path: pathlib.Path,
) -> dict[str, Any]:
    required = required_artifacts(release_id)
    referenced = collect_markdown_references(index_path) if index_path.exists() else []

    missing_required: list[str] = []
    missing_referenced: list[str] = []
    file_rows: list[dict[str, Any]] = []

    for rel_path in required:
        full = repo_root / rel_path
        exists = full.exists()
        if not exists:
            missing_required.append(rel_path)
        file_rows.append(
            {
                "path": rel_path,
                "category": "required",
                "exists": exists,
                "sha256": sha256_file(full) if exists else None,
            }
        )

    for rel_path in referenced:
        full = repo_root / rel_path
        exists = full.exists()
        if not exists:
            missing_referenced.append(rel_path)
        file_rows.append(
            {
                "path": rel_path,
                "category": "index-reference",
                "exists": exists,
                "sha256": sha256_file(full) if exists else None,
            }
        )

    file_rows_sorted = sorted(file_rows, key=lambda row: (row["category"], row["path"]))
    failing_rows = [row for row in file_rows_sorted if not row["exists"]]
    result = {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "index_path": str(index_path.relative_to(repo_root)) if index_path.exists() else str(index_path),
        "required_artifact_count": len(required),
        "index_reference_count": len(referenced),
        "missing_required_count": len(missing_required),
        "missing_index_reference_count": len(missing_referenced),
        "missing_required": sorted(missing_required),
        "missing_index_references": sorted(missing_referenced),
        "overall_status": "PASS" if not failing_rows else "FAIL",
        "artifacts": file_rows_sorted,
    }
    return result


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Release Artifact Integrity Report",
        "",
        f"Release Candidate: {payload['release_id']}",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        f"Index Path: `{payload['index_path']}`",
        f"Overall Status: **{payload['overall_status']}**",
        "",
        "## Summary",
        "",
        f"- Required artifacts: {payload['required_artifact_count']}",
        f"- Index references: {payload['index_reference_count']}",
        f"- Missing required artifacts: {payload['missing_required_count']}",
        f"- Missing index references: {payload['missing_index_reference_count']}",
        "",
        "## Artifact Register",
        "",
        "| Category | Path | Exists | SHA256 |",
        "|---|---|---|---|",
    ]
    for row in payload["artifacts"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    row["category"],
                    f"`{row['path']}`",
                    "yes" if row["exists"] else "no",
                    row["sha256"] or "-",
                ]
            )
            + " |"
        )

    if payload["missing_required_count"] > 0 or payload["missing_index_reference_count"] > 0:
        lines.extend(
            [
                "",
                "## Missing Artifacts",
                "",
            ]
        )
        for missing in payload["missing_required"]:
            lines.append(f"- required: `{missing}`")
        for missing in payload["missing_index_references"]:
            lines.append(f"- index-reference: `{missing}`")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def run() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    index_path = (repo_root / args.index_path).resolve()

    if not repo_root.exists():
        print("Release artifact integrity: FAIL")
        print(f"error: repo root not found: {repo_root}")
        return 1
    if not index_path.exists():
        print("Release artifact integrity: FAIL")
        print(f"error: index path not found: {index_path}")
        return 1

    payload = evaluate_release_artifacts(
        repo_root=repo_root,
        release_id=args.release_id,
        index_path=index_path,
    )
    output_md = pathlib.Path(args.output_md).resolve()
    output_json = pathlib.Path(args.output_json).resolve()
    write_json(output_json, payload)
    write_markdown(output_md, payload)

    print(
        "Release artifact integrity: PASS"
        if payload["overall_status"] == "PASS"
        else "Release artifact integrity: FAIL"
    )
    print(f"Required artifacts: {payload['required_artifact_count']}")
    print(f"Index references: {payload['index_reference_count']}")
    print(f"Missing required artifacts: {payload['missing_required_count']}")
    print(f"Missing index references: {payload['missing_index_reference_count']}")

    if args.fail_on_errors and payload["overall_status"] != "PASS":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(run())
