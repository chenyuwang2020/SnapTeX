"""Standalone uninstaller for Pix2Text Windows app.

Removes all app data: models, config, and cache.
Usage:  python uninstall.py
"""

import os
import shutil
from pathlib import Path


def app_data_dir() -> Path:
    base = os.environ.get("LOCALAPPDATA", os.path.expanduser("~"))
    return Path(base) / "SnapTeX"


def get_size_mb(path: Path) -> float:
    total = 0
    if path.exists():
        for f in path.rglob("*"):
            if f.is_file():
                total += f.stat().st_size
    return total / (1024 * 1024)


def main():
    data_dir = app_data_dir()

    if not data_dir.exists():
        print(f"No data found at: {data_dir}")
        print("Nothing to clean up.")
        return

    size = get_size_mb(data_dir)
    print(f"Pix2Text data directory: {data_dir}")
    print(f"Total size: {size:.1f} MB")
    print()
    print("Contents:")
    for item in sorted(data_dir.iterdir()):
        if item.is_dir():
            dir_size = get_size_mb(item)
            print(f"  [DIR]  {item.name}/  ({dir_size:.1f} MB)")
        else:
            file_size = item.stat().st_size / (1024 * 1024)
            print(f"  [FILE] {item.name}  ({file_size:.2f} MB)")

    print()
    answer = input("Delete all data? This cannot be undone. [y/N]: ").strip().lower()
    if answer == "y":
        shutil.rmtree(data_dir, ignore_errors=True)
        print(f"Deleted: {data_dir}")
        print("Uninstall complete.")
    else:
        print("Cancelled.")


if __name__ == "__main__":
    main()
