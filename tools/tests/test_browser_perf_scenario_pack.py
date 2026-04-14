from __future__ import annotations

import json
import pathlib
import tempfile
import unittest

from tools.browser_perf_scenario_pack import load_pack, scenario_command


class BrowserPerfScenarioPackTests(unittest.TestCase):
    def test_load_pack_requires_non_empty_scenarios(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            path = pathlib.Path(td) / "pack.json"
            path.write_text('{"id":"x","scenarios":[]}', encoding="utf-8")
            with self.assertRaises(ValueError):
                load_pack(path)

    def test_scenario_command_builds_expected_harness_invocation(self) -> None:
        scenario = {
            "id": "baseline",
            "width": 512,
            "height": 512,
            "interactions": 50,
        }
        out = pathlib.Path("reports/performance/scenarios/baseline.json")
        cmd = scenario_command(scenario, out)
        self.assertEqual(cmd[0:2], ["node", "tools/web_renderer_perf_harness.mjs"])
        self.assertIn("--width", cmd)
        self.assertIn("--height", cmd)
        self.assertIn("--interactions", cmd)
        self.assertIn("--output", cmd)
        self.assertEqual(cmd[-1], str(out))

    def test_scenario_command_rejects_invalid_dimensions(self) -> None:
        scenario = {
            "id": "broken",
            "width": 0,
            "height": 512,
            "interactions": 50,
        }
        with self.assertRaises(ValueError):
            scenario_command(scenario, pathlib.Path("x.json"))

    def test_repository_scenario_pack_is_valid(self) -> None:
        root = pathlib.Path(__file__).resolve().parents[2]
        pack_path = root / "reports/performance/profiles/browser-scenario-pack-RC-2026.02.22.json"
        payload = json.loads(pack_path.read_text(encoding="utf-8"))
        scenarios = payload.get("scenarios")
        self.assertIsInstance(scenarios, list)
        self.assertGreaterEqual(len(scenarios), 3)
        for scenario in scenarios:
            self.assertIn("id", scenario)
            self.assertIn("width", scenario)
            self.assertIn("height", scenario)
            self.assertIn("interactions", scenario)


if __name__ == "__main__":
    unittest.main()
