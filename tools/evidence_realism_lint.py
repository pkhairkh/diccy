#!/usr/bin/env python3
"""Lint release evidence for replay-style synthetic hash patterns and origin metadata completeness."""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from typing import Any

SHA256_RE = re.compile(r"sha256:([0-9a-fA-F]{64})")
ALLOWED_ORIGINS = {"synthetic", "de-identified", "external-independent"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Evidence realism lint for release evidence artifacts.")
    parser.add_argument("--release-id", required=True, help="Release identifier used to scope evidence files.")
    parser.add_argument(
        "--repo-root",
        default=".",
        help="Repository root containing reports/ and optional manifest path.",
    )
    parser.add_argument(
        "--manifest",
        default=None,
        help="Optional traceability manifest path for origin-class validation.",
    )
    parser.add_argument(
        "--fail-on-findings",
        action="store_true",
        help="Exit non-zero when findings are present.",
    )
    return parser.parse_args()


def collect_release_files(repo_root: pathlib.Path, release_id: str) -> list[pathlib.Path]:
    reports_root = repo_root / "reports"
    if not reports_root.exists():
        return []
    files: list[pathlib.Path] = []
    for path in reports_root.rglob("*"):
        if path.is_file() and release_id in path.name and path.suffix.lower() in {".md", ".json", ".csv", ".log"}:
            files.append(path)
    return sorted(files)


def detect_hash_findings(path: pathlib.Path) -> list[str]:
    findings: list[str] = []
    text = path.read_text(errors="ignore")
    hashes = SHA256_RE.findall(text)
    if not hashes:
        return findings

    normalized = [value.lower() for value in hashes]
    counts: dict[str, int] = {}
    for digest in normalized:
        counts[digest] = counts.get(digest, 0) + 1
    duplicates = [digest for digest, count in counts.items() if count > 1]
    if duplicates:
        findings.append(
            f"{path}: duplicate sha256 digests detected ({len(duplicates)} unique duplicates)"
        )

    prefix_groups: dict[str, list[int]] = {}
    for digest in normalized:
        prefix = digest[:-4]
        suffix = int(digest[-4:], 16)
        prefix_groups.setdefault(prefix, []).append(suffix)

    for prefix, suffixes in prefix_groups.items():
        unique_suffixes = sorted(set(suffixes))
        if len(unique_suffixes) < 3:
            continue
        if all((b - a) == 1 for a, b in zip(unique_suffixes, unique_suffixes[1:])):
            findings.append(
                f"{path}: replay-style sequential hash suffix pattern detected (prefix={prefix[:8]}...)")

    return findings


def validate_manifest_origins(repo_root: pathlib.Path, manifest_path: pathlib.Path) -> list[str]:
    findings: list[str] = []
    if not manifest_path.exists():
        return [f"manifest not found: {manifest_path}"]

    payload = json.loads(manifest_path.read_text())
    claims = payload.get("claims", [])
    entries = payload.get("release_index_entries", [])

    if not isinstance(claims, list):
        findings.append("manifest claims must be a list")
        claims = []
    if not isinstance(entries, list):
        findings.append("manifest release_index_entries must be a list")
        entries = []

    for claim in claims:
        claim_id = str(claim.get("claim_id", "")).strip() or "<missing-claim-id>"
        evidence_paths = claim.get("evidence_paths", [])
        evidence_origin = claim.get("evidence_origin", {})
        if not isinstance(evidence_paths, list):
            findings.append(f"{claim_id}: evidence_paths must be a list")
            continue
        if not isinstance(evidence_origin, dict):
            findings.append(f"{claim_id}: evidence_origin must be an object")
            continue

        for evidence_path in evidence_paths:
            evidence_path_str = str(evidence_path).strip()
            if not evidence_path_str:
                continue
            origin = evidence_origin.get(evidence_path_str)
            if origin not in ALLOWED_ORIGINS:
                findings.append(
                    f"{claim_id}: missing/invalid origin for {evidence_path_str} (expected one of {sorted(ALLOWED_ORIGINS)})"
                )
            resolved = (repo_root / evidence_path_str).resolve()
            if not resolved.exists():
                findings.append(f"{claim_id}: evidence path missing on disk: {evidence_path_str}")

    for entry in entries:
        entry_id = str(entry.get("entry_id", "")).strip() or "<missing-entry-id>"
        origin = entry.get("entry_origin")
        if origin not in ALLOWED_ORIGINS:
            findings.append(
                f"{entry_id}: missing/invalid entry_origin (expected one of {sorted(ALLOWED_ORIGINS)})"
            )

    return findings


def run_lint(repo_root: pathlib.Path, release_id: str, manifest: pathlib.Path | None) -> list[str]:
    findings: list[str] = []

    for path in collect_release_files(repo_root, release_id):
        findings.extend(detect_hash_findings(path))

    if manifest is not None:
        findings.extend(validate_manifest_origins(repo_root, manifest))

    return findings


def run() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    manifest = pathlib.Path(args.manifest).resolve() if args.manifest else None

    findings = run_lint(repo_root=repo_root, release_id=args.release_id, manifest=manifest)

    if findings:
        print("Evidence realism lint: FAIL")
        print(f"Findings: {len(findings)}")
        for finding in findings:
            print(f"- {finding}")
        if args.fail_on_findings:
            return 1
        return 0

    print("Evidence realism lint: PASS")
    scanned = len(collect_release_files(repo_root, args.release_id))
    print(f"Scanned release files: {scanned}")
    if manifest:
        print(f"Manifest origin validation: {manifest}")
    return 0


if __name__ == "__main__":
    sys.exit(run())
