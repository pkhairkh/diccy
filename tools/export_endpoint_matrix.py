#!/usr/bin/env python3
"""
Export deterministic endpoint matrix Markdown from runtime route definitions.

Usage:
  python3 tools/export_endpoint_matrix.py \
    --output docs/12-API-Surface-and-Crate-Boundaries.md \
    --release-id RC-2026.02.22
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
from datetime import datetime, timezone
from typing import Any, Iterable


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        required=True,
        help="Markdown documentation file to update with generated endpoint matrix.",
    )
    parser.add_argument(
        "--release-id",
        default="RC-UNSET",
        help="Release identifier shown in generated matrix metadata.",
    )
    return parser.parse_args()


def run_matrix_command(cmd: list[str]) -> list[dict[str, Any]]:
    output = subprocess.check_output(cmd, text=True).strip()
    if not output:
        raise SystemExit(f"matrix command produced no output: {' '.join(cmd)}")
    return json.loads(output)


def load_route_contracts() -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    web_routes = run_matrix_command(
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
    workflow_routes = run_matrix_command(
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
    return web_routes, workflow_routes


def fmt_bool(value: bool) -> str:
    return "yes" if value else "no"


def sort_routes(rows: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    return sorted(rows, key=lambda row: (row["path"], row["method"], row.get("operation", "")))


def render_web_table(rows: Iterable[dict[str, Any]]) -> list[str]:
    rows = sort_routes(rows)
    lines = [
        "| Method | Path | Operation | Required feature | State | Content-Type |",
        "|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            "| {} | {} | {} | {} | {} | {} |".format(
                row["method"],
                row["path"],
                row["operation"],
                row["required_feature"],
                row["state"],
                row.get("content_type") or "N/A",
            )
        )
    if len(lines) == 2:
        lines.append("| *No rows* |  |  |  |  |  |")
    return lines


def render_workflow_table(rows: Iterable[dict[str, Any]]) -> list[str]:
    rows = sort_routes(rows)
    lines = [
        "| Method | Path | Operation | Requires writer role | Requires idempotency-key | Content-Type |",
        "|---|---|---|---:|---:|---|",
    ]
    for row in rows:
        lines.append(
            "| {} | {} | {} | {} | {} | {} |".format(
                row["method"],
                row["path"],
                row["operation"],
                fmt_bool(row["requires_writer_role"]),
                fmt_bool(row["requires_idempotency_key"]),
                row.get("content_type") or "N/A",
            )
        )
    if len(lines) == 2:
        lines.append("| *No rows* |  |  |  |  |  |")
    return lines


def render_markdown(
    web_routes: list[dict[str, Any]],
    workflow_routes: list[dict[str, Any]],
    *,
    release_id: str,
) -> str:
    generated_at = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    sections = [
        "<!-- endpoint-matrix:auto:start -->",
        "# Endpoint matrix (code-generated)",
        "",
        f"Generated: {generated_at} (UTC) for {release_id}.",
        "",
        "## dicom-web routes",
        "",
        *render_web_table(web_routes),
        "",
        "## dicom-workflow-server routes",
        "",
        *render_workflow_table(workflow_routes),
        "",
        "<!-- endpoint-matrix:auto:end -->",
        "",
    ]
    return "\n".join(sections)


def replace_block(document: str, matrix_block: str) -> str:
    start_marker = "<!-- endpoint-matrix:auto:start -->"
    end_marker = "<!-- endpoint-matrix:auto:end -->"
    start = document.find(start_marker)
    end = document.find(end_marker)
    if start == -1 or end == -1 or end < start:
        raise SystemExit(
            "missing endpoint matrix markers in output document; "
            "expected <!-- endpoint-matrix:auto:start --> ... <!-- endpoint-matrix:auto:end -->"
        )
    return f"{document[:start]}{matrix_block}{document[end + len(end_marker):]}"


def main() -> int:
    args = parse_args()
    web_routes, workflow_routes = load_route_contracts()
    if not isinstance(web_routes, list) or not isinstance(workflow_routes, list):
        raise SystemExit("unexpected matrix payload; expected list payload")

    output_path = pathlib.Path(args.output)
    document = output_path.read_text(encoding="utf-8")
    rendered = render_markdown(web_routes, workflow_routes, release_id=args.release_id)
    output_path.write_text(replace_block(document, rendered), encoding="utf-8")
    print(f"endpoint-matrix: updated {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
