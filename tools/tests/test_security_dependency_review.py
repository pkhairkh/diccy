from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import security_dependency_review


class SecurityDependencyReviewTests(unittest.TestCase):
    def test_classify_source(self) -> None:
        self.assertEqual(security_dependency_review.classify_source(None), "workspace/path")
        self.assertEqual(
            security_dependency_review.classify_source("registry+https://github.com/rust-lang/crates.io-index"),
            "registry",
        )
        self.assertEqual(
            security_dependency_review.classify_source("git+https://example.org/repo"),
            "git",
        )
        self.assertEqual(
            security_dependency_review.classify_source("sparse+https://example.org/index"),
            "other",
        )

    def test_evaluate_packages_flags_git_and_missing_checksum(self) -> None:
        packages = [
            {
                "name": "a",
                "version": "1.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
            },
            {
                "name": "b",
                "version": "2.0.0",
                "source": "git+https://example.org/repo",
                "checksum": "abc",
            },
            {
                "name": "local",
                "version": "0.1.0",
            },
        ]
        sbom, findings = security_dependency_review.evaluate_packages(packages)
        self.assertEqual(len(sbom), 3)
        finding_ids = {finding["id"] for finding in findings}
        self.assertIn("DEP-GIT-SOURCE", finding_ids)
        self.assertIn("DEP-MISSING-CHECKSUM", finding_ids)

    def test_evaluate_packages_clean_registry_and_workspace(self) -> None:
        packages = [
            {
                "name": "serde",
                "version": "1.0.0",
                "source": "registry+https://github.com/rust-lang/crates.io-index",
                "checksum": "a" * 64,
            },
            {
                "name": "dicom-core",
                "version": "0.1.0",
            },
        ]
        _, findings = security_dependency_review.evaluate_packages(packages)
        self.assertEqual(findings, [])

    def test_is_codec_dependency_detects_codec_packages(self) -> None:
        self.assertTrue(security_dependency_review.is_codec_dependency("jpeg-decoder"))
        self.assertTrue(security_dependency_review.is_codec_dependency("hayro-jpeg2000"))
        self.assertFalse(security_dependency_review.is_codec_dependency("serde"))

    def test_scan_workspace_unsafe_usage_collects_crate_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = pathlib.Path(tmp_dir)
            source = root / "crates/example/src/lib.rs"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text(
                "\n".join(
                    [
                        "pub fn safe() {}",
                        "unsafe fn do_unsafe() {}",
                        "// unsafe block in comments should not count",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            findings = security_dependency_review.scan_workspace_unsafe_usage(root)
            self.assertEqual(len(findings), 1)
            self.assertEqual(findings[0]["path"], "crates/example/src/lib.rs")
            self.assertEqual(findings[0]["line"], 2)


if __name__ == "__main__":
    unittest.main()
