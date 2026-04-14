#!/usr/bin/env python3
"""
Deterministic security fuzz campaign runner.

Runs selected fuzz targets in offline mode with bounded runs and emits:
- markdown campaign report
- JSON summary
- markdown crash inventory

This runner is designed for release evidence generation when `cargo-fuzz`
tooling is unavailable in the environment.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from dataclasses import dataclass
from typing import Any


DEFAULT_TARGETS = [
    "dicom_io_p10",
    "dimse_pdu",
    "dimse_command",
    "dicomweb_request",
    "dicom_pixel_pipeline",
    "dicom_pixel_rle",
    "dicom_pixel_jpeg_baseline",
]

SEED_RE = re.compile(r"INFO:\s+(\d+)\s+files found")
DONE_RE = re.compile(r"Done\s+(\d+)\s+runs")
RSS_RE = re.compile(r"rss:\s*(\d+)Mb")

CRASH_MARKERS = [
    "AddressSanitizer",
    "UndefinedBehaviorSanitizer",
    "libFuzzer: deadly signal",
    "ERROR: libFuzzer",
    "thread 'main' panicked at",
]


@dataclass(frozen=True)
class TargetRun:
    target: str
    command: list[str]
    returncode: int
    seeds: int | None
    runs: int | None
    rss_mb: int | None
    corpus_path: str | None
    has_crash: bool
    has_coverage_warning: bool
    log_path: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run deterministic security fuzz campaign.")
    parser.add_argument(
        "--manifest-path",
        default="fuzz/Cargo.toml",
        help="Path to fuzz Cargo.toml manifest.",
    )
    parser.add_argument(
        "--fuzz-corpus-root",
        default="fuzz/corpus",
        help="Root path containing per-target corpus directories.",
    )
    parser.add_argument(
        "--target",
        action="append",
        dest="targets",
        help="Fuzz target to run; can be repeated. Defaults to curated release set.",
    )
    parser.add_argument(
        "--runs-per-target",
        type=int,
        default=128,
        help="Bounded libFuzzer run count per target.",
    )
    parser.add_argument(
        "--output-md",
        required=True,
        help="Path to markdown campaign report output.",
    )
    parser.add_argument(
        "--output-json",
        required=True,
        help="Path to JSON campaign summary output.",
    )
    parser.add_argument(
        "--output-crash-md",
        required=True,
        help="Path to markdown crash inventory output.",
    )
    parser.add_argument(
        "--log-dir",
        required=True,
        help="Directory to store per-target execution logs.",
    )
    parser.add_argument(
        "--run-label",
        required=True,
        help="Release run label to embed in artifacts.",
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="Run cargo commands with --offline.",
    )
    parser.add_argument(
        "--fail-on-crash",
        action="store_true",
        help="Return non-zero when a target crash is detected.",
    )
    return parser.parse_args()


def detect_crash(text: str, returncode: int) -> bool:
    if returncode != 0:
        return True
    for marker in CRASH_MARKERS:
        if marker in text:
            return True
    return False


def parse_seed_count(text: str) -> int | None:
    match = SEED_RE.search(text)
    return int(match.group(1)) if match else None


def parse_done_runs(text: str) -> int | None:
    match = DONE_RE.search(text)
    return int(match.group(1)) if match else None


def parse_rss_mb(text: str) -> int | None:
    matches = RSS_RE.findall(text)
    if not matches:
        return None
    return int(matches[-1])


def run_target(
    *,
    manifest_path: pathlib.Path,
    corpus_root: pathlib.Path,
    target: str,
    runs_per_target: int,
    offline: bool,
    log_dir: pathlib.Path,
) -> TargetRun:
    corpus_dir = corpus_root / target
    command = ["cargo", "run"]
    if offline:
        command.append("--offline")
    command.extend(
        [
            "--manifest-path",
            str(manifest_path),
            "--bin",
            target,
            "--",
            f"-runs={runs_per_target}",
        ]
    )
    corpus_path: str | None = None
    if corpus_dir.exists() and corpus_dir.is_dir():
        command.append(str(corpus_dir))
        corpus_path = str(corpus_dir)

    proc = subprocess.run(command, check=False, capture_output=True, text=True)
    combined = (proc.stdout or "") + (proc.stderr or "")

    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / f"{target}.log"
    log_path.write_text(combined)

    return TargetRun(
        target=target,
        command=command,
        returncode=proc.returncode,
        seeds=parse_seed_count(combined),
        runs=parse_done_runs(combined),
        rss_mb=parse_rss_mb(combined),
        corpus_path=corpus_path,
        has_crash=detect_crash(combined, proc.returncode),
        has_coverage_warning="no interesting inputs were found so far" in combined,
        log_path=str(log_path),
    )


def to_serializable(run: TargetRun) -> dict[str, Any]:
    return {
        "target": run.target,
        "command": run.command,
        "returncode": run.returncode,
        "seeds": run.seeds,
        "runs": run.runs,
        "rss_mb": run.rss_mb,
        "corpus_path": run.corpus_path,
        "has_crash": run.has_crash,
        "has_coverage_warning": run.has_coverage_warning,
        "log_path": run.log_path,
    }


def write_markdown(
    output_path: pathlib.Path,
    *,
    run_label: str,
    generated_at: str,
    digest: str,
    runs: list[TargetRun],
) -> None:
    crash_count = sum(1 for run in runs if run.has_crash)
    coverage_warn_count = sum(1 for run in runs if run.has_coverage_warning)
    lines = [
        "# Security Fuzz Campaign Report",
        "",
        f"Run Label: {run_label}",
        f"Generated At (UTC): {generated_at}",
        f"Report Digest (sha256): `{digest}`",
        "",
        "## Summary",
        "",
        f"- Targets: {len(runs)}",
        f"- Crash findings: {crash_count}",
        f"- Coverage warnings: {coverage_warn_count}",
        "",
        "## Target Results",
        "",
        "| Target | Return Code | Seeds | Runs | RSS (MB) | Crash | Coverage Warning | Log |",
        "|---|---:|---:|---:|---:|---|---|---|",
    ]
    for run in runs:
        lines.append(
            "| "
            + " | ".join(
                [
                    run.target,
                    str(run.returncode),
                    str(run.seeds) if run.seeds is not None else "N/A",
                    str(run.runs) if run.runs is not None else "N/A",
                    str(run.rss_mb) if run.rss_mb is not None else "N/A",
                    "YES" if run.has_crash else "NO",
                    "YES" if run.has_coverage_warning else "NO",
                    f"`{run.log_path}`",
                ]
            )
            + " |"
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n")


def write_crash_inventory(output_path: pathlib.Path, runs: list[TargetRun]) -> None:
    crashes = [run for run in runs if run.has_crash]
    lines = [
        "# Security Fuzz Crash Inventory",
        "",
        f"Total crash findings: {len(crashes)}",
        "",
    ]
    if not crashes:
        lines.extend(
            [
                "No crashes detected in this campaign.",
                "",
            ]
        )
    else:
        lines.extend(
            [
                "| Target | Severity | Return Code | Log |",
                "|---|---|---:|---|",
            ]
        )
        for run in crashes:
            lines.append(
                "| "
                + " | ".join(
                    [
                        run.target,
                        "Critical",
                        str(run.returncode),
                        f"`{run.log_path}`",
                    ]
                )
                + " |"
            )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n")


def write_json(output_path: pathlib.Path, payload: dict[str, Any]) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def run_campaign() -> int:
    args = parse_args()
    if args.runs_per_target <= 0:
        print("Security fuzz campaign: FAIL")
        print("error: runs-per-target must be > 0")
        return 1

    manifest_path = pathlib.Path(args.manifest_path).resolve()
    if not manifest_path.exists():
        print("Security fuzz campaign: FAIL")
        print(f"error: manifest not found: {manifest_path}")
        return 1
    corpus_root = pathlib.Path(args.fuzz_corpus_root).resolve()
    output_md = pathlib.Path(args.output_md).resolve()
    output_json = pathlib.Path(args.output_json).resolve()
    output_crash_md = pathlib.Path(args.output_crash_md).resolve()
    log_dir = pathlib.Path(args.log_dir).resolve()

    targets = args.targets if args.targets else DEFAULT_TARGETS
    runs = [
        run_target(
            manifest_path=manifest_path,
            corpus_root=corpus_root,
            target=target,
            runs_per_target=args.runs_per_target,
            offline=args.offline,
            log_dir=log_dir,
        )
        for target in targets
    ]
    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    digest_input = json.dumps([to_serializable(run) for run in runs], sort_keys=True).encode("utf-8")
    digest = hashlib.sha256(digest_input).hexdigest()

    payload = {
        "run_label": args.run_label,
        "generated_at_utc": generated_at,
        "manifest_path": str(manifest_path),
        "corpus_root": str(corpus_root),
        "targets": [to_serializable(run) for run in runs],
        "crash_count": sum(1 for run in runs if run.has_crash),
        "coverage_warning_count": sum(1 for run in runs if run.has_coverage_warning),
        "report_digest_sha256": digest,
    }
    write_markdown(output_md, run_label=args.run_label, generated_at=generated_at, digest=digest, runs=runs)
    write_crash_inventory(output_crash_md, runs)
    write_json(output_json, payload)

    crash_count = payload["crash_count"]
    print("Security fuzz campaign: PASS" if crash_count == 0 else "Security fuzz campaign: FAIL")
    print(f"Targets: {len(runs)}")
    print(f"Crash findings: {crash_count}")
    print(f"Coverage warnings: {payload['coverage_warning_count']}")
    print(f"Digest: sha256:{digest}")
    if args.fail_on_crash and crash_count > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(run_campaign())
