from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

TOOLS_DIR = pathlib.Path(__file__).resolve().parents[1]
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import security_fuzz_campaign


class SecurityFuzzCampaignTests(unittest.TestCase):
    def test_detect_crash_from_returncode(self) -> None:
        self.assertTrue(security_fuzz_campaign.detect_crash("ok", 1))
        self.assertFalse(security_fuzz_campaign.detect_crash("ok", 0))

    def test_detect_crash_from_marker(self) -> None:
        text = "INFO: ... AddressSanitizer: heap-use-after-free"
        self.assertTrue(security_fuzz_campaign.detect_crash(text, 0))

    def test_parse_helpers(self) -> None:
        text = (
            "INFO:        7 files found in /tmp/corpus\n"
            "#128 DONE ... rss: 44Mb\n"
            "Done 128 runs in 0 second(s)\n"
        )
        self.assertEqual(security_fuzz_campaign.parse_seed_count(text), 7)
        self.assertEqual(security_fuzz_campaign.parse_done_runs(text), 128)
        self.assertEqual(security_fuzz_campaign.parse_rss_mb(text), 44)

    def test_write_crash_inventory_no_crashes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            output = pathlib.Path(tmp_dir) / "crashes.md"
            run = security_fuzz_campaign.TargetRun(
                target="dimse_pdu",
                command=["cargo", "run"],
                returncode=0,
                seeds=2,
                runs=10,
                rss_mb=26,
                corpus_path=None,
                has_crash=False,
                has_coverage_warning=True,
                log_path="/tmp/log",
            )
            security_fuzz_campaign.write_crash_inventory(output, [run])
            text = output.read_text()
            self.assertIn("Total crash findings: 0", text)
            self.assertIn("No crashes detected", text)


if __name__ == "__main__":
    unittest.main()
