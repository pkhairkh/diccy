#!/usr/bin/env python3
"""
Deterministic release preflight runner.

Runs release-critical gate commands in a fixed order, writes per-gate logs, and
emits markdown+JSON summary artifacts for GO/NO-GO governance.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import shlex
import subprocess
import sys
import time
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run deterministic release preflight gates.")
    parser.add_argument("--release-id", required=True, help="Release identifier (for example RC-2026.02.11).")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--gates-dir",
        default="reports/analytical/gates",
        help="Directory where gate logs are written.",
    )
    parser.add_argument(
        "--summary-md",
        help="Optional markdown summary output path. Default is reports/release/release-preflight-summary-<release-id>.md.",
    )
    parser.add_argument(
        "--summary-json",
        help="Optional JSON summary output path. Default is reports/release/release-preflight-summary-<release-id>.json.",
    )
    parser.add_argument(
        "--fail-fast",
        action="store_true",
        help="Stop at first gate failure.",
    )
    return parser.parse_args()


def build_gate_plan(release_id: str) -> list[dict[str, Any]]:
    return [
        {
            "id": "cargo-fmt",
            "description": "Rust format gate.",
            "command": ["cargo", "fmt", "--all"],
        },
        {
            "id": "cargo-build",
            "description": "Rust build gate.",
            "command": ["cargo", "build"],
        },
        {
            "id": "cargo-test",
            "description": "Rust test gate.",
            "command": ["cargo", "test"],
        },
        {
            "id": "cargo-test-workflow-main-tests",
            "description": "Workflow feature-gated test gate.",
            "command": ["cargo", "test", "-p", "dicom-workflow-server", "--features", "workflow-main-tests"],
        },
        {
            "id": "cargo-clippy",
            "description": "Rust lint gate.",
            "command": ["cargo", "clippy", "--all-targets", "--all-features", "--", "-D", "warnings"],
        },
        {
            "id": "claim-surface",
            "description": "Claim-surface lint gate.",
            "command": ["python3", "tools/claim_surface_lint.py"],
        },
        {
            "id": "req-completeness",
            "description": "REQ completeness lint gate.",
            "command": [
                "python3",
                "tools/req_completeness_lint.py",
                "--docs-dir",
                "docs",
                "--baseline",
                f"reports/traceability/req-completeness-baseline-{release_id}.json",
                "--output-json",
                f"reports/traceability/req-completeness-lint-summary-{release_id}.json",
            ],
        },
        {
            "id": "traceability",
            "description": "Traceability linkage gate.",
            "command": [
                "python3",
                "tools/traceability_report.py",
                "--linkage-manifest",
                f"reports/traceability/release-traceability-manifest-{release_id}.json",
                "--baseline-json",
                f"reports/traceability/traceability-baseline-{release_id}.json",
                "--output-md",
                f"reports/traceability/traceability-snapshot-{release_id}.md",
                "--output-json",
                f"reports/traceability/traceability-snapshot-{release_id}.json",
                "--fail-on-invalid-links",
            ],
        },
        {
            "id": "determinism-manifest",
            "description": "Determinism manifest verification gate.",
            "command": [
                "python3",
                "tools/generate_determinism_manifest.py",
                "--release-id",
                release_id,
                "--profile-id",
                "cpu-oracle-baseline-v1",
                "--output",
                f"reports/analytical/determinism-manifest-{release_id}.toml",
                "--verify-existing",
            ],
        },
        {
            "id": "cross-target-matrix",
            "description": "Cross-target reproducibility matrix gate.",
            "command": [
                "python3",
                "tools/reproducibility_compare.py",
                "--manifest",
                f"arm64=reports/analytical/reproducibility/actual-aarch64-apple-darwin-{release_id}.toml",
                "--manifest",
                f"x86_64-projected=reports/analytical/reproducibility/projected-x86_64-unknown-linux-gnu-{release_id}.toml",
                "--manifest",
                f"wasm32-projected=reports/analytical/reproducibility/projected-wasm32-unknown-unknown-{release_id}.toml",
                "--output-md",
                f"reports/analytical/reproducibility/matrix-{release_id}.md",
                "--output-json",
                f"reports/analytical/reproducibility/matrix-{release_id}.json",
                "--mismatch-budget",
                "0",
            ],
        },
        {
            "id": "artifact-integrity",
            "description": "Release artifact integrity gate.",
            "command": [
                "python3",
                "tools/release_artifact_integrity.py",
                "--release-id",
                release_id,
                "--output-md",
                f"reports/release/release-artifact-integrity-{release_id}.md",
                "--output-json",
                f"reports/release/release-artifact-integrity-{release_id}.json",
                "--fail-on-errors",
            ],
        },
    ]


def run_gate(
    *,
    repo_root: pathlib.Path,
    release_id: str,
    gate: dict[str, Any],
    gates_dir: pathlib.Path,
) -> dict[str, Any]:
    command: list[str] = gate["command"]
    gate_id = str(gate["id"])
    started_at = dt.datetime.now(dt.timezone.utc)
    start = time.monotonic()
    completed = subprocess.run(
        command,
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    duration_seconds = round(time.monotonic() - start, 3)
    finished_at = dt.datetime.now(dt.timezone.utc)

    log_path = gates_dir / f"s15-preflight-{gate_id}-{release_id}.log"
    log_lines = [
        f"gate_id: {gate_id}",
        f"description: {gate['description']}",
        f"release_id: {release_id}",
        f"started_at_utc: {started_at.strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"finished_at_utc: {finished_at.strftime('%Y-%m-%dT%H:%M:%SZ')}",
        f"duration_seconds: {duration_seconds}",
        f"command: {' '.join(shlex.quote(part) for part in command)}",
        f"exit_code: {completed.returncode}",
        "",
        "[stdout]",
        completed.stdout,
        "[stderr]",
        completed.stderr,
    ]
    log_path.parent.mkdir(parents=True, exist_ok=True)
    log_path.write_text("\n".join(log_lines))

    return {
        "gate_id": gate_id,
        "description": gate["description"],
        "command": command,
        "status": "PASS" if completed.returncode == 0 else "FAIL",
        "exit_code": completed.returncode,
        "duration_seconds": duration_seconds,
        "log_path": str(log_path.relative_to(repo_root)),
    }


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Release Preflight Summary",
        "",
        f"Release Candidate: {payload['release_id']}",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        f"Overall Status: **{payload['overall_status']}**",
        "",
        "## Gate Outcomes",
        "",
        "| Gate ID | Status | Exit Code | Duration (s) | Log Path |",
        "|---|---|---:|---:|---|",
    ]
    for gate in payload["gates"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    gate["gate_id"],
                    gate["status"],
                    str(gate["exit_code"]),
                    f"{gate['duration_seconds']:.3f}",
                    f"`{gate['log_path']}`",
                ]
            )
            + " |"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def run() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    gates_dir = (repo_root / args.gates_dir).resolve()
    release_id = args.release_id
    summary_md = (
        pathlib.Path(args.summary_md).resolve()
        if args.summary_md
        else (repo_root / f"reports/release/release-preflight-summary-{release_id}.md").resolve()
    )
    summary_json = (
        pathlib.Path(args.summary_json).resolve()
        if args.summary_json
        else (repo_root / f"reports/release/release-preflight-summary-{release_id}.json").resolve()
    )

    if not repo_root.exists():
        print("Release preflight: FAIL")
        print(f"error: repo root not found: {repo_root}")
        return 1

    gates = build_gate_plan(release_id)
    results: list[dict[str, Any]] = []
    for gate in gates:
        result = run_gate(repo_root=repo_root, release_id=release_id, gate=gate, gates_dir=gates_dir)
        results.append(result)
        if args.fail_fast and result["status"] != "PASS":
            break

    failing = [result for result in results if result["status"] != "PASS"]
    payload = {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "overall_status": "PASS" if not failing else "FAIL",
        "gate_count": len(results),
        "failed_gate_count": len(failing),
        "gates": results,
    }
    write_json(summary_json, payload)
    write_markdown(summary_md, payload)

    print("Release preflight: PASS" if not failing else "Release preflight: FAIL")
    print(f"Gates executed: {len(results)}")
    print(f"Failed gates: {len(failing)}")
    print(f"Summary markdown: {summary_md}")
    print(f"Summary json: {summary_json}")
    return 1 if failing else 0


if __name__ == "__main__":
    sys.exit(run())
