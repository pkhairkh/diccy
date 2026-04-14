#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys


NON_PRIMITIVE_MARKERS = (
    "/components/components/",
    "/components/composites/",
    "/components/panels/",
)
SOURCE_SUFFIXES = (".vue", ".ts", ".tsx", ".js", ".jsx")
CLASS_ATTR_PATTERN = re.compile(r'class\s*=\s*"([^"]+)"')
STYLE_ATTR_PATTERN = re.compile(r'(^|\s)(:style|style)\s*=')


def is_non_primitive(path: pathlib.Path) -> bool:
    normalized = path.as_posix().lower()
    return any(marker in normalized for marker in NON_PRIMITIVE_MARKERS)


def tokenize_class_value(value: str) -> list[str]:
    return [token for token in value.strip().split() if token]


def scan_file(path: pathlib.Path) -> list[dict[str, object]]:
    violations: list[dict[str, object]] = []
    for line_no, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1):
        for match in CLASS_ATTR_PATTERN.finditer(line):
            tokens = tokenize_class_value(match.group(1))
            if len(tokens) > 1:
                violations.append(
                    {
                        "rule": "raw_multi_token_class",
                        "line": line_no,
                        "excerpt": line.strip(),
                    }
                )
        if STYLE_ATTR_PATTERN.search(line):
            violations.append(
                {
                    "rule": "style_attribute_non_primitive",
                    "line": line_no,
                    "excerpt": line.strip(),
                }
            )
    return violations


def parse_style_exceptions(repo_root: pathlib.Path) -> set[str]:
    issues = repo_root / "FRONTEND_ISSUES.md"
    if not issues.exists():
        return set()
    approved: set[str] = set()
    for line in issues.read_text(encoding="utf-8", errors="replace").splitlines():
        if not line.startswith("| FE-"):
            continue
        parts = [part.strip() for part in line.split("|") if part.strip()]
        if len(parts) < 4:
            continue
        file_path = parts[2]
        pattern = parts[3]
        if ":style" in pattern or "style" in pattern:
            approved.add((repo_root / file_path).resolve().as_posix())
    return approved


def scan(frontend_root: pathlib.Path, style_exceptions: set[str]) -> list[dict[str, object]]:
    if not frontend_root.exists():
        return []
    findings: list[dict[str, object]] = []
    for path in sorted(frontend_root.rglob("*")):
        if not path.is_file() or path.suffix not in SOURCE_SUFFIXES:
            continue
        if not is_non_primitive(path):
            continue
        violations = scan_file(path)
        if path.resolve().as_posix() in style_exceptions:
            violations = [v for v in violations if v["rule"] != "style_attribute_non_primitive"]
        if not violations:
            continue
        findings.append(
            {
                "file": path.as_posix(),
                "violations": violations,
            }
        )
    return findings


def group_by_rule(findings: list[dict[str, object]]) -> dict[str, list[str]]:
    grouped: dict[str, list[str]] = {}
    for finding in findings:
        file_path = str(finding["file"])
        for violation in finding["violations"]:
            rule = str(violation["rule"])
            grouped.setdefault(rule, []).append(file_path)
    for key in grouped:
        grouped[key] = sorted(set(grouped[key]))
    return grouped


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Lint non-primitive frontend files for class-policy violations and emit grouped output."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root path")
    parser.add_argument(
        "--frontend-root",
        default="frontend/src",
        help="Frontend source root to scan for non-primitive files",
    )
    parser.add_argument(
        "--report",
        default="reports/docs/frontend-class-policy-report.json",
        help="Machine-readable report output path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Exit non-zero when violations are present",
    )
    args = parser.parse_args()

    repo_root = pathlib.Path(args.repo_root).resolve()
    frontend_root = (repo_root / args.frontend_root).resolve()
    style_exceptions = parse_style_exceptions(repo_root)
    findings = scan(frontend_root, style_exceptions=style_exceptions)
    grouped = group_by_rule(findings)
    payload = {
        "repo_root": str(repo_root),
        "frontend_root": str(frontend_root),
        "violation_file_count": len(findings),
        "violations_grouped_by_rule": grouped,
        "files": findings,
    }

    report_path = repo_root / args.report
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print("Frontend class-policy lint report")
    print(f"Frontend root: {frontend_root.relative_to(repo_root) if frontend_root.exists() else args.frontend_root}")
    if findings:
        print(f"Violation files: {len(findings)}")
        for rule, files in sorted(grouped.items()):
            print(f" - {rule}: {len(files)} file(s)")
            for file_path in files:
                print(f"   - {pathlib.Path(file_path).relative_to(repo_root)}")
    else:
        print("Violation files: 0")
    print(f"Report: {report_path.relative_to(repo_root)}")

    if args.check and findings:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
