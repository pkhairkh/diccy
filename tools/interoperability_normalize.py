#!/usr/bin/env python3
"""Normalize external interoperability capture logs into controlled JSONL schema."""

from __future__ import annotations

import argparse
import csv
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any

SHA256_RE = re.compile(r"^sha256:[0-9a-f]{64}$")

PROTOCOL_MAP = {
    "dicomweb": "DICOMweb",
    "dimse": "DIMSE",
    "mwl": "MWL",
    "mpps": "MPPS",
}
PHASE_MAP = {
    "positive": "positive",
    "negative": "negative",
    "pos": "positive",
    "neg": "negative",
}
VERDICT_MAP = {
    "pass": "PASS",
    "fail": "FAIL",
    "blocked": "BLOCKED",
}

DEFAULT_THRESHOLDS = {
    "DICOMweb": 10,
    "DIMSE": 8,
    "MWL": 4,
    "MPPS": 4,
}

FIELD_ALIASES = {
    "case_id": ["case_id", "case", "caseid", "id"],
    "target_system": ["target_system", "target", "target_id", "site_target"],
    "interface_id": ["interface_id", "interface", "surface", "interfaceid"],
    "protocol": ["protocol", "protocol_family", "service"],
    "phase": ["phase", "path_type", "pathtype"],
    "request_hash": ["request_hash", "request_digest", "request_sha256", "req_hash", "req_sha256"],
    "response_hash": ["response_hash", "response_digest", "response_sha256", "resp_hash", "resp_sha256"],
    "verdict": ["verdict", "result", "outcome", "status"],
    "evidence_ref": ["evidence_ref", "evidence", "reference"],
    "note": ["note", "comments", "comment"],
}


class NormalizeError(RuntimeError):
    pass


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Normalize interoperability raw logs into controlled JSONL schema.")
    parser.add_argument("--input", action="append", required=True, help="Input raw log path (.jsonl, .json, .csv).")
    parser.add_argument("--output", required=True, help="Output normalized JSONL path.")
    parser.add_argument("--release-id", required=True, help="Release identifier for emitted summary metadata.")
    parser.add_argument("--summary-json", default=None, help="Optional normalized summary JSON output path.")
    parser.add_argument(
        "--enforce-thresholds",
        action="store_true",
        help="Fail when minimum per-protocol case-volume thresholds are not met.",
    )
    parser.add_argument(
        "--threshold",
        action="append",
        default=[],
        help="Protocol threshold override in form <Protocol>=<Count>. Repeatable.",
    )
    return parser.parse_args()


def load_json_rows(path: pathlib.Path) -> list[dict[str, Any]]:
    payload = json.loads(path.read_text())
    if isinstance(payload, list):
        rows = payload
    elif isinstance(payload, dict) and isinstance(payload.get("rows"), list):
        rows = payload["rows"]
    else:
        raise NormalizeError(f"{path}: JSON must be an array of rows or object with 'rows' list")
    out: list[dict[str, Any]] = []
    for idx, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            raise NormalizeError(f"{path}: row {idx} is not an object")
        out.append(row)
    return out


def load_jsonl_rows(path: pathlib.Path) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for idx, raw in enumerate(path.read_text().splitlines(), start=1):
        line = raw.strip()
        if not line:
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise NormalizeError(f"{path}: line {idx} invalid JSON: {exc}") from exc
        if not isinstance(payload, dict):
            raise NormalizeError(f"{path}: line {idx} row is not an object")
        out.append(payload)
    return out


def load_csv_rows(path: pathlib.Path) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        for idx, row in enumerate(reader, start=1):
            if row is None:
                raise NormalizeError(f"{path}: csv row {idx} invalid")
            out.append({str(k): "" if v is None else v for k, v in row.items()})
    return out


def load_rows(path: pathlib.Path) -> list[dict[str, Any]]:
    suffix = path.suffix.lower()
    if suffix == ".jsonl":
        return load_jsonl_rows(path)
    if suffix == ".json":
        return load_json_rows(path)
    if suffix == ".csv":
        return load_csv_rows(path)
    raise NormalizeError(f"{path}: unsupported extension '{suffix}'")


def get_alias_value(row: dict[str, Any], field: str) -> str:
    aliases = FIELD_ALIASES[field]
    index = {str(k).strip().lower(): v for k, v in row.items()}
    for alias in aliases:
        raw = index.get(alias.lower())
        if raw is None:
            continue
        if isinstance(raw, str) and raw.strip():
            return raw.strip()
        if isinstance(raw, (int, float)):
            return str(raw)
    return ""


def canonical_protocol(value: str) -> str:
    key = value.strip().lower()
    result = PROTOCOL_MAP.get(key)
    if result is None:
        raise NormalizeError(f"invalid protocol '{value}'")
    return result


def canonical_phase(value: str) -> str:
    key = value.strip().lower()
    result = PHASE_MAP.get(key)
    if result is None:
        raise NormalizeError(f"invalid phase '{value}'")
    return result


def canonical_verdict(value: str) -> str:
    key = value.strip().lower()
    result = VERDICT_MAP.get(key)
    if result is None:
        raise NormalizeError(f"invalid verdict '{value}'")
    return result


def canonical_hash(value: str, *, field: str) -> str:
    text = value.strip().lower()
    if re.fullmatch(r"[0-9a-f]{64}", text):
        text = f"sha256:{text}"
    if not SHA256_RE.fullmatch(text):
        raise NormalizeError(f"invalid {field}: '{value}'")
    return text


def normalize_row(row: dict[str, Any], *, source: str, index: int) -> dict[str, str]:
    case_id = get_alias_value(row, "case_id")
    target_system = get_alias_value(row, "target_system")
    interface_id = get_alias_value(row, "interface_id")
    protocol_raw = get_alias_value(row, "protocol")
    phase_raw = get_alias_value(row, "phase")
    req_hash_raw = get_alias_value(row, "request_hash")
    rsp_hash_raw = get_alias_value(row, "response_hash")
    verdict_raw = get_alias_value(row, "verdict")

    missing = []
    for field, value in [
        ("case_id", case_id),
        ("target_system", target_system),
        ("interface_id", interface_id),
        ("protocol", protocol_raw),
        ("phase", phase_raw),
        ("request_hash", req_hash_raw),
        ("response_hash", rsp_hash_raw),
        ("verdict", verdict_raw),
    ]:
        if not value:
            missing.append(field)
    if missing:
        joined = ", ".join(missing)
        raise NormalizeError(f"{source}: row {index} missing required fields: {joined}")

    try:
        protocol = canonical_protocol(protocol_raw)
        phase = canonical_phase(phase_raw)
        verdict = canonical_verdict(verdict_raw)
        request_hash = canonical_hash(req_hash_raw, field="request_hash")
        response_hash = canonical_hash(rsp_hash_raw, field="response_hash")
    except NormalizeError as exc:
        raise NormalizeError(f"{source}: row {index} {exc}") from exc

    evidence_ref = get_alias_value(row, "evidence_ref")
    note = get_alias_value(row, "note")

    return {
        "case_id": case_id,
        "target_system": target_system,
        "interface_id": interface_id,
        "protocol": protocol,
        "phase": phase,
        "request_hash": request_hash,
        "response_hash": response_hash,
        "verdict": verdict,
        "evidence_ref": evidence_ref,
        "note": note,
    }


def parse_threshold_overrides(values: list[str]) -> dict[str, int]:
    out: dict[str, int] = {}
    for value in values:
        if "=" not in value:
            raise NormalizeError(f"invalid threshold override '{value}', expected <Protocol>=<Count>")
        protocol_raw, count_raw = value.split("=", 1)
        protocol = canonical_protocol(protocol_raw.strip())
        try:
            count = int(count_raw.strip())
        except ValueError as exc:
            raise NormalizeError(f"invalid threshold count '{count_raw}'") from exc
        if count < 0:
            raise NormalizeError(f"threshold count must be >= 0: {value}")
        out[protocol] = count
    return out


def run() -> int:
    args = parse_args()

    thresholds = dict(DEFAULT_THRESHOLDS)
    thresholds.update(parse_threshold_overrides(args.threshold))

    all_rows: list[dict[str, str]] = []
    errors: list[str] = []

    for input_value in args.input:
        path = pathlib.Path(input_value).resolve()
        if not path.exists():
            errors.append(f"input not found: {path}")
            continue
        try:
            rows = load_rows(path)
        except NormalizeError as exc:
            errors.append(str(exc))
            continue
        for idx, row in enumerate(rows, start=1):
            try:
                normalized = normalize_row(row, source=str(path), index=idx)
            except NormalizeError as exc:
                errors.append(str(exc))
                continue
            all_rows.append(normalized)

    if not all_rows:
        errors.append("no valid normalized rows produced")

    seen_case_ids: set[str] = set()
    for row in all_rows:
        case_id = row["case_id"]
        if case_id in seen_case_ids:
            errors.append(f"duplicate case_id: {case_id}")
        seen_case_ids.add(case_id)

    protocol_counts = {protocol: 0 for protocol in sorted(PROTOCOL_MAP.values())}
    for row in all_rows:
        protocol_counts[row["protocol"]] += 1

    if args.enforce_thresholds:
        for protocol, threshold in thresholds.items():
            observed = protocol_counts.get(protocol, 0)
            if observed < threshold:
                errors.append(
                    f"threshold not met for {protocol}: observed={observed} threshold={threshold}"
                )

    if errors:
        print("Interoperability normalize: FAIL")
        print(f"Findings: {len(errors)}")
        for error in errors:
            print(f"- {error}")
        return 1

    rows_sorted = sorted(
        all_rows,
        key=lambda row: (
            row["case_id"],
            row["target_system"],
            row["interface_id"],
            row["protocol"],
            row["phase"],
        ),
    )

    output_path = pathlib.Path(args.output).resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text("\n".join(json.dumps(row, sort_keys=True) for row in rows_sorted) + "\n")

    if args.summary_json:
        summary_path = pathlib.Path(args.summary_json).resolve()
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        summary_payload: dict[str, Any] = {
            "release_id": args.release_id,
            "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "total_rows": len(rows_sorted),
            "protocol_counts": protocol_counts,
            "thresholds": thresholds,
            "inputs": [str(pathlib.Path(value).resolve()) for value in args.input],
            "output": str(output_path),
        }
        summary_path.write_text(json.dumps(summary_payload, indent=2, sort_keys=True) + "\n")

    print("Interoperability normalize: PASS")
    print(f"Rows: {len(rows_sorted)}")
    for protocol in sorted(protocol_counts):
        print(f"{protocol}: {protocol_counts[protocol]}")
    if args.enforce_thresholds:
        print("Threshold enforcement: PASS")
    return 0


if __name__ == "__main__":
    sys.exit(run())
