from __future__ import annotations

import json
import os
import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import evidence_sign_verify


class EvidenceSignVerifyTests(unittest.TestCase):
    def test_sign_and_verify_roundtrip(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports").mkdir(parents=True)
            sample = root / "reports" / "sample.md"
            sample.write_text("sample evidence\n")

            os.environ["EVIDENCE_SIGNING_KEY"] = "unit-test-secret"
            bundle = evidence_sign_verify.sign_bundle(
                repo_root=root,
                release_id="RC-TEST",
                key_id="unit-key",
                key_env="EVIDENCE_SIGNING_KEY",
                artifacts=["reports/sample.md"],
            )

            ok, errors = evidence_sign_verify.verify_bundle(
                repo_root=root,
                key_env="EVIDENCE_SIGNING_KEY",
                bundle=bundle,
            )
            self.assertTrue(ok)
            self.assertEqual(errors, [])

    def test_verify_detects_artifact_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            (root / "reports").mkdir(parents=True)
            sample = root / "reports" / "sample.md"
            sample.write_text("before\n")

            os.environ["EVIDENCE_SIGNING_KEY"] = "unit-test-secret"
            bundle = evidence_sign_verify.sign_bundle(
                repo_root=root,
                release_id="RC-TEST",
                key_id="unit-key",
                key_env="EVIDENCE_SIGNING_KEY",
                artifacts=["reports/sample.md"],
            )

            sample.write_text("after\n")
            ok, errors = evidence_sign_verify.verify_bundle(
                repo_root=root,
                key_env="EVIDENCE_SIGNING_KEY",
                bundle=bundle,
            )
            self.assertFalse(ok)
            joined = "\n".join(errors)
            self.assertIn("artifact digest mismatch", joined)

    def test_compute_signature_stable_with_sorted_artifacts(self) -> None:
        os.environ["EVIDENCE_SIGNING_KEY"] = "unit-test-secret"
        bundle = {
            "release_id": "RC-TEST",
            "key_id": "unit-key",
            "signature_algorithm": "hmac-sha256",
            "hash_algorithm": "sha256",
            "artifacts": [
                {"path": "b", "size_bytes": 1, "sha256": "22"},
                {"path": "a", "size_bytes": 1, "sha256": "11"},
            ],
        }
        key = os.environ["EVIDENCE_SIGNING_KEY"].encode("utf-8")
        sig_a = evidence_sign_verify.compute_signature(bundle, key)
        bundle["artifacts"] = list(reversed(bundle["artifacts"]))
        sig_b = evidence_sign_verify.compute_signature(bundle, key)
        self.assertEqual(sig_a, sig_b)


if __name__ == "__main__":
    unittest.main()
