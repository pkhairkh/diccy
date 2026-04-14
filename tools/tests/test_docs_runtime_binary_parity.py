from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
DOCS_12 = ROOT / "docs/12-API-Surface-and-Crate-Boundaries.md"
DOCS_40 = ROOT / "docs/40-Reference-Deployment-Topology.md"
CRATES_DIR = ROOT / "crates"


def _iter_binary_crates() -> dict[str, pathlib.Path]:
    binaries: dict[str, pathlib.Path] = {}
    for manifest_path in CRATES_DIR.glob("*/Cargo.toml"):
        text = manifest_path.read_text(encoding="utf-8", errors="replace")
        match = re.search(r'(?m)^name\s*=\s*"([^"]+)"', text)
        assert match, f"missing package name in {manifest_path}"
        package_name = match.group(1)
        crate_dir = manifest_path.parent
        bin_candidate = crate_dir / "src" / "main.rs"
        if bin_candidate.exists():
            binaries[package_name] = bin_candidate
            continue
        bin_candidate = crate_dir / "src" / "bin" / f"{package_name}.rs"
        if bin_candidate.exists():
            binaries[package_name] = bin_candidate
    return binaries


def _extract_declared_runnable_services(text: str) -> set[str]:
    names: set[str] = set()
    names.update(re.findall(r"cargo run -p ([a-zA-Z0-9_-]+)", text))
    names.update(re.findall(r"Binary entrypoint: `([a-zA-Z0-9_-]+)`", text))
    names.update(re.findall(r"\bcargo run --bin ([a-zA-Z0-9_-]+)", text))
    return {name.strip() for name in names if name}


def _extract_services_with_launch_contract(text: str) -> dict[str, bool]:
    services: dict[str, bool] = {}
    service_sections = re.findall(
        r"cargo run -p ([a-zA-Z0-9_-]+)(.*?)((?:\n|$))",
        text,
    )
    for service, body, _ in service_sections:
        if service:
            has_bind_hint = " --bind " in body or "--bind" in body
            services[service.strip()] = has_bind_hint
    return services


class DocsRuntimeBinaryParityTests(unittest.TestCase):
    def test_doc_declared_services_have_runtime_entrypoint(self) -> None:
        docs_12_text = DOCS_12.read_text(encoding="utf-8", errors="replace")
        docs_40_text = DOCS_40.read_text(encoding="utf-8", errors="replace")
        declared_services = _extract_declared_runnable_services(
            docs_12_text + "\n" + docs_40_text
        )

        self.assertNotEqual(len(declared_services), 0, "no declared services found")

        binaries = _iter_binary_crates()
        for service in sorted(declared_services):
            self.assertIn(
                service,
                binaries,
                f"service claim '{service}' lacks a Rust binary entrypoint under crates/{service}",
            )

    def test_doc_service_startup_contract_matches_main_rs(self) -> None:
        docs_12_text = DOCS_12.read_text(encoding="utf-8", errors="replace")
        docs_40_text = DOCS_40.read_text(encoding="utf-8", errors="replace")
        contracts = _extract_services_with_launch_contract(
            docs_12_text + "\n" + docs_40_text
        )

        binaries = _iter_binary_crates()
        for service, has_bind_hint in sorted(contracts.items()):
            binary_path = binaries.get(service)
            if binary_path is None:
                self.skipTest(f"binary entrypoint for {service} not present in source tree")
                continue

            source = binary_path.read_text(encoding="utf-8", errors="replace")
            self.assertIn(
                "fn main(",
                source,
                f"{service} startup contract has no fn main() in {binary_path}",
            )
            if has_bind_hint:
                bind_present = any(token in source for token in ("--bind", "bind", "bind_addr", "bind_addr"))
                self.assertTrue(
                    bind_present,
                    f"{service} claims bind-level startup contract in docs but source lacks bind parsing in {binary_path}",
                )

    def test_compiled_binary_presence_for_declared_profiles(self) -> None:
        declared_binaries = _extract_declared_runnable_services(
            DOCS_12.read_text(encoding="utf-8", errors="replace")
        )
        binaries = _iter_binary_crates()
        release_dir = ROOT / "target" / "release"
        if not release_dir.exists():
            self.skipTest("release binaries are not available")

        missing_files: list[str] = []
        for service in sorted(declared_binaries):
            binary_path = binaries.get(service)
            if binary_path is None:
                continue
            binary_target_name = service
            candidate = release_dir / binary_target_name
            if not candidate.exists():
                missing_files.append(service)

        self.assertEqual(
            len(missing_files),
            0,
            f"missing compiled binaries in target/release: {', '.join(missing_files)}",
        )


if __name__ == "__main__":
    unittest.main()
