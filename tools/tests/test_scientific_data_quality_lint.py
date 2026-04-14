from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import scientific_data_quality_lint


REQUIRED_HEADER = [
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


def write_dataset(path: pathlib.Path, rows: list[list[str]]) -> None:
    lines = [",".join(REQUIRED_HEADER)]
    lines.extend(",".join(cells) for cells in rows)
    path.write_text("\n".join(lines) + "\n")


def build_clean_rows() -> list[list[str]]:
    rows: list[list[str]] = []
    record_id = 1
    for site in ("SITE-X", "SITE-Y"):
        for endpoint, base, alt in (
            ("EVP-SE-001", "COMPLETE", "PARTIAL"),
            ("EVP-SE-002", "STABLE", "BORDERLINE"),
        ):
            for i in range(40):
                disagreement = i % 20 == 19
                if disagreement:
                    reviewer_1 = base
                    reviewer_2 = alt
                    agreement = "NO"
                    adjudication_required = "YES"
                    final_decision = base
                else:
                    reviewer_1 = base
                    reviewer_2 = base
                    agreement = "YES"
                    adjudication_required = "NO"
                    final_decision = base

                rows.append(
                    [
                        f"EVT-{record_id:04d}",
                        site,
                        endpoint,
                        f"CASE-{site}-{endpoint[-3:]}-{i:03d}",
                        reviewer_1,
                        reviewer_2,
                        agreement,
                        adjudication_required,
                        final_decision,
                        f"DEC-{record_id:04d}",
                    ]
                )
                record_id += 1
    return rows


def write_ingestion_log(path: pathlib.Path, site_status: list[tuple[str, str, str]]) -> None:
    lines = [
        "| Batch ID | Site Code | Claim Context | Cases Accepted | Cases Rejected | De-identification Check | Envelope Check | Hash Manifest |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for idx, (site, deid, envelope) in enumerate(site_status, start=1):
        lines.append(
            f"| EVT-BATCH-{idx:03d} | {site} | CLM-003, CLM-004 | 80 | 2 | {deid} | {envelope} | sha256:{idx:064x} |"
        )
    path.write_text("\n".join(lines) + "\n")


class ScientificDataQualityLintTests(unittest.TestCase):
    def test_passes_clean_dataset_and_ingestion_log(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            dataset = root / "dataset.csv"
            ingestion = root / "ingestion.md"

            write_dataset(dataset, build_clean_rows())
            write_ingestion_log(
                ingestion,
                [("SITE-X", "PASS", "PASS"), ("SITE-Y", "PASS", "PASS")],
            )

            findings = scientific_data_quality_lint.run_lint(
                dataset_path=dataset,
                ingestion_log_path=ingestion,
                min_site_count=2,
                min_records_per_site=50,
                min_evp_se_001_per_site=30,
                min_evp_se_002_per_site=40,
                max_adjudication_rate=0.20,
            )
            self.assertEqual(findings, [])

    def test_fails_on_ingestion_deidentification_violation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            dataset = root / "dataset.csv"
            ingestion = root / "ingestion.md"

            write_dataset(dataset, build_clean_rows())
            write_ingestion_log(
                ingestion,
                [("SITE-X", "PASS", "PASS"), ("SITE-Y", "FAIL", "PASS")],
            )

            findings = scientific_data_quality_lint.run_lint(
                dataset_path=dataset,
                ingestion_log_path=ingestion,
                min_site_count=2,
                min_records_per_site=50,
                min_evp_se_001_per_site=30,
                min_evp_se_002_per_site=40,
                max_adjudication_rate=0.20,
            )
            joined = "\n".join(findings)
            self.assertIn("de-identification check is `FAIL`", joined)

    def test_fails_on_protocol_threshold_violation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            dataset = root / "dataset.csv"
            ingestion = root / "ingestion.md"

            rows = build_clean_rows()[:90]
            write_dataset(dataset, rows)
            write_ingestion_log(
                ingestion,
                [("SITE-X", "PASS", "PASS"), ("SITE-Y", "PASS", "PASS")],
            )

            findings = scientific_data_quality_lint.run_lint(
                dataset_path=dataset,
                ingestion_log_path=ingestion,
                min_site_count=2,
                min_records_per_site=50,
                min_evp_se_001_per_site=30,
                min_evp_se_002_per_site=40,
                max_adjudication_rate=0.20,
            )
            joined = "\n".join(findings)
            self.assertIn("records/site threshold failed", joined)


if __name__ == "__main__":
    unittest.main()
