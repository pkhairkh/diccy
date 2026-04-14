from __future__ import annotations

from dataclasses import dataclass
import pathlib
import re


@dataclass(frozen=True)
class RuntimeEnvContractSpec:
    component: str
    prefix: str
    docs_file: pathlib.Path
    docs_heading: str
    code_files: tuple[pathlib.Path, ...]


RUNTIME_ENV_CONTRACTS: tuple[RuntimeEnvContractSpec, ...] = (
    RuntimeEnvContractSpec(
        component="dicom-web-server",
        prefix="DICOM_WEB_",
        docs_file=pathlib.Path("docs/12-API-Surface-and-Crate-Boundaries.md"),
        docs_heading="### `dicom-web-server` (env-contract)",
        code_files=(
            pathlib.Path("crates/dicom-web-server/src/main.rs"),
            pathlib.Path("crates/dicom-env-contract/src/lib.rs"),
        ),
    ),
    RuntimeEnvContractSpec(
        component="dicom-workflow-server",
        prefix="DICOM_WORKFLOW_",
        docs_file=pathlib.Path("docs/12-API-Surface-and-Crate-Boundaries.md"),
        docs_heading="### `dicom-workflow-server` (env-contract)",
        code_files=(
            pathlib.Path("crates/dicom-workflow-server/src/main.rs"),
            pathlib.Path("crates/dicom-env-contract/src/lib.rs"),
        ),
    ),
    RuntimeEnvContractSpec(
        component="dicom-dimse-service",
        prefix="DICOM_DIMSE_",
        docs_file=pathlib.Path("docs/12-API-Surface-and-Crate-Boundaries.md"),
        docs_heading="### `dicom-dimse-service` (env-contract)",
        code_files=(
            pathlib.Path("crates/dicom-dimse-service/src/bin/dicom-dimse-service.rs"),
            pathlib.Path("crates/dicom-env-contract/src/lib.rs"),
        ),
    ),
)


def _read_text(path: pathlib.Path) -> str:
    if not path.exists():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def _extract_markdown_section(markdown: str, heading: str, fallback_service: str) -> str:
    lines = markdown.splitlines()
    start = None
    for idx, line in enumerate(lines):
        if line.strip() == heading:
            start = idx + 1
            break
    if start is None:
        marker = f"| `{fallback_service}`"
        for line in lines:
            if line.startswith(marker):
                return line
        return ""

    end = len(lines)
    for idx in range(start, len(lines)):
        if lines[idx].startswith("### `"):
            end = idx
            break
    return "\n".join(lines[start:end])


def _extract_env_vars_from_docs(text: str, prefix: str) -> set[str]:
    pattern = re.compile(rf"\b({re.escape(prefix)}[A-Z0-9_]+)\b")
    candidates = {match.group(1) for match in pattern.finditer(text)}
    return {token for token in candidates if not token.endswith("_")}


def _extract_env_vars_from_rust_code(text: str, prefix: str) -> set[str]:
    escaped_prefix = re.escape(prefix)
    patterns = (
        re.compile(rf'env::var\(\s*"({escaped_prefix}[A-Z0-9_]+)"\s*\)'),
        re.compile(rf'parse_env_[a-z0-9_]+\(\s*"({escaped_prefix}[A-Z0-9_]+)"'),
        re.compile(rf'"({escaped_prefix}[A-Z0-9_]+)"\s*,?\s*'),
    )

    env_vars: set[str] = set()
    for pattern in patterns:
        for match in pattern.finditer(text):
            token = match.group(1)
            if not token.endswith("_"):
                env_vars.add(token)
    return env_vars


def _extract_env_var_refs(text: str, prefix: str) -> dict[str, tuple[str, int]]:
    patterns = (
        re.compile(rf'env::var\(\s*"({re.escape(prefix)}[A-Z0-9_]+)"\s*\)'),
        re.compile(rf'parse_env_[a-z0-9_]+\(\s*"({re.escape(prefix)}[A-Z0-9_]+)"'),
        re.compile(rf'"({re.escape(prefix)}[A-Z0-9_]+)"\s*,?\s*'),
    )
    refs: dict[str, tuple[str, int]] = {}
    for line_no, line in enumerate(text.splitlines(), start=1):
        for pattern in patterns:
            for match in pattern.finditer(line):
                token = match.group(1)
                if token.endswith("_"):
                    continue
                refs.setdefault(token, ("", line_no))
    return refs


def find_line(path: pathlib.Path, token: str) -> int | None:
    if not path.exists():
        return None
    for idx, line in enumerate(path.read_text(encoding="utf-8", errors="replace").splitlines(), start=1):
        if token in line:
            return idx
    return None


def _normalize_components(components: tuple[str, ...] | None) -> set[str] | None:
    if not components:
        return None
    return {component.strip() for component in components if component.strip()}


def extract_runtime_env_contract(
    repo_root: pathlib.Path,
    components: tuple[str, ...] | None = None,
) -> list[dict[str, object]]:
    contracts: list[dict[str, object]] = []
    component_filter = _normalize_components(components)

    for spec in RUNTIME_ENV_CONTRACTS:
        if component_filter and spec.component not in component_filter:
            continue

        docs_path = repo_root / spec.docs_file
        docs_text = _read_text(docs_path)
        docs_section = _extract_markdown_section(
            docs_text,
            spec.docs_heading,
            fallback_service=spec.component,
        )
        docs_env_vars = set()
        if docs_section:
            docs_env_vars = _extract_env_vars_from_docs(docs_section, spec.prefix)

        code_env_vars: set[str] = set()
        existing_code_files: list[str] = []
        code_env_refs: dict[str, tuple[str, int]] = {}
        for code_file in spec.code_files:
            full_path = repo_root / code_file
            if not full_path.exists():
                continue
            existing_code_files.append(code_file.as_posix())
            text = _read_text(full_path)
            code_env_vars |= _extract_env_vars_from_rust_code(text, spec.prefix)
            line_refs = _extract_env_var_refs(text, spec.prefix)
            for token, line_ref in line_refs.items():
                if token not in code_env_refs:
                    code_env_refs[token] = (code_file.as_posix(), line_ref[1])

        missing_in_docs = sorted(code_env_vars - docs_env_vars)
        documented_not_in_code = sorted(docs_env_vars - code_env_vars)
        env_var_source_refs = {
            token: {
                "path": code_env_refs[token][0] if token in code_env_refs else "",
                "line": code_env_refs[token][1] if token in code_env_refs else 0,
            }
            for token in sorted(code_env_vars)
        }

        contracts.append(
            {
                "component": spec.component,
                "prefix": spec.prefix,
                "docs_file": spec.docs_file.as_posix(),
                "docs_heading": spec.docs_heading,
                "code_files": existing_code_files,
                "code_env_vars": sorted(code_env_vars),
                "code_env_var_refs": env_var_source_refs,
                "docs_env_vars": sorted(docs_env_vars),
                "missing_in_docs": missing_in_docs,
                "documented_not_in_code": documented_not_in_code,
            }
        )

    return contracts


def runtime_env_contract_has_drift(contracts: list[dict[str, object]]) -> bool:
    for contract in contracts:
        if contract["missing_in_docs"] or contract["documented_not_in_code"]:
            return True
    return False
