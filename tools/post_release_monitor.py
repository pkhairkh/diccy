#!/usr/bin/env python3
"""
Post-release drift monitoring utility.

Generates:
- monitoring job definitions (periodic hooks)
- cycle execution report from release evidence signals
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate post-release monitoring jobs and cycle report.")
    parser.add_argument("--release-id", required=True, help="Release identifier (for example RC-2026.02.11).")
    parser.add_argument(
        "--reproducibility-json",
        required=True,
        help="Path to reproducibility matrix JSON (tools/reproducibility_compare.py output).",
    )
    parser.add_argument(
        "--interoperability-json",
        required=True,
        help="Path to interoperability hash register JSON.",
    )
    parser.add_argument(
        "--traceability-json",
        required=True,
        help="Path to traceability snapshot JSON.",
    )
    parser.add_argument(
        "--artifact-integrity-json",
        required=True,
        help="Path to release artifact integrity JSON.",
    )
    parser.add_argument("--jobs-output-md", required=True, help="Monitoring jobs markdown output path.")
    parser.add_argument("--jobs-output-json", required=True, help="Monitoring jobs JSON output path.")
    parser.add_argument("--cycle-output-md", required=True, help="Monitoring cycle markdown output path.")
    parser.add_argument("--cycle-output-json", required=True, help="Monitoring cycle JSON output path.")
    parser.add_argument(
        "--cycle-id",
        default="cycle-1",
        help="Monitoring cycle identifier.",
    )
    parser.add_argument(
        "--fail-on-threshold-breach",
        action="store_true",
        help="Exit non-zero when any monitored signal breaches threshold.",
    )
    return parser.parse_args()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    return json.loads(path.read_text())


def build_jobs(release_id: str) -> list[dict[str, str]]:
    return [
        {
            "job_id": "DRIFT-REP-001",
            "domain": "reproducibility",
            "cadence": "daily",
            "owner": "Reproducibility Lead",
            "command": (
                "python3 tools/reproducibility_compare.py "
                f"--manifest arm64=reports/analytical/reproducibility/actual-aarch64-apple-darwin-{release_id}.toml "
                f"--manifest x86_64-projected=reports/analytical/reproducibility/projected-x86_64-unknown-linux-gnu-{release_id}.toml "
                f"--manifest wasm32-projected=reports/analytical/reproducibility/projected-wasm32-unknown-unknown-{release_id}.toml "
                f"--output-md reports/analytical/reproducibility/matrix-{release_id}.md "
                f"--output-json reports/analytical/reproducibility/matrix-{release_id}.json --mismatch-budget 0"
            ),
            "threshold": "mismatch_count <= 0",
        },
        {
            "job_id": "DRIFT-IOP-002",
            "domain": "interoperability",
            "cadence": "weekly",
            "owner": "Interoperability Lead",
            "command": (
                "python3 tools/interoperability_hash_verify.py "
                f"--summary reports/interoperability/execution/dicomweb-capture-summary-{release_id}.json "
                f"--summary reports/interoperability/execution/dimse-capture-summary-{release_id}.json "
                f"--summary reports/interoperability/execution/workflow-capture-summary-{release_id}.json "
                f"--summary reports/interoperability/execution/negative-capture-summary-{release_id}.json "
                f"--output-md reports/interoperability/hash-verification-register-{release_id}.md "
                f"--output-json reports/interoperability/hash-verification-register-{release_id}.json --fail-on-non-pass"
            ),
            "threshold": "non_pass_cases <= 0",
        },
        {
            "job_id": "DRIFT-TRC-003",
            "domain": "traceability",
            "cadence": "weekly",
            "owner": "Traceability Lead",
            "command": (
                "python3 tools/traceability_report.py "
                f"--linkage-manifest reports/traceability/release-traceability-manifest-{release_id}.json "
                f"--baseline-json reports/traceability/traceability-baseline-{release_id}.json "
                f"--output-md reports/traceability/traceability-snapshot-{release_id}.md "
                f"--output-json reports/traceability/traceability-snapshot-{release_id}.json --fail-on-invalid-links"
            ),
            "threshold": "missing_references_count <= 0 and invalid_links_count <= 0",
        },
        {
            "job_id": "DRIFT-RPK-004",
            "domain": "release-package",
            "cadence": "on every release or hotfix cut",
            "owner": "Release Manager",
            "command": (
                "python3 tools/release_artifact_integrity.py "
                f"--release-id {release_id} "
                f"--output-md reports/release/release-artifact-integrity-{release_id}.md "
                f"--output-json reports/release/release-artifact-integrity-{release_id}.json --fail-on-errors"
            ),
            "threshold": "missing_required_count <= 0 and missing_index_reference_count <= 0",
        },
    ]


def evaluate_signals(
    *,
    reproducibility: dict[str, Any],
    interoperability: dict[str, Any],
    traceability: dict[str, Any],
    artifact_integrity: dict[str, Any],
) -> list[dict[str, Any]]:
    signals: list[dict[str, Any]] = []
    mismatch_count = int(reproducibility.get("mismatch_count", 0))
    signals.append(
        {
            "signal_id": "SIG-DRIFT-REP",
            "domain": "reproducibility",
            "metric": "mismatch_count",
            "value": mismatch_count,
            "threshold": "<= 0",
            "status": "PASS" if mismatch_count <= 0 else "FAIL",
        }
    )

    non_pass_cases = int(interoperability.get("non_pass_cases", 0))
    signals.append(
        {
            "signal_id": "SIG-DRIFT-IOP",
            "domain": "interoperability",
            "metric": "non_pass_cases",
            "value": non_pass_cases,
            "threshold": "<= 0",
            "status": "PASS" if non_pass_cases <= 0 else "FAIL",
        }
    )

    missing_refs = int(traceability.get("missing_references_count", 0))
    invalid_links = int(traceability.get("invalid_links_count", 0))
    signals.append(
        {
            "signal_id": "SIG-DRIFT-TRC",
            "domain": "traceability",
            "metric": "missing_references_count",
            "value": missing_refs,
            "threshold": "<= 0",
            "status": "PASS" if missing_refs <= 0 else "FAIL",
        }
    )
    signals.append(
        {
            "signal_id": "SIG-DRIFT-LNK",
            "domain": "traceability",
            "metric": "invalid_links_count",
            "value": invalid_links,
            "threshold": "<= 0",
            "status": "PASS" if invalid_links <= 0 else "FAIL",
        }
    )

    missing_required = int(artifact_integrity.get("missing_required_count", 0))
    missing_index_refs = int(artifact_integrity.get("missing_index_reference_count", 0))
    signals.append(
        {
            "signal_id": "SIG-DRIFT-RPK",
            "domain": "release-package",
            "metric": "missing_required_count",
            "value": missing_required,
            "threshold": "<= 0",
            "status": "PASS" if missing_required <= 0 else "FAIL",
        }
    )
    signals.append(
        {
            "signal_id": "SIG-DRIFT-IDX",
            "domain": "release-package",
            "metric": "missing_index_reference_count",
            "value": missing_index_refs,
            "threshold": "<= 0",
            "status": "PASS" if missing_index_refs <= 0 else "FAIL",
        }
    )
    return signals


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_jobs_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Drift Monitoring Job Definitions",
        "",
        f"Release Candidate: {payload['release_id']}",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        "",
        "| Job ID | Domain | Cadence | Owner | Threshold |",
        "|---|---|---|---|---|",
    ]
    for job in payload["jobs"]:
        lines.append(
            "| "
            + " | ".join(
                [job["job_id"], job["domain"], job["cadence"], job["owner"], job["threshold"]]
            )
            + " |"
        )
    lines.extend(["", "## Commands", ""])
    for job in payload["jobs"]:
        lines.append(f"- `{job['job_id']}`: `{job['command']}`")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def write_cycle_markdown(path: pathlib.Path, payload: dict[str, Any]) -> None:
    lines = [
        "# Post-Release Monitoring Cycle",
        "",
        f"Release Candidate: {payload['release_id']}",
        f"Cycle ID: {payload['cycle_id']}",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        f"Overall Status: **{payload['overall_status']}**",
        "",
        "| Signal ID | Domain | Metric | Value | Threshold | Status |",
        "|---|---|---|---:|---|---|",
    ]
    for signal in payload["signals"]:
        lines.append(
            "| "
            + " | ".join(
                [
                    signal["signal_id"],
                    signal["domain"],
                    signal["metric"],
                    str(signal["value"]),
                    signal["threshold"],
                    signal["status"],
                ]
            )
            + " |"
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def run() -> int:
    args = parse_args()
    reproducibility = load_json(pathlib.Path(args.reproducibility_json).resolve())
    interoperability = load_json(pathlib.Path(args.interoperability_json).resolve())
    traceability = load_json(pathlib.Path(args.traceability_json).resolve())
    artifact_integrity = load_json(pathlib.Path(args.artifact_integrity_json).resolve())

    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    jobs = build_jobs(args.release_id)
    jobs_payload = {
        "release_id": args.release_id,
        "generated_at_utc": generated_at,
        "jobs": jobs,
    }

    signals = evaluate_signals(
        reproducibility=reproducibility,
        interoperability=interoperability,
        traceability=traceability,
        artifact_integrity=artifact_integrity,
    )
    failing = [signal for signal in signals if signal["status"] != "PASS"]
    cycle_payload = {
        "release_id": args.release_id,
        "cycle_id": args.cycle_id,
        "generated_at_utc": generated_at,
        "overall_status": "PASS" if not failing else "FAIL",
        "signal_count": len(signals),
        "failing_signal_count": len(failing),
        "signals": signals,
    }

    jobs_md = pathlib.Path(args.jobs_output_md).resolve()
    jobs_json = pathlib.Path(args.jobs_output_json).resolve()
    cycle_md = pathlib.Path(args.cycle_output_md).resolve()
    cycle_json = pathlib.Path(args.cycle_output_json).resolve()

    write_json(jobs_json, jobs_payload)
    write_json(cycle_json, cycle_payload)
    write_jobs_markdown(jobs_md, jobs_payload)
    write_cycle_markdown(cycle_md, cycle_payload)

    print("Post-release monitor: PASS" if not failing else "Post-release monitor: FAIL")
    print(f"Jobs generated: {len(jobs)}")
    print(f"Signals evaluated: {len(signals)}")
    print(f"Failing signals: {len(failing)}")
    print(f"Jobs markdown: {jobs_md}")
    print(f"Cycle markdown: {cycle_md}")

    if args.fail_on_threshold_breach and failing:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(run())
