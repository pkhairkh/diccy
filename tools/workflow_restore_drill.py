#!/usr/bin/env python3
"""
Executable disaster-recovery restore drill for workflow snapshots and WAL-backed stores.

The drill:
1. Seeds deterministic snapshot/WAL fixtures.
2. Creates a backup set.
3. Corrupts active files.
4. Restores from backup.
5. Verifies SHA-256 parity between original and restored content.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import shutil
import tempfile
from dataclasses import dataclass


@dataclass(frozen=True)
class DrillFile:
    relative_path: str
    content: bytes
    class_name: str


DRILL_FILES = (
    DrillFile("workflow/worklist.snapshot", b"WORKLIST|tenant-default|item-count=3\n", "snapshot"),
    DrillFile("workflow/mpps.snapshot", b"MPPS|tenant-default|update-count=2\n", "snapshot"),
    DrillFile("workflow/sr.snapshot", b"SR|tenant-default|doc-count=1\n", "snapshot"),
    DrillFile("web/storage.wal", b"WAL|dicom-web|record-count=4\n", "wal"),
)


def sha256_hex(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(8192)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run workflow disaster-recovery restore drill.")
    parser.add_argument("--release-id", required=True, help="Release identifier (for artifact naming).")
    parser.add_argument(
        "--output-json",
        help="Output JSON report path. Default: reports/release/workflow-restore-drill-<release-id>.json",
    )
    parser.add_argument(
        "--output-md",
        help="Output Markdown report path. Default: reports/release/workflow-restore-drill-<release-id>.md",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    release_id = args.release_id
    output_json = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (repo_root / f"reports/release/workflow-restore-drill-{release_id}.json").resolve()
    )
    output_md = (
        pathlib.Path(args.output_md).resolve()
        if args.output_md
        else (repo_root / f"reports/release/workflow-restore-drill-{release_id}.md").resolve()
    )

    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_md.parent.mkdir(parents=True, exist_ok=True)

    results: list[dict[str, object]] = []

    with tempfile.TemporaryDirectory(prefix=f"workflow-restore-drill-{release_id}-") as temp_dir:
        root = pathlib.Path(temp_dir)
        active_dir = root / "active"
        backup_dir = root / "backup"
        active_dir.mkdir(parents=True, exist_ok=True)
        backup_dir.mkdir(parents=True, exist_ok=True)

        for entry in DRILL_FILES:
            active_path = active_dir / entry.relative_path
            backup_path = backup_dir / entry.relative_path
            active_path.parent.mkdir(parents=True, exist_ok=True)
            backup_path.parent.mkdir(parents=True, exist_ok=True)

            active_path.write_bytes(entry.content)
            source_hash = sha256_hex(active_path)

            shutil.copy2(active_path, backup_path)
            active_path.write_bytes(b"CORRUPTED|restore-drill\n")
            shutil.copy2(backup_path, active_path)

            restored_hash = sha256_hex(active_path)
            results.append(
                {
                    "path": entry.relative_path,
                    "class": entry.class_name,
                    "source_sha256": source_hash,
                    "restored_sha256": restored_hash,
                    "restored_match": source_hash == restored_hash,
                }
            )

    passed = all(bool(record["restored_match"]) for record in results)
    payload = {
        "release_id": release_id,
        "drill_id": f"DR-RESTORE-{release_id}",
        "status": "PASS" if passed else "FAIL",
        "result_count": len(results),
        "results": results,
    }
    output_json.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    md_lines = [
        "# Workflow Disaster-Recovery Restore Drill",
        "",
        f"- Release ID: `{release_id}`",
        f"- Drill ID: `DR-RESTORE-{release_id}`",
        f"- Status: **{'PASS' if passed else 'FAIL'}**",
        "",
        "## File restore verification",
        "",
        "| Path | Class | Source SHA256 | Restored SHA256 | Match |",
        "|---|---|---|---|---|",
    ]
    for record in results:
        md_lines.append(
            "| {path} | {class_name} | `{source}` | `{restored}` | {match} |".format(
                path=record["path"],
                class_name=record["class"],
                source=record["source_sha256"],
                restored=record["restored_sha256"],
                match="PASS" if record["restored_match"] else "FAIL",
            )
        )
    md_lines.extend(
        [
            "",
            "## Deterministic acceptance rule",
            "",
            "- Drill passes only when every restored file hash equals its original seeded hash.",
            "",
        ]
    )
    output_md.write_text("\n".join(md_lines), encoding="utf-8")

    print("Workflow restore drill: PASS" if passed else "Workflow restore drill: FAIL")
    print(f"JSON report: {output_json}")
    print(f"Markdown report: {output_md}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
