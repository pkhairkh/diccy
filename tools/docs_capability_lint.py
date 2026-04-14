#!/usr/bin/env python3
"""Capability-claim lint for major docs."""

from __future__ import annotations

import argparse
import pathlib
import re
import sys
from dataclasses import dataclass


DEFAULT_REPO_ROOT = pathlib.Path(__file__).resolve().parents[1]
TARGET_DOCS = (
    "README.md",
    "docs/01-Vision-and-Scope.md",
    "docs/06-Rendering-and-Interaction.md",
    "docs/08-WASM-Target.md",
)

STATUS_PATTERN = re.compile(r"As of\s+\d{4}-\d{2}-\d{2}")
STATUS_REF_PATTERN = re.compile(r"docs/31-Implementation-Status\.md")


@dataclass(frozen=True)
class Violation:
    path: str
    message: str


PROHIBITED_PATTERNS: tuple[tuple[str, re.Pattern[str]], ...] = (
    (
        "unsupported webgpu fallback claim",
        re.compile(r"WASM\s+via\s+`wgpu`/WebGPU\s+with\s+WebGL\s+fallback", re.IGNORECASE),
    ),
    (
        "unsupported mpr in-scope claim",
        re.compile(r"Basic\s+volume\s+assembly\s+and\s+MPR\s+for\s+selected\s+modalities\s+within\s+envelope", re.IGNORECASE),
    ),
    (
        "unsupported pixelcodec trait claim",
        re.compile(r"trait\s+PixelCodec\s*\{", re.IGNORECASE),
    ),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Lint capability claims in major docs.")
    parser.add_argument(
        "--repo-root",
        default=str(DEFAULT_REPO_ROOT),
        help="Repository root path.",
    )
    return parser.parse_args()


def run(repo_root: pathlib.Path) -> int:
    violations: list[Violation] = []

    for rel in TARGET_DOCS:
        path = repo_root / rel
        if not path.exists():
            violations.append(Violation(rel, "required target doc missing"))
            continue
        text = path.read_text(errors="ignore")

        for label, regex in PROHIBITED_PATTERNS:
            if regex.search(text):
                violations.append(Violation(rel, label))

        if not STATUS_PATTERN.search(text):
            violations.append(
                Violation(rel, "missing date-stamped capability assertion (As of YYYY-MM-DD)")
            )

        if not STATUS_REF_PATTERN.search(text):
            violations.append(
                Violation(rel, "missing reference to docs/31-Implementation-Status.md")
            )

    if violations:
        print("Capability claim lint: FAIL")
        print(f"Violations: {len(violations)}")
        for v in violations:
            print(f"{v.path}: {v.message}")
        return 1

    print("Capability claim lint: PASS")
    print("Violations: 0")
    return 0


def main() -> int:
    args = parse_args()
    repo_root = pathlib.Path(args.repo_root).resolve()
    return run(repo_root)


if __name__ == "__main__":
    sys.exit(main())
