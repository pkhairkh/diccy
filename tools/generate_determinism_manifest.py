#!/usr/bin/env python3
"""
Generate a release-scoped determinism manifest from corpus/manifest.toml.

The output is deterministic for a fixed tuple:
  (release_id, profile_id, source manifest bytes)
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        default="corpus/manifest.toml",
        help="Path to corpus manifest TOML.",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Path to write determinism manifest TOML.",
    )
    parser.add_argument(
        "--release-id",
        required=True,
        help="Release identifier, e.g. RC-2026.02.11.",
    )
    parser.add_argument(
        "--profile-id",
        default="cpu-oracle-baseline-v1",
        help="Determinism profile identifier.",
    )
    parser.add_argument(
        "--verify-existing",
        action="store_true",
        help="Verify existing output exactly matches generated content.",
    )
    return parser.parse_args()


def sha256_bytes(data: bytes) -> str:
    digest = hashlib.sha256()
    digest.update(data)
    return digest.hexdigest()


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
                raise ValueError("output entry without sample context")
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
                raise ValueError("sample key without sample context")
            current_sample[key] = value
        elif section == "output":
            if current_output is None:
                raise ValueError("output key without output context")
            current_output[key] = value
    return manifest


def sort_expected_outputs(items: list[dict]) -> list[dict]:
    return sorted(
        [
            {
                "config_id": entry["config_id"],
                "frame_index": int(entry["frame_index"]),
                "format": entry["format"],
                "sha256": entry["sha256"],
            }
            for entry in items
        ],
        key=lambda entry: (
            entry["config_id"],
            entry["frame_index"],
            entry["format"],
            entry["sha256"],
        ),
    )


def normalize_manifest(manifest: dict) -> dict:
    samples = []
    for sample in manifest.get("samples", []):
        normalized = {
            "id": sample["id"],
            "sha256": sample["sha256"],
            "sop_class_uid": sample["sop_class_uid"],
            "transfer_syntax_uid": sample["transfer_syntax_uid"],
            "expected_outputs": sort_expected_outputs(sample.get("expected_outputs", [])),
        }
        if sample.get("source_kind") is not None:
            normalized["source_kind"] = sample["source_kind"]
        if sample.get("feature") is not None:
            normalized["feature"] = sample["feature"]
        samples.append(normalized)
    samples.sort(key=lambda sample: sample["id"])
    return {
        "version": int(manifest["version"]),
        "hash_algorithm": manifest["hash_algorithm"],
        "samples": samples,
    }


def toml_escape(value: str) -> str:
    return value.replace("\\", "\\\\").replace('"', '\\"')


def render_manifest(
    *,
    release_id: str,
    profile_id: str,
    source_manifest_rel: str,
    source_manifest_sha256: str,
    normalized: dict,
) -> str:
    payload = {
        "release_id": release_id,
        "profile_id": profile_id,
        "manifest": normalized,
    }
    determinism_sha256 = sha256_bytes(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    )
    sample_count = len(normalized["samples"])
    expected_output_count = sum(
        len(sample["expected_outputs"]) for sample in normalized["samples"]
    )

    lines = [
        "schema_version = 1",
        f'release_id = "{toml_escape(release_id)}"',
        f'profile_id = "{toml_escape(profile_id)}"',
        f'source_manifest = "{toml_escape(source_manifest_rel)}"',
        f'source_manifest_sha256 = "{source_manifest_sha256}"',
        f'hash_algorithm = "{toml_escape(normalized["hash_algorithm"])}"',
        f"sample_count = {sample_count}",
        f"expected_output_count = {expected_output_count}",
        f'determinism_sha256 = "{determinism_sha256}"',
    ]

    for sample in normalized["samples"]:
        lines.append("")
        lines.append("[[samples]]")
        lines.append(f'id = "{toml_escape(sample["id"])}"')
        lines.append(f'sha256 = "{sample["sha256"]}"')
        lines.append(f'sop_class_uid = "{toml_escape(sample["sop_class_uid"])}"')
        lines.append(
            f'transfer_syntax_uid = "{toml_escape(sample["transfer_syntax_uid"])}"'
        )
        if sample.get("source_kind") is not None:
            lines.append(f'source_kind = "{toml_escape(sample["source_kind"])}"')
        if sample.get("feature") is not None:
            lines.append(f'feature = "{toml_escape(sample["feature"])}"')
        for output in sample["expected_outputs"]:
            lines.append("")
            lines.append("[[samples.expected_outputs]]")
            lines.append(f'config_id = "{toml_escape(output["config_id"])}"')
            lines.append(f'frame_index = {int(output["frame_index"])}')
            lines.append(f'format = "{toml_escape(output["format"])}"')
            lines.append(f'sha256 = "{output["sha256"]}"')
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    source_path = (repo_root / args.source).resolve()
    output_path = (repo_root / args.output).resolve()

    source_bytes = source_path.read_bytes()
    source_manifest_sha256 = sha256_bytes(source_bytes)
    normalized = normalize_manifest(load_manifest(source_path))
    rendered = render_manifest(
        release_id=args.release_id,
        profile_id=args.profile_id,
        source_manifest_rel=str(Path(args.source)),
        source_manifest_sha256=source_manifest_sha256,
        normalized=normalized,
    )

    if args.verify_existing:
        if not output_path.exists():
            raise SystemExit(f"missing output file for verification: {output_path}")
        existing = output_path.read_text(encoding="utf-8")
        if existing != rendered:
            raise SystemExit(
                f"determinism manifest mismatch: regenerate {output_path} from {source_path}"
            )
        print(f"determinism manifest verified: {output_path}")
        return 0

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(rendered, encoding="utf-8")
    print(f"wrote {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
