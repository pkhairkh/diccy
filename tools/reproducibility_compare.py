#!/usr/bin/env python3
"""
Compare cross-target determinism manifests and enforce a mismatch budget.

Usage example:
  python3 tools/reproducibility_compare.py \
    --manifest x86_64=reports/.../actual-x86_64....toml \
    --manifest arm64=reports/.../actual-aarch64....toml \
    --manifest wasm32=reports/.../actual-wasm32....toml \
    --output-md reports/.../matrix.md \
    --output-json reports/.../matrix.json \
    --mismatch-budget 0
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict, List, Tuple


OutputKey = Tuple[str, str, int, str]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        action="append",
        required=True,
        metavar="LABEL=PATH",
        help="Target label and manifest path.",
    )
    parser.add_argument("--output-md", required=True, help="Markdown matrix output path.")
    parser.add_argument("--output-json", required=True, help="JSON output path.")
    parser.add_argument(
        "--mismatch-budget",
        type=int,
        default=0,
        help="Maximum allowed mismatched output keys.",
    )
    return parser.parse_args()


def parse_scalar(raw: str):
    raw = raw.strip()
    if raw.startswith('"') and raw.endswith('"'):
        return raw[1:-1]
    return int(raw)


def load_manifest(path: Path) -> dict:
    manifest: dict = {"samples": []}
    section = "root"
    current_sample: dict | None = None
    current_output: dict | None = None

    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == "[[samples]]":
            section = "sample"
            current_sample = {"expected_outputs": []}
            manifest["samples"].append(current_sample)
            current_output = None
            continue
        if stripped == "[[samples.expected_outputs]]":
            section = "output"
            if current_sample is None:
                raise ValueError(f"output entry without sample in {path}")
            current_output = {}
            current_sample["expected_outputs"].append(current_output)
            continue
        if "=" not in stripped:
            continue
        key, raw_value = (part.strip() for part in stripped.split("=", 1))
        value = parse_scalar(raw_value)
        if section == "root":
            manifest[key] = value
        elif section == "sample":
            if current_sample is None:
                raise ValueError(f"sample key without sample in {path}")
            current_sample[key] = value
        else:
            if current_output is None:
                raise ValueError(f"output key without output in {path}")
            current_output[key] = value
    return manifest


def parse_manifest_arg(arg: str) -> Tuple[str, Path]:
    if "=" not in arg:
        raise ValueError(f"invalid --manifest value {arg!r}, expected LABEL=PATH")
    label, raw_path = arg.split("=", 1)
    return label.strip(), Path(raw_path).resolve()


def build_output_index(manifest: dict) -> Dict[OutputKey, str]:
    out: Dict[OutputKey, str] = {}
    for sample in manifest.get("samples", []):
        sample_id = sample["id"]
        for output in sample.get("expected_outputs", []):
            key: OutputKey = (
                sample_id,
                output["config_id"],
                int(output["frame_index"]),
                output["format"],
            )
            out[key] = output["sha256"]
    return out


def render_markdown(summary_rows: List[dict], mismatch_rows: List[dict]) -> str:
    lines = [
        "# Cross-Target Reproducibility Matrix",
        "",
        "## Target Summary",
        "",
        "| Target | Profile | Sample Count | Output Count | Determinism SHA256 | Source Manifest SHA256 |",
        "|---|---|---:|---:|---|---|",
    ]
    for row in summary_rows:
        lines.append(
            f"| {row['target']} | {row['profile_id']} | {row['sample_count']} | "
            f"{row['expected_output_count']} | {row['determinism_sha256']} | {row['source_manifest_sha256']} |"
        )

    lines.extend(["", "## Mismatch Rows", ""])
    if not mismatch_rows:
        lines.append("No mismatches detected.")
        lines.append("")
        return "\n".join(lines)

    lines.append(
        "| Sample ID | Config | Frame | Format | Target Hashes |"
    )
    lines.append("|---|---|---:|---|---|")
    for row in mismatch_rows:
        target_hashes = ", ".join(
            f"{target}:{value}" for target, value in sorted(row["hashes"].items())
        )
        lines.append(
            f"| {row['sample_id']} | {row['config_id']} | {row['frame_index']} | "
            f"{row['format']} | {target_hashes} |"
        )
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()

    manifests: Dict[str, dict] = {}
    for item in args.manifest:
        label, path = parse_manifest_arg(item)
        manifests[label] = load_manifest(path)

    labels = sorted(manifests.keys())
    indexes = {label: build_output_index(manifest) for label, manifest in manifests.items()}
    all_keys = sorted({key for index in indexes.values() for key in index.keys()})

    mismatch_rows: List[dict] = []
    for sample_id, config_id, frame_index, fmt in all_keys:
        hashes: Dict[str, str] = {}
        for label in labels:
            hashes[label] = indexes[label].get(
                (sample_id, config_id, frame_index, fmt), "<missing>"
            )
        unique_hashes = set(hashes.values())
        if len(unique_hashes) > 1:
            mismatch_rows.append(
                {
                    "sample_id": sample_id,
                    "config_id": config_id,
                    "frame_index": frame_index,
                    "format": fmt,
                    "hashes": hashes,
                }
            )

    summary_rows: List[dict] = []
    for label in labels:
        manifest = manifests[label]
        summary_rows.append(
            {
                "target": label,
                "profile_id": manifest.get("profile_id", ""),
                "sample_count": int(manifest.get("sample_count", 0)),
                "expected_output_count": int(manifest.get("expected_output_count", 0)),
                "determinism_sha256": manifest.get("determinism_sha256", ""),
                "source_manifest_sha256": manifest.get("source_manifest_sha256", ""),
            }
        )

    output_md = Path(args.output_md)
    output_md.parent.mkdir(parents=True, exist_ok=True)
    output_md.write_text(render_markdown(summary_rows, mismatch_rows), encoding="utf-8")

    result = {
        "targets": labels,
        "mismatch_budget": args.mismatch_budget,
        "mismatch_count": len(mismatch_rows),
        "pass": len(mismatch_rows) <= args.mismatch_budget,
        "summary": summary_rows,
        "mismatches": mismatch_rows,
    }
    output_json = Path(args.output_json)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_json.write_text(json.dumps(result, indent=2, sort_keys=True), encoding="utf-8")

    print(f"targets: {', '.join(labels)}")
    print(f"mismatch_count: {len(mismatch_rows)}")
    print(f"mismatch_budget: {args.mismatch_budget}")
    print(f"matrix_md: {output_md}")
    print(f"matrix_json: {output_json}")

    return 0 if len(mismatch_rows) <= args.mismatch_budget else 1


if __name__ == "__main__":
    raise SystemExit(main())
