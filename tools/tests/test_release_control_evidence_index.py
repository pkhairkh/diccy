from __future__ import annotations

import json
import pathlib
import tempfile
import unittest

import sys

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import release_control_evidence_index


class ReleaseControlEvidenceIndexTests(unittest.TestCase):
    def _seed_repo(self, root: pathlib.Path, release_id: str) -> None:
        (root / "reports/release").mkdir(parents=True, exist_ok=True)
        (root / "reports/traceability").mkdir(parents=True, exist_ok=True)
        (root / "reports/security").mkdir(parents=True, exist_ok=True)
        (root / "reports/performance").mkdir(parents=True, exist_ok=True)
        (root / "dist/profiles").mkdir(parents=True, exist_ok=True)

        (root / f"reports/release/release-notes-{release_id}.md").write_text("# Notes\n", encoding="utf-8")
        (root / f"reports/release/release-preflight-summary-{release_id}.json").write_text(
            json.dumps({"release_id": release_id, "overall_status": "PASS", "failed_gate_count": 0}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-artifact-integrity-{release_id}.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/release/release-controls-gate-{release_id}.json").write_text(
            json.dumps({"release_id": release_id, "overall_status": "PASS", "failed_check_count": 0}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/traceability/release-traceability-manifest-{release_id}.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / f"reports/security/security-gate-decision-{release_id}.md").write_text("PASS\n", encoding="utf-8")
        (root / f"reports/performance/performance-scalability-gate-decision-{release_id}.md").write_text(
            "PASS\n",
            encoding="utf-8",
        )
        (root / "dist/profiles/profiles.metadata.json").write_text(
            json.dumps({"release_id": release_id}) + "\n",
            encoding="utf-8",
        )
        (root / "dist/profiles/profiles.metadata.bundle.signed.json").write_text("{}", encoding="utf-8")

        for profile in release_control_evidence_index.REQUIRED_PROFILES:
            (root / f"dist/profiles/{profile}.{release_id}.tar.gz").write_text("stub", encoding="utf-8")

    def test_build_index_passes_when_all_artifacts_present(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed_repo(root, release_id)
            payload = release_control_evidence_index.build_index(repo_root=root, release_id=release_id)
            self.assertEqual(payload["overall_status"], "PASS")
            self.assertEqual(payload["failed_check_count"], 0)

    def test_build_index_fails_when_preflight_summary_is_failed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            release_id = "RC-TEST"
            self._seed_repo(root, release_id)
            (root / f"reports/release/release-preflight-summary-{release_id}.json").write_text(
                json.dumps({"release_id": release_id, "overall_status": "FAIL", "failed_gate_count": 1}) + "\n",
                encoding="utf-8",
            )
            payload = release_control_evidence_index.build_index(repo_root=root, release_id=release_id)
            self.assertEqual(payload["overall_status"], "FAIL")
            self.assertGreater(payload["failed_check_count"], 0)


if __name__ == "__main__":
    unittest.main()
