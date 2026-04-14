#!/usr/bin/env python3
"""
Generate deterministic 24h soak-test artifact templates.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Generate soak reliability artifacts.")
    parser.add_argument("--repo-root", default=".", help="Repository root.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument(
        "--profile",
        default="reports/performance/profiles/soak-profile-RC-2026.02.11.json",
        help="Soak profile path.",
    )
    parser.add_argument("--duration-hours", type=int, default=24, help="Soak duration in hours.")
    parser.add_argument("--seed", type=int, default=242424, help="Deterministic scenario seed.")
    parser.add_argument(
        "--output-json",
        default="reports/performance/soak-reliability-<release-id>.json",
        help="Output JSON path.",
    )
    parser.add_argument(
        "--output-md",
        default="reports/performance/soak-reliability-<release-id>.md",
        help="Output markdown path.",
    )
    return parser.parse_args()


def load_profile(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("profile must be a JSON object")
    return payload


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    profile_path = (repo_root / args.profile).resolve()
    output_json = (repo_root / args.output_json.replace("<release-id>", args.release_id)).resolve()
    output_md = (repo_root / args.output_md.replace("<release-id>", args.release_id)).resolve()

    profile = load_profile(profile_path)
    workload_count = len(profile.get("workloads", [])) if isinstance(profile.get("workloads"), list) else 0
    scenario_key = f"{args.release_id}|{profile_path}|{args.duration_hours}|{args.seed}|{workload_count}"
    scenario_sha = hashlib.sha256(scenario_key.encode("utf-8")).hexdigest()

    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "release_id": args.release_id,
        "duration_hours": args.duration_hours,
        "seed": args.seed,
        "profile": str(profile_path),
        "workload_count": workload_count,
        "scenario_sha256": scenario_sha,
        "status": "TEMPLATE",
        "required_metrics": ["availability_pct", "error_rate_pct", "p95_latency_ms", "p99_latency_ms"],
    }
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_md.write_text(
        "\n".join(
            [
                f"# Soak Reliability Artifact - {args.release_id}",
                "",
                f"- Duration hours: `{args.duration_hours}`",
                f"- Seed: `{args.seed}`",
                f"- Workload count: `{workload_count}`",
                f"- Scenario SHA256: `{scenario_sha}`",
                "",
                "## Deterministic scenario input",
                f"- Profile: `{profile_path}`",
                "",
                "## 24h soak result fields",
                "- [ ] availability_pct",
                "- [ ] error_rate_pct",
                "- [ ] p95_latency_ms",
                "- [ ] p99_latency_ms",
                "",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"json: {output_json}")
    print(f"md: {output_md}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
