#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass

from runtime_env_contract_lib import extract_runtime_env_contract, find_line


DENYLIST_FOR_IMPLEMENTED = (
    "remain deferred until implementation lands",
    "deferred until implementation",
    "is not implemented",
)

DEPLOYMENT_CLAIM_FILES = {
    "README.md",
    "docs/12-API-Surface-and-Crate-Boundaries.md",
    "docs/33-Productization-Profiles-and-Playbooks.md",
    "docs/40-Reference-Deployment-Topology.md",
}

DEPLOYMENT_CLAIM_SERVICES = (
    "dicom-dimse-service",
    "dicom-web-server",
    "dicom-workflow-server",
    "dicom-visualizer",
    "viewer-wasm",
    "viewer-wgpu",
)

DEPLOYMENT_CLAIM_AMBIGUOUS_TERMS = (
    " optional ",
    " included ",
    "standalone",
    "runtime-integrated",
    "runtime integrated",
)

DEPLOYMENT_CLAIM_MARKERS = (
    "backend-services-with-dimse",
    "backend-services",
    "workstation",
    "framework-core",
    "dist/profiles/",
    "artifact",
    "profile",
    "package_profiles.sh",
)

DEFERRED_WHITELIST = (
    "deferred",
    "planned",
    "target architecture",
    "not active in current release baseline",
)


@dataclass(frozen=True)
class SymbolStatus:
    symbol: str
    status: str


def parse_symbol_statuses(path: pathlib.Path) -> list[SymbolStatus]:
    rows: list[SymbolStatus] = []
    if not path.exists():
        return rows

    row_pattern = re.compile(r"^\|\s*`([^`]+)`\s*\|\s*([^|]+?)\s*\|")
    for raw in path.read_text(encoding="utf-8").splitlines():
        match = row_pattern.match(raw.strip())
        if not match:
            continue
        symbol = match.group(1).strip()
        status = match.group(2).strip().lower()
        if status in {"implemented", "deferred", "planned", "partially implemented"}:
            rows.append(SymbolStatus(symbol=symbol, status=status))
    return rows


def markdown_targets(repo_root: pathlib.Path) -> list[pathlib.Path]:
    docs_dir = repo_root / "docs"
    files = [repo_root / "README.md"]
    if docs_dir.exists():
        files.extend(sorted(docs_dir.glob("*.md")))
    return [p for p in files if p.exists() and not p.name.startswith("._")]


def tokenize_symbol(symbol: str) -> tuple[str, str]:
    return (f"`{symbol}`", symbol)


def add_env_var_contract_violations(
    repo_root: pathlib.Path,
    violations: list[dict[str, object]],
) -> list[dict[str, object]]:
    contracts = extract_runtime_env_contract(repo_root)

    for contract in contracts:
        docs_file = contract["docs_file"]
        docs_path = repo_root / str(docs_file)
        component = contract["component"]

        heading = str(contract["docs_heading"])
        heading_line = find_line(docs_path, heading) or 1

        for env_var in contract["missing_in_docs"]:
            violations.append(
                {
                    "file": docs_file,
                    "line": heading_line,
                    "symbol": env_var,
                    "status": "contract",
                    "kind": "env_var_missing_in_docs",
                    "message": (
                        f"runtime env var '{env_var}' for {component} is not documented in {docs_file}"
                    ),
                    "text": heading,
                }
            )

        for env_var in contract["documented_not_in_code"]:
            line = find_line(docs_path, env_var) or heading_line
            violations.append(
                {
                    "file": docs_file,
                    "line": line,
                    "symbol": env_var,
                    "status": "contract",
                    "kind": "env_var_documented_not_in_code",
                    "message": (
                        f"documented env var '{env_var}' for {component} is not found in runtime code contract"
                    ),
                    "text": env_var,
                }
            )

    return contracts


def add_docs_only_contract_guardrail_violations(
    contracts: list[dict[str, object]],
    violations: list[dict[str, object]],
) -> None:
    for contract in contracts:
        missing = contract["missing_in_docs"]
        extra = contract["documented_not_in_code"]
        if not missing and not extra:
            continue
        docs_file = str(contract["docs_file"])
        heading = str(contract["docs_heading"])
        violations.append(
            {
                "file": docs_file,
                "line": 1,
                "symbol": "env-contract-guardrail",
                "status": "guardrail",
                "kind": "docs_only_env_contract_change",
                "message": (
                    f"guardrail: docs-only env-contract update detected for {contract['component']}; update parser/runtime contract code alongside docs"
                ),
                "text": heading,
            }
        )


def add_deployment_claim_violations(
    repo_root: pathlib.Path,
    scanned: list[pathlib.Path],
    violations: list[dict[str, object]],
) -> None:
    for file_path in scanned:
        rel = file_path.relative_to(repo_root).as_posix()
        if rel not in DEPLOYMENT_CLAIM_FILES:
            continue

        for line_no, raw_line in enumerate(
            file_path.read_text(encoding="utf-8", errors="replace").splitlines(),
            start=1,
        ):
            line = raw_line.lower()
            if not any(term in line for term in DEPLOYMENT_CLAIM_SERVICES):
                continue
            if not any(token in line for token in DEPLOYMENT_CLAIM_AMBIGUOUS_TERMS):
                continue
            if any(marker in line for marker in DEPLOYMENT_CLAIM_MARKERS):
                continue
            violations.append(
                {
                    "file": rel,
                    "line": line_no,
                    "symbol": "deployment-posture",
                    "status": "docs-compliance",
                    "kind": "deployment_posture_ambiguity",
                    "message": (
                        "deployment claim uses ambiguous status wording without explicit profile/artifact anchor"
                    ),
                    "text": raw_line.strip(),
                }
            )


def lint(repo_root: pathlib.Path) -> tuple[list[dict[str, object]], list[pathlib.Path], list[dict[str, object]]]:
    status_path = repo_root / "docs" / "31-Implementation-Status.md"
    symbol_rows = parse_symbol_statuses(status_path)
    implemented = [row.symbol for row in symbol_rows if row.status == "implemented"]
    deferred = [row.symbol for row in symbol_rows if row.status == "deferred"]

    violations: list[dict[str, object]] = []
    scanned = markdown_targets(repo_root)

    for file_path in scanned:
        rel = file_path.relative_to(repo_root).as_posix()
        if rel == "docs/31-Implementation-Status.md":
            continue

        lines = file_path.read_text(encoding="utf-8", errors="replace").splitlines()
        for index, line in enumerate(lines, start=1):
            low = line.lower()

            for symbol in implemented:
                if not any(token in line for token in tokenize_symbol(symbol)):
                    continue
                for phrase in DENYLIST_FOR_IMPLEMENTED:
                    if phrase in low:
                        violations.append(
                            {
                                "file": rel,
                                "line": index,
                                "symbol": symbol,
                                "status": "implemented",
                                "kind": "implemented_symbol_deferred_claim",
                                "message": (
                                    f"implemented symbol '{symbol}' appears with contradiction phrase '{phrase}'"
                                ),
                                "text": line.strip(),
                            }
                        )
                        break

            for symbol in deferred:
                if not any(token in line for token in tokenize_symbol(symbol)):
                    continue
                if "implemented" in low and not any(p in low for p in DEFERRED_WHITELIST):
                    violations.append(
                        {
                            "file": rel,
                            "line": index,
                            "symbol": symbol,
                            "status": "deferred",
                            "kind": "deferred_symbol_implemented_claim",
                            "message": (
                                f"deferred symbol '{symbol}' appears with implementation claim"
                            ),
                            "text": line.strip(),
                        }
                    )

    contracts = add_env_var_contract_violations(repo_root, violations)
    add_docs_only_contract_guardrail_violations(contracts, violations)
    add_deployment_claim_violations(repo_root, scanned, violations)
    return violations, scanned, contracts


def main() -> int:
    parser = argparse.ArgumentParser(description="Detect docs drift vs docs/31 symbol statuses")
    parser.add_argument("--repo-root", default=".", help="Repository root path")
    parser.add_argument(
        "--report",
        default="reports/docs/drift-report.json",
        help="Path for machine-readable report artifact",
    )
    args = parser.parse_args()

    root = pathlib.Path(args.repo_root).resolve()
    report_path = root / args.report

    violations, scanned, contracts = lint(root)
    payload = {
        "repo_root": str(root),
        "scanned_markdown_files": [p.relative_to(root).as_posix() for p in scanned],
        "violation_count": len(violations),
        "violations": violations,
        "denylist_for_implemented": list(DENYLIST_FOR_IMPLEMENTED),
        "deferred_whitelist": list(DEFERRED_WHITELIST),
        "runtime_env_contracts": contracts,
    }

    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if violations:
        print("Docs drift lint: FAIL")
        print(f"Violations: {len(violations)}")
        for v in violations:
            print(
                f" - {v['file']}:{v['line']} [{v['kind']}] {v['message']}"
            )
        print(f"Report: {report_path.relative_to(root)}")
        return 1

    print("Docs drift lint: PASS")
    print(f"Scanned markdown files: {len(scanned)}")
    print("Violations: 0")
    print(f"Report: {report_path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
