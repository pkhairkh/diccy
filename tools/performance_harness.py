#!/usr/bin/env python3
"""
Deterministic performance harness for release-gate evidence.

Reads a workload profile JSON, executes command-based workloads, and writes
machine-readable + markdown summary outputs.

Usage:
  python3 tools/performance_harness.py \
    --profile reports/performance/profiles/baseline-profile-RC-2026.02.11.json \
    --output-md reports/performance/baseline-results-RC-2026.02.11.md \
    --output-json reports/performance/baseline-results-RC-2026.02.11.json \
    --run-label baseline \
    --fail-on-threshold-breach
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import platform
import pathlib
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from typing import Any


RSS_RE = re.compile(r"(\d+)\s+maximum resident set size")
RSS_GNU_RE = re.compile(r"RSS_KIB=(\d+)")


@dataclass(frozen=True)
class Violation:
    workload_id: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run deterministic performance workloads.")
    parser.add_argument("--profile", required=True, help="Path to workload profile JSON.")
    parser.add_argument("--output-md", required=True, help="Output markdown report path.")
    parser.add_argument("--output-json", required=True, help="Output JSON report path.")
    parser.add_argument("--run-label", required=True, help="Label for this run.")
    parser.add_argument(
        "--fail-on-threshold-breach",
        action="store_true",
        help="Fail when any workload violates thresholds.",
    )
    return parser.parse_args()


def percentile(values: list[float], p: float) -> float:
    if not values:
        return 0.0
    if len(values) == 1:
        return values[0]
    sorted_values = sorted(values)
    rank = (len(sorted_values) - 1) * p
    low = math.floor(rank)
    high = math.ceil(rank)
    if low == high:
        return sorted_values[int(rank)]
    lower = sorted_values[low]
    upper = sorted_values[high]
    weight = rank - low
    return lower + (upper - lower) * weight


def read_profile(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text())
    if not isinstance(payload, dict):
        raise ValueError("profile must be an object")
    workloads = payload.get("workloads")
    if not isinstance(workloads, list) or not workloads:
        raise ValueError("profile must define non-empty workloads")
    return payload


def parse_rss_kib(stderr_text: str) -> int | None:
    bsd_match = RSS_RE.search(stderr_text)
    if bsd_match:
        return int(bsd_match.group(1))
    gnu_match = RSS_GNU_RE.search(stderr_text)
    if gnu_match:
        return int(gnu_match.group(1))
    return None


def instrumented_command(command: list[str], capture_rss: bool) -> tuple[list[str], bool]:
    if not capture_rss:
        return command, False
    time_bin = shutil.which("/usr/bin/time")
    if not time_bin:
        return command, False
    if platform.system() == "Darwin":
        return [time_bin, "-l", *command], True
    return [time_bin, "-f", "RSS_KIB=%M", *command], True


def run_single_command(command: list[str], capture_rss: bool) -> tuple[int, float, int | None, str, str]:
    full_cmd, rss_enabled = instrumented_command(command=command, capture_rss=capture_rss)
    start = time.perf_counter()
    proc = subprocess.run(full_cmd, check=False, capture_output=True, text=True)
    elapsed_s = time.perf_counter() - start
    rss_kib = parse_rss_kib(proc.stderr) if rss_enabled else None
    return proc.returncode, elapsed_s, rss_kib, proc.stdout, proc.stderr


def growth_pct(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    first = values[0]
    last = values[-1]
    if first <= 0:
        return 0.0
    return ((last - first) / first) * 100.0


def run_workload(workload: dict[str, Any]) -> tuple[dict[str, Any], list[Violation]]:
    required_fields = ["id", "category", "command", "iterations", "operations_per_iteration"]
    violations: list[Violation] = []
    for field in required_fields:
        if field not in workload:
            raise ValueError(f"workload missing required field: {field}")

    workload_id = str(workload["id"])
    category = str(workload["category"])
    command = workload["command"]
    if not isinstance(command, list) or not command or not all(isinstance(part, str) for part in command):
        raise ValueError(f"{workload_id}: command must be non-empty list[str]")

    iterations = int(workload["iterations"])
    operations_per_iteration = int(workload["operations_per_iteration"])
    if iterations <= 0:
        raise ValueError(f"{workload_id}: iterations must be > 0")
    if operations_per_iteration <= 0:
        raise ValueError(f"{workload_id}: operations_per_iteration must be > 0")
    think_time_ms = int(workload.get("think_time_ms", 0))
    if think_time_ms < 0:
        raise ValueError(f"{workload_id}: think_time_ms must be >= 0")
    capture_rss = bool(workload.get("capture_rss", True))

    slo_latency_ms = float(workload.get("slo_latency_p95_ms", 0.0))
    slo_latency_p99_ms = float(workload.get("slo_latency_p99_ms", 0.0))
    slo_throughput_ops_s = float(workload.get("slo_throughput_ops_s", 0.0))
    error_budget_pct = float(workload.get("error_budget_pct", 0.0))
    rss_growth_limit_pct = float(workload.get("rss_growth_limit_pct", 0.0))
    latency_growth_limit_pct = float(workload.get("latency_growth_limit_pct", 0.0))
    observed_gpu_cache_bytes = float(workload.get("observed_gpu_cache_bytes", 0.0))
    gpu_cache_budget_bytes = float(workload.get("gpu_cache_budget_bytes", 0.0))
    budget_class = str(workload.get("budget_class", ""))

    durations_ms: list[float] = []
    rss_kib_values: list[float] = []
    failures = 0
    run_records: list[dict[str, Any]] = []

    for run_index in range(1, iterations + 1):
        returncode, elapsed_s, rss_kib, _stdout, _stderr = run_single_command(command=command, capture_rss=capture_rss)
        duration_ms = elapsed_s * 1000.0
        durations_ms.append(duration_ms)
        if rss_kib is not None:
            rss_kib_values.append(float(rss_kib))
        if returncode != 0:
            failures += 1
        run_records.append(
            {
                "run": run_index,
                "returncode": returncode,
                "duration_ms": round(duration_ms, 3),
                "rss_kib": rss_kib,
            }
        )
        if think_time_ms > 0 and run_index < iterations:
            time.sleep(think_time_ms / 1000.0)

    total_duration_s = sum(durations_ms) / 1000.0
    total_operations = iterations * operations_per_iteration
    throughput_ops_s = (total_operations / total_duration_s) if total_duration_s > 0 else 0.0

    p50_ms = percentile(durations_ms, 0.50)
    p95_ms = percentile(durations_ms, 0.95)
    p99_ms = percentile(durations_ms, 0.99)
    max_ms = max(durations_ms) if durations_ms else 0.0
    error_rate_pct = (failures / iterations) * 100.0 if iterations else 0.0

    rss_growth = growth_pct(rss_kib_values) if rss_kib_values else 0.0
    latency_growth = growth_pct(durations_ms)

    if slo_latency_ms > 0 and p95_ms > slo_latency_ms:
        violations.append(Violation(workload_id, f"p95 latency {p95_ms:.2f}ms exceeds SLO {slo_latency_ms:.2f}ms"))
    if slo_latency_p99_ms > 0 and p99_ms > slo_latency_p99_ms:
        violations.append(
            Violation(
                workload_id,
                f"p99 latency {p99_ms:.2f}ms exceeds SLO {slo_latency_p99_ms:.2f}ms",
            )
        )
    if slo_throughput_ops_s > 0 and throughput_ops_s < slo_throughput_ops_s:
        violations.append(
            Violation(
                workload_id,
                f"throughput {throughput_ops_s:.2f} ops/s below SLO {slo_throughput_ops_s:.2f} ops/s",
            )
        )
    if error_rate_pct > error_budget_pct:
        violations.append(
            Violation(
                workload_id,
                f"error rate {error_rate_pct:.2f}% exceeds error budget {error_budget_pct:.2f}%",
            )
        )
    if rss_growth_limit_pct > 0 and rss_growth > rss_growth_limit_pct:
        violations.append(
            Violation(
                workload_id,
                f"rss growth {rss_growth:.2f}% exceeds limit {rss_growth_limit_pct:.2f}%",
            )
        )
    if latency_growth_limit_pct > 0 and latency_growth > latency_growth_limit_pct:
        violations.append(
            Violation(
                workload_id,
                f"latency growth {latency_growth:.2f}% exceeds limit {latency_growth_limit_pct:.2f}%",
            )
        )
    if gpu_cache_budget_bytes > 0 and observed_gpu_cache_bytes > gpu_cache_budget_bytes:
        violations.append(
            Violation(
                workload_id,
                "gpu cache bytes "
                f"{observed_gpu_cache_bytes:.0f} exceeds budget {gpu_cache_budget_bytes:.0f}",
            )
        )

    return (
        {
            "id": workload_id,
            "category": category,
            "budget_class": budget_class,
            "command": command,
            "iterations": iterations,
            "operations_per_iteration": operations_per_iteration,
            "total_operations": total_operations,
            "total_duration_s": round(total_duration_s, 3),
            "p50_latency_ms": round(p50_ms, 3),
            "p95_latency_ms": round(p95_ms, 3),
            "p99_latency_ms": round(p99_ms, 3),
            "max_latency_ms": round(max_ms, 3),
            "throughput_ops_s": round(throughput_ops_s, 3),
            "error_rate_pct": round(error_rate_pct, 3),
            "rss_growth_pct": round(rss_growth, 3),
            "latency_growth_pct": round(latency_growth, 3),
            "slo_latency_p95_ms": slo_latency_ms,
            "slo_latency_p99_ms": slo_latency_p99_ms,
            "slo_throughput_ops_s": slo_throughput_ops_s,
            "error_budget_pct": error_budget_pct,
            "rss_growth_limit_pct": rss_growth_limit_pct,
            "latency_growth_limit_pct": latency_growth_limit_pct,
            "observed_gpu_cache_bytes": observed_gpu_cache_bytes,
            "gpu_cache_budget_bytes": gpu_cache_budget_bytes,
            "runs": run_records,
        },
        violations,
    )


def write_markdown(
    output_path: pathlib.Path,
    *,
    run_label: str,
    generated_at: str,
    workload_results: list[dict[str, Any]],
    digest: str,
    violations: list[Violation],
) -> None:
    lines = [
        "# Performance Harness Report",
        "",
        f"Run Label: {run_label}",
        f"Generated At (UTC): {generated_at}",
        f"Report Digest (sha256): `{digest}`",
        "",
        "## Summary",
        "",
        f"- Workloads: {len(workload_results)}",
        f"- Violations: {len(violations)}",
        "",
        "## Workload Results",
        "",
        "| Workload ID | Category | Iterations | p95 Latency (ms) | p99 Latency (ms) | Throughput (ops/s) | Error Rate (%) | RSS Growth (%) | Latency Growth (%) | Result |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    violation_by_workload: dict[str, list[str]] = {}
    for violation in violations:
        violation_by_workload.setdefault(violation.workload_id, []).append(violation.message)

    for row in workload_results:
        wid = row["id"]
        failed = wid in violation_by_workload
        lines.append(
            "| "
            + " | ".join(
                [
                    wid,
                    row["category"],
                    str(row["iterations"]),
                    f"{row['p95_latency_ms']:.3f}",
                    f"{row['p99_latency_ms']:.3f}",
                    f"{row['throughput_ops_s']:.3f}",
                    f"{row['error_rate_pct']:.3f}",
                    f"{row['rss_growth_pct']:.3f}",
                    f"{row['latency_growth_pct']:.3f}",
                    "FAIL" if failed else "PASS",
                ]
            )
            + " |"
        )

    if violations:
        lines.extend(["", "## Violations", ""])
        for violation in violations:
            lines.append(f"- {violation.workload_id}: {violation.message}")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n")


def write_json(output_path: pathlib.Path, payload: dict[str, Any]) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def run() -> int:
    args = parse_args()
    profile_path = pathlib.Path(args.profile).resolve()
    output_md = pathlib.Path(args.output_md).resolve()
    output_json = pathlib.Path(args.output_json).resolve()

    if not profile_path.exists():
        print("Performance harness: FAIL")
        print(f"error: profile not found: {profile_path}")
        return 1

    profile = read_profile(profile_path)
    workload_defs = profile["workloads"]

    workload_results: list[dict[str, Any]] = []
    violations: list[Violation] = []
    for workload in workload_defs:
        result, result_violations = run_workload(workload)
        workload_results.append(result)
        violations.extend(result_violations)

    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    digest_material = json.dumps(workload_results, sort_keys=True, separators=(",", ":")).encode("utf-8")
    digest = hashlib.sha256(digest_material).hexdigest()

    payload = {
        "run_label": args.run_label,
        "generated_at_utc": generated_at,
        "profile_path": str(profile_path),
        "workloads": workload_results,
        "violations": [{"workload_id": v.workload_id, "message": v.message} for v in violations],
        "report_digest_sha256": digest,
    }

    write_markdown(
        output_md,
        run_label=args.run_label,
        generated_at=generated_at,
        workload_results=workload_results,
        digest=digest,
        violations=violations,
    )
    write_json(output_json, payload)

    if violations and args.fail_on_threshold_breach:
        print("Performance harness: FAIL")
        print(f"Workloads: {len(workload_results)}")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            print(f"{violation.workload_id}: {violation.message}")
        return 1

    print("Performance harness: PASS")
    print(f"Workloads: {len(workload_results)}")
    print(f"Violations: {len(violations)}")
    print(f"Digest: sha256:{digest}")
    return 0


if __name__ == "__main__":
    sys.exit(run())
