#!/usr/bin/env python3
"""
Project Structure Scanner
Generates a tree view of the entire project structure.
Robust against CWD execution.
"""

import os
from pathlib import Path

# --- Configuration ---
# Script location: .../collections/scripts/get_structure.py
SCRIPT_DIR = Path(__file__).resolve().parent
# Collections root: .../collections
COLLECTIONS_ROOT = SCRIPT_DIR.parent
# Project root (screener123): .../
PROJECT_ROOT = COLLECTIONS_ROOT.parent

# Output location: .../collections/docs/project_structure.txt
OUTPUT_DIR = COLLECTIONS_ROOT / "docs"
OUTPUT_FILE = OUTPUT_DIR / "project_structure.txt"

# Excluded folders
EXCLUDED_DIRS = {
    'bin', 'obj', 'Debug', 'Release', 
    'build', 'dist', '__pycache__', 
    '.git', '.vs', '.idea', '.vscode',
    'node_modules', '.github'
}

def write_tree(f, path: Path, prefix=''):
    try:
        # Get all items, sorted
        try:
            items = sorted(os.listdir(path))
        except FileNotFoundError:
            return

        # Separate dirs and files
        dirs = []
        files = []
        
        for item in items:
            full_path = path / item
            if full_path.is_dir():
                if item not in EXCLUDED_DIRS:
                    dirs.append(item)
            else:
                files.append(item)
        
        all_items = dirs + files

        for i, item in enumerate(all_items):
            is_last = (i == len(all_items) - 1)
            connector = '`- ' if is_last else '|- '
            
            f.write(f"{prefix}{connector}{item}\n")
            
            if item in dirs:
                extension = '  ' if is_last else '| '
                write_tree(f, path / item, prefix + extension)
                
    except PermissionError:
        f.write(f"{prefix}!- [Permission Denied]\n")

if __name__ == '__main__':
    print(f"Scanning project root: {PROJECT_ROOT}")
    print(f"Output target: {OUTPUT_FILE}")
    
    # Ensure output directory exists
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    try:
        with open(OUTPUT_FILE, 'w', encoding='utf-8') as f:
            f.write(f"Project Structure for: {PROJECT_ROOT.name}\n")
            f.write(f"Root Path: {PROJECT_ROOT}\n")
            f.write(".\n")
            write_tree(f, PROJECT_ROOT)
        print("✅ Structure generated successfully.")
    except Exception as e:
        print(f"❌ Error: {e}")
