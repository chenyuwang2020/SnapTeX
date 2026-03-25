"""Configuration management for SnapTeX.

All app data (config + models) lives under a single directory:
  %LOCALAPPDATA%\\SnapTeX\\
"""

import json
import os
import shutil
from dataclasses import dataclass, asdict
from pathlib import Path

from snaptex import __version__

APP_VERSION = __version__


def app_data_dir() -> Path:
    """The single root directory for all SnapTeX app data."""
    base = os.environ.get("LOCALAPPDATA", os.path.expanduser("~"))
    return Path(base) / "SnapTeX"


def models_dir() -> Path:
    return app_data_dir() / "models"


def config_path() -> Path:
    return app_data_dir() / "config.json"


def log_path() -> Path:
    return app_data_dir() / "snaptex.log"


def setup_environment():
    """Set PIX2TEXT_HOME so pix2text's LatexOCR stores models in our data dir.

    Must be called BEFORE importing OCR modules.
    """
    mdir = models_dir()
    mdir.mkdir(parents=True, exist_ok=True)
    os.environ["PIX2TEXT_HOME"] = str(mdir)


def get_data_size_mb() -> float:
    """Return total size of the app data directory in MB."""
    total = 0
    root = app_data_dir()
    if root.exists():
        for f in root.rglob("*"):
            if f.is_file():
                total += f.stat().st_size
    return total / (1024 * 1024)


def remove_all_data():
    """Remove the entire app data directory. For uninstall."""
    root = app_data_dir()
    if root.exists():
        shutil.rmtree(root, ignore_errors=True)


@dataclass
class AppConfig:
    device: str = "cpu"
    auto_monitor_clipboard: bool = True
    auto_copy_result: bool = True
    window_stay_on_top: bool = True

    @classmethod
    def load(cls, path: Path = None) -> "AppConfig":
        path = path or config_path()
        if path.exists():
            try:
                with open(path, "r", encoding="utf-8") as f:
                    data = json.load(f)
                return cls(**{k: v for k, v in data.items() if k in cls.__dataclass_fields__})
            except Exception:
                pass
        return cls()

    def save(self, path: Path = None):
        path = path or config_path()
        path.parent.mkdir(parents=True, exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(asdict(self), f, indent=2, ensure_ascii=False)
