#!/usr/bin/env python3
"""
Fix missing imports in extracted integration test files.
Strategy: Look at the source file's imports and add them to the test file,
translating `use crate::` and other internal references to external crate imports.
"""

import os
import re
import subprocess
from pathlib import Path

WORKSPACE = Path("/home/z/diccy")
PATH_PREFIX = "export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

def run_cargo(args):
    """Run a cargo command and return output."""
    cmd = f"{PATH_PREFIX} && cargo {args}"
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True, cwd=WORKSPACE, timeout=300)
    return result.stdout + result.stderr

def get_source_imports(src_file):
    """Get all use statements from a source file (outside of test modules)."""
    with open(src_file) as f:
        content = f.read()
    
    # Remove test modules so we only get production imports
    # Simple approach: only get use statements before the first #[cfg(test)]
    pre_test = content.split('#[cfg(test)]')[0]
    
    imports = []
    for line in pre_test.split('\n'):
        stripped = line.strip()
        if stripped.startswith('use ') and stripped.endswith(';'):
            imports.append(stripped)
    
    return imports

def find_source_for_test(test_file):
    """Find the source file a test was extracted from."""
    with open(test_file) as f:
        first_lines = f.read(500)
    
    m = re.search(r'Auto-extracted from (.+)', first_lines)
    if m:
        return m.group(1).strip()
    return None

def fix_imports_for_crate(crate_name, test_file, src_file):
    """Add missing imports to a test file based on the source file's imports."""
    with open(test_file) as f:
        content = f.read()
    
    src_imports = get_source_imports(src_file)
    
    # Convert internal imports to external crate imports
    # use crate:: -> use crate_name::
    # For integration tests, we need to import from the crate being tested
    # and from its dependencies
    
    new_imports = []
    for imp in src_imports:
        # Skip use super::* (already replaced)
        if 'use super::' in imp:
            continue
        
        # Convert `use crate::item` to `use crate_name::item` if it's an internal reference
        # But only if it references the current crate
        if imp.startswith('use crate::'):
            # In integration tests, `crate` refers to the crate being tested
            # So use crate::item is actually correct!
            # But wait - integration test files are separate crates, so `crate` refers to the test crate
            # We need to use the actual crate name
            converted = imp.replace('use crate::', f'use {crate_name}::')
            new_imports.append(converted)
        else:
            # External crate imports - keep as-is
            new_imports.append(imp)
    
    # Find existing imports in the test file
    existing_imports = set()
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped.startswith('use ') and stripped.endswith(';'):
            existing_imports.add(stripped)
    
    # Add missing imports at the top (after the header comment)
    imports_to_add = [imp for imp in new_imports if imp not in existing_imports]
    
    if imports_to_add:
        # Find the position after the header comment and existing imports
        lines = content.split('\n')
        insert_idx = 0
        
        # Skip header comments
        for i, line in enumerate(lines):
            if line.startswith('//') or line.strip() == '':
                insert_idx = i + 1
            else:
                break
        
        # Skip existing use statements
        for i in range(insert_idx, len(lines)):
            if lines[i].strip().startswith('use ') and lines[i].strip().endswith(';'):
                insert_idx = i + 1
            elif lines[i].strip().startswith('use ') and not lines[i].strip().endswith(';'):
                # Multi-line import
                insert_idx = i + 1
                while insert_idx < len(lines) and not lines[insert_idx].strip().endswith(';'):
                    insert_idx += 1
                insert_idx += 1
            elif lines[i].strip() == '' or lines[i].strip().startswith('//'):
                continue
            else:
                break
        
        # Insert new imports
        import_block = '\n'.join(imports_to_add)
        lines.insert(insert_idx, import_block)
        
        with open(test_file, 'w') as f:
            f.write('\n'.join(lines))
        
        return len(imports_to_add)
    
    return 0

def make_private_fields_pub(crate_name, type_name, fields):
    """Make specific struct fields pub(crate) so tests can access them."""
    # Find the struct definition
    for root, dirs, files in os.walk(WORKSPACE / "crates" / crate_name.replace('_', '-') / "src"):
        for f in files:
            if f.endswith('.rs'):
                filepath = os.path.join(root, f)
                with open(filepath) as fh:
                    content = fh.read()
                
                # Find the struct definition
                pattern = rf'(pub\s+)?struct\s+{type_name}\s*\{{'
                m = re.search(pattern, content)
                if m:
                    # Find the struct body and make specified fields pub
                    struct_start = m.end() - 1
                    depth = 0
                    i = struct_start
                    while i < len(content):
                        if content[i] == '{':
                            depth += 1
                        elif content[i] == '}':
                            depth -= 1
                            if depth == 0:
                                break
                        i += 1
                    
                    struct_body = content[struct_start + 1:i]
                    new_body = struct_body
                    
                    for field in fields:
                        # Replace `field: Type` with `pub(crate) field: Type`
                        # But only if it's not already pub
                        pattern = rf'(\n\s+)(?!pub\s)({field}\s*:)'
                        new_body = re.sub(pattern, rf'\1pub(crate) \2', new_body)
                    
                    if new_body != struct_body:
                        new_content = content[:struct_start + 1] + new_body + content[i:]
                        with open(filepath, 'w') as fh:
                            fh.write(new_content)
                        print(f"  Made fields {fields} pub(crate) in {type_name} ({filepath})")

def main():
    # Get all test files with their source files
    test_files = []
    for root, dirs, files in os.walk(WORKSPACE / "crates"):
        if '/tests/' in root:
            for f in files:
                if f.endswith('.rs'):
                    test_file = os.path.join(root, f)
                    src_file = find_source_for_test(test_file)
                    if src_file:
                        # Convert relative path to absolute
                        if not src_file.startswith('/'):
                            src_file = str(WORKSPACE / src_file)
                        test_files.append((test_file, src_file))
    
    print(f"Found {len(test_files)} test files with source references")
    
    # Fix imports for all test files
    fixed_count = 0
    for test_file, src_file in test_files:
        if os.path.exists(src_file):
            # Determine crate name from test file path
            parts = Path(test_file).parts
            crates_idx = parts.index('crates')
            crate_dir_name = parts[crates_idx + 1]
            crate_name = crate_dir_name.replace('-', '_')
            
            added = fix_imports_for_crate(crate_name, test_file, src_file)
            if added > 0:
                print(f"  Added {added} imports to {test_file}")
                fixed_count += 1
    
    print(f"\nFixed imports in {fixed_count} files")

if __name__ == '__main__':
    main()
