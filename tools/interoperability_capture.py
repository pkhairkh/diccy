#!/usr/bin/env python3
"""
Interoperability evidence capture tool.

Reads newline-delimited JSON trace rows, validates required fields, and emits:
- deterministic markdown matrix,
- machine-readable JSON summary.

Usage:
  python3 tools/interoperability_capture.py \
    --input reports/interoperability/dry-run/input-traces-RC-2026.02.11.jsonl \
    --output-md reports/interoperability/dry-run/capture-matrix-RC-2026.02.11.md \
    --output-json reports/interoperability/dry-run/capture-summary-RC-2026.02.11.json \
    --release-id RC-2026.02.11
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
PHASES = {"positive", "negative"}
VERDICTS = {"PASS", "FAIL", "BLOCKED"}
REQUIRED_FIELDS = (
    "case_id",
    "target_system",
    "interface_id",
    "protocol",
    "phase",
    "request_hash",
    "response_hash",
    "verdict",
)


@dataclass(frozen=True)
class Violation:
    line: int
    message: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Interoperability evidence capture utility.")
    parser.add_argument("--input", required=True, help="Input JSONL trace file.")
    parser.add_argument("--output-md", required=True, help="Output markdown matrix path.")
    parser.add_argument("--output-json", required=True, help="Output JSON summary path.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument(
        "--fail-on-non-pass",
        action="store_true",
        help="Return non-zero when any row verdict is not PASS.",
    )
    return parser.parse_args()


def parse_jsonl(input_path: pathlib.Path) -> tuple[list[dict[str, str]], list[Violation]]:
    rows: list[dict[str, str]] = []
    violations: list[Violation] = []
    for line_no, raw_line in enumerate(input_path.read_text().splitlines(), start=1):
        line = raw_line.strip()
        if not line:
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            violations.append(Violation(line_no, f"invalid JSON: {exc}"))
            continue
        if not isinstance(value, dict):
            violations.append(Violation(line_no, "trace row must be a JSON object"))
            continue
        normalized: dict[str, str] = {}
        for field in REQUIRED_FIELDS:
            raw = value.get(field)
            if not isinstance(raw, str) or not raw.strip():
                violations.append(Violation(line_no, f"missing/invalid required field: {field}"))
                continue
            normalized[field] = raw.strip()
        if set(normalized) != set(REQUIRED_FIELDS):
            continue
        phase = normalized["phase"]
        if phase not in PHASES:
            violations.append(Violation(line_no, f"invalid phase '{phase}'"))
        verdict = normalized["verdict"]
        if verdict not in VERDICTS:
            violations.append(Violation(line_no, f"invalid verdict '{verdict}'"))
        request_hash = normalized["request_hash"]
        response_hash = normalized["response_hash"]
        if not SHA256_RE.match(request_hash):
            violations.append(Violation(line_no, "request_hash must be sha256:<64 hex chars>"))
        if not SHA256_RE.match(response_hash):
            violations.append(Violation(line_no, "response_hash must be sha256:<64 hex chars>"))

        note_value = value.get("note", "")
        evidence_ref = value.get("evidence_ref", "")
        normalized["note"] = note_value.strip() if isinstance(note_value, str) else ""
        normalized["evidence_ref"] = evidence_ref.strip() if isinstance(evidence_ref, str) else ""
        rows.append(normalized)
    return rows, violations


def stable_digest(rows: list[dict[str, str]]) -> str:
    payload = json.dumps(rows, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def write_markdown(
    output_path: pathlib.Path,
    *,
    release_id: str,
    generated_at: str,
    rows: list[dict[str, str]],
    verdict_counts: dict[str, int],
    capture_digest: str,
) -> None:
    header = [
        "# Interoperability Capture Matrix",
        "",
        f"Release: {release_id}",
        f"Generated At (UTC): {generated_at}",
        f"Capture Digest (sha256): `{capture_digest}`",
        "",
        "## Summary",
        "",
        f"- Total rows: {len(rows)}",
        f"- PASS: {verdict_counts.get('PASS', 0)}",
        f"- FAIL: {verdict_counts.get('FAIL', 0)}",
        f"- BLOCKED: {verdict_counts.get('BLOCKED', 0)}",
        "",
        "## Rows",
        "",
        "| Case ID | Target System | Interface ID | Protocol | Phase | Request Hash | Response Hash | Verdict | Evidence Ref | Note |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    body = []
    for row in rows:
        body.append(
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
                    row.get("evidence_ref", "") or "N/A",
                    row.get("note", "") or "N/A",
                ]
            )
            + " |"
        )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(header + body) + "\n")


def write_summary_json(
    output_path: pathlib.Path,
    *,
    release_id: str,
    generated_at: str,
    rows: list[dict[str, str]],
    verdict_counts: dict[str, int],
    capture_digest: str,
) -> None:
    payload: dict[str, Any] = {
        "release_id": release_id,
        "generated_at_utc": generated_at,
        "total_rows": len(rows),
        "verdict_counts": verdict_counts,
        "capture_digest_sha256": capture_digest,
        "rows": rows,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def run() -> int:
    args = parse_args()
    input_path = pathlib.Path(args.input).resolve()
    output_md = pathlib.Path(args.output_md).resolve()
    output_json = pathlib.Path(args.output_json).resolve()

    if not input_path.exists():
        print(f"Interoperability capture: FAIL")
        print(f"error: input does not exist: {input_path}")
        return 1

    rows, violations = parse_jsonl(input_path)
    if not rows:
        violations.append(Violation(0, "no valid rows found"))

    if violations:
        print("Interoperability capture: FAIL")
        print(f"Violations: {len(violations)}")
        for violation in violations:
            prefix = f"line {violation.line}" if violation.line else "global"
            print(f"{prefix}: {violation.message}")
        return 1

    rows_sorted = sorted(
        rows,
        key=lambda r: (
            r["case_id"],
            r["target_system"],
            r["interface_id"],
            r["protocol"],
            r["phase"],
        ),
    )
    verdict_counts = {
        verdict: sum(1 for row in rows_sorted if row["verdict"] == verdict)
        for verdict in sorted(VERDICTS)
    }
    capture_digest = stable_digest(rows_sorted)
    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    write_markdown(
        output_md,
        release_id=args.release_id,
        generated_at=generated_at,
        rows=rows_sorted,
        verdict_counts=verdict_counts,
        capture_digest=capture_digest,
    )
    write_summary_json(
        output_json,
        release_id=args.release_id,
        generated_at=generated_at,
        rows=rows_sorted,
        verdict_counts=verdict_counts,
        capture_digest=capture_digest,
    )

    if args.fail_on_non_pass and verdict_counts.get("PASS", 0) != len(rows_sorted):
        print("Interoperability capture: FAIL")
        print("error: non-pass verdicts present and --fail-on-non-pass enabled")
        print(f"PASS={verdict_counts.get('PASS', 0)} FAIL={verdict_counts.get('FAIL', 0)} BLOCKED={verdict_counts.get('BLOCKED', 0)}")
        return 1

    print("Interoperability capture: PASS")
    print(f"Rows: {len(rows_sorted)}")
    print(f"PASS={verdict_counts.get('PASS', 0)} FAIL={verdict_counts.get('FAIL', 0)} BLOCKED={verdict_counts.get('BLOCKED', 0)}")
    print(f"Capture digest: sha256:{capture_digest}")
    return 0


if __name__ == "__main__":
    sys.exit(run())
