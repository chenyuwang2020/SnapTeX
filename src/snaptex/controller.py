"""Central controller that wires all components together."""

import logging
import os
import shutil
import subprocess

from PySide6.QtCore import QObject, Slot
from PySide6.QtWidgets import QApplication, QMessageBox

from snaptex.config import (
    AppConfig, APP_VERSION, app_data_dir, log_path,
    get_data_size_mb, remove_all_data,
)
from snaptex.clipboard import ClipboardMonitor
from snaptex.workers import ModelLoader, RecognitionWorker
from snaptex.ui import ResultWindow, TrayManager, SettingsDialog

logger = logging.getLogger(__name__)


class AppController(QObject):
    """Main controller that coordinates all app components."""

    def __init__(self, app: QApplication, config: AppConfig):
        super().__init__()
        self._app = app
        self._config = config
        self._model_ready = False

        self._result_window = ResultWindow(stay_on_top=config.window_stay_on_top)
        self._tray = TrayManager(app, self)
        self._clipboard_monitor = ClipboardMonitor(app.clipboard())
        self._worker = RecognitionWorker()
        self._model_loader = ModelLoader(config)

        self._model_loader.progress.connect(self._on_model_progress)
        self._model_loader.finished_ok.connect(self._on_model_loaded)
        self._model_loader.error.connect(self._on_model_error)

        self._clipboard_monitor.image_detected.connect(self._on_image_detected)

        self._worker.processing_started.connect(self._on_processing_started)
        self._worker.result_ready.connect(self._on_result_ready)
        self._worker.error_occurred.connect(self._on_recognition_error)

        self._result_window.copy_requested.connect(self._copy_result)
        self._result_window.quit_requested.connect(self.quit_app)

    def start(self):
        self._worker.start()
        self._result_window.set_loading(True)
        self._model_loader.start()
        self._result_window.show()

    def show_window(self):
        self._result_window.show()
        self._result_window.raise_()
        self._result_window.activateWindow()

    @Slot()
    def manual_recognize(self):
        if self._clipboard_monitor.grab_image_now():
            pass
        else:
            self._result_window.set_status("\u526a\u8d34\u677f\u4e2d\u6ca1\u6709\u56fe\u7247")

    def toggle_monitor(self, checked: bool):
        if checked:
            self._clipboard_monitor.enable()
            self._result_window.set_status("\u5c31\u7eea - \u6b63\u5728\u76d1\u542c\u526a\u8d34\u677f")
        else:
            self._clipboard_monitor.disable()
            self._result_window.set_status("\u5c31\u7eea - \u526a\u8d34\u677f\u76d1\u542c\u5df2\u6682\u505c")

    def show_settings(self):
        dialog = SettingsDialog(self._config, self._result_window)
        dialog.exec()

    def quit_app(self):
        self._worker.stop()
        self._worker.wait(3000)
        self._app.quit()

    def open_data_dir(self):
        path = str(app_data_dir())
        os.makedirs(path, exist_ok=True)
        subprocess.Popen(["explorer", path])

    def export_log(self):
        src = log_path()
        if not src.exists():
            self._tray.show_message("SnapTeX", "\u65e5\u5fd7\u6587\u4ef6\u4e0d\u5b58\u5728")
            return
        desktop = os.path.join(os.path.expanduser("~"), "Desktop")
        dst = os.path.join(desktop, "snaptex.log")
        shutil.copy2(str(src), dst)
        self._tray.show_message("SnapTeX", f"\u65e5\u5fd7\u5df2\u5bfc\u51fa\u5230\u684c\u9762\n{dst}")

    def show_about(self):
        QMessageBox.about(
            self._result_window,
            "\u5173\u4e8e SnapTeX",
            f"<h3>SnapTeX v{APP_VERSION}</h3>"
            f"<p>\u622a\u56fe\u8bc6\u522b\u516c\u5f0f\u4e0e\u6587\u5b57\u7684\u684c\u9762\u5de5\u5177</p>"
            f"<p><b>\u516c\u5f0f\u8bc6\u522b</b>\uff1apix2text (LatexOCR)<br>"
            f"<b>\u6587\u5b57\u8bc6\u522b</b>\uff1aRapidOCR (PaddleOCR)</p>"
            f"<p>\u6570\u636e\u76ee\u5f55\uff1a<br><code>{app_data_dir()}</code></p>"
            f"<p>\u65e5\u5fd7\u6587\u4ef6\uff1a<br><code>{log_path()}</code></p>"
        )

    def uninstall_data(self):
        size_mb = get_data_size_mb()
        path = str(app_data_dir())
        reply = QMessageBox.warning(
            self._result_window,
            "\u786e\u8ba4\u6e05\u9664\u6570\u636e",
            f"\u5c06\u5220\u9664\u4ee5\u4e0b\u76ee\u5f55\u4e2d\u7684\u6240\u6709\u6570\u636e"
            f"\uff08\u6a21\u578b + \u914d\u7f6e\uff0c\u5171 {size_mb:.1f} MB\uff09\uff1a\n\n"
            f"{path}\n\n"
            f"\u6b64\u64cd\u4f5c\u4e0d\u53ef\u64a4\u9500\u3002\u786e\u5b9a\u8981\u7ee7\u7eed\u5417\uff1f",
            QMessageBox.StandardButton.Yes | QMessageBox.StandardButton.No,
            QMessageBox.StandardButton.No,
        )
        if reply == QMessageBox.StandardButton.Yes:
            self._worker.stop()
            self._worker.wait(3000)
            remove_all_data()
            self._app.quit()

    # --- Model loading callbacks ---

    def _on_model_progress(self, msg: str):
        self._result_window.set_status(msg)

    def _on_model_loaded(self, latex_ocr):
        self._model_ready = True
        self._worker.set_model(latex_ocr)
        self._tray.set_ready()
        self._result_window.set_loading(False)

        if self._config.auto_monitor_clipboard:
            self._clipboard_monitor.enable()
            self._result_window.set_status("\u5c31\u7eea - \u622a\u56fe\u540e\u81ea\u52a8\u8bc6\u522b")
        else:
            self._result_window.set_status("\u5c31\u7eea")

        self._tray.show_message("SnapTeX", "\u6a21\u578b\u52a0\u8f7d\u5b8c\u6210\uff01")

    def _on_model_error(self, msg: str):
        self._result_window.set_loading(False)
        self._result_window.set_status(f"\u6a21\u578b\u52a0\u8f7d\u5931\u8d25: {msg}")
        self._tray.show_message(
            "SnapTeX",
            f"\u6a21\u578b\u52a0\u8f7d\u5931\u8d25: {msg}\n"
            f"\u8bf7\u901a\u8fc7\u6258\u76d8\u83dc\u5355\u201c\u5bfc\u51fa\u65e5\u5fd7\u201d\u83b7\u53d6\u8be6\u7ec6\u4fe1\u606f"
        )
        logger.error("Model loading failed: %s", msg)

    # --- Recognition callbacks ---

    def _on_image_detected(self, pil_image):
        mode = self._result_window.get_mode()
        if mode == "formula" and not self._model_ready:
            self._result_window.set_status("\u516c\u5f0f\u6a21\u578b\u5c1a\u672a\u52a0\u8f7d\u5b8c\u6210")
            return
        self._result_window.set_image(pil_image)
        self._worker.recognize(pil_image, mode)

    def _on_processing_started(self):
        self._result_window.set_status("\u6b63\u5728\u8bc6\u522b...")

    def _on_result_ready(self, text: str):
        self._result_window.set_result(text)
        self._result_window.set_status("\u8bc6\u522b\u5b8c\u6210")

        if self._config.auto_copy_result and text.strip():
            self._clipboard_monitor.set_ignore_next()
            self._app.clipboard().setText(text)
            self._result_window.flash_copied()
            mode = self._result_window.get_mode()
            label = "LaTeX" if mode == "formula" else "\u6587\u672c"
            self._tray.show_message("SnapTeX", f"{label} \u5df2\u590d\u5236\u5230\u526a\u8d34\u677f")

        self.show_window()

    def _on_recognition_error(self, msg: str):
        self._result_window.set_status(f"\u8bc6\u522b\u5931\u8d25: {msg}")
        self._tray.show_message(
            "SnapTeX",
            f"\u8bc6\u522b\u5931\u8d25: {msg}\n"
            f"\u53ef\u901a\u8fc7\u6258\u76d8\u83dc\u5355\u201c\u5bfc\u51fa\u65e5\u5fd7\u201d\u67e5\u770b\u8be6\u60c5"
        )
        logger.error("Recognition failed: %s", msg)

    def _copy_result(self):
        text = self._result_window.get_result()
        if text.strip():
            self._clipboard_monitor.set_ignore_next()
            self._app.clipboard().setText(text)
            self._result_window.set_status("\u5df2\u590d\u5236\u5230\u526a\u8d34\u677f")
