#!/usr/bin/env python3
"""
Release-governance gate for production release controls.

Checks:
- release notes align with template sections and include populated values
- preflight + artifact-integrity artifacts exist
- preflight summary contains no failed gates
- release_id is consistent across release evidence
- expected profile artifacts exist and include capability.manifest.json
- signed evidence bundle verifies with evidence_sign_verify.py
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import subprocess
import sys
import tarfile
from typing import Any

SECTION_RE = re.compile(r"^##\s+(.+?)\s*$")
RELEASE_ID_INLINE_RE = re.compile(r"`([^`]+)`")
RELEASE_ID_KEY_RE = re.compile(r"release_id[^A-Za-z0-9]+([A-Za-z0-9._-]+)", re.IGNORECASE)

REQUIRED_PROFILES = [
    "framework-core",
    "workstation",
    "backend-services",
    "backend-services-with-dimse",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate release governance controls.")
    parser.add_argument("--release-id", required=True, help="Release identifier.")
    parser.add_argument("--repo-root", default=".", help="Repository root path.")
    parser.add_argument(
        "--release-notes-template",
        default="reports/release/release-notes-template.md",
        help="Release notes template path.",
    )
    parser.add_argument(
        "--release-notes",
        help="Release notes path. Default: reports/release/release-notes-<release-id>.md",
    )
    parser.add_argument(
        "--preflight-json",
        help="Preflight summary JSON path. Default: reports/release/release-preflight-summary-<release-id>.json",
    )
    parser.add_argument(
        "--artifact-integrity-json",
        help="Artifact integrity JSON path. Default: reports/release/release-artifact-integrity-<release-id>.json",
    )
    parser.add_argument(
        "--traceability-manifest-json",
        help="Traceability manifest path. Default: reports/traceability/release-traceability-manifest-<release-id>.json",
    )
    parser.add_argument(
        "--security-gate-decision",
        help="Security gate decision path. Default: reports/security/security-gate-decision-<release-id>.md",
    )
    parser.add_argument(
        "--performance-gate-decision",
        help="Performance gate decision path. Default: reports/performance/performance-scalability-gate-decision-<release-id>.md",
    )
    parser.add_argument(
        "--profiles-dir",
        default="dist/profiles",
        help="Profile artifact directory.",
    )
    parser.add_argument(
        "--profiles-metadata",
        default="dist/profiles/profiles.metadata.json",
        help="Profiles metadata JSON path.",
    )
    parser.add_argument(
        "--signed-bundle",
        default="dist/profiles/profiles.metadata.bundle.signed.json",
        help="Signed evidence bundle path for verification.",
    )
    parser.add_argument(
        "--key-env",
        default="EVIDENCE_SIGNING_KEY",
        help="Environment variable used for evidence verification.",
    )
    parser.add_argument(
        "--output-json",
        help="Output JSON report path. Default: reports/release/release-controls-gate-<release-id>.json",
    )
    parser.add_argument(
        "--skip-signature-verify",
        action="store_true",
        help="Skip signed evidence verification (for dry-run/testing only).",
    )
    return parser.parse_args()


def parse_sections(path: pathlib.Path) -> tuple[list[str], dict[str, list[str]]]:
    headings: list[str] = []
    sections: dict[str, list[str]] = {}
    current: str | None = None
    for raw_line in path.read_text(encoding="utf-8", errors="ignore").splitlines():
        match = SECTION_RE.match(raw_line.strip())
        if match:
            current = match.group(1).strip()
            headings.append(current)
            sections.setdefault(current, [])
            continue
        if current is not None:
            sections[current].append(raw_line.rstrip())
    return headings, sections


def section_populated(lines: list[str]) -> bool:
    for line in lines:
        stripped = line.strip()
        if not stripped:
            continue
        if stripped.startswith("-"):
            stripped = stripped.lstrip("-").strip()
        if not stripped:
            continue
        lowered = stripped.lower()
        if "<release" in lowered:
            continue
        if lowered.endswith(":"):
            continue
        if lowered.startswith("declaration:"):
            continue
        if lowered.startswith("if `moved`") or lowered.startswith("if `unchanged`"):
            continue
        return True
    return False


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return payload


def add_result(results: list[dict[str, Any]], check_id: str, passed: bool, details: str) -> None:
    results.append(
        {
            "check_id": check_id,
            "status": "PASS" if passed else "FAIL",
            "details": details,
        }
    )


def extract_release_id_from_notes(path: pathlib.Path) -> str | None:
    headings, sections = parse_sections(path)
    if "Release ID" in headings:
        for line in sections.get("Release ID", []):
            match = RELEASE_ID_INLINE_RE.search(line)
            if match:
                return match.group(1).strip()
            key_match = RELEASE_ID_KEY_RE.search(line)
            if key_match:
                return key_match.group(1).strip()
    text = path.read_text(encoding="utf-8", errors="ignore")
    key_match = RELEASE_ID_KEY_RE.search(text)
    if key_match:
        return key_match.group(1).strip()
    return None


def capability_manifest_contains(path: pathlib.Path, binary_name: str) -> tuple[bool, str]:
    with tarfile.open(path, "r:gz") as archive:
        names = set(archive.getnames())
        if "capability.manifest.json" not in names:
            return (False, "missing capability.manifest.json")
        handle = archive.extractfile("capability.manifest.json")
        if handle is None:
            return (False, "unable to extract capability.manifest.json")
        payload = json.loads(handle.read().decode("utf-8"))
    direct = payload.get("binary", [])
    expanded = payload.get("binaries", [])
    all_names: set[str] = set()
    if isinstance(direct, list):
        all_names.update(str(item) for item in direct)
    if isinstance(expanded, list):
        for item in expanded:
            if isinstance(item, dict) and isinstance(item.get("name"), str):
                all_names.add(item["name"])
    return (binary_name in all_names, f"manifest binaries: {sorted(all_names)}")


def run_gate(args: argparse.Namespace) -> tuple[int, dict[str, Any]]:
    repo_root = pathlib.Path(args.repo_root).resolve()
    release_id = args.release_id
    release_notes_template = (repo_root / args.release_notes_template).resolve()
    release_notes = (
        pathlib.Path(args.release_notes).resolve()
        if args.release_notes
        else (repo_root / f"reports/release/release-notes-{release_id}.md").resolve()
    )
    preflight_json = (
        pathlib.Path(args.preflight_json).resolve()
        if args.preflight_json
        else (repo_root / f"reports/release/release-preflight-summary-{release_id}.json").resolve()
    )
    artifact_integrity_json = (
        pathlib.Path(args.artifact_integrity_json).resolve()
        if args.artifact_integrity_json
        else (repo_root / f"reports/release/release-artifact-integrity-{release_id}.json").resolve()
    )
    traceability_manifest = (
        pathlib.Path(args.traceability_manifest_json).resolve()
        if args.traceability_manifest_json
        else (repo_root / f"reports/traceability/release-traceability-manifest-{release_id}.json").resolve()
    )
    security_gate_decision = (
        pathlib.Path(args.security_gate_decision).resolve()
        if args.security_gate_decision
        else (repo_root / f"reports/security/security-gate-decision-{release_id}.md").resolve()
    )
    performance_gate_decision = (
        pathlib.Path(args.performance_gate_decision).resolve()
        if args.performance_gate_decision
        else (repo_root / f"reports/performance/performance-scalability-gate-decision-{release_id}.md").resolve()
    )
    profiles_dir = (repo_root / args.profiles_dir).resolve()
    profiles_metadata = (repo_root / args.profiles_metadata).resolve()
    signed_bundle = (repo_root / args.signed_bundle).resolve()

    results: list[dict[str, Any]] = []
    release_id_sources: dict[str, str] = {}

    if not release_notes_template.exists():
        add_result(results, "release-notes-template-exists", False, f"missing {release_notes_template}")
    elif not release_notes.exists():
        add_result(results, "release-notes-exists", False, f"missing {release_notes}")
    else:
        template_headings, _ = parse_sections(release_notes_template)
        notes_headings, notes_sections = parse_sections(release_notes)
        missing_sections = [section for section in template_headings if section not in notes_headings]
        unpopulated_sections = [
            section for section in template_headings if section in notes_sections and not section_populated(notes_sections[section])
        ]
        add_result(
            results,
            "release-notes-template-sections",
            not missing_sections,
            "missing sections: " + ", ".join(missing_sections) if missing_sections else "all template sections present",
        )
        add_result(
            results,
            "release-notes-template-populated",
            not unpopulated_sections,
            "unpopulated sections: " + ", ".join(unpopulated_sections)
            if unpopulated_sections
            else "all required template sections populated",
        )
        release_notes_id = extract_release_id_from_notes(release_notes)
        if release_notes_id:
            release_id_sources["release-notes"] = release_notes_id
        else:
            add_result(results, "release-notes-release-id", False, "release_id not found in release notes")

    if preflight_json.exists():
        preflight_payload = load_json(preflight_json)
        preflight_id = preflight_payload.get("release_id")
        if isinstance(preflight_id, str):
            release_id_sources["preflight-summary"] = preflight_id
        failing_gates = [
            gate
            for gate in preflight_payload.get("gates", [])
            if isinstance(gate, dict) and gate.get("status") != "PASS"
        ]
        gate_count = int(preflight_payload.get("failed_gate_count", -1))
        add_result(results, "preflight-summary-exists", True, f"found {preflight_json}")
        add_result(
            results,
            "preflight-summary-gates-pass",
            gate_count == 0 and not failing_gates and preflight_payload.get("overall_status") == "PASS",
            f"failed_gate_count={gate_count}, failing_gates={len(failing_gates)}",
        )
    else:
        add_result(results, "preflight-summary-exists", False, f"missing {preflight_json}")
        add_result(results, "preflight-summary-gates-pass", False, "cannot evaluate failed gate count")

    if artifact_integrity_json.exists():
        artifact_payload = load_json(artifact_integrity_json)
        artifact_id = artifact_payload.get("release_id")
        if isinstance(artifact_id, str):
            release_id_sources["artifact-integrity"] = artifact_id
        add_result(results, "artifact-integrity-json-exists", True, f"found {artifact_integrity_json}")
    else:
        add_result(results, "artifact-integrity-json-exists", False, f"missing {artifact_integrity_json}")

    if traceability_manifest.exists():
        traceability_payload = load_json(traceability_manifest)
        traceability_id = traceability_payload.get("release_id")
        if isinstance(traceability_id, str):
            release_id_sources["traceability-manifest"] = traceability_id
        add_result(results, "traceability-manifest-exists", True, f"found {traceability_manifest}")
    else:
        add_result(results, "traceability-manifest-exists", False, f"missing {traceability_manifest}")

    if profiles_metadata.exists():
        metadata_payload = load_json(profiles_metadata)
        metadata_id = metadata_payload.get("release_id")
        if isinstance(metadata_id, str):
            release_id_sources["profiles-metadata"] = metadata_id
        add_result(results, "profiles-metadata-exists", True, f"found {profiles_metadata}")
    else:
        add_result(results, "profiles-metadata-exists", False, f"missing {profiles_metadata}")

    add_result(
        results,
        "security-gate-decision-exists",
        security_gate_decision.exists(),
        f"path={security_gate_decision}",
    )
    add_result(
        results,
        "performance-gate-decision-exists",
        performance_gate_decision.exists(),
        f"path={performance_gate_decision}",
    )

    inconsistent_sources = {
        source: observed_release_id
        for source, observed_release_id in release_id_sources.items()
        if observed_release_id != release_id
    }
    add_result(
        results,
        "release-id-consistency",
        not inconsistent_sources and bool(release_id_sources),
        "consistent release_id across sources"
        if not inconsistent_sources and release_id_sources
        else f"inconsistent sources: {inconsistent_sources}",
    )

    existing_profile_artifacts: list[str] = []
    missing_profile_artifacts: list[str] = []
    missing_manifest_profiles: list[str] = []
    for profile in REQUIRED_PROFILES:
        artifact = profiles_dir / f"{profile}.{release_id}.tar.gz"
        if artifact.exists():
            existing_profile_artifacts.append(str(artifact.relative_to(repo_root)))
            try:
                with tarfile.open(artifact, "r:gz") as archive:
                    names = set(archive.getnames())
                if "capability.manifest.json" not in names:
                    missing_manifest_profiles.append(profile)
            except tarfile.TarError:
                missing_manifest_profiles.append(profile)
        else:
            missing_profile_artifacts.append(str(artifact.relative_to(repo_root)))

    add_result(
        results,
        "required-profile-artifacts-exist",
        not missing_profile_artifacts,
        "all required profile artifacts present"
        if not missing_profile_artifacts
        else "missing: " + ", ".join(missing_profile_artifacts),
    )
    add_result(
        results,
        "capability-manifest-present-in-profiles",
        not missing_manifest_profiles,
        "all profile archives include capability.manifest.json"
        if not missing_manifest_profiles
        else "missing capability.manifest.json for profiles: " + ", ".join(missing_manifest_profiles),
    )

    if args.skip_signature_verify:
        add_result(results, "signed-evidence-verify", True, "skipped by --skip-signature-verify")
    else:
        if not signed_bundle.exists():
            add_result(results, "signed-evidence-verify", False, f"missing signed bundle {signed_bundle}")
        else:
            verify_cmd = [
                "python3",
                str((repo_root / "tools/evidence_sign_verify.py").resolve()),
                "verify",
                "--bundle",
                str(signed_bundle),
                "--repo-root",
                str(repo_root),
                "--key-env",
                args.key_env,
            ]
            completed = subprocess.run(
                verify_cmd,
                cwd=repo_root,
                text=True,
                capture_output=True,
                check=False,
            )
            add_result(
                results,
                "signed-evidence-verify",
                completed.returncode == 0,
                (completed.stdout + completed.stderr).strip() or f"exit_code={completed.returncode}",
            )

    failures = [result for result in results if result["status"] != "PASS"]
    payload: dict[str, Any] = {
        "release_id": release_id,
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "overall_status": "PASS" if not failures else "FAIL",
        "failed_check_count": len(failures),
        "checks": results,
    }
    return (0 if not failures else 1, payload)


def main() -> int:
    args = parse_args()
    code, payload = run_gate(args)
    repo_root = pathlib.Path(args.repo_root).resolve()
    output_path = (
        pathlib.Path(args.output_json).resolve()
        if args.output_json
        else (repo_root / f"reports/release/release-controls-gate-{args.release_id}.json").resolve()
    )
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    print("Release controls gate: PASS" if code == 0 else "Release controls gate: FAIL")
    print(f"release_id: {payload['release_id']}")
    print(f"failed_checks: {payload['failed_check_count']}")
    print(f"report: {output_path}")
    return code


if __name__ == "__main__":
    sys.exit(main())
