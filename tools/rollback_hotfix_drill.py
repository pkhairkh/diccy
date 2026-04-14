#!/usr/bin/env python3
"""
Deterministic rollback/hotfix drill automation.

Generates release-scoped artifacts under reports/release and verifies that
required rollback anchors exist for the target release identifier.
"""

from __future__ import annotations

import argparse
import json
import pathlib


REQUIRED_RELEASE_ANCHORS = (
    "release-preflight-summary-{release_id}.json",
    "release-artifact-integrity-{release_id}.json",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run deterministic rollback/hotfix drill.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument(
        "--output-json",
        help="Output JSON report path. Default: reports/release/rollback-hotfix-drill-<release-id>.json",
    )
    parser.add_argument(
        "--output-md",
        help="Output Markdown report path. Default: reports/release/rollback-hotfix-drill-<release-id>.md",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(__file__).resolve().parent.parent
    release_dir = repo_root / "reports" / "release"
    release_dir.mkdir(parents=True, exist_ok=True)
    release_id = args.release_id

    output_json = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (release_dir / f"rollback-hotfix-drill-{release_id}.json").resolve()
    )
    output_md = (
        pathlib.Path(args.output_md).resolve()
        if args.output_md
        else (release_dir / f"rollback-hotfix-drill-{release_id}.md").resolve()
    )

    checks: list[dict[str, object]] = []
    for pattern in REQUIRED_RELEASE_ANCHORS:
        relative = pathlib.Path("reports/release") / pattern.format(release_id=release_id)
        absolute = repo_root / relative
        checks.append(
            {
                "check": f"anchor-exists:{relative.name}",
                "path": str(relative),
                "passed": absolute.exists(),
                "detail": "found" if absolute.exists() else "missing",
            }
        )

    passed = all(bool(item["passed"]) for item in checks)
    payload = {
        "release_id": release_id,
        "drill_id": f"RBHF-{release_id}",
        "status": "PASS" if passed else "FAIL",
        "checks": checks,
        "step_order": [
            "freeze-release-candidate",
            "select-last-known-good-artifacts",
            "simulate-rollback-config-apply",
            "simulate-hotfix-apply",
            "verify-governance-anchor-continuity",
        ],
    }
    output_json.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    lines = [
        "# Rollback and Hotfix Drill",
        "",
        f"- Release ID: `{release_id}`",
        f"- Drill ID: `RBHF-{release_id}`",
        f"- Status: **{'PASS' if passed else 'FAIL'}**",
        "",
        "## Verification checks",
        "",
        "| Check | Path | Result | Detail |",
        "|---|---|---|---|",
    ]
    for item in checks:
        lines.append(
            "| {check} | `{path}` | {result} | {detail} |".format(
                check=item["check"],
                path=item["path"],
                result="PASS" if item["passed"] else "FAIL",
                detail=item["detail"],
            )
        )
    lines.extend(
        [
            "",
            "## Deterministic step order",
            "",
            "1. freeze-release-candidate",
            "2. select-last-known-good-artifacts",
            "3. simulate-rollback-config-apply",
            "4. simulate-hotfix-apply",
            "5. verify-governance-anchor-continuity",
            "",
        ]
    )
    output_md.write_text("\n".join(lines), encoding="utf-8")

    print("Rollback/hotfix drill: PASS" if passed else "Rollback/hotfix drill: FAIL")
    print(f"JSON report: {output_json}")
    print(f"Markdown report: {output_md}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
