#!/usr/bin/env python3
"""Sign and verify release evidence bundles using deterministic HMAC-SHA256."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import hmac
import json
import pathlib
import sys
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Sign or verify release evidence bundles.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    sign = subparsers.add_parser("sign", help="Sign a set of evidence artifacts.")
    sign.add_argument("--release-id", required=True, help="Release identifier (for example RC-2026.02.12).")
    sign.add_argument("--key-id", required=True, help="Signing key identifier label.")
    sign.add_argument(
        "--key-env",
        default="EVIDENCE_SIGNING_KEY",
        help="Environment variable containing the signing key bytes.",
    )
    sign.add_argument(
        "--repo-root",
        default=".",
        help="Repository root used to resolve relative artifact paths.",
    )
    sign.add_argument(
        "--artifact",
        action="append",
        required=True,
        help="Artifact path relative to repo root. May be repeated.",
    )
    sign.add_argument("--output", required=True, help="Signed bundle JSON output path.")

    verify = subparsers.add_parser("verify", help="Verify a signed evidence bundle.")
    verify.add_argument("--bundle", required=True, help="Signed bundle JSON path.")
    verify.add_argument(
        "--key-env",
        default="EVIDENCE_SIGNING_KEY",
        help="Environment variable containing the signing key bytes.",
    )
    verify.add_argument(
        "--repo-root",
        default=".",
        help="Repository root used to resolve relative artifact paths.",
    )

    return parser.parse_args()


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 64)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def get_key_from_env(env_name: str) -> bytes:
    value = __import__("os").environ.get(env_name)
    if value is None or value == "":
        raise ValueError(f"signing key missing in environment variable: {env_name}")
    return value.encode("utf-8")


def build_canonical_payload(bundle: dict[str, Any]) -> dict[str, Any]:
    return {
        "release_id": bundle["release_id"],
        "key_id": bundle["key_id"],
        "signature_algorithm": bundle["signature_algorithm"],
        "hash_algorithm": bundle["hash_algorithm"],
        "artifacts": sorted(bundle["artifacts"], key=lambda item: item["path"]),
    }


def compute_signature(bundle: dict[str, Any], key: bytes) -> str:
    canonical = build_canonical_payload(bundle)
    encoded = json.dumps(canonical, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hmac.new(key, encoded, hashlib.sha256).hexdigest()


def sign_bundle(
    *,
    repo_root: pathlib.Path,
    release_id: str,
    key_id: str,
    key_env: str,
    artifacts: list[str],
) -> dict[str, Any]:
    key = get_key_from_env(key_env)

    rows: list[dict[str, Any]] = []
    for rel in sorted(set(artifacts)):
        path = (repo_root / rel).resolve()
        if not path.exists() or not path.is_file():
            raise FileNotFoundError(f"artifact not found: {rel}")
        rows.append(
            {
                "path": rel,
                "size_bytes": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )

    bundle: dict[str, Any] = {
        "release_id": release_id,
        "key_id": key_id,
        "signature_algorithm": "hmac-sha256",
        "hash_algorithm": "sha256",
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "artifacts": rows,
    }
    bundle["signature"] = compute_signature(bundle, key)
    return bundle


def verify_bundle(*, repo_root: pathlib.Path, key_env: str, bundle: dict[str, Any]) -> tuple[bool, list[str]]:
    errors: list[str] = []

    expected_algorithm = "hmac-sha256"
    if bundle.get("signature_algorithm") != expected_algorithm:
        errors.append("unsupported signature_algorithm")

    expected_hash_algorithm = "sha256"
    if bundle.get("hash_algorithm") != expected_hash_algorithm:
        errors.append("unsupported hash_algorithm")

    artifacts = bundle.get("artifacts", [])
    if not isinstance(artifacts, list) or not artifacts:
        errors.append("bundle artifacts must be a non-empty list")
        return (False, errors)

    for item in artifacts:
        rel = item.get("path")
        expected_sha = item.get("sha256")
        if not isinstance(rel, str) or not rel:
            errors.append("artifact path missing")
            continue
        path = (repo_root / rel).resolve()
        if not path.exists() or not path.is_file():
            errors.append(f"artifact missing: {rel}")
            continue
        observed_sha = sha256_file(path)
        if observed_sha != expected_sha:
            errors.append(f"artifact digest mismatch: {rel}")

    key = get_key_from_env(key_env)
    expected_signature = compute_signature(bundle, key)
    observed_signature = bundle.get("signature")
    if observed_signature != expected_signature:
        errors.append("bundle signature mismatch")

    return (len(errors) == 0, errors)


def run() -> int:
    args = parse_args()

    if args.command == "sign":
        repo_root = pathlib.Path(args.repo_root).resolve()
        bundle = sign_bundle(
            repo_root=repo_root,
            release_id=args.release_id,
            key_id=args.key_id,
            key_env=args.key_env,
            artifacts=args.artifact,
        )
        output = pathlib.Path(args.output).resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(bundle, indent=2, sort_keys=True) + "\n")
        print("Evidence signing: PASS")
        print(f"release_id: {bundle['release_id']}")
        print(f"artifact_count: {len(bundle['artifacts'])}")
        print(f"bundle: {output}")
        return 0

    if args.command == "verify":
        repo_root = pathlib.Path(args.repo_root).resolve()
        bundle_path = pathlib.Path(args.bundle).resolve()
        if not bundle_path.exists():
            print("Evidence verify: FAIL")
            print(f"error: bundle not found: {bundle_path}")
            return 1
        bundle = json.loads(bundle_path.read_text())
        ok, errors = verify_bundle(repo_root=repo_root, key_env=args.key_env, bundle=bundle)
        if ok:
            print("Evidence verify: PASS")
            print(f"bundle: {bundle_path}")
            print(f"artifact_count: {len(bundle['artifacts'])}")
            return 0
        print("Evidence verify: FAIL")
        for error in errors:
            print(f"error: {error}")
        return 1

    print("Evidence sign/verify: FAIL")
    print("error: unknown command")
    return 1


if __name__ == "__main__":
    sys.exit(run())
