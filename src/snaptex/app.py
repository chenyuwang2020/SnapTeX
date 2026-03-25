"""SnapTeX application entry point."""

import os
import signal
import sys
import logging
from logging.handlers import RotatingFileHandler

from snaptex.config import setup_environment, AppConfig, app_data_dir

# Set up data directories BEFORE importing OCR modules.
setup_environment()

from PySide6.QtWidgets import QApplication
from PySide6.QtGui import QIcon
from PySide6.QtCore import QTimer

from snaptex.controller import AppController

_RESOURCES = os.path.join(os.path.dirname(__file__), "resources")
_ICON_PATH = os.path.join(_RESOURCES, "icon.ico")


def _setup_logging():
    log_file = os.path.join(str(app_data_dir()), "snaptex.log")
    os.makedirs(os.path.dirname(log_file), exist_ok=True)
    logging.basicConfig(
        level=logging.DEBUG,
        format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
        handlers=[
            RotatingFileHandler(
                log_file, maxBytes=5 * 1024 * 1024, backupCount=2, encoding="utf-8"
            ),
            logging.StreamHandler(sys.stderr),
        ],
    )
    return logging.getLogger(__name__)


def main():
    logger = _setup_logging()
    logger.info("SnapTeX starting, frozen=%s", getattr(sys, "frozen", False))

    app = QApplication(sys.argv)
    app.setApplicationName("SnapTeX")
    if os.path.exists(_ICON_PATH):
        app.setWindowIcon(QIcon(_ICON_PATH))

    # Allow Ctrl+C in terminal to quit.
    signal.signal(signal.SIGINT, lambda *_: app.quit())
    timer = QTimer()
    timer.timeout.connect(lambda: None)
    timer.start(200)

    config = AppConfig.load()
    controller = AppController(app, config)
    controller.start()

    sys.exit(app.exec())


if __name__ == "__main__":
    try:
        main()
    except Exception:
        logging.getLogger(__name__).critical("Fatal error", exc_info=True)
        raise
