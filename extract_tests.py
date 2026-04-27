#!/usr/bin/env python3
"""
S13-T8: Extract inline tests from production source to tests/ directories.

For each Rust source file containing #[cfg(test)] mod ... { ... } blocks:
1. Extract the test module body
2. Create a corresponding integration test file in tests/
3. Fix imports (use super::* → use crate_name::*)
4. Remove the #[cfg(test)] mod block from the source file
"""

import os
import re
import sys
import subprocess
from pathlib import Path

WORKSPACE = Path("/home/z/diccy/crates")

def find_brace_end(content, start):
    """Find the matching closing brace for an opening brace at position start."""
    depth = 0
    i = start
    while i < len(content):
        if content[i] == '{':
            depth += 1
        elif content[i] == '}':
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1

def extract_test_modules(content):
    """Extract all #[cfg(test)] mod ... { ... } blocks from content.
    Returns list of (start_pos, end_pos, mod_name, mod_body) tuples."""
    results = []
    # Match #[cfg(test)] followed by mod name {
    pattern = re.compile(r'#\[cfg\(test\)\]\s*(?:\n\s*)?mod\s+(\w+)\s*\{')
    
    for m in pattern.finditer(content):
        mod_name = m.group(1)
        # Find the opening brace
        brace_start = m.end() - 1  # The '{' is the last char of the match
        brace_end = find_brace_end(content, brace_start)
        if brace_end == -1:
            print(f"  WARNING: Could not find matching brace for mod {mod_name}")
            continue
        
        mod_body = content[brace_start + 1:brace_end]
        # Include the #[cfg(test)] and any preceding blank line
        start = m.start()
        # Include one preceding blank line if exists
        if start > 0 and content[start-1] == '\n':
            start -= 1
            if start > 0 and content[start-1] == '\n':
                start -= 1
        
        results.append((start, brace_end + 1, mod_name, mod_body))
    
    return results

def get_crate_name(src_file):
    """Get the crate name from the Cargo.toml in the crate directory."""
    # Walk up to find the crate root (directory containing Cargo.toml)
    p = Path(src_file)
    # Find the crates/.../ directory
    parts = p.parts
    crates_idx = parts.index('crates')
    crate_dir = Path(*parts[:crates_idx + 2])
    cargo_toml = crate_dir / "Cargo.toml"
    
    if cargo_toml.exists():
        with open(cargo_toml) as f:
            for line in f:
                m = re.match(r'^name\s*=\s*"([^"]+)"', line)
                if m:
                    return m.group(1)
    # Fallback: use directory name with hyphens
    return crate_dir.name.replace('_', '-')

def get_module_path(src_file):
    """Get the module path relative to the crate's src/ directory."""
    p = Path(src_file)
    parts = p.parts
    crates_idx = parts.index('crates')
    crate_dir = Path(*parts[:crates_idx + 2])
    src_dir = crate_dir / "src"
    return p.relative_to(src_dir)

def fix_imports(mod_body, crate_name, src_file):
    """Fix imports for integration test context.
    - use super::* → use crate_name::*;
    - use super::item → use crate_name::item;
    - use crate::... stays the same
    """
    lines = mod_body.split('\n')
    fixed_lines = []
    
    for line in lines:
        stripped = line.strip()
        
        # Replace use super::* with use crate_name::*
        if stripped == 'use super::*;' or stripped.startswith('use super::*;'):
            line = line.replace('use super::*;', f'use {crate_name}::*;')
        # Replace use super::item with use crate_name::item
        elif stripped.startswith('use super::') and 'use super::*' not in stripped:
            line = line.replace('use super::', f'use {crate_name}::')
        # use crate::... stays as-is for integration tests  
        # (in integration tests, crate refers to the crate being tested)
        
        fixed_lines.append(line)
    
    return '\n'.join(fixed_lines)

def generate_test_file(crate_name, mod_name, mod_body, src_file):
    """Generate the content for an integration test file."""
    fixed_body = fix_imports(mod_body, crate_name, src_file)
    
    # Determine a good filename based on the source module
    module_path = get_module_path(src_file)
    module_stem = module_path.stem  # e.g., "clinical", "lib", "main"
    
    if module_stem in ('lib', 'main'):
        # Tests from lib.rs or main.rs go to tests/ directly
        if mod_name == 'tests':
            filename = f"inline_tests.rs"
        else:
            filename = f"inline_{mod_name}.rs"
    else:
        # Tests from submodules get a prefixed name
        if mod_name == 'tests':
            filename = f"{module_stem}_tests.rs"
        else:
            filename = f"{module_stem}_{mod_name}.rs"
    
    header = f"""// Auto-extracted from {src_file}
// S13-T8: Move inline tests to tests/ directories

"""
    
    return filename, header + fixed_body

def process_file(src_file):
    """Process a single source file and extract its test modules."""
    with open(src_file, 'r') as f:
        content = f.read()
    
    test_modules = extract_test_modules(content)
    if not test_modules:
        return []
    
    crate_name = get_crate_name(src_file)
    parts = Path(src_file).parts
    crates_idx = parts.index('crates')
    crate_dir = Path(*parts[:crates_idx + 2])
    tests_dir = crate_dir / "tests"
    
    created_files = []
    
    for start, end, mod_name, mod_body in test_modules:
        filename, file_content = generate_test_file(crate_name, mod_name, mod_body, src_file)
        test_file = tests_dir / filename
        
        # Handle filename collisions
        if test_file.exists():
            base = test_file.stem
            ext = test_file.suffix
            counter = 1
            while test_file.exists():
                test_file = tests_dir / f"{base}_{counter}{ext}"
                counter += 1
            filename = test_file.name
        
        tests_dir.mkdir(parents=True, exist_ok=True)
        with open(test_file, 'w') as f:
            f.write(file_content)
        
        created_files.append((str(test_file), filename))
        print(f"  Created: {test_file}")
    
    # Remove test modules from source file (process from end to preserve positions)
    new_content = content
    for start, end, mod_name, mod_body in reversed(test_modules):
        new_content = new_content[:start] + new_content[end:]
    
    # Clean up trailing whitespace/blank lines at end of file
    new_content = new_content.rstrip() + '\n'
    
    with open(src_file, 'w') as f:
        f.write(new_content)
    
    print(f"  Removed {len(test_modules)} test module(s) from {src_file}")
    return created_files

def process_standalone_tests_rs(src_file):
    """Handle the special case of src/tests.rs files that are included via mod tests;"""
    # These are already in a separate file but still in src/
    # We just need to move them to tests/ directory
    crate_name = get_crate_name(src_file)
    parts = Path(src_file).parts
    crates_idx = parts.index('crates')
    crate_dir = Path(*parts[:crates_idx + 2])
    tests_dir = crate_dir / "tests"
    
    with open(src_file, 'r') as f:
        content = f.read()
    
    # Fix imports
    fixed_content = fix_imports(content, crate_name, src_file)
    
    # Add header
    header = f"""// Auto-extracted from {src_file}
// S13-T8: Move inline tests to tests/ directories

"""
    
    test_file = tests_dir / "inline_tests.rs"
    tests_dir.mkdir(parents=True, exist_ok=True)
    with open(test_file, 'w') as f:
        f.write(header + fixed_content)
    
    # Remove the mod tests; declaration from lib.rs
    lib_rs = crate_dir / "src" / "lib.rs"
    if lib_rs.exists():
        with open(lib_rs, 'r') as f:
            lib_content = f.read()
        
        # Remove "mod tests;" or "mod tests {" lines
        lib_content = re.sub(r'\n\s*mod tests;\s*\n', '\n', lib_content)
        lib_content = re.sub(r'\n\s*#\[cfg\(test\)\]\s*\n\s*mod tests;\s*\n', '\n', lib_content)
        lib_content = lib_content.rstrip() + '\n'
        
        with open(lib_rs, 'w') as f:
            f.write(lib_content)
        print(f"  Removed mod tests; from {lib_rs}")
    
    # Delete the src/tests.rs file
    os.remove(src_file)
    print(f"  Moved {src_file} → {test_file} (and deleted original)")
    
    return [(str(test_file), "inline_tests.rs")]

def find_all_test_files():
    """Find all source files with inline tests."""
    test_files = []
    for root, dirs, files in os.walk(WORKSPACE):
        # Skip target directories
        if 'target' in root:
            continue
        for f in files:
            if f.endswith('.rs'):
                filepath = os.path.join(root, f)
                if '/src/' in filepath:
                    with open(filepath, 'r') as fh:
                        content = fh.read()
                    if '#[test]' in content:
                        test_files.append(filepath)
    return test_files

def main():
    # Phase 1: Find all files with inline tests
    test_files = find_all_test_files()
    print(f"Found {len(test_files)} source files with inline tests")
    
    all_created = []
    
    # Phase 2: Process each file
    # Sort by number of tests (biggest first)
    file_counts = []
    for f in test_files:
        with open(f) as fh:
            count = fh.read().count('#[test]')
        file_counts.append((count, f))
    file_counts.sort(reverse=True)
    
    for count, f in file_counts:
        print(f"\nProcessing: {f} ({count} tests)")
        
        # Special case: src/tests.rs files
        if os.path.basename(f) == 'tests.rs' and 'mod tests' in open(f).read()[:100]:
            created = process_standalone_tests_rs(f)
        else:
            created = process_file(f)
        all_created.extend(created)
    
    print(f"\n\n=== SUMMARY ===")
    print(f"Created {len(all_created)} integration test files:")
    for path, name in all_created:
        print(f"  {path}")
    
    print(f"\nNow run: cargo test --workspace")

if __name__ == '__main__':
    main()
