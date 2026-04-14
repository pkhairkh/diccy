#!/usr/bin/env python3
"""Fail-closed scientific dataset quality lint for blinded external validation cohorts."""

from __future__ import annotations

import argparse
import csv
import pathlib
import sys
from collections import Counter

REQUIRED_COLUMNS = [
    "record_id",
    "site_code",
    "endpoint_id",
    "case_id",
    "reviewer_1",
    "reviewer_2",
    "agreement",
    "adjudication_required",
    "final_decision",
    "decision_id",
]

ALLOWED_ENDPOINTS = {"EVP-SE-001", "EVP-SE-002"}
ALLOWED_BINARY = {"YES", "NO"}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Scientific data quality lint.")
    parser.add_argument(
        "--dataset",
        required=True,
        help="Path to blinded assessment dataset CSV.",
    )
    parser.add_argument(
        "--ingestion-log",
        default=None,
        help="Optional ingestion log markdown for de-identification and envelope checks.",
    )
    parser.add_argument(
        "--min-site-count",
        type=int,
        default=2,
        help="Minimum number of independent sites required in dataset.",
    )
    parser.add_argument(
        "--min-records-per-site",
        type=int,
        default=50,
        help="Minimum blinded records per site.",
    )
    parser.add_argument(
        "--min-evp-se-001-per-site",
        type=int,
        default=30,
        help="Minimum EVT-SE-001 records per site (protocol control for CLM-003).",
    )
    parser.add_argument(
        "--min-evp-se-002-per-site",
        type=int,
        default=40,
        help="Minimum EVT-SE-002 records per site (protocol control for CLM-004).",
    )
    parser.add_argument(
        "--max-adjudication-rate",
        type=float,
        default=0.20,
        help="Maximum allowed adjudication trigger rate.",
    )
    parser.add_argument(
        "--fail-on-findings",
        action="store_true",
        help="Exit non-zero when findings are present.",
    )
    return parser.parse_args()


def parse_dataset(path: pathlib.Path) -> tuple[list[dict[str, str]], list[str]]:
    findings: list[str] = []
    if not path.exists():
        return [], [f"dataset not found: {path}"]

    with path.open(newline="") as handle:
        reader = csv.DictReader(handle)
        header = reader.fieldnames or []
        if header != REQUIRED_COLUMNS:
            findings.append(
                "dataset header mismatch: "
                f"expected {REQUIRED_COLUMNS}, got {header}"
            )
        rows = list(reader)

    if not rows:
        findings.append("dataset contains no data rows")

    seen_record_ids: set[str] = set()
    seen_decision_ids: set[str] = set()

    for index, row in enumerate(rows, start=2):
        prefix = f"row {index}"

        for column in REQUIRED_COLUMNS:
            if not row.get(column, "").strip():
                findings.append(f"{prefix}: missing value in column `{column}`")

        record_id = row.get("record_id", "").strip()
        decision_id = row.get("decision_id", "").strip()
        endpoint_id = row.get("endpoint_id", "").strip()
        agreement = row.get("agreement", "").strip()
        adjudication_required = row.get("adjudication_required", "").strip()
        reviewer_1 = row.get("reviewer_1", "").strip()
        reviewer_2 = row.get("reviewer_2", "").strip()
        final_decision = row.get("final_decision", "").strip()

        if record_id in seen_record_ids:
            findings.append(f"{prefix}: duplicate record_id `{record_id}`")
        seen_record_ids.add(record_id)

        if decision_id in seen_decision_ids:
            findings.append(f"{prefix}: duplicate decision_id `{decision_id}`")
        seen_decision_ids.add(decision_id)

        if endpoint_id not in ALLOWED_ENDPOINTS:
            findings.append(f"{prefix}: unsupported endpoint_id `{endpoint_id}`")

        if agreement not in ALLOWED_BINARY:
            findings.append(f"{prefix}: invalid agreement `{agreement}`")

        if adjudication_required not in ALLOWED_BINARY:
            findings.append(
                f"{prefix}: invalid adjudication_required `{adjudication_required}`"
            )

        if agreement == "YES" and adjudication_required != "NO":
            findings.append(
                f"{prefix}: agreement=YES requires adjudication_required=NO"
            )

        if agreement == "NO" and adjudication_required != "YES":
            findings.append(
                f"{prefix}: agreement=NO requires adjudication_required=YES"
            )

        if adjudication_required == "NO" and final_decision not in {reviewer_1, reviewer_2}:
            findings.append(
                f"{prefix}: non-adjudicated final_decision must match reviewer labels"
            )

        if adjudication_required == "YES" and not final_decision:
            findings.append(f"{prefix}: adjudicated row requires final_decision")

    return rows, findings


def parse_ingestion_site_checks(path: pathlib.Path) -> tuple[dict[str, tuple[str, str]], list[str]]:
    findings: list[str] = []
    if not path.exists():
        return {}, [f"ingestion log not found: {path}"]

    site_checks: dict[str, tuple[str, str]] = {}
    for raw in path.read_text(errors="ignore").splitlines():
        line = raw.strip()
        if not line.startswith("|") or "EVT-BATCH-" not in line:
            continue

        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) < 8:
            findings.append("ingestion log batch row malformed")
            continue

        site = cells[1]
        deid_check = cells[5].upper()
        envelope_check = cells[6].upper()
        site_checks[site] = (deid_check, envelope_check)

        if deid_check != "PASS":
            findings.append(f"ingestion log site {site}: de-identification check is `{deid_check}`")
        if envelope_check != "PASS":
            findings.append(f"ingestion log site {site}: envelope check is `{envelope_check}`")

    if not site_checks:
        findings.append("ingestion log has no EVT-BATCH rows")

    return site_checks, findings


def run_lint(
    *,
    dataset_path: pathlib.Path,
    ingestion_log_path: pathlib.Path | None,
    min_site_count: int,
    min_records_per_site: int,
    min_evp_se_001_per_site: int,
    min_evp_se_002_per_site: int,
    max_adjudication_rate: float,
) -> list[str]:
    rows, findings = parse_dataset(dataset_path)
    if not rows:
        return findings

    by_site = Counter(row["site_code"] for row in rows)
    if len(by_site) < min_site_count:
        findings.append(
            f"site-count threshold failed: found {len(by_site)}, required >= {min_site_count}"
        )

    for site, count in sorted(by_site.items()):
        if count < min_records_per_site:
            findings.append(
                f"site {site}: records/site threshold failed ({count} < {min_records_per_site})"
            )

    by_site_endpoint = Counter((row["site_code"], row["endpoint_id"]) for row in rows)
    for site in sorted(by_site):
        se001_count = by_site_endpoint.get((site, "EVP-SE-001"), 0)
        se002_count = by_site_endpoint.get((site, "EVP-SE-002"), 0)
        if se001_count < min_evp_se_001_per_site:
            findings.append(
                f"site {site}: EVP-SE-001 threshold failed ({se001_count} < {min_evp_se_001_per_site})"
            )
        if se002_count < min_evp_se_002_per_site:
            findings.append(
                f"site {site}: EVP-SE-002 threshold failed ({se002_count} < {min_evp_se_002_per_site})"
            )

    adjudication_count = sum(1 for row in rows if row["adjudication_required"] == "YES")
    adjudication_rate = adjudication_count / len(rows)
    if adjudication_rate > max_adjudication_rate:
        findings.append(
            "adjudication-rate threshold failed: "
            f"{adjudication_rate:.4f} > {max_adjudication_rate:.4f}"
        )

    unresolved = [
        row["record_id"]
        for row in rows
        if row["adjudication_required"] == "YES" and not row["final_decision"].strip()
    ]
    if unresolved:
        findings.append(f"unresolved adjudications found: {len(unresolved)}")

    if ingestion_log_path is not None:
        site_checks, log_findings = parse_ingestion_site_checks(ingestion_log_path)
        findings.extend(log_findings)

        if site_checks:
            dataset_sites = set(by_site)
            log_sites = set(site_checks)
            missing_sites = sorted(dataset_sites - log_sites)
            if missing_sites:
                findings.append(
                    "dataset site(s) missing from ingestion log: " + ", ".join(missing_sites)
                )

    return findings


def main() -> int:
    args = parse_args()

    findings = run_lint(
        dataset_path=pathlib.Path(args.dataset).resolve(),
        ingestion_log_path=pathlib.Path(args.ingestion_log).resolve() if args.ingestion_log else None,
        min_site_count=args.min_site_count,
        min_records_per_site=args.min_records_per_site,
        min_evp_se_001_per_site=args.min_evp_se_001_per_site,
        min_evp_se_002_per_site=args.min_evp_se_002_per_site,
        max_adjudication_rate=args.max_adjudication_rate,
    )

    if findings:
        print("Scientific data quality lint: FAIL")
        print(f"Findings: {len(findings)}")
        for finding in findings:
            print(f"- {finding}")
        if args.fail_on_findings:
            return 1
        return 0

    row_count = 0
    with pathlib.Path(args.dataset).open(newline="") as handle:
        row_count = sum(1 for _ in csv.DictReader(handle))

    print("Scientific data quality lint: PASS")
    print(f"Dataset: {args.dataset}")
    print(f"Rows: {row_count}")
    if args.ingestion_log:
        print(f"Ingestion log: {args.ingestion_log}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
