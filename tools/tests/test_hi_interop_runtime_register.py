from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
REQ_HI_RE = re.compile(r"REQ-HI-\d{3}")

RUNTIME_TEST_PATHS = [
    "crates/dicom-web/tests/hi_interop_runtime_controls.rs",
    "crates/dicom-web/tests/hi_http_response_contract.rs",
    "crates/dicom-web-server/tests/hi_service_preflight_resilience.rs",
    "crates/dicom-web-server/tests/hi_http_status_contract.rs",
    "crates/dicom-workflow-server/tests/hi_workflow_runtime_contract.rs",
    "crates/dicom-dimse-service/tests/hi_dimse_protocol_contract.rs",
    "crates/dicom-net/tests/hi_association_contract.rs",
    "crates/dicom-query/tests/hi_query_contract_controls.rs",
    "crates/dicom-storage/tests/hi_storage_runtime_integrity.rs",
    "crates/dicom-visualizer/tests/hi_batch_manifest_contract.rs",
    "crates/diccy/tests/hi_runtime_config_contract.rs",
    "crates/viewer-wasm/tests/hi_wasm_boundary_contract.rs",
    "crates/viewer-wgpu/tests/hi_gpu_runtime_contract.rs",
    "crates/pack-enhanced/tests/hi_enhanced_geometry_contract.rs",
    "crates/pack-gsps/tests/hi_gsps_contract.rs",
    "crates/pack-seg/tests/hi_seg_overlay_contract.rs",
    "crates/pack-rt/tests/hi_rt_overlay_contract.rs",
    "crates/pack-sr/tests/hi_sr_extraction_contract.rs",
    "crates/dicom-worklist/tests/hi_worklist_persistence_contract.rs",
    "crates/dicom-mpps/tests/hi_mpps_persistence_contract.rs",
]


class HiInteropRuntimeRegisterTests(unittest.TestCase):
    def test_runtime_register_files_exist_with_req_hi_annotations(self) -> None:
        # REQ-HI-255, REQ-HI-279, REQ-HI-359, REQ-HI-439
        missing: list[str] = []
        missing_req_annotations: list[str] = []
        for rel_path in RUNTIME_TEST_PATHS:
            path = ROOT / rel_path
            if not path.exists():
                missing.append(rel_path)
                continue
            text = path.read_text(errors="ignore")
            if not REQ_HI_RE.search(text):
                missing_req_annotations.append(rel_path)

        self.assertEqual(missing, [], f"missing runtime register tests: {missing}")
        self.assertEqual(
            missing_req_annotations,
            [],
            f"runtime tests missing REQ-HI markers: {missing_req_annotations}",
        )

    def test_runtime_register_paths_are_stable_unique_and_rust_scoped(self) -> None:
        # REQ-HI-255, REQ-HI-320, REQ-HI-340, REQ-HI-439
        self.assertEqual(len(RUNTIME_TEST_PATHS), len(set(RUNTIME_TEST_PATHS)))
        self.assertGreaterEqual(len(RUNTIME_TEST_PATHS), 20)
        for rel_path in RUNTIME_TEST_PATHS:
            self.assertTrue(rel_path.startswith("crates/"))
            self.assertTrue(rel_path.endswith(".rs"))


if __name__ == "__main__":
    unittest.main()
