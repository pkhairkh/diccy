from __future__ import annotations

import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "public_api_claim_evidence_gate.py"


class PublicApiClaimEvidenceGateTests(unittest.TestCase):
    def _seed_base(self, root: pathlib.Path) -> None:
        (root / "reports/traceability").mkdir(parents=True, exist_ok=True)
        (root / "docs").mkdir(parents=True, exist_ok=True)
        (root / "crates/sample/tests").mkdir(parents=True, exist_ok=True)
        (root / "crates/sample/src").mkdir(parents=True, exist_ok=True)
        (root / "docs/12-API-Surface-and-Crate-Boundaries.md").write_text("# API\n", encoding="utf-8")
        (root / "crates/sample/src/lib.rs").write_text("pub fn f() {}\n", encoding="utf-8")
        (root / "crates/sample/tests/api.rs").write_text("// test\n", encoding="utf-8")

    def test_passes_with_valid_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_base(root)
            (root / "reports/traceability/public-api-claim-evidence-manifest.json").write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "API-CLM-001",
                                "owner": "Workflow API",
                                "source_doc": "docs/12-API-Surface-and-Crate-Boundaries.md",
                                "implementation_evidence_paths": ["crates/sample/src/lib.rs"],
                                "test_evidence_paths": ["crates/sample/tests/api.rs"],
                            }
                        ]
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0)

    def test_fails_when_test_evidence_is_missing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            self._seed_base(root)
            (root / "reports/traceability/public-api-claim-evidence-manifest.json").write_text(
                json.dumps(
                    {
                        "claims": [
                            {
                                "claim_id": "API-CLM-001",
                                "owner": "Workflow API",
                                "source_doc": "docs/12-API-Surface-and-Crate-Boundaries.md",
                                "implementation_evidence_paths": ["crates/sample/src/lib.rs"],
                                "test_evidence_paths": ["crates/sample/tests/missing.rs"],
                            }
                        ]
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                ["python3", str(SCRIPT), "--repo-root", str(root)],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main()
