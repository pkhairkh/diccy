#!/usr/bin/env python3
"""
Offline dependency and SBOM review utility for release evidence.

Reads Cargo.lock, emits:
- deterministic SBOM JSON
- markdown vulnerability/policy review report
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import pathlib
import re
import sys
from typing import Any

CODEC_DEPENDENCY_KEYWORDS = (
    "jpeg",
    "j2k",
    "jpeg2000",
    "jpegls",
    "charls",
    "codec",
    "tiff",
    "png",
    "image",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Offline security dependency review.")
    parser.add_argument("--cargo-lock", default="Cargo.lock", help="Path to Cargo.lock.")
    parser.add_argument(
        "--output-sbom-json",
        required=True,
        help="Path to SBOM JSON output.",
    )
    parser.add_argument(
        "--output-md",
        required=True,
        help="Path to markdown dependency review report output.",
    )
    parser.add_argument(
        "--run-label",
        required=True,
        help="Run label for release evidence.",
    )
    parser.add_argument(
        "--fail-on-blocking-findings",
        action="store_true",
        help="Fail when blocking policy findings are detected.",
    )
    return parser.parse_args()


def load_lock(path: pathlib.Path) -> dict[str, Any]:
    """
    Parse Cargo.lock package rows using a deterministic line parser.
    """
    packages: list[dict[str, Any]] = []
    current: dict[str, Any] | None = None
    key_value = re.compile(r'^([A-Za-z0-9_]+)\s*=\s*"(.*)"\s*$')
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line == "[[package]]":
            if current and "name" in current and "version" in current:
                packages.append(current)
            current = {}
            continue
        if current is None:
            continue
        match = key_value.match(line)
        if not match:
            continue
        key = match.group(1)
        value = match.group(2)
        if key in {"name", "version", "source", "checksum"}:
            current[key] = value
    if current and "name" in current and "version" in current:
        packages.append(current)
    return {"package": packages}


def classify_source(source: str | None) -> str:
    if source is None:
        return "workspace/path"
    if source.startswith("registry+"):
        return "registry"
    if source.startswith("git+"):
        return "git"
    return "other"


def is_codec_dependency(name: str) -> bool:
    lowered = name.strip().lower()
    return any(keyword in lowered for keyword in CODEC_DEPENDENCY_KEYWORDS)


def scan_workspace_unsafe_usage(workspace_root: pathlib.Path) -> list[dict[str, Any]]:
    findings: list[dict[str, Any]] = []
    for path in sorted(workspace_root.rglob("*.rs")):
        rel = path.relative_to(workspace_root)
        if not str(rel).startswith("crates/"):
            continue
        for line_no, line in enumerate(path.read_text(encoding="utf-8", errors="ignore").splitlines(), start=1):
            stripped = line.strip()
            if stripped.startswith("//"):
                continue
            if "unsafe " in stripped or stripped.startswith("unsafe{") or stripped.startswith("unsafe{"):
                findings.append(
                    {
                        "path": str(rel),
                        "line": line_no,
                        "snippet": stripped[:200],
                    }
                )
    return findings


def evaluate_packages(packages: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    sbom_packages: list[dict[str, Any]] = []
    findings: list[dict[str, str]] = []
    for package in sorted(packages, key=lambda row: (str(row.get("name", "")), str(row.get("version", "")))):
        name = str(package.get("name", ""))
        version = str(package.get("version", ""))
        source = package.get("source")
        source_str = str(source) if source is not None else None
        source_kind = classify_source(source_str)
        checksum = package.get("checksum")
        checksum_str = str(checksum) if checksum is not None else None

        if source_kind == "git":
            findings.append(
                {
                    "severity": "P1",
                    "id": "DEP-GIT-SOURCE",
                    "package": f"{name}@{version}",
                    "detail": "git dependency source is not allowed in release baseline",
                }
            )
        if source_kind == "other":
            findings.append(
                {
                    "severity": "P1",
                    "id": "DEP-UNKNOWN-SOURCE",
                    "package": f"{name}@{version}",
                    "detail": f"unknown dependency source '{source_str}'",
                }
            )
        if source_kind == "registry" and not checksum_str:
            findings.append(
                {
                    "severity": "P1",
                    "id": "DEP-MISSING-CHECKSUM",
                    "package": f"{name}@{version}",
                    "detail": "registry dependency missing checksum in Cargo.lock",
                }
            )

        sbom_packages.append(
            {
                "name": name,
                "version": version,
                "source": source_str,
                "source_kind": source_kind,
                "checksum": checksum_str,
            }
        )
    return sbom_packages, findings


def write_json(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def write_markdown(path: pathlib.Path, *, payload: dict[str, Any], findings: list[dict[str, str]]) -> None:
    registry_count = sum(1 for package in payload["packages"] if package["source_kind"] == "registry")
    workspace_count = sum(1 for package in payload["packages"] if package["source_kind"] == "workspace/path")
    git_count = sum(1 for package in payload["packages"] if package["source_kind"] == "git")
    codec_count = len(payload.get("codec_dependencies", []))
    unsafe_count = len(payload.get("workspace_unsafe_usage", []))
    lines = [
        "# Dependency and SBOM Vulnerability Review",
        "",
        f"Run Label: {payload['run_label']}",
        f"Generated At (UTC): {payload['generated_at_utc']}",
        f"SBOM Digest (sha256): `{payload['sbom_digest_sha256']}`",
        "",
        "## Summary",
        "",
        f"- Total packages: {len(payload['packages'])}",
        f"- Registry packages: {registry_count}",
        f"- Workspace/path packages: {workspace_count}",
        f"- Git packages: {git_count}",
        f"- Codec dependency entries: {codec_count}",
        f"- Workspace unsafe usages: {unsafe_count}",
        f"- Blocking findings: {len(findings)}",
        "",
    ]
    if findings:
        lines.extend(
            [
                "## Blocking Findings",
                "",
                "| Severity | Finding ID | Package | Detail |",
                "|---|---|---|---|",
            ]
        )
        for finding in findings:
            lines.append(
                "| "
                + " | ".join(
                    [
                        finding["severity"],
                        finding["id"],
                        finding["package"],
                        finding["detail"],
                    ]
                )
                + " |"
            )
    else:
        lines.extend(
            [
                "## Blocking Findings",
                "",
                "No blocking dependency-source or checksum policy findings detected.",
            ]
        )
    lines.extend(
        [
            "",
            "## Codec Dependency Snapshot",
            "",
        ]
    )
    codec_deps = payload.get("codec_dependencies", [])
    if codec_deps:
        lines.extend(
            [
                "| Package | Version | Source Kind |",
                "|---|---|---|",
            ]
        )
        for row in codec_deps:
            lines.append(f"| {row['name']} | {row['version']} | {row['source_kind']} |")
    else:
        lines.append("No codec-related dependencies detected.")

    lines.extend(
        [
            "",
            "## Workspace Unsafe Usage Snapshot",
            "",
        ]
    )
    unsafe_rows = payload.get("workspace_unsafe_usage", [])
    if unsafe_rows:
        lines.extend(
            [
                "| File | Line | Snippet |",
                "|---|---:|---|",
            ]
        )
        for row in unsafe_rows[:100]:
            snippet = str(row["snippet"]).replace("|", "\\|")
            lines.append(f"| `{row['path']}` | {row['line']} | `{snippet}` |")
        if len(unsafe_rows) > 100:
            lines.append(f"| ... | ... | truncated {len(unsafe_rows) - 100} additional rows |")
    else:
        lines.append("No unsafe usage markers found under `crates/`.")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n")


def run_review() -> int:
    args = parse_args()
    lock_path = pathlib.Path(args.cargo_lock).resolve()
    if not lock_path.exists():
        print("Security dependency review: FAIL")
        print(f"error: Cargo.lock not found: {lock_path}")
        return 1

    lock_payload = load_lock(lock_path)
    raw_packages = lock_payload.get("package", [])
    if not isinstance(raw_packages, list):
        print("Security dependency review: FAIL")
        print("error: invalid Cargo.lock package array")
        return 1

    packages, findings = evaluate_packages(raw_packages)
    codec_dependencies = [row for row in packages if is_codec_dependency(str(row.get("name", "")))]
    workspace_unsafe_usage = scan_workspace_unsafe_usage(lock_path.parent)
    generated_at = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    digest_source = json.dumps(packages, sort_keys=True, separators=(",", ":")).encode("utf-8")
    digest = hashlib.sha256(digest_source).hexdigest()

    payload: dict[str, Any] = {
        "run_label": args.run_label,
        "generated_at_utc": generated_at,
        "cargo_lock_path": str(lock_path),
        "packages": packages,
        "codec_dependencies": codec_dependencies,
        "workspace_unsafe_usage": workspace_unsafe_usage,
        "blocking_findings": findings,
        "sbom_digest_sha256": digest,
    }

    output_sbom = pathlib.Path(args.output_sbom_json).resolve()
    output_md = pathlib.Path(args.output_md).resolve()
    write_json(output_sbom, payload)
    write_markdown(output_md, payload=payload, findings=findings)

    print("Security dependency review: PASS" if not findings else "Security dependency review: FAIL")
    print(f"Packages: {len(packages)}")
    print(f"Blocking findings: {len(findings)}")
    print(f"SBOM digest: sha256:{digest}")

    if args.fail_on_blocking_findings and findings:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(run_review())
