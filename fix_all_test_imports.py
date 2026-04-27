#!/usr/bin/env python3
"""
Comprehensive fix for all test import issues across the workspace.
- Adds missing crate imports to test files
- Makes private TAG constants, functions, and types public (with doc comments)
"""

import os
import re
import subprocess
from pathlib import Path

WORKSPACE = Path("/home/z/diccy")

def run_cargo(args):
    cmd = f'export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH" && cargo {args}'
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=WORKSPACE, timeout=300)
    return result.stdout + result.stderr

# Map crate dir names to their public crate names (underscored)
def get_crate_name(crate_dir):
    return crate_dir.replace('-', '_')

# Common import mappings: what source files typically import
COMMON_IMPORTS = {
    'dicom_core': ['Dataset', 'Element', 'Error', 'ErrorKind', 'Result', 'Tag', 'Value', 'Vr'],
    'dicom_pixel': ['DisplayFrame', 'PixelFormat'],
}

def add_imports_to_test(test_file, imports_dict):
    """Add missing imports to a test file."""
    with open(test_file) as f:
        content = f.read()

    lines = content.split('\n')

    # Find existing use statements
    existing_uses = set()
    use_end_idx = 0
    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith('use ') and stripped.endswith(';'):
            existing_uses.add(stripped)
            use_end_idx = i + 1
        elif stripped.startswith('use ') and not stripped.endswith(';'):
            # Multi-line import
            existing_uses.add(stripped)
            use_end_idx = i + 1

    # Generate new import lines
    new_imports = []
    for crate, items in imports_dict.items():
        # Check which items are actually used in the test file
        needed = []
        for item in items:
            # Simple heuristic: if the item name appears in the test code
            if re.search(rf'\b{item}\b', content) and f'use {crate}::' not in content:
                needed.append(item)

        if needed:
            import_line = f'    use {crate}::{{{", ".join(needed)}}};'
            if import_line not in existing_uses:
                new_imports.append(import_line)

    if new_imports:
        # Insert after existing use statements
        for imp in new_imports:
            lines.insert(use_end_idx, imp)
            use_end_idx += 1

        with open(test_file, 'w') as f:
            f.write('\n'.join(lines))
        return len(new_imports)
    return 0

def make_constants_public(src_file):
    """Make private TAG_ and other constants public with doc comments."""
    with open(src_file) as f:
        content = f.read()

    changed = False

    # Make const TAG_... public
    pattern = re.compile(r'^const (TAG_\w+): Tag = Tag\((0x[\da-fA-F]+), (0x[\da-fA-F]+)\);', re.MULTILINE)
    for m in pattern.finditer(content):
        name = m.group(1)
        group = m.group(2)
        elem = m.group(3)
        old = m.group(0)
        # Check if already pub
        if not content[max(0, m.start()-20):m.start()].strip().endswith('pub'):
            new = f'/// DICOM Tag ({group},{elem})\npub {old}'
            content = content.replace(old, new, 1)
            changed = True

    # Make private helper functions public (read_str, read_u16, read_bytes, etc.)
    helper_pattern = re.compile(r'^fn (read_\w+|missing_required_tag|invalid_tag_value|format_ds)\b', re.MULTILINE)
    for m in helper_pattern.finditer(content):
        old = m.group(0)
        fn_name = m.group(1)
        new = f'/// Helper function for {fn_name}\npub {old}'
        content = content.replace(old, new, 1)
        changed = True

    # Make private FRAME_TIME_EPS-like constants public
    eps_pattern = re.compile(r'^const (\w+_EPS): f64 = ', re.MULTILINE)
    for m in eps_pattern.finditer(content):
        old = m.group(0)
        name = m.group(1)
        new = f'/// Epsilon value for {name}\npub {old}'
        content = content.replace(old, new, 1)
        changed = True

    if changed:
        with open(src_file, 'w') as f:
            f.write(content)

    return changed

def fix_all_pack_crates():
    """Fix all pack-* and modality-* crates."""
    for crate_dir in sorted(WORKSPACE.glob("crates/pack-*")) + sorted(WORKSPACE.glob("crates/modality-*")):
        crate_name = get_crate_name(crate_dir.name)
        lib_rs = crate_dir / "src" / "lib.rs"

        if not lib_rs.exists():
            continue

        # Make constants public
        make_constants_public(str(lib_rs))

        # Fix test files
        tests_dir = crate_dir / "tests"
        if tests_dir.exists():
            for test_file in sorted(tests_dir.glob("*.rs")):
                # Only add imports to inline test files
                if 'inline' in test_file.name or test_file.stem.endswith('_tests'):
                    add_imports_to_test(str(test_file), COMMON_IMPORTS)

def fix_all_dicom_crates():
    """Fix all dicom-* crates."""
    for crate_dir in sorted(WORKSPACE.glob("crates/dicom-*")):
        crate_name = get_crate_name(crate_dir.name)

        # Fix test files
        tests_dir = crate_dir / "tests"
        if tests_dir.exists():
            for test_file in sorted(tests_dir.glob("*.rs")):
                if 'inline' in test_file.name or test_file.stem.endswith('_tests'):
                    add_imports_to_test(str(test_file), COMMON_IMPORTS)

def main():
    print("=== Fixing all pack crates ===")
    fix_all_pack_crates()

    print("=== Fixing all dicom crates ===")
    fix_all_dicom_crates()

    print("=== Done ===")

if __name__ == '__main__':
    main()
