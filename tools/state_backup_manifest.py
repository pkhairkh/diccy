#!/usr/bin/env python3
"""
Generate immutable backup manifest and release-linked restore verification report.
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
    parser = argparse.ArgumentParser(description="Generate state backup manifest and restore report.")
    parser.add_argument("--repo-root", default=".", help="Repository root.")
    parser.add_argument("--state-root", required=True, help="State directory to inventory.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument(
        "--output-manifest",
        default="reports/release/state-backup-manifest-<release-id>.json",
        help="Output manifest path (relative to repo root).",
    )
    parser.add_argument(
        "--output-restore-report",
        default="reports/release/state-restore-verification-<release-id>.md",
        help="Output restore verification report path (relative to repo root).",
    )
    return parser.parse_args()


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(8192)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def collect_state_files(state_root: pathlib.Path) -> list[dict[str, Any]]:
    files: list[dict[str, Any]] = []
    for path in sorted(state_root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(state_root).as_posix()
        files.append(
            {
                "relative_path": rel,
                "size_bytes": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )
    return files


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    state_root = (repo_root / args.state_root).resolve()
    if not state_root.exists() or not state_root.is_dir():
        print(f"error: state root not found or not a directory: {state_root}")
        return 1

    output_manifest_rel = args.output_manifest.replace("<release-id>", args.release_id)
    output_report_rel = args.output_restore_report.replace("<release-id>", args.release_id)
    output_manifest = (repo_root / output_manifest_rel).resolve()
    output_report = (repo_root / output_report_rel).resolve()

    files = collect_state_files(state_root)
    payload = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "release_id": args.release_id,
        "state_root": str(state_root),
        "immutable": True,
        "file_count": len(files),
        "files": files,
    }
    manifest_json = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    manifest_sha = hashlib.sha256(manifest_json.encode("utf-8")).hexdigest()

    output_manifest.parent.mkdir(parents=True, exist_ok=True)
    output_manifest.write_text(manifest_json, encoding="utf-8")

    output_report.parent.mkdir(parents=True, exist_ok=True)
    output_report.write_text(
        "\n".join(
            [
                f"# State Restore Verification - {args.release_id}",
                "",
                "## Backup manifest",
                f"- Path: `{output_manifest_rel}`",
                f"- SHA256: `{manifest_sha}`",
                f"- Files captured: `{len(files)}`",
                "",
                "## Restore verification checklist",
                "- [ ] Manifest file hashes validated before restore",
                "- [ ] Restored files match expected relative paths",
                "- [ ] Restored files match expected sha256 values",
                "- [ ] Runtime restarted successfully with restored state",
                "",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"manifest: {output_manifest}")
    print(f"restore-report: {output_report}")
    print(f"files: {len(files)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
