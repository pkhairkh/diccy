from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REQ_HI_RE = re.compile(r"REQ-HI-\d{3}")


class HiReqTraceabilityTests(unittest.TestCase):
    def test_hi_requirement_corpus_is_indexed_across_runtime_registers(self) -> None:
        hi_req_text = (ROOT / "HI_REQ.md").read_text(errors="ignore")
        runtime_test_text = ""
        for path in (ROOT / "crates").rglob("hi_*.rs"):
            runtime_test_text += path.read_text(errors="ignore")
        for path in (ROOT / "tools" / "tests").glob("test_hi_*.py"):
            runtime_test_text += path.read_text(errors="ignore")

        hi_req_ids = set(REQ_HI_RE.findall(hi_req_text))
        self.assertGreaterEqual(len(hi_req_ids), 340)

        for anchor in ["REQ-HI-145", "REQ-HI-195", "REQ-HI-240", "REQ-HI-255", "REQ-HI-439"]:
            self.assertIn(anchor, runtime_test_text)

    def test_chunk_mapped_hi_test_files_exist_with_req_annotations(self) -> None:
        mapped_paths = [
            "crates/viewer-core/tests/hi_viewer_determinism.rs",
            "crates/viewer-core/tests/hi_overlay_ordering.rs",
            "crates/viewer-core/tests/hi_error_rendering.rs",
            "crates/viewer-core/tests/hi_viewer_controls.rs",
            "crates/viewer-core/tests/hi_error_safety_controls.rs",
            "crates/viewer-core/tests/hi_comparison_layout_controls.rs",
            "crates/dicom-workflow-server/tests/hi_workflow_state.rs",
            "crates/dicom-web/tests/hi_context_tuple_integrity.rs",
            "crates/dicom-web/tests/hi_context_safety_controls.rs",
            "crates/dicom-visualizer/tests/hi_export_controls.rs",
            "crates/dicom-auth/tests/hi_governance_identity_controls.rs",
            "crates/dicom-web-server/tests/hi_auth_session_controls.rs",
            "crates/dicom-workflow-server/tests/hi_privacy_modes.rs",
            "tools/tests/test_hi_req_traceability.py",
            "tools/tests/test_hi_accessibility_checklist.py",
            "tools/tests/test_hi_localization_controls.py",
            "tools/tests/test_hi_release_gate_matrix.py",
            "tools/tests/test_hi_critical_task_register.py",
            "tools/tests/test_hi_accessibility_interaction_layout.py",
            "tools/tests/test_hi_negative_path_matrix.py",
            "tools/tests/test_hi_change_control_register.py",
            "tools/tests/test_hi_interop_runtime_register.py",
        ]

        missing = []
        without_req_markers = []
        for rel_path in mapped_paths:
            path = ROOT / rel_path
            if not path.exists():
                missing.append(rel_path)
                continue
            text = path.read_text(errors="ignore")
            if not REQ_HI_RE.search(text):
                without_req_markers.append(rel_path)

        self.assertEqual(missing, [], f"missing mapped files: {missing}")
        self.assertEqual(
            without_req_markers,
            [],
            f"mapped files missing REQ-HI markers: {without_req_markers}",
        )


if __name__ == "__main__":
    unittest.main()
