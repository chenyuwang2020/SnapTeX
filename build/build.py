"""SnapTeX build script.

Usage:
    python build/build.py                 # PyInstaller only
    python build/build.py --installer     # PyInstaller + Inno Setup
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

BUILD_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = BUILD_DIR.parent
SPEC_FILE = BUILD_DIR / "snaptex.spec"
ISS_FILE = BUILD_DIR / "installer.iss"
DIST_DIR = BUILD_DIR / "dist"
OUTPUT_DIR = BUILD_DIR / "output"

ISCC_PATHS = [
    Path(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"),
    Path(r"C:\Program Files\Inno Setup 6\ISCC.exe"),
]


def find_iscc():
    for p in ISCC_PATHS:
        if p.exists():
            return str(p)
    return None


def run_pyinstaller():
    print("=" * 60)
    print("Step 1: PyInstaller")
    print("=" * 60)

    cmd = [
        sys.executable, "-m", "PyInstaller",
        "--noconfirm",
        "--distpath", str(DIST_DIR),
        "--workpath", str(BUILD_DIR / "build"),
        str(SPEC_FILE),
    ]
    print(f"  Running: {' '.join(cmd)}\n")
    subprocess.run(cmd, check=True, cwd=str(PROJECT_ROOT))

    exe_path = DIST_DIR / "SnapTeX" / "SnapTeX.exe"
    if exe_path.exists():
        total = sum(f.stat().st_size for f in (DIST_DIR / "SnapTeX").rglob("*") if f.is_file())
        print(f"\n  OK: {exe_path}")
        print(f"  Size: {total / 1024 / 1024:.0f} MB")
    else:
        print("\n  ERROR: SnapTeX.exe not found!")
        sys.exit(1)


def run_inno_setup():
    print("\n" + "=" * 60)
    print("Step 2: Inno Setup")
    print("=" * 60)

    iscc = find_iscc()
    if not iscc:
        print("  ERROR: Inno Setup not found!")
        print("  Install from: https://jrsoftware.org/isdl.php")
        sys.exit(1)

    OUTPUT_DIR.mkdir(exist_ok=True)

    cmd = [iscc, str(ISS_FILE)]
    print(f"  Running: {' '.join(cmd)}\n")
    subprocess.run(cmd, check=True, cwd=str(BUILD_DIR))

    setup_exe = OUTPUT_DIR / "SnapTeX-Setup.exe"
    if setup_exe.exists():
        size_mb = setup_exe.stat().st_size / 1024 / 1024
        print(f"\n  OK: {setup_exe}")
        print(f"  Size: {size_mb:.0f} MB")
    else:
        print("\n  ERROR: SnapTeX-Setup.exe not found!")
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="Build SnapTeX")
    parser.add_argument("--installer", action="store_true",
                        help="Also build Inno Setup installer")
    args = parser.parse_args()

    run_pyinstaller()

    if args.installer:
        run_inno_setup()
        print(f"\n{'=' * 60}")
        print("BUILD COMPLETE")
        print(f"{'=' * 60}")
    else:
        print(f"\n{'=' * 60}")
        print("BUILD COMPLETE (PyInstaller only)")
        print(f"  Run: build/dist/SnapTeX/SnapTeX.exe")
        print(f"  Add --installer to also build the setup package")
        print(f"{'=' * 60}")


if __name__ == "__main__":
    main()
