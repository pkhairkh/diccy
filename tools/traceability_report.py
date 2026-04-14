#!/usr/bin/env python3
"""
Generate a deterministic traceability report for release gating.

This report maps REQ identifiers from docs to:
- Rust references,
- test references,
- report evidence references.

It also validates optional release linkage manifests and reports deltas against
an optional baseline snapshot.

Usage:
  python3 tools/traceability_report.py
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from collections import defaultdict
from typing import Any


REQ_RE = re.compile(r"REQ-[A-Z]+-\d{3}")
MUST_RE = re.compile(r"\bMUST\b")

ROOT = pathlib.Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate REQ traceability report.")
    parser.add_argument(
        "--root",
        default=str(ROOT),
        help="Repository root path.",
    )
    parser.add_argument(
        "--output-md",
        help="Optional markdown report output path.",
    )
    parser.add_argument(
        "--output-json",
        help="Optional JSON report output path.",
    )
    parser.add_argument(
        "--baseline-json",
        help="Optional baseline JSON report path for delta computation.",
    )
    parser.add_argument(
        "--linkage-manifest",
        help="Optional release linkage manifest JSON path.",
    )
    parser.add_argument(
        "--allow-missing",
        action="store_true",
        help="Do not fail when MUST-level REQs are missing test references.",
    )
    parser.add_argument(
        "--fail-on-invalid-links",
        action="store_true",
        help="Fail when linkage manifest references missing files or REQ IDs.",
    )
    return parser.parse_args()


def iter_markdown_files(root: pathlib.Path, base: str) -> list[pathlib.Path]:
    base_dir = root / base
    if not base_dir.exists():
        return []
    return sorted(path for path in base_dir.rglob("*.md") if "target" not in path.parts)


def collect_all_reqs(root: pathlib.Path) -> set[str]:
    reqs: set[str] = set()
    for path in iter_markdown_files(root, "docs"):
        text = path.read_text(errors="ignore")
        reqs.update(REQ_RE.findall(text))
    return reqs


def collect_must_reqs(root: pathlib.Path) -> dict[str, list[str]]:
    locations: dict[str, list[str]] = defaultdict(list)
    docs_dir = root / "docs"
    for path in iter_markdown_files(root, "docs"):
        rel = path.relative_to(root)
        for line_no, line in enumerate(path.read_text(errors="ignore").splitlines(), start=1):
            if "REQ-" not in line:
                continue
            if not MUST_RE.search(line):
                continue
            for req in REQ_RE.findall(line):
                locations[req].append(f"{rel}:{line_no}")
    # Stable ordering for report output
    return {req: sorted(paths) for req, paths in sorted(locations.items())}


def collect_rust_and_test_refs(root: pathlib.Path) -> tuple[dict[str, list[str]], dict[str, list[str]]]:
    rust_refs: dict[str, list[str]] = defaultdict(list)
    test_refs: dict[str, list[str]] = defaultdict(list)
    for path in sorted(root.rglob("*.rs")):
        if "target" in path.parts:
            continue
        rel = path.relative_to(root)
        text = path.read_text(errors="ignore")
        is_test_like = "tests" in path.parts or "#[test]" in text or "mod tests" in text
        for line_no, line in enumerate(text.splitlines(), start=1):
            if "REQ-" not in line:
                continue
            for req in REQ_RE.findall(line):
                location = f"{rel}:{line_no}"
                rust_refs[req].append(location)
                if is_test_like:
                    test_refs[req].append(location)

    for path in sorted(root.rglob("*.py")):
        if "target" in path.parts:
            continue
        rel = path.relative_to(root)
        is_test_like = "tests" in path.parts or path.name.startswith("test_")
        if not is_test_like:
            continue
        text = path.read_text(errors="ignore")
        for line_no, line in enumerate(text.splitlines(), start=1):
            if "REQ-" not in line:
                continue
            for req in REQ_RE.findall(line):
                test_refs[req].append(f"{rel}:{line_no}")

    return (
        {req: sorted(items) for req, items in sorted(rust_refs.items())},
        {req: sorted(items) for req, items in sorted(test_refs.items())},
    )


def collect_report_refs(root: pathlib.Path) -> dict[str, list[str]]:
    refs: dict[str, list[str]] = defaultdict(list)
    for path in iter_markdown_files(root, "reports"):
        rel = path.relative_to(root)
        for line_no, line in enumerate(path.read_text(errors="ignore").splitlines(), start=1):
            if "REQ-" not in line:
                continue
            for req in REQ_RE.findall(line):
                refs[req].append(f"{rel}:{line_no}")
    return {req: sorted(items) for req, items in sorted(refs.items())}


def validate_manifest(
    root: pathlib.Path,
    manifest_path: pathlib.Path,
    *,
    all_reqs: set[str],
    test_refs: dict[str, list[str]],
    report_refs: dict[str, list[str]],
) -> tuple[list[str], list[dict[str, Any]], list[dict[str, Any]]]:
    invalid_links: list[str] = []
    claim_clusters: list[dict[str, Any]] = []
    release_index_clusters: list[dict[str, Any]] = []

    if not manifest_path.exists():
        return ([f"manifest not found: {manifest_path}"], claim_clusters, release_index_clusters)

    payload = json.loads(manifest_path.read_text())
    claims = payload.get("claims", [])
    release_entries = payload.get("release_index_entries", [])
    if not isinstance(claims, list):
        invalid_links.append("manifest claims must be a list")
        claims = []
    if not isinstance(release_entries, list):
        invalid_links.append("manifest release_index_entries must be a list")
        release_entries = []

    for claim in claims:
        if not isinstance(claim, dict):
            invalid_links.append("manifest claim entry must be an object")
            continue
        claim_id = str(claim.get("claim_id", "")).strip()
        source_doc = str(claim.get("source_doc", "")).strip()
        req_ids_raw = claim.get("req_ids", [])
        evidence_paths_raw = claim.get("evidence_paths", [])
        gate_logs_raw = claim.get("gate_logs", [])

        req_ids = [str(req).strip() for req in req_ids_raw] if isinstance(req_ids_raw, list) else []
        evidence_paths = (
            [str(path).strip() for path in evidence_paths_raw] if isinstance(evidence_paths_raw, list) else []
        )
        gate_logs = [str(path).strip() for path in gate_logs_raw] if isinstance(gate_logs_raw, list) else []

        if not claim_id:
            invalid_links.append("manifest claim missing claim_id")
            continue

        if not source_doc:
            invalid_links.append(f"{claim_id}: missing source_doc")
        else:
            source_path = root / source_doc
            if not source_path.exists():
                invalid_links.append(f"{claim_id}: missing source_doc path {source_doc}")

        for req in req_ids:
            if req not in all_reqs:
                invalid_links.append(f"{claim_id}: unknown REQ ID {req}")

        for evidence in evidence_paths:
            if not evidence:
                continue
            if not (root / evidence).exists():
                invalid_links.append(f"{claim_id}: missing evidence path {evidence}")

        for gate_log in gate_logs:
            if not gate_log:
                continue
            if not (root / gate_log).exists():
                invalid_links.append(f"{claim_id}: missing gate log {gate_log}")

        req_with_test = sorted(req for req in req_ids if req in test_refs)
        req_with_report = sorted(req for req in req_ids if req in report_refs)
        claim_clusters.append(
            {
                "claim_id": claim_id,
                "source_doc": source_doc,
                "req_ids": req_ids,
                "evidence_paths": evidence_paths,
                "gate_logs": gate_logs,
                "req_with_test_refs": req_with_test,
                "req_with_report_refs": req_with_report,
                "missing_req_test_refs": sorted(req for req in req_ids if req not in test_refs),
                "missing_req_report_refs": sorted(req for req in req_ids if req not in report_refs),
            }
        )

    for entry in release_entries:
        if not isinstance(entry, dict):
            invalid_links.append("manifest release index entry must be an object")
            continue
        entry_id = str(entry.get("entry_id", "")).strip()
        index_path = str(entry.get("index_path", "")).strip()
        cluster_ids_raw = entry.get("cluster_ids", [])
        gate_logs_raw = entry.get("gate_logs", [])
        cluster_ids = [str(cluster).strip() for cluster in cluster_ids_raw] if isinstance(cluster_ids_raw, list) else []
        gate_logs = [str(path).strip() for path in gate_logs_raw] if isinstance(gate_logs_raw, list) else []

        if not entry_id:
            invalid_links.append("manifest release index entry missing entry_id")
            continue
        if not index_path:
            invalid_links.append(f"{entry_id}: missing index_path")
        elif not (root / index_path).exists():
            invalid_links.append(f"{entry_id}: missing index_path file {index_path}")

        for gate_log in gate_logs:
            if gate_log and not (root / gate_log).exists():
                invalid_links.append(f"{entry_id}: missing gate log {gate_log}")

        release_index_clusters.append(
            {
                "entry_id": entry_id,
                "index_path": index_path,
                "cluster_ids": cluster_ids,
                "gate_logs": gate_logs,
            }
        )

    # Stable order for deterministic output
    return (
        sorted(set(invalid_links)),
        sorted(claim_clusters, key=lambda item: item["claim_id"]),
        sorted(release_index_clusters, key=lambda item: item["entry_id"]),
    )


def compute_delta(current: dict[str, Any], baseline_path: pathlib.Path | None) -> dict[str, int] | None:
    if baseline_path is None or not baseline_path.exists():
        return None
    baseline = json.loads(baseline_path.read_text())
    return {
        "must_reqs": int(current["must_reqs_count"]) - int(baseline.get("must_reqs_count", 0)),
        "missing_references": int(current["missing_references_count"])
        - int(baseline.get("missing_references_count", 0)),
        "invalid_links": int(current["invalid_links_count"]) - int(baseline.get("invalid_links_count", 0)),
    }


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Traceability Snapshot",
        "",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        f"Root: `{payload['root']}`",
        "",
        "## Summary",
        "",
        f"- MUST-level REQs in docs: {payload['must_reqs_count']}",
        f"- REQ references in Rust: {payload['rust_req_refs_count']}",
        f"- REQ references in tests: {payload['test_req_refs_count']}",
        f"- REQ references in reports: {payload['report_req_refs_count']}",
        f"- Missing references: {payload['missing_references_count']}",
        f"- Invalid links: {payload['invalid_links_count']}",
        "",
    ]
    delta = payload.get("delta")
    if delta is not None:
        lines.extend(
            [
                "## Delta vs Baseline",
                "",
                f"- MUST REQs delta: {delta['must_reqs']}",
                f"- Missing references delta: {delta['missing_references']}",
                f"- Invalid links delta: {delta['invalid_links']}",
                "",
            ]
        )

    missing: list[str] = payload.get("missing_references", [])
    if missing:
        lines.extend(["## Missing MUST-level REQ References", ""])
        for req in missing:
            lines.append(f"- {req}")
        lines.append("")

    invalid: list[str] = payload.get("invalid_links", [])
    if invalid:
        lines.extend(["## Invalid Links", ""])
        for issue in invalid:
            lines.append(f"- {issue}")
        lines.append("")

    clusters: list[dict[str, Any]] = payload.get("claim_clusters", [])
    if clusters:
        lines.extend(
            [
                "## Claim Clusters",
                "",
                "| Claim ID | REQs | With Test Refs | With Report Refs | Missing Test Refs | Missing Report Refs |",
                "|---|---:|---:|---:|---:|---:|",
            ]
        )
        for cluster in clusters:
            lines.append(
                "| "
                + " | ".join(
                    [
                        cluster["claim_id"],
                        str(len(cluster["req_ids"])),
                        str(len(cluster["req_with_test_refs"])),
                        str(len(cluster["req_with_report_refs"])),
                        str(len(cluster["missing_req_test_refs"])),
                        str(len(cluster["missing_req_report_refs"])),
                    ]
                )
                + " |"
            )
        lines.append("")

    release_entries: list[dict[str, Any]] = payload.get("release_index_clusters", [])
    if release_entries:
        lines.extend(
            [
                "## Release Evidence Index Bindings",
                "",
                "| Entry ID | Index Path | Cluster Count | Gate Log Count |",
                "|---|---|---:|---:|",
            ]
        )
        for entry in release_entries:
            lines.append(
                "| "
                + " | ".join(
                    [
                        entry["entry_id"],
                        f"`{entry['index_path']}`",
                        str(len(entry["cluster_ids"])),
                        str(len(entry["gate_logs"])),
                    ]
                )
                + " |"
            )
        lines.append("")

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def run() -> int:
    args = parse_args()
    root = pathlib.Path(args.root).resolve()
    docs_dir = root / "docs"
    if not docs_dir.exists():
        print(f"error: docs directory not found at {docs_dir}")
        return 1

    all_reqs = collect_all_reqs(root)
    must_req_locations = collect_must_reqs(root)
    rust_refs, test_refs = collect_rust_and_test_refs(root)
    report_refs = collect_report_refs(root)

    must_reqs = sorted(must_req_locations.keys())
    rust_req_set = set(rust_refs.keys())
    test_req_set = set(test_refs.keys())
    report_req_set = set(report_refs.keys())
    missing = sorted(req for req in must_reqs if req not in test_req_set)

    invalid_links: list[str] = []
    claim_clusters: list[dict[str, Any]] = []
    release_index_clusters: list[dict[str, Any]] = []
    manifest_path = None
    if args.linkage_manifest:
        manifest_path = pathlib.Path(args.linkage_manifest).resolve()
        invalid_links, claim_clusters, release_index_clusters = validate_manifest(
            root,
            manifest_path,
            all_reqs=all_reqs,
            test_refs=test_refs,
            report_refs=report_refs,
        )

    payload: dict[str, Any] = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "root": str(root),
        "must_reqs_count": len(must_reqs),
        "all_reqs_count": len(all_reqs),
        "rust_req_refs_count": len(rust_req_set),
        "test_req_refs_count": len(test_req_set),
        "report_req_refs_count": len(report_req_set),
        "missing_references_count": len(missing),
        "missing_references": missing,
        "invalid_links_count": len(invalid_links),
        "invalid_links": invalid_links,
        "must_req_locations": must_req_locations,
        "manifest_path": str(manifest_path) if manifest_path else None,
        "claim_clusters": claim_clusters,
        "release_index_clusters": release_index_clusters,
    }

    baseline_path = pathlib.Path(args.baseline_json).resolve() if args.baseline_json else None
    payload["delta"] = compute_delta(payload, baseline_path)

    if args.output_json:
        write_json(pathlib.Path(args.output_json).resolve(), payload)
    if args.output_md:
        write_markdown(pathlib.Path(args.output_md).resolve(), payload)

    # Preserve canonical output fields used by gates.
    print(f"MUST-level REQs in docs: {len(must_reqs)}")
    print(f"REQ references in Rust: {len(rust_req_set)}")
    print(f"Missing references: {len(missing)}")
    for req in missing:
        print(req)
    print(f"Invalid links: {len(invalid_links)}")
    for issue in invalid_links:
        print(issue)

    should_fail_missing = not args.allow_missing and len(missing) > 0
    should_fail_invalid = args.fail_on_invalid_links and len(invalid_links) > 0
    return 1 if (should_fail_missing or should_fail_invalid) else 0


if __name__ == "__main__":
    sys.exit(run())
