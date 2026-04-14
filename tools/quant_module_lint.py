#!/usr/bin/env python3
"""
Quantification module completeness lint.

Validates docs/26 module completeness and alignment to docs/22 claim rows.

Usage:
  python3 tools/quant_module_lint.py
  python3 tools/quant_module_lint.py --repo-root /path/to/repo
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from dataclasses import dataclass


DEFAULT_REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
DOC_26 = "docs/26-Scientific-Grade-Quantification-Validation-Program.md"
DOC_22 = "docs/22-Claim-to-Evidence-Matrix.md"


@dataclass(frozen=True)
class Violation:
    location: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lint quantification module completeness.")
    parser.add_argument(
        "--repo-root",
        default=str(DEFAULT_REPO_ROOT),
        help="Repository root (default: detected from script path).",
    )
    return parser.parse_args()


def split_table_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def parse_table(text: str, required_headers: list[str]) -> list[dict[str, str]]:
    lines = text.splitlines()
    for idx, line in enumerate(lines):
        stripped = line.strip()
        if not stripped.startswith("|"):
            continue
        header_cells = split_table_row(stripped)
        if not all(header in header_cells for header in required_headers):
            continue
        if idx + 1 >= len(lines):
            continue
        delimiter = lines[idx + 1].strip()
        if not delimiter.startswith("|"):
            continue
        rows: list[dict[str, str]] = []
        cursor = idx + 2
        while cursor < len(lines):
            row_line = lines[cursor].strip()
            if not row_line.startswith("|"):
                break
            row_cells = split_table_row(row_line)
            if len(row_cells) < len(header_cells):
                row_cells.extend([""] * (len(header_cells) - len(row_cells)))
            if len(row_cells) > len(header_cells):
                row_cells = row_cells[: len(header_cells)]
            row = dict(zip(header_cells, row_cells))
            if any(value for value in row.values()):
                rows.append(row)
            cursor += 1
        return rows
    raise ValueError(f"table with headers not found: {required_headers}")


def parse_id_list(value: str) -> list[str]:
    parts = [part.strip() for part in value.split(",")]
    return [part for part in parts if part and not part.startswith("N/A")]


def evidence_path(repo_root: pathlib.Path, evidence_id: str) -> pathlib.Path | None:
    if evidence_id.startswith("ANL-"):
        return repo_root / "reports" / "analytical" / f"{evidence_id}.md"
    if evidence_id.startswith("CLI-"):
        return repo_root / "reports" / "clinical" / f"{evidence_id}.md"
    if evidence_id.startswith("PMCF-"):
        return repo_root / "reports" / "pmcf" / f"{evidence_id}.md"
    return None


def run_lint(repo_root: pathlib.Path) -> tuple[list[Violation], int, int]:
    docs26 = (repo_root / DOC_26).read_text(errors="ignore")
    docs22 = (repo_root / DOC_22).read_text(errors="ignore")

    module_rows = parse_table(
        docs26,
        [
            "Module ID",
            "Analytical Protocol ID",
            "Clinical Protocol ID",
            "Uncertainty ID",
            "Limits-of-Use ID",
            "Evidence IDs",
            "Status",
        ],
    )
    claim_rows = parse_table(docs22, ["Claim ID", "Claim Text", "Current Status"])
    alignment_rows = parse_table(docs22, ["Module ID", "Claim IDs", "Evidence IDs", "Alignment Status"])

    claim_ids = {
        row["Claim ID"].strip()
        for row in claim_rows
        if re.match(r"^CLM-\d{3}$", row["Claim ID"].strip())
    }
    alignment_by_module = {row["Module ID"].strip(): row for row in alignment_rows}

    violations: list[Violation] = []
    active_count = 0
    deferred_count = 0

    for row in module_rows:
        module_id = row["Module ID"].strip()
        status = row["Status"].strip().lower()
        is_active = status.startswith("active")
        is_deferred = status.startswith("deferred")
        if not is_active and not is_deferred:
            violations.append(Violation(module_id, "status must be Active(...) or Deferred(...)"))
            continue
        if is_active:
            active_count += 1
        if is_deferred:
            deferred_count += 1

        if not re.match(r"^QNT-\d{3}$", module_id):
            violations.append(Violation(module_id, "invalid module id format"))

        if is_active:
            for field in (
                "Analytical Protocol ID",
                "Clinical Protocol ID",
                "Uncertainty ID",
                "Limits-of-Use ID",
            ):
                value = row[field].strip()
                if not value or value.startswith("N/A"):
                    violations.append(Violation(module_id, f"missing required field: {field}"))

            evidence_ids = parse_id_list(row["Evidence IDs"])
            if not evidence_ids:
                violations.append(Violation(module_id, "active module must include evidence IDs"))
                evidence_ids = []

            required_prefixes = {"ANL-", "CLI-", "PMCF-"}
            seen_prefixes = set()
            for evidence_id in evidence_ids:
                if not re.match(r"^(ANL|CLI|PMCF)-\d{3}$", evidence_id):
                    violations.append(Violation(module_id, f"invalid evidence id: {evidence_id}"))
                    continue
                path = evidence_path(repo_root, evidence_id)
                if path is None or not path.exists():
                    violations.append(Violation(module_id, f"missing evidence artifact: {evidence_id}"))
                    continue
                if evidence_id.startswith("ANL-"):
                    seen_prefixes.add("ANL-")
                elif evidence_id.startswith("CLI-"):
                    seen_prefixes.add("CLI-")
                elif evidence_id.startswith("PMCF-"):
                    seen_prefixes.add("PMCF-")
            missing_prefixes = required_prefixes - seen_prefixes
            if missing_prefixes:
                violations.append(
                    Violation(module_id, f"active module missing evidence class(es): {', '.join(sorted(missing_prefixes))}")
                )

            alignment = alignment_by_module.get(module_id)
            if alignment is None:
                violations.append(Violation(module_id, "missing module claim alignment row in docs/22"))
                continue

            claim_id_list = parse_id_list(alignment["Claim IDs"])
            if not claim_id_list:
                violations.append(Violation(module_id, "active module must map to at least one claim ID"))
            for claim_id in claim_id_list:
                if claim_id not in claim_ids:
                    violations.append(Violation(module_id, f"unknown claim id in alignment: {claim_id}"))

            aligned_evidence_ids = parse_id_list(alignment["Evidence IDs"])
            for evidence_id in aligned_evidence_ids:
                if evidence_id not in evidence_ids:
                    violations.append(
                        Violation(
                            module_id,
                            f"alignment evidence id not present in module evidence list: {evidence_id}",
                        )
                    )
        else:
            if module_id in alignment_by_module:
                claim_id_list = parse_id_list(alignment_by_module[module_id]["Claim IDs"])
                if claim_id_list:
                    violations.append(
                        Violation(module_id, "deferred module must not map to active claim IDs")
                    )

    return violations, active_count, deferred_count


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    try:
        violations, active_count, deferred_count = run_lint(repo_root)
    except Exception as exc:  # pragma: no cover - defensive top-level fail-closed behavior
        print(f"Quant module lint: FAIL")
        print(f"error: {exc}")
        return 1

    if violations:
        print("Quant module lint: FAIL")
        print(f"Active modules: {active_count}")
        print(f"Deferred modules: {deferred_count}")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            print(f"{violation.location}: {violation.message}")
        return 1

    print("Quant module lint: PASS")
    print(f"Active modules: {active_count}")
    print(f"Deferred modules: {deferred_count}")
    print("Violations: 0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
