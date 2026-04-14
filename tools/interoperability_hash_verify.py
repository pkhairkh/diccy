#!/usr/bin/env python3
"""
Verify interoperability case hashes across one or more capture summaries.

Usage:
  python3 tools/interoperability_hash_verify.py \
    --summary reports/interoperability/execution/dicomweb-capture-summary-RC-2026.02.11.json \
    --summary reports/interoperability/execution/dimse-capture-summary-RC-2026.02.11.json \
    --output-md reports/interoperability/hash-verification-register-RC-2026.02.11.md \
    --output-json reports/interoperability/hash-verification-register-RC-2026.02.11.json
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Any


SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")


@dataclass(frozen=True)
class Violation:
    source: str
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Verify interoperability hash coverage.")
    parser.add_argument(
        "--summary",
        action="append",
        required=True,
        help="Capture summary JSON path. Repeatable.",
    )
    parser.add_argument("--output-md", required=True, help="Output markdown register.")
    parser.add_argument("--output-json", required=True, help="Output JSON register.")
    parser.add_argument(
        "--fail-on-non-pass",
        action="store_true",
        help="Fail if any row verdict is not PASS.",
    )
    return parser.parse_args()


def load_summary(path: pathlib.Path) -> tuple[list[dict[str, str]], list[Violation]]:
    violations: list[Violation] = []
    rows_out: list[dict[str, str]] = []
    try:
        payload = json.loads(path.read_text())
    except Exception as exc:
        return [], [Violation(str(path), f"invalid JSON: {exc}")]
    rows = payload.get("rows")
    if not isinstance(rows, list):
        return [], [Violation(str(path), "missing rows list")]
    for idx, value in enumerate(rows, start=1):
        if not isinstance(value, dict):
            violations.append(Violation(str(path), f"row {idx} is not an object"))
            continue
        required = (
            "case_id",
            "target_system",
            "interface_id",
            "protocol",
            "phase",
            "request_hash",
            "response_hash",
            "verdict",
        )
        row: dict[str, str] = {}
        missing = False
        for field in required:
            raw = value.get(field)
            if not isinstance(raw, str) or not raw.strip():
                violations.append(Violation(str(path), f"row {idx} missing field: {field}"))
                missing = True
            else:
                row[field] = raw.strip()
        if missing:
            continue
        if not SHA256_RE.match(row["request_hash"]):
            violations.append(Violation(str(path), f"row {idx} invalid request_hash"))
        if not SHA256_RE.match(row["response_hash"]):
            violations.append(Violation(str(path), f"row {idx} invalid response_hash"))
        row["summary_path"] = str(path)
        rows_out.append(row)
    return rows_out, violations


def stable_row_digest(row: dict[str, str]) -> str:
    material = "|".join(
        [
            row["case_id"],
            row["target_system"],
            row["interface_id"],
            row["protocol"],
            row["phase"],
            row["request_hash"],
            row["response_hash"],
            row["verdict"],
        ]
    ).encode("utf-8")
    return hashlib.sha256(material).hexdigest()


def write_markdown(
    output_path: pathlib.Path,
    *,
    generated_at: str,
    rows: list[dict[str, str]],
    digest: str,
    total_non_pass: int,
) -> None:
    lines = [
        "# Interoperability Hash Verification Register",
        "",
        f"Generated At (UTC): {generated_at}",
        f"Register Digest (sha256): `{digest}`",
        "",
        "## Summary",
        "",
        f"- Total cases: {len(rows)}",
        f"- Non-pass verdict rows: {total_non_pass}",
        "",
        "## Case Register",
        "",
        "| Case ID | Target | Interface | Protocol | Phase | Request Hash | Response Hash | Verdict | Row Digest (sha256) | Source Summary |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for row in rows:
        lines.append(
            "| "
            + " | ".join(
                [
                    row["case_id"],
                    row["target_system"],
                    row["interface_id"],
                    row["protocol"],
                    row["phase"],
                    row["request_hash"],
                    row["response_hash"],
                    row["verdict"],
                    row["row_digest"],
                    row["summary_path"],
                ]
            )
            + " |"
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(lines) + "\n")


def write_json(output_path: pathlib.Path, payload: dict[str, Any]) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def run() -> int:
    args = parse_args()
    summary_paths = [pathlib.Path(value).resolve() for value in args.summary]
    violations: list[Violation] = []
    rows: list[dict[str, str]] = []
    for path in summary_paths:
        if not path.exists():
            violations.append(Violation(str(path), "summary file does not exist"))
            continue
        loaded_rows, loaded_violations = load_summary(path)
        rows.extend(loaded_rows)
        violations.extend(loaded_violations)

    dedup: dict[str, dict[str, str]] = {}
    for row in rows:
        key = row["case_id"]
        comparable = {
            "target_system": row["target_system"],
            "interface_id": row["interface_id"],
            "protocol": row["protocol"],
            "phase": row["phase"],
            "request_hash": row["request_hash"],
            "response_hash": row["response_hash"],
            "verdict": row["verdict"],
        }
        existing = dedup.get(key)
        if existing is None:
            dedup[key] = comparable
        elif existing != comparable:
            violations.append(Violation(key, "conflicting duplicate case definitions across summaries"))

    for row in rows:
        row["row_digest"] = stable_row_digest(row)

    rows_sorted = sorted(
        rows,
        key=lambda row: (
            row["case_id"],
            row["target_system"],
            row["interface_id"],
            row["protocol"],
            row["phase"],
        ),
    )

    total_non_pass = sum(1 for row in rows_sorted if row["verdict"] != "PASS")
    if args.fail_on_non_pass and total_non_pass:
        violations.append(Violation("global", "non-pass verdict rows present"))

    register_material = json.dumps(rows_sorted, sort_keys=True, separators=(",", ":")).encode("utf-8")
    register_digest = hashlib.sha256(register_material).hexdigest()
    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    output_md = pathlib.Path(args.output_md).resolve()
    output_json = pathlib.Path(args.output_json).resolve()

    if violations:
        print("Interoperability hash verify: FAIL")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            print(f"{violation.source}: {violation.message}")
        return 1

    write_markdown(
        output_md,
        generated_at=generated_at,
        rows=rows_sorted,
        digest=register_digest,
        total_non_pass=total_non_pass,
    )
    write_json(
        output_json,
        {
            "generated_at_utc": generated_at,
            "total_cases": len(rows_sorted),
            "non_pass_cases": total_non_pass,
            "register_digest_sha256": register_digest,
            "rows": rows_sorted,
        },
    )

    print("Interoperability hash verify: PASS")
    print(f"Cases: {len(rows_sorted)}")
    print(f"Non-pass verdict rows: {total_non_pass}")
    print(f"Register digest: sha256:{register_digest}")
    return 0


if __name__ == "__main__":
    sys.exit(run())
