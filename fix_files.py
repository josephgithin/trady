#!/usr/bin/env python3
import os
import shutil
from pathlib import Path

def create_directory(path):
    """Create directory if it doesn't exist"""
    Path(path).mkdir(parents=True, exist_ok=True)

def write_file(path, content):
    """Write content to file, creating parent directories if needed"""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content)

def main():
    # Base project directory (assuming script is run from project root)
    src_dir = "src"
    base_dir = Path(src_dir)

    # Create main module directories
    module_dirs = [
        "common",
        "domain",
        "infra",
        "ui",
    ]

    for dir_name in module_dirs:
        create_directory(base_dir / dir_name)

    # Create subdirectories
    subdirs = [
        "infra/event_bus",
        "infra/adapters",
        "infra/broker_adapters",
        "infra/execution",
        "domain/strategies",
    ]

    for subdir in subdirs:
        create_directory(base_dir / subdir)

    # Define mod.rs files content
    mod_files = {
        "common/mod.rs": """pub mod types;
pub mod enums;
pub mod constants;
pub mod utils;
""",
        "domain/mod.rs": """pub mod events;
pub mod interfaces;
pub mod models;
pub mod strategies;
""",
        "infra/mod.rs": """pub mod adapters;
pub mod broker_adapters;
pub mod event_bus;
pub mod execution;
""",
        "ui/mod.rs": """pub mod console;
pub mod logger;
pub mod metrics;
pub mod audit;
""",
        "domain/strategies/mod.rs": """pub mod arbitrage;
pub mod mean_reversion;
pub mod base;
""",
        "infra/adapters/mod.rs": """pub mod kraken;
pub mod coinbase;
pub mod exchange;
""",
        "infra/broker_adapters/mod.rs": """pub mod interactive_broker_adapter;
pub mod base;
""",
        "infra/event_bus/mod.rs": "pub mod zmq;\n",
        "infra/execution/mod.rs": "pub mod manager;\n",
    }

    # Write mod.rs files
    print("Creating module files...")
    for file_path, content in mod_files.items():
        write_file(base_dir / file_path, content)
        print(f"Created {file_path}")

    # Move existing files to their new locations (if they exist)
    file_moves = [
        ("common.rs", "common/types.rs"),  # Split common.rs content appropriately
        ("lib.rs", "lib.rs"),  # Keep in root
        ("main.rs", "main.rs"),  # Keep in root
        ("config.rs", "config.rs"),  # Keep in root
        ("domain/events.rs", "domain/events.rs"),
        ("domain/interfaces.rs", "domain/interfaces.rs"),
        ("domain/models.rs", "domain/models.rs"),
        ("domain/strategies/arbitrage.rs", "domain/strategies/arbitrage.rs"),
        ("domain/strategies/mean_reversion.rs", "domain/strategies/mean_reversion.rs"),
        ("domain/strategies/base.rs", "domain/strategies/base.rs"),
        ("infra/event_bus/zmq.rs", "infra/event_bus/zmq.rs"),
        ("infra/adapters/kraken.rs", "infra/adapters/kraken.rs"),
        ("infra/adapters/coinbase.rs", "infra/adapters/coinbase.rs"),
        ("infra/adapters/exchange.rs", "infra/adapters/exchange.rs"),
        ("infra/broker_adapters/interactive_broker_adapter.rs", 
         "infra/broker_adapters/interactive_broker_adapter.rs"),
        ("infra/broker_adapters/base.rs", "infra/broker_adapters/base.rs"),
        ("infra/execution/manager.rs", "infra/execution/manager.rs"),
        ("ui/console.rs", "ui/console.rs"),
        ("ui/logger.rs", "ui/logger.rs"),
        ("ui/metrics.rs", "ui/metrics.rs"),
        ("ui/audit.rs", "ui/audit.rs"),
    ]

    print("\nMoving files to their new locations...")
    for src, dst in file_moves:
        src_path = base_dir / src
        dst_path = base_dir / dst
        try:
            if src_path.exists():
                print(f"Moving {src} to {dst}")
                dst_path.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(src_path), str(dst_path))
            else:
                print(f"Source file {src} not found, skipping...")
        except Exception as e:
            print(f"Error moving {src} to {dst}: {e}")

    # Create directories and empty files for any missing required files
    required_files = [
        "common/utils.rs",
        "common/constants.rs",
        "common/enums.rs",
    ]

    print("\nCreating empty files for missing required modules...")
    for file_path in required_files:
        full_path = base_dir / file_path
        if not full_path.exists():
            print(f"Creating empty file {file_path}")
            write_file(full_path, "// Add content here\n")

    print("\nModule structure has been organized. Please verify the changes.")
    print("Note: You may need to manually split common.rs into separate files under the common directory.")
    print("\nNext steps:")
    print("1. Verify all files are in the correct locations")
    print("2. Split common.rs content into types.rs, enums.rs, constants.rs, and utils.rs")
    print("3. Update import paths in your code if necessary")

if __name__ == "__main__":
    main()
