#!/usr/bin/env python3
"""
Fail-closed claim-surface lint.

This tool scans Markdown files and rejects prohibited claim language outside
the explicit informative allowlist.

Usage:
  python3 tools/claim_surface_lint.py
  python3 tools/claim_surface_lint.py --root README.md --root docs
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass


DEFAULT_REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_SCAN_ROOTS = ("README.md", "docs")
DEFAULT_ALLOW_PATHS = ("docs/15-Regulatory-and-Standards-Mapping.md",)


@dataclass(frozen=True)
class Pattern:
    pattern_id: str
    regex: re.Pattern[str]
    description: str


@dataclass(frozen=True)
class Violation:
    file: str
    line: int
    pattern_id: str
    description: str
    excerpt: str


PATTERNS: tuple[Pattern, ...] = (
    Pattern(
        pattern_id="CLAIM-001",
        regex=re.compile(
            r"\bfor\s+diagnostic\s+use\b|\bdiagnostic\s+(?:use|purpose|software)\b",
            flags=re.IGNORECASE,
        ),
        description="diagnostic-use claim language",
    ),
    Pattern(
        pattern_id="CLAIM-002",
        regex=re.compile(r"\bprimary diagn[o0]sis\b", flags=re.IGNORECASE),
        description="primary-diagnosis claim language",
    ),
    Pattern(
        pattern_id="CLAIM-003",
        regex=re.compile(r"\bFDA\s+clear(?:ed|ance)\b", flags=re.IGNORECASE),
        description="FDA-clearance claim language",
    ),
    Pattern(
        pattern_id="CLAIM-004",
        regex=re.compile(r"\bFDA\s+approved\b", flags=re.IGNORECASE),
        description="FDA-approval claim language",
    ),
    Pattern(
        pattern_id="CLAIM-005",
        regex=re.compile(r"\bCE[- ]mar[kc](?:ed|ing)?\b", flags=re.IGNORECASE),
        description="CE-marking claim language",
    ),
    Pattern(
        pattern_id="CLAIM-006",
        regex=re.compile(r"\bMDR\s+class\b", flags=re.IGNORECASE),
        description="MDR classification claim language",
    ),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Claim-surface lint for README/docs markdown.")
    parser.add_argument(
        "--repo-root",
        default=str(DEFAULT_REPO_ROOT),
        help="Repository root path (default: auto-detected from script location).",
    )
    parser.add_argument(
        "--root",
        action="append",
        dest="roots",
        help="Repo-relative scan root (file or directory). Repeatable. Default: README.md + docs.",
    )
    parser.add_argument(
        "--allow-path",
        action="append",
        dest="allow_paths",
        help=(
            "Repo-relative markdown path exempt from claim-language checks. "
            "Repeatable. Default: docs/15-Regulatory-and-Standards-Mapping.md."
        ),
    )
    parser.add_argument(
        "--report",
        help="Optional JSON report output path.",
    )
    return parser.parse_args()


def resolve_repo_relative(repo_root: pathlib.Path, value: str) -> pathlib.Path:
    candidate = pathlib.Path(value)
    if candidate.is_absolute():
        return candidate
    return repo_root / candidate


def collect_markdown_files(repo_root: pathlib.Path, roots: list[str]) -> list[pathlib.Path]:
    files: set[pathlib.Path] = set()
    for root_entry in roots:
        root_path = resolve_repo_relative(repo_root, root_entry)
        if root_path.is_file():
            if root_path.suffix.lower() == ".md":
                files.add(root_path.resolve())
            continue
        if root_path.is_dir():
            for path in root_path.rglob("*.md"):
                files.add(path.resolve())
            continue
    return sorted(files, key=lambda p: str(p).lower())


def to_repo_relpath(repo_root: pathlib.Path, path: pathlib.Path) -> str:
    return path.resolve().relative_to(repo_root.resolve()).as_posix()


def scan_file(
    repo_root: pathlib.Path,
    file_path: pathlib.Path,
    allow_paths: set[str],
) -> list[Violation]:
    relpath = to_repo_relpath(repo_root, file_path)
    if relpath in allow_paths:
        return []

    violations: list[Violation] = []
    in_code_fence = False
    for line_no, line in enumerate(file_path.read_text(errors="ignore").splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("```"):
            in_code_fence = not in_code_fence
            continue
        if in_code_fence:
            continue
        # Ignore inline code literals so policy text can document denylist terms.
        line_for_scan = re.sub(r"`[^`]*`", "", line)
        for pattern in PATTERNS:
            if pattern.regex.search(line_for_scan):
                violations.append(
                    Violation(
                        file=relpath,
                        line=line_no,
                        pattern_id=pattern.pattern_id,
                        description=pattern.description,
                        excerpt=line.strip(),
                    )
                )
    return violations


def run_lint(repo_root: pathlib.Path, roots: list[str], allow_paths: list[str], report: pathlib.Path | None = None) -> int:
    normalized_allow_paths = {
        pathlib.Path(path).as_posix().lstrip("./")
        for path in allow_paths
    }
    markdown_files = collect_markdown_files(repo_root, roots)

    violations: list[Violation] = []
    for file_path in markdown_files:
        violations.extend(scan_file(repo_root, file_path, normalized_allow_paths))

    payload = {
        "status": "FAIL" if violations else "PASS",
        "scanned_markdown_files": len(markdown_files),
        "scan_roots": roots,
        "allow_paths": sorted(normalized_allow_paths),
        "violations_count": len(violations),
        "violations": [
            {
                "file": violation.file,
                "line": violation.line,
                "pattern_id": violation.pattern_id,
                "description": violation.description,
                "excerpt": violation.excerpt,
            }
            for violation in violations
        ],
    }
    if report is not None:
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if violations:
        print("Claim surface lint: FAIL")
        print(f"Scanned markdown files: {len(markdown_files)}")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            print(
                f"{violation.file}:{violation.line}: "
                f"{violation.pattern_id} ({violation.description}) | {violation.excerpt}"
            )
        return 1

    print("Claim surface lint: PASS")
    print(f"Scanned markdown files: {len(markdown_files)}")
    print("Violations: 0")
    return 0


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    roots = args.roots or list(DEFAULT_SCAN_ROOTS)
    allow_paths = args.allow_paths or list(DEFAULT_ALLOW_PATHS)
    report = pathlib.Path(args.report).resolve() if args.report else None
    return run_lint(repo_root=repo_root, roots=roots, allow_paths=allow_paths, report=report)


if __name__ == "__main__":
    sys.exit(main())
