#!/usr/bin/env python3
"""Generate machine-readable compatibility capability artifacts.

Outputs:
  - dicomweb_route_matrix.json
  - workflow_route_matrix.json
  - unsupported_feature_ledger.json
  - capability_manifest.json
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
from datetime import datetime, timezone
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        default="reports/compatibility",
        help="Directory where generated artifacts are written.",
    )
    parser.add_argument(
        "--release-id",
        default="UNSET",
        help="Release/build identifier for artifact metadata.",
    )
    return parser.parse_args()


def run_json_command(cmd: list[str]) -> list[dict[str, Any]]:
    out = subprocess.check_output(cmd, text=True).strip()
    if not out:
        raise SystemExit(f"empty output from: {' '.join(cmd)}")
    payload = json.loads(out)
    if not isinstance(payload, list):
        raise SystemExit(f"expected list payload from: {' '.join(cmd)}")
    return payload


def sort_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(
        rows,
        key=lambda row: (
            str(row.get("path", "")),
            str(row.get("method", "")),
            str(row.get("operation", "")),
        ),
    )


def unsupported_rows(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    blocked_states = {"blocked", "partial", "not-exposed"}
    out: list[dict[str, Any]] = []
    for row in rows:
        state = str(row.get("state", "")).strip().lower()
        if state in blocked_states:
            out.append(row)
    return sort_rows(out)


def main() -> int:
    args = parse_args()
    output_dir = pathlib.Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    web_routes = sort_rows(
        run_json_command(
            [
                "cargo",
                "run",
                "--quiet",
                "--package",
                "dicom-web",
                "--features",
                "qido,wado,stow",
                "--example",
                "route_matrix",
            ]
        )
    )
    workflow_routes = sort_rows(
        run_json_command(
            [
                "cargo",
                "run",
                "--quiet",
                "--package",
                "dicom-workflow-server",
                "--example",
                "workflow_route_matrix",
            ]
        )
    )

    unsupported = unsupported_rows(web_routes)
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    manifest = {
        "generated_at_utc": now,
        "release_id": args.release_id,
        "dicomweb_route_count": len(web_routes),
        "workflow_route_count": len(workflow_routes),
        "unsupported_feature_count": len(unsupported),
        "artifacts": [
            "dicomweb_route_matrix.json",
            "workflow_route_matrix.json",
            "unsupported_feature_ledger.json",
        ],
    }

    (output_dir / "dicomweb_route_matrix.json").write_text(
        json.dumps(web_routes, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output_dir / "workflow_route_matrix.json").write_text(
        json.dumps(workflow_routes, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output_dir / "unsupported_feature_ledger.json").write_text(
        json.dumps(unsupported, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    (output_dir / "capability_manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )

    print(f"capability-artifacts: wrote {output_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
