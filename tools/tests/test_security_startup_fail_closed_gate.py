from __future__ import annotations

import pathlib
import importlib.util
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "security_startup_fail_closed_gate.py"
SPEC = importlib.util.spec_from_file_location("security_startup_fail_closed_gate", SCRIPT)
assert SPEC and SPEC.loader
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class SecurityStartupFailClosedGateTests(unittest.TestCase):
    def test_case_matrix_includes_test_only_env_rejection(self) -> None:
        binaries = {
            "web": pathlib.Path("/tmp/dicom-web-server"),
            "workflow": pathlib.Path("/tmp/dicom-workflow-server"),
            "dimse": pathlib.Path("/tmp/dicom-dimse-service"),
        }
        cases = gate.build_cases(binaries)
        case_ids = {case_id for case_id, _, _ in cases}
        self.assertIn("web-test-only-env-rejected-in-production", case_ids)
        self.assertIn("workflow-test-only-env-rejected-in-production", case_ids)
        self.assertIn("dimse-test-only-env-rejected-in-production", case_ids)


if __name__ == "__main__":
    unittest.main()
