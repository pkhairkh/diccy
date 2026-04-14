#!/usr/bin/env python3
"""
Resource budget gate for CPU/memory/GPU usage from performance harness outputs.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Resource budget gate.")
    parser.add_argument("--repo-root", default=".", help="Repository root.")
    parser.add_argument("--report-json", required=True, help="Performance harness JSON report.")
    parser.add_argument(
        "--budget-json",
        default="reports/performance/resource-budget-baseline.json",
        help="Resource budget baseline JSON.",
    )
    parser.add_argument(
        "--output-json",
        default="reports/performance/resource-budget-gate.json",
        help="Output report path.",
    )
    return parser.parse_args()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must be a JSON object")
    return payload


def resolve_budget(table: dict[str, Any], workload_id: str) -> float:
    raw = table.get(workload_id, table.get("default", 0.0))
    return float(raw)


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    report_path = (repo_root / args.report_json).resolve()
    budget_path = (repo_root / args.budget_json).resolve()
    output_json = (repo_root / args.output_json).resolve()

    report = load_json(report_path)
    budget = load_json(budget_path)
    cpu_budget = budget.get("cpu_p95_latency_ms", {})
    mem_budget = budget.get("memory_rss_growth_pct", {})
    gpu_budget = budget.get("gpu_cache_bytes", {})
    if not isinstance(cpu_budget, dict) or not isinstance(mem_budget, dict) or not isinstance(gpu_budget, dict):
        raise ValueError("budget tables must be objects")

    findings: list[str] = []
    for row in report.get("workloads", []):
        if not isinstance(row, dict):
            continue
        workload_id = str(row.get("id", "unknown"))
        p95 = float(row.get("p95_latency_ms", 0.0))
        rss_growth = float(row.get("rss_growth_pct", 0.0))
        observed_gpu = float(row.get("observed_gpu_cache_bytes", 0.0))

        cpu_limit = resolve_budget(cpu_budget, workload_id)
        mem_limit = resolve_budget(mem_budget, workload_id)
        gpu_limit = resolve_budget(gpu_budget, workload_id)

        if cpu_limit > 0 and p95 > cpu_limit:
            findings.append(
                f"{workload_id}: cpu p95 {p95:.3f}ms exceeds budget {cpu_limit:.3f}ms"
            )
        if mem_limit > 0 and rss_growth > mem_limit:
            findings.append(
                f"{workload_id}: memory rss growth {rss_growth:.3f}% exceeds budget {mem_limit:.3f}%"
            )
        if gpu_limit > 0 and observed_gpu > gpu_limit:
            findings.append(
                f"{workload_id}: gpu cache bytes {observed_gpu:.0f} exceeds budget {gpu_limit:.0f}"
            )

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not findings else "FAIL",
        "findings": findings,
        "report_path": str(report_path),
        "budget_path": str(budget_path),
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if findings:
        print("Resource budget gate: FAIL")
        for item in findings:
            print(f" - {item}")
        print(f"report: {output_json}")
        return 1

    print("Resource budget gate: PASS")
    print(f"report: {output_json}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
