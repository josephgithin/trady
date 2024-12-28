import os
import json
from pathlib import Path

def scan_rust_project(project_path: str) -> dict:
    """Scan a Rust project and create a simple path:content mapping"""
    project_path = Path(project_path)
    files_dict = {}

    for root, _, files in os.walk(project_path):
        root_path = Path(root)
        
        # Skip target directory and other common excludes
        if any(p in str(root_path) for p in ['target/', '.git/', 'node_modules/', '.idea/']):
            continue

        for file_name in files:
            file_path = root_path / file_name
            
            # Get relative path from project root
            rel_path = str(file_path.relative_to(project_path))
            
            # Skip certain files
            if any(p in rel_path for p in ['.git/', '.idea/']):
                continue

            try:
                with open(file_path, 'r', encoding='utf-8') as f:
                    content = f.read()
                    if content.strip():  # Only add non-empty files
                        files_dict[rel_path] = content
            except Exception as e:
                print(f"Warning: Could not read {rel_path}: {e}")

    return files_dict

def main():
    import argparse
    parser = argparse.ArgumentParser(description='Convert Rust project to simple JSON format')
    parser.add_argument('project_path', help='Path to the Rust project')
    parser.add_argument('--output', '-o', default='project.json', help='Output JSON file')
    args = parser.parse_args()

    try:
        print(f"Scanning project: {args.project_path}")
        files_dict = scan_rust_project(args.project_path)
        
        with open(args.output, 'w', encoding='utf-8') as f:
            json.dump(files_dict, f, indent=2)
        
        print(f"\nProject exported to: {args.output}")
        print(f"Total files: {len(files_dict)}")
        
    except Exception as e:
        print(f"Error: {e}")
        return 1
    
    return 0

if __name__ == "__main__":
    exit(main())

