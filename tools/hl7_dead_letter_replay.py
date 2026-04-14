#!/usr/bin/env python3
"""
HL7 dead-letter queue replay tooling.

Supports deterministic local operations on the persisted workflow DLQ snapshot:
- list records
- replay a record in operator workflow (dry-run report generation)
- prune retained record count
"""

from __future__ import annotations

import argparse
import json
import pathlib
from dataclasses import dataclass, asdict


@dataclass
class DlqRecord:
    id: str
    source: str
    message_type: str
    reason: str
    payload_excerpt: str
    created_at_ms: int
    scope: str
    subscription_id: str
    event_id: str
    correlation_id: str
    sequence: int
    attempt: int
    max_attempts: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="HL7 dead-letter replay tooling.")
    parser.add_argument(
        "--dlq-path",
        default="./state/workflow/workflow.audit.log.hl7-failures.dlq",
        help="DLQ snapshot path produced by dicom-workflow-server.",
    )
    parser.add_argument(
        "--action",
        choices=("list", "replay", "prune"),
        required=True,
        help="Tool action.",
    )
    parser.add_argument("--failure-id", help="Failure record ID for replay action.")
    parser.add_argument(
        "--max-entries",
        type=int,
        default=256,
        help="Retention cap for prune action.",
    )
    parser.add_argument(
        "--output-json",
        help="Optional output report JSON path for replay/list actions.",
    )
    return parser.parse_args()


def load_records(path: pathlib.Path) -> list[DlqRecord]:
    if not path.exists():
        return []
    out: list[DlqRecord] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line:
            continue
        parts = line.split("\t")
        if len(parts) != 13:
            continue
        try:
            out.append(
                DlqRecord(
                    id=parts[0],
                    source=parts[1],
                    message_type=parts[2],
                    reason=parts[3],
                    payload_excerpt=parts[4],
                    created_at_ms=int(parts[5]),
                    scope=parts[6],
                    subscription_id=parts[7],
                    event_id=parts[8],
                    correlation_id=parts[9],
                    sequence=int(parts[10]),
                    attempt=int(parts[11]),
                    max_attempts=int(parts[12]),
                )
            )
        except ValueError:
            continue
    return out


def persist_records(path: pathlib.Path, records: list[DlqRecord]) -> None:
    lines = []
    for record in records:
        lines.append(
            "\t".join(
                [
                    record.id,
                    record.source,
                    record.message_type,
                    record.reason,
                    record.payload_excerpt,
                    str(record.created_at_ms),
                    record.scope,
                    record.subscription_id,
                    record.event_id,
                    record.correlation_id,
                    str(record.sequence),
                    str(record.attempt),
                    str(record.max_attempts),
                ]
            )
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + ("\n" if lines else ""), encoding="utf-8")


def write_report(path: pathlib.Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    dlq_path = pathlib.Path(args.dlq_path).resolve()
    records = load_records(dlq_path)

    if args.action == "list":
        payload = {
            "dlq_path": str(dlq_path),
            "record_count": len(records),
            "records": [asdict(record) for record in records],
        }
        if args.output_json:
            write_report(pathlib.Path(args.output_json).resolve(), payload)
        else:
            print(json.dumps(payload, indent=2))
        return 0

    if args.action == "replay":
        if not args.failure_id:
            raise SystemExit("--failure-id is required for replay action")
        record = next((entry for entry in records if entry.id == args.failure_id), None)
        if record is None:
            raise SystemExit(f"failure id not found: {args.failure_id}")
        payload = {
            "action": "replay",
            "status": "accepted",
            "failure_id": record.id,
            "scope": record.scope,
            "source": record.source,
            "message_type": record.message_type,
            "subscription_id": record.subscription_id,
            "event_id": record.event_id,
            "correlation_id": record.correlation_id,
            "payload_excerpt": record.payload_excerpt,
        }
        if args.output_json:
            write_report(pathlib.Path(args.output_json).resolve(), payload)
        else:
            print(json.dumps(payload, indent=2))
        return 0

    if args.max_entries < 1:
        raise SystemExit("--max-entries must be >= 1")
    retained = records[: args.max_entries]
    persist_records(dlq_path, retained)
    payload = {
        "action": "prune",
        "dlq_path": str(dlq_path),
        "before": len(records),
        "after": len(retained),
        "max_entries": args.max_entries,
    }
    if args.output_json:
        write_report(pathlib.Path(args.output_json).resolve(), payload)
    else:
        print(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
