"""PyInstaller entry point for SnapTeX."""

import sys
import os

if getattr(sys, "frozen", False):
    sys.path.insert(0, sys._MEIPASS)

from snaptex.config import setup_environment
setup_environment()

from snaptex.app import main
main()
