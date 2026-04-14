#!/usr/bin/env python3
"""Generate WebGPU adoption/fallback dashboard report from harness JSON artifacts."""

from __future__ import annotations

import argparse
import glob
import json
from pathlib import Path
from statistics import mean


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render WebGPU dashboard markdown from harness reports.")
    parser.add_argument(
        "--inputs",
        default="reports/performance/web-renderer-harness*.json",
        help="Glob for harness JSON files.",
    )
    parser.add_argument(
        "--output",
        default="reports/performance/webgpu-dashboard.md",
        help="Dashboard markdown path.",
    )
    return parser.parse_args()


def as_pct(value: float) -> str:
    return f"{value * 100:.2f}%"


def main() -> int:
    args = parse_args()
    files = sorted(glob.glob(args.inputs))
    rows = []
    for raw_path in files:
        path = Path(raw_path)
        try:
            payload = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        adoption = payload.get("adoption", {})
        fallback = payload.get("fallback", {})
        rows.append(
            {
                "file": path.name,
                "started_at": payload.get("started_at", "-"),
                "dataset": payload.get("dataset", {}),
                "interactions": payload.get("interactions", 0),
                "active": payload.get("backend", {}).get("active", "-"),
                "adoption_rate": float(adoption.get("webgpu_adoption_rate", 0.0)),
                "fallback_freq": float(fallback.get("fallback_frequency", 0.0)),
                "fallback_reason": fallback.get("last_fallback_reason") or "-",
            }
        )

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    if not rows:
        output_path.write_text(
            "# WebGPU Dashboard\n\nNo harness artifacts matched the requested input pattern.\n",
            encoding="utf-8",
        )
        print(f"webgpu dashboard report wrote {output_path} (no data)")
        return 0

    avg_adoption = mean(row["adoption_rate"] for row in rows)
    avg_fallback = mean(row["fallback_freq"] for row in rows)

    lines = [
        "# WebGPU Dashboard",
        "",
        "## Summary",
        "",
        f"- Runs analyzed: **{len(rows)}**",
        f"- Mean WebGPU adoption rate: **{as_pct(avg_adoption)}**",
        f"- Mean fallback frequency: **{as_pct(avg_fallback)}**",
        "",
        "## Runs",
        "",
        "| Artifact | Started at | Dataset | Interactions | Active backend | WebGPU adoption | Fallback frequency | Last fallback reason |",
        "|---|---|---|---:|---|---:|---:|---|",
    ]
    for row in rows:
        dataset = row["dataset"]
        dims = f"{dataset.get('width', '?')}x{dataset.get('height', '?')}"
        lines.append(
            "| "
            f"{row['file']} | "
            f"{row['started_at']} | "
            f"{dims} | "
            f"{row['interactions']} | "
            f"{row['active']} | "
            f"{as_pct(row['adoption_rate'])} | "
            f"{as_pct(row['fallback_freq'])} | "
            f"{row['fallback_reason']} |"
        )

    output_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"webgpu dashboard report wrote {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
