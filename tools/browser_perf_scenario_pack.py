#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run browser performance scenario pack.")
    parser.add_argument(
        "--pack",
        default=None,
        help="Scenario pack JSON path.",
    )
    parser.add_argument(
        "--output-dir",
        default="reports/performance/scenarios",
        help="Directory for scenario output JSON artifacts.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print scenario commands without executing them.",
    )
    return parser.parse_args()


def load_pack(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("scenario pack must be a JSON object")
    scenarios = payload.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise ValueError("scenario pack must include a non-empty scenarios list")
    return payload


def scenario_command(scenario: dict[str, Any], output_path: pathlib.Path) -> list[str]:
    scenario_id = str(scenario["id"])
    width = int(scenario["width"])
    height = int(scenario["height"])
    interactions = int(scenario["interactions"])
    if width <= 0 or height <= 0 or interactions <= 0:
        raise ValueError(f"{scenario_id}: width/height/interactions must be > 0")
    return [
        "node",
        "tools/web_renderer_perf_harness.mjs",
        "--width",
        str(width),
        "--height",
        str(height),
        "--interactions",
        str(interactions),
        "--output",
        str(output_path),
    ]


def main() -> int:
    args = parse_args()
    if args.pack:
        pack_path = pathlib.Path(args.pack).resolve()
    else:
        candidates = sorted(pathlib.Path("reports/performance/profiles").glob("browser-scenario-pack-*.json"))
        if not candidates:
            raise ValueError("no browser scenario pack found under reports/performance/profiles")
        pack_path = candidates[-1].resolve()
    payload = load_pack(pack_path)
    output_dir = pathlib.Path(args.output_dir).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    scenarios: list[dict[str, Any]] = payload["scenarios"]
    commands: list[list[str]] = []
    for scenario in scenarios:
        scenario_id = str(scenario["id"])
        output_path = output_dir / f"{scenario_id}.json"
        command = scenario_command(scenario, output_path)
        commands.append(command)
        if args.dry_run:
            print("DRY-RUN:", " ".join(command))
            continue
        subprocess.run(command, check=True)

    summary_path = output_dir / "index.json"
    summary = {
        "pack": str(pack_path),
        "scenario_count": len(commands),
        "outputs": [str(output_dir / f"{str(s['id'])}.json") for s in scenarios],
    }
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"wrote scenario summary: {summary_path}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:  # pragma: no cover
        print(f"browser_perf_scenario_pack failed: {error}", file=sys.stderr)
        raise SystemExit(1)
