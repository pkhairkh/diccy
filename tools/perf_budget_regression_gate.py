#!/usr/bin/env python3
"""
Performance regression gate for p95/p99 budgets derived from env-contract defaults.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any


BUDGET_RE = re.compile(
    r"route latency budgets default to qido `(\d+)/(\d+)`, wado `(\d+)/(\d+)`, stow `(\d+)/(\d+)`",
    re.IGNORECASE,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Gate sustained p95/p99 budget regressions.")
    parser.add_argument("--repo-root", default=".", help="Repository root.")
    parser.add_argument("--current-json", required=True, help="Current performance harness JSON.")
    parser.add_argument("--previous-json", help="Previous performance harness JSON for sustained checks.")
    parser.add_argument(
        "--docs-path",
        default="docs/12-API-Surface-and-Crate-Boundaries.md",
        help="Docs path containing default route latency budgets.",
    )
    parser.add_argument(
        "--output-json",
        default="reports/performance/perf-budget-regression-gate.json",
        help="Output report path.",
    )
    return parser.parse_args()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must be a JSON object")
    return payload


def parse_budgets_from_docs(path: pathlib.Path) -> dict[str, dict[str, float]]:
    text = path.read_text(encoding="utf-8")
    match = BUDGET_RE.search(text)
    if not match:
        raise ValueError(f"unable to parse latency budgets from {path}")
    qido_p95, qido_p99, wado_p95, wado_p99, stow_p95, stow_p99 = [float(value) for value in match.groups()]
    return {
        "qido": {"p95": qido_p95, "p99": qido_p99},
        "wado": {"p95": wado_p95, "p99": wado_p99},
        "stow": {"p95": stow_p95, "p99": stow_p99},
    }


def budget_breaches(report: dict[str, Any], budgets: dict[str, dict[str, float]]) -> dict[str, list[str]]:
    out: dict[str, list[str]] = {}
    workloads = report.get("workloads", [])
    if not isinstance(workloads, list):
        return out
    for row in workloads:
        if not isinstance(row, dict):
            continue
        workload_id = str(row.get("id", "unknown"))
        budget_class = str(row.get("budget_class", "")).strip().lower()
        if budget_class not in budgets:
            continue
        class_budget = budgets[budget_class]
        p95 = float(row.get("p95_latency_ms", 0.0))
        p99 = float(row.get("p99_latency_ms", 0.0))
        issues: list[str] = []
        if p95 > class_budget["p95"]:
            issues.append(
                f"p95 {p95:.3f}ms exceeds {budget_class} budget {class_budget['p95']:.3f}ms"
            )
        if p99 > class_budget["p99"]:
            issues.append(
                f"p99 {p99:.3f}ms exceeds {budget_class} budget {class_budget['p99']:.3f}ms"
            )
        if issues:
            out[workload_id] = issues
    return out


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    current_path = (repo_root / args.current_json).resolve()
    previous_path = (repo_root / args.previous_json).resolve() if args.previous_json else None
    docs_path = (repo_root / args.docs_path).resolve()
    output_json = (repo_root / args.output_json).resolve()

    budgets = parse_budgets_from_docs(docs_path)
    current_report = load_json(current_path)
    current_breaches = budget_breaches(current_report, budgets)

    previous_breaches: dict[str, list[str]] = {}
    if previous_path and previous_path.exists():
        previous_breaches = budget_breaches(load_json(previous_path), budgets)

    if previous_breaches:
        sustained = sorted(workload_id for workload_id in current_breaches if workload_id in previous_breaches)
    else:
        sustained = sorted(current_breaches.keys())

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not sustained else "FAIL",
        "budgets": budgets,
        "current_breaches": current_breaches,
        "previous_breaches": previous_breaches,
        "sustained_breaches": sustained,
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if sustained:
        print("Performance budget regression gate: FAIL")
        for workload_id in sustained:
            for issue in current_breaches.get(workload_id, []):
                print(f"{workload_id}: {issue}")
        print(f"report: {output_json}")
        return 1

    print("Performance budget regression gate: PASS")
    print(f"report: {output_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
