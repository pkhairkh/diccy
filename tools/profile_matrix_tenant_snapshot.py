#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import sys


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Generate multi-tenant configuration snapshots from profile_matrix.json."
    )
    parser.add_argument("--repo-root", default=".", help="Repository root")
    parser.add_argument(
        "--matrix",
        default="tools/profile_matrix.json",
        help="Profile matrix input path",
    )
    parser.add_argument(
        "--tenants",
        default="tenant-default,tenant-a,tenant-b",
        help="Comma-separated tenant ids for snapshot generation",
    )
    parser.add_argument(
        "--output",
        default="reports/docs/profile-matrix-tenant-snapshots.json",
        help="Snapshot output path",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = pathlib.Path(args.repo_root).resolve()
    matrix_path = root / args.matrix
    output_path = root / args.output

    matrix = json.loads(matrix_path.read_text(encoding="utf-8"))
    profiles = matrix.get("profiles", {})
    tenants = [tenant.strip() for tenant in args.tenants.split(",") if tenant.strip()]

    snapshots: list[dict[str, object]] = []
    for tenant in tenants:
        for profile_name, profile in profiles.items():
            snapshots.append(
                {
                    "tenant": tenant,
                    "profile": profile_name,
                    "features": profile.get("features", []),
                    "binaries": profile.get("binaries", []),
                    "required_for_documentation": bool(profile.get("required_for_documentation", False)),
                }
            )

    payload = {
        "tenant_count": len(tenants),
        "profile_count": len(profiles),
        "snapshot_count": len(snapshots),
        "snapshots": snapshots,
    }
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(f"Generated tenant snapshots: {len(snapshots)}")
    print(f"Output: {output_path.relative_to(root)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
