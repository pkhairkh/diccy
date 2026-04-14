#!/usr/bin/env python3
"""Symbol linkage lint from docs status map to code."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from dataclasses import dataclass


DEFAULT_REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
STATUS_DOC = "docs/31-Implementation-Status.md"


@dataclass(frozen=True)
class SymbolRow:
    symbol: str
    status: str
    source_line: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify implemented symbols in docs exist in code.")
    parser.add_argument("--repo-root", default=str(DEFAULT_REPO_ROOT), help="Repository root path.")
    return parser.parse_args()


def parse_symbol_rows(status_doc: pathlib.Path) -> list[SymbolRow]:
    rows: list[SymbolRow] = []
    row_pattern = re.compile(r"^\|\s*`([^`]+)`\s*\|\s*([^|]+?)\s*\|")

    for line_no, line in enumerate(status_doc.read_text(errors="ignore").splitlines(), start=1):
        match = row_pattern.match(line)
        if not match:
            continue
        rows.append(SymbolRow(symbol=match.group(1).strip(), status=match.group(2).strip(), source_line=line_no))

    return rows


def collect_code_text(repo_root: pathlib.Path) -> str:
    chunks: list[str] = []
    for path in (repo_root / "crates").rglob("*.rs"):
        chunks.append(path.read_text(errors="ignore"))
    return "\n".join(chunks)


def run(repo_root: pathlib.Path) -> int:
    status_path = repo_root / STATUS_DOC
    if not status_path.exists():
        print("Docs symbol lint: FAIL")
        print(f"Missing status doc: {STATUS_DOC}")
        return 1

    rows = parse_symbol_rows(status_path)
    if not rows:
        print("Docs symbol lint: FAIL")
        print("No symbol rows found in status map")
        return 1

    code_text = collect_code_text(repo_root)

    violations: list[str] = []
    for row in rows:
        normalized_status = row.status.lower()
        if normalized_status.startswith("implemented"):
            if re.search(rf"\b{re.escape(row.symbol)}\b", code_text) is None:
                violations.append(
                    f"{STATUS_DOC}:{row.source_line}: implemented symbol `{row.symbol}` not found in crates/*.rs"
                )

    if violations:
        print("Docs symbol lint: FAIL")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            print(violation)
        return 1

    print("Docs symbol lint: PASS")
    print("Violations: 0")
    return 0


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    return run(repo_root)


if __name__ == "__main__":
    sys.exit(main())
