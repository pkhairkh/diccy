#!/usr/bin/env python3
"""
Cross-check selected profile and envelope claims across docs/03, docs/12, docs/33, and docs/49.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any


DOCS12_SECTION_RE = re.compile(r"^### `([^`]+)` \(env-contract\)")
DOCS33_PROFILE_HEADER_RE = re.compile(r"^\|\s*`([^`]+)`\s*\|")
DOCS49_HEADER_RE = re.compile(r"^\|\s*Capability\s*\|\s*`([^`]+)`")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate cross-doc profile claim parity.")
    parser.add_argument("--docs-envelope", default="docs/03-DICOM-Conformance-Envelope.md")
    parser.add_argument("--docs-service-contract", default="docs/12-API-Surface-and-Crate-Boundaries.md")
    parser.add_argument(
        "--docs-profile-playbook",
        default="docs/33-Productization-Profiles-and-Playbooks.md",
    )
    parser.add_argument("--docs-capability-matrix", default="docs/49-Runtime-Profile-Capability-Matrix.md")
    parser.add_argument("--profile-matrix", default="tools/profile_matrix.json")
    parser.add_argument(
        "--report",
        default="reports/docs/profile-docs-parity.json",
    )
    parser.add_argument("--fail-on-findings", action="store_true")
    return parser.parse_args()


def load_json(path: pathlib.Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} must be a JSON object")
    return payload


def read_lines(path: pathlib.Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def parse_profiles_from_matrix(path: pathlib.Path) -> set[str]:
    payload = load_json(path)
    profiles = payload.get("profiles")
    if not isinstance(profiles, dict):
        return set()
    return set(profiles.keys())


def parse_profile_tokens_from_docs33(path: pathlib.Path) -> set[str]:
    lines = read_lines(path)
    profiles: set[str] = set()
    in_runtime_table = False
    for line in lines:
        if line.startswith("## Runtime artifacts by profile"):
            in_runtime_table = True
            continue
        if in_runtime_table:
            if line.startswith("## ") and not line.startswith("## Runtime artifacts by profile"):
                break
            match = DOCS33_PROFILE_HEADER_RE.match(line.strip())
            if match:
                token = match.group(1)
                if token != "Profile":
                    profiles.add(token)
    return profiles


def parse_profile_tokens_from_docs49(path: pathlib.Path) -> set[str]:
    for line in read_lines(path):
        match = DOCS49_HEADER_RE.match(line.strip())
        if match:
            profiles = set()
            header_line = line.strip().strip("|").split("|")
            for item in header_line[1:]:
                candidate = item.strip().strip("`").strip()
                if candidate and candidate != "Capability":
                    profiles.add(candidate)
            return profiles
    return set()


def parse_env_sections_from_docs12(path: pathlib.Path) -> set[str]:
    sections: set[str] = set()
    for line in read_lines(path):
        match = DOCS12_SECTION_RE.match(line.strip())
        if match:
            sections.add(match.group(1))
    return sections


def parse_envelope_version(path: pathlib.Path) -> str:
    for line in read_lines(path):
        match = re.search(r"`envelope_version`:\s*`([^`]+)`", line)
        if match:
            return match.group(1).strip()
    return ""


def main() -> int:
    args = parse_args()
    docs_envelope = pathlib.Path(args.docs_envelope)
    docs_service_contract = pathlib.Path(args.docs_service_contract)
    docs_profile_playbook = pathlib.Path(args.docs_profile_playbook)
    docs_capability_matrix = pathlib.Path(args.docs_capability_matrix)
    profile_matrix = pathlib.Path(args.profile_matrix)
    report_path = pathlib.Path(args.report).resolve()

    findings: list[str] = []

    matrix_profiles = parse_profiles_from_matrix(profile_matrix)
    if not docs_profile_playbook.exists():
        findings.append(f"missing profile playbook doc: {docs_profile_playbook}")
        docs33_profiles = set()
    else:
        docs33_profiles = parse_profile_tokens_from_docs33(docs_profile_playbook)

    if not docs_capability_matrix.exists():
        findings.append(f"missing profile capability matrix doc: {docs_capability_matrix}")
        docs49_profiles = set()
    else:
        docs49_profiles = parse_profile_tokens_from_docs49(docs_capability_matrix)

    if not docs_service_contract.exists():
        findings.append(f"missing service contract doc: {docs_service_contract}")
        docs12_sections = set()
    else:
        docs12_sections = parse_env_sections_from_docs12(docs_service_contract)

    if not profile_matrix.exists():
        findings.append(f"missing profile matrix file: {profile_matrix}")
    if not docs_envelope.exists():
        findings.append(f"missing envelope contract doc: {docs_envelope}")

    if matrix_profiles and docs33_profiles and matrix_profiles != docs33_profiles:
        findings.append(
            "docs33 profile tokens do not match profile matrix: "
            + f"matrix={sorted(matrix_profiles)} docs33={sorted(docs33_profiles)}"
        )
    if matrix_profiles and docs49_profiles and matrix_profiles != docs49_profiles:
        findings.append(
            "docs49 profile columns do not match profile matrix: "
            + f"matrix={sorted(matrix_profiles)} docs49={sorted(docs49_profiles)}"
        )

    if matrix_profiles:
        if "dicom-web-server" not in docs12_sections:
            findings.append("docs12 missing env-contract section for dicom-web-server")
        if "dicom-workflow-server" not in docs12_sections:
            findings.append("docs12 missing env-contract section for dicom-workflow-server")
        if "dicom-dimse-service" not in docs12_sections:
            findings.append("docs12 missing env-contract section for dicom-dimse-service")

    envelope_version = parse_envelope_version(docs_envelope)
    if not envelope_version:
        findings.append("docs/03 missing envelope_version declaration")

    payload = {
        "generated_on_utc": dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "status": "PASS" if not findings else "FAIL",
        "findings": findings,
        "matrix_profiles": sorted(matrix_profiles),
        "docs33_profiles": sorted(docs33_profiles),
        "docs49_profiles": sorted(docs49_profiles),
        "docs12_contract_sections": sorted(docs12_sections),
        "envelope_version": envelope_version,
    }
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if findings:
        print("Profile docs-parity gate: FAIL")
        for finding in findings:
            print(f" - {finding}")
        report_display = report_path if not report_path.is_relative_to(pathlib.Path.cwd()) else report_path.relative_to(pathlib.Path.cwd())
        print(f"Report: {report_display}")
        return 1

    print("Profile docs-parity gate: PASS")
    report_display = report_path if not report_path.is_relative_to(pathlib.Path.cwd()) else report_path.relative_to(pathlib.Path.cwd())
    print(f"Report: {report_display}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
