"""UI components: ResultWindow, TrayManager, SettingsDialog."""

import logging
import os

from PIL import Image as PILImage
from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QLabel, QPlainTextEdit,
    QPushButton, QMenu, QDialog, QFormLayout,
    QCheckBox, QLineEdit, QMessageBox, QApplication,
    QProgressBar,
)
from PySide6.QtWebEngineWidgets import QWebEngineView
from PySide6.QtWebEngineCore import QWebEngineSettings
from PySide6.QtGui import QFont, QAction, QPixmap, QImage, QMouseEvent, QIcon
from PySide6.QtCore import Qt, Signal, QUrl, QTimer

try:
    from PySide6.QtWidgets import QSystemTrayIcon
except ImportError:
    from PySide6.QtGui import QSystemTrayIcon

logger = logging.getLogger(__name__)

_RESOURCES_DIR = os.path.join(os.path.dirname(__file__), "resources")
_KATEX_DIR = os.path.join(_RESOURCES_DIR, "katex")
_ICON_PATH = os.path.join(_RESOURCES_DIR, "icon.ico")
_ICON_PNG_PATH = os.path.join(_RESOURCES_DIR, "icon.png")

_FONT = '"Segoe UI", "Microsoft YaHei", "PingFang SC", sans-serif'
_MONO = '"Cascadia Code", "Consolas", "Microsoft YaHei", monospace'

_WINDOW_QSS = f"""
QWidget#MainContainer {{
    background: #FFFFFF;
    border: 1px solid #E2E2E2;
    border-radius: 12px;
}}
QLabel {{
    font-family: {_FONT};
    color: #333;
}}
QLabel#titleLabel {{
    font-size: 14px;
    font-weight: 600;
    color: #222;
}}
QLabel#statusLabel {{
    font-size: 11px;
    color: #999;
}}
QPlainTextEdit {{
    font-family: {_MONO};
    font-size: 13px;
    background: #F8F9FA;
    border: 1px solid #E8E8E8;
    border-radius: 8px;
    padding: 8px 10px;
    color: #333;
    selection-background-color: #4A90D9;
    selection-color: #FFF;
}}
QPushButton#modeBtn {{
    font-family: {_FONT};
    font-size: 11px;
    padding: 3px 16px;
    border: 1.5px solid #E0E0E0;
    border-radius: 14px;
    background: #FFF;
    color: #888;
}}
QPushButton#modeBtn:checked {{
    background: #4A90D9;
    border-color: #4A90D9;
    color: #FFF;
    font-weight: 600;
}}
QPushButton#modeBtn:hover:!checked {{
    border-color: #B0B0B0;
    color: #555;
}}
QPushButton#copyBtn {{
    font-family: {_FONT};
    font-size: 12px;
    font-weight: 600;
    padding: 7px 28px;
    border: none;
    border-radius: 8px;
    background: #4A90D9;
    color: #FFF;
}}
QPushButton#copyBtn:hover {{
    background: #3D7FCA;
}}
QPushButton#copyBtn:pressed {{
    background: #2E6AB0;
}}
QPushButton#closeBtn {{
    font-size: 14px;
    border: none;
    background: transparent;
    color: #CCC;
    padding: 2px;
}}
QPushButton#closeBtn:hover {{
    color: #E74C3C;
}}
QProgressBar {{
    border: none;
    border-radius: 3px;
    background: #EBEBEB;
    max-height: 4px;
}}
QProgressBar::chunk {{
    background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
        stop:0 #4A90D9, stop:1 #6FB3F2);
    border-radius: 3px;
}}
"""


def _build_katex_html() -> str:
    css_path = os.path.join(_KATEX_DIR, "katex.min.css").replace("\\", "/")
    js_path = os.path.join(_KATEX_DIR, "katex.min.js").replace("\\", "/")
    return f"""<!DOCTYPE html>
<html><head><meta charset="utf-8">
<link rel="stylesheet" href="file:///{css_path}">
<script src="file:///{js_path}"></script>
<style>
  body {{
    margin: 0; padding: 16px;
    display: flex; align-items: center; justify-content: center;
    min-height: calc(100vh - 32px);
    background: #FFFFFF; overflow: auto;
  }}
  #container {{ font-size: 1.4em; text-align: center; color: #333; }}
  .placeholder {{ color: #D0D0D0; font-style: italic; font-family: {_FONT}; font-size: 0.75em; }}
</style></head><body>
<div id="container"><span class="placeholder">\u516c\u5f0f\u9884\u89c8</span></div>
<script>
function renderLatex(tex) {{
  var el = document.getElementById('container');
  if (!tex || !tex.trim()) {{
    el.innerHTML = '<span class="placeholder">\u516c\u5f0f\u9884\u89c8</span>';
    return;
  }}
  try {{
    katex.render(tex, el, {{ displayMode:true, throwOnError:false, trust:true, strict:false }});
  }} catch(e) {{
    el.innerHTML = '<span style="color:#c00;font-size:0.75em">' + e.message + '</span>';
  }}
}}
</script></body></html>"""


def _pil_to_qpixmap(pil_image: PILImage.Image) -> QPixmap:
    img = pil_image.convert("RGBA")
    data = img.tobytes("raw", "RGBA")
    qimg = QImage(data, img.width, img.height, QImage.Format.Format_RGBA8888)
    return QPixmap.fromImage(qimg)


class ResultWindow(QWidget):
    copy_requested = Signal()
    mode_changed = Signal(str)
    quit_requested = Signal()

    def __init__(self, stay_on_top: bool = True):
        super().__init__()
        flags = Qt.WindowType.FramelessWindowHint | Qt.WindowType.Window
        if stay_on_top:
            flags |= Qt.WindowType.WindowStaysOnTopHint
        self.setWindowFlags(flags)
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.resize(460, 400)
        self._drag_pos = None
        self._current_mode = "formula"
        self._copy_timer = QTimer(self)
        self._copy_timer.setSingleShot(True)
        self._copy_timer.timeout.connect(self._restore_copy_btn)

        # Container
        container = QWidget(self)
        container.setObjectName("MainContainer")
        container.setStyleSheet(_WINDOW_QSS)
        outer = QVBoxLayout(self)
        outer.setContentsMargins(2, 2, 2, 2)  # shadow space
        outer.addWidget(container)

        layout = QVBoxLayout(container)
        layout.setContentsMargins(16, 12, 16, 14)
        layout.setSpacing(10)

        # ── Title bar ──
        title_bar = QHBoxLayout()
        title_bar.setSpacing(8)

        logo_label = QLabel()
        logo_pixmap = QPixmap(_ICON_PNG_PATH)
        if not logo_pixmap.isNull():
            logo_label.setPixmap(logo_pixmap.scaled(
                48, 48,
                Qt.AspectRatioMode.KeepAspectRatio,
                Qt.TransformationMode.SmoothTransformation,
            ))
        logo_label.setFixedSize(28, 28)
        logo_label.setScaledContents(True)
        title_bar.addWidget(logo_label)

        title = QLabel("SnapTeX")
        title.setObjectName("titleLabel")
        title_bar.addWidget(title)
        title_bar.addStretch()

        self._formula_btn = QPushButton("\u516c\u5f0f")
        self._formula_btn.setObjectName("modeBtn")
        self._formula_btn.setCheckable(True)
        self._formula_btn.setChecked(True)
        self._formula_btn.clicked.connect(lambda: self._set_mode("formula"))
        title_bar.addWidget(self._formula_btn)

        self._text_btn = QPushButton("\u6587\u5b57")
        self._text_btn.setObjectName("modeBtn")
        self._text_btn.setCheckable(True)
        self._text_btn.clicked.connect(lambda: self._set_mode("text"))
        title_bar.addWidget(self._text_btn)

        title_bar.addSpacing(6)

        minimize_btn = QPushButton("\u2500")
        minimize_btn.setObjectName("closeBtn")
        minimize_btn.setFixedSize(22, 22)
        minimize_btn.clicked.connect(self.hide)
        title_bar.addWidget(minimize_btn)

        close_btn = QPushButton("\u2715")
        close_btn.setObjectName("closeBtn")
        close_btn.setFixedSize(22, 22)
        close_btn.clicked.connect(self.close)
        title_bar.addWidget(close_btn)

        layout.addLayout(title_bar)

        # ── Status + progress (compact) ──
        self._status_label = QLabel("\u6b63\u5728\u52a0\u8f7d\u6a21\u578b...")
        self._status_label.setObjectName("statusLabel")
        layout.addWidget(self._status_label)

        self._progress = QProgressBar()
        self._progress.setRange(0, 0)
        self._progress.setFixedHeight(4)
        layout.addWidget(self._progress)

        # ── Thumbnail (only shown when image is set) ──
        self._thumbnail = QLabel()
        self._thumbnail.setFixedHeight(48)
        self._thumbnail.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._thumbnail.setStyleSheet(
            "background: #F5F5F5; border-radius: 6px; border: 1px solid #EBEBEB;"
        )
        self._thumbnail.hide()
        layout.addWidget(self._thumbnail)

        # ── KaTeX preview (formula mode only) ──
        self._preview_container = QWidget()
        self._preview_container.setFixedHeight(120)
        preview_layout = QVBoxLayout(self._preview_container)
        preview_layout.setContentsMargins(0, 0, 0, 0)

        self._preview = QWebEngineView()
        self._preview.page().settings().setAttribute(
            QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, True
        )
        self._preview.setHtml(_build_katex_html(), QUrl.fromLocalFile(_KATEX_DIR + "/"))
        self._preview.setStyleSheet(
            "border: 1px solid #EBEBEB; border-radius: 8px; background: #FFF;"
        )
        preview_layout.addWidget(self._preview)
        layout.addWidget(self._preview_container)

        # ── Result text (editable) ──
        self._result_text = QPlainTextEdit()
        self._result_text.setPlaceholderText("\u8bc6\u522b\u7ed3\u679c...")
        self._result_text.setMaximumHeight(100)
        layout.addWidget(self._result_text)

        # ── Copy button ──
        bottom = QHBoxLayout()
        bottom.addStretch()
        self._copy_btn = QPushButton("\u590d\u5236 LaTeX")
        self._copy_btn.setObjectName("copyBtn")
        self._copy_btn.clicked.connect(self._on_copy_clicked)
        bottom.addWidget(self._copy_btn)
        bottom.addStretch()
        layout.addLayout(bottom)

    # ── Mode ──
    def _set_mode(self, mode: str):
        self._current_mode = mode
        self._formula_btn.setChecked(mode == "formula")
        self._text_btn.setChecked(mode == "text")
        self._preview_container.setVisible(mode == "formula")
        self._copy_btn.setText(
            "\u590d\u5236 LaTeX" if mode == "formula" else "\u590d\u5236\u6587\u672c"
        )
        self.mode_changed.emit(mode)

    def get_mode(self) -> str:
        return self._current_mode

    # ── Copy with feedback ──
    def _on_copy_clicked(self):
        self.copy_requested.emit()
        self.flash_copied()

    def flash_copied(self):
        """Show green '✓ 已复制' feedback on the copy button."""
        self._copy_btn.setText("\u2713 \u5df2\u590d\u5236")
        self._copy_btn.setStyleSheet(
            "QPushButton#copyBtn { background: #27AE60; }"
        )
        self._copy_timer.start(1200)

    def _restore_copy_btn(self):
        label = "\u590d\u5236 LaTeX" if self._current_mode == "formula" else "\u590d\u5236\u6587\u672c"
        self._copy_btn.setText(label)
        self._copy_btn.setStyleSheet("")

    # ── Public API ──
    def set_status(self, text: str):
        self._status_label.setText(text)

    def set_loading(self, loading: bool):
        self._progress.setVisible(loading)

    def set_result(self, text: str):
        self._result_text.setPlainText(text)
        if self._current_mode == "formula":
            self._render_preview(text)

    def set_image(self, pil_image: PILImage.Image):
        pixmap = _pil_to_qpixmap(pil_image)
        scaled = pixmap.scaled(
            self._thumbnail.width(), self._thumbnail.height(),
            Qt.AspectRatioMode.KeepAspectRatio,
            Qt.TransformationMode.SmoothTransformation,
        )
        self._thumbnail.setPixmap(scaled)
        self._thumbnail.show()

    def get_result(self) -> str:
        return self._result_text.toPlainText()

    def _render_preview(self, latex_str: str):
        escaped = latex_str.replace("\\", "\\\\").replace("'", "\\'").replace("\n", " ")
        self._preview.page().runJavaScript(f"renderLatex('{escaped}')")

    # ── Drag ──
    def mousePressEvent(self, event: QMouseEvent):
        if event.button() == Qt.MouseButton.LeftButton:
            self._drag_pos = event.globalPosition().toPoint() - self.frameGeometry().topLeft()

    def mouseMoveEvent(self, event: QMouseEvent):
        if self._drag_pos and event.buttons() & Qt.MouseButton.LeftButton:
            self.move(event.globalPosition().toPoint() - self._drag_pos)

    def mouseReleaseEvent(self, event: QMouseEvent):
        self._drag_pos = None

    def closeEvent(self, event):
        event.accept()
        self.quit_requested.emit()


class TrayManager:
    def __init__(self, app: QApplication, controller):
        self._app = app
        self._controller = controller
        self.tray = QSystemTrayIcon(app)
        self.tray.setIcon(QIcon(_ICON_PATH))
        self.tray.setToolTip("SnapTeX - \u6b63\u5728\u52a0\u8f7d...")
        self._build_menu()
        self.tray.activated.connect(self._on_activated)
        self.tray.show()

    def _build_menu(self):
        menu = QMenu()

        self.action_show = QAction("\u663e\u793a\u7a97\u53e3")
        self.action_show.triggered.connect(self._controller.show_window)
        menu.addAction(self.action_show)

        self.action_recognize = QAction("\u4ece\u526a\u8d34\u677f\u8bc6\u522b")
        self.action_recognize.triggered.connect(self._controller.manual_recognize)
        self.action_recognize.setEnabled(False)
        menu.addAction(self.action_recognize)

        menu.addSeparator()

        self.action_monitor = QAction("\u81ea\u52a8\u76d1\u542c\u526a\u8d34\u677f")
        self.action_monitor.setCheckable(True)
        self.action_monitor.setChecked(True)
        self.action_monitor.triggered.connect(self._controller.toggle_monitor)
        menu.addAction(self.action_monitor)

        menu.addSeparator()

        self.action_settings = QAction("\u8bbe\u7f6e...")
        self.action_settings.triggered.connect(self._controller.show_settings)
        menu.addAction(self.action_settings)

        self.action_open_data = QAction("\u6253\u5f00\u6570\u636e\u76ee\u5f55")
        self.action_open_data.triggered.connect(self._controller.open_data_dir)
        menu.addAction(self.action_open_data)

        self.action_export_log = QAction("\u5bfc\u51fa\u65e5\u5fd7\u5230\u684c\u9762")
        self.action_export_log.triggered.connect(self._controller.export_log)
        menu.addAction(self.action_export_log)

        menu.addSeparator()

        self.action_about = QAction("\u5173\u4e8e SnapTeX")
        self.action_about.triggered.connect(self._controller.show_about)
        menu.addAction(self.action_about)

        self.action_uninstall = QAction("\u6e05\u9664\u6240\u6709\u6570\u636e\u5e76\u9000\u51fa")
        self.action_uninstall.triggered.connect(self._controller.uninstall_data)
        menu.addAction(self.action_uninstall)

        menu.addSeparator()

        self.action_quit = QAction("\u9000\u51fa")
        self.action_quit.triggered.connect(self._controller.quit_app)
        menu.addAction(self.action_quit)

        self.tray.setContextMenu(menu)

    def _on_activated(self, reason):
        if reason == QSystemTrayIcon.ActivationReason.DoubleClick:
            self._controller.show_window()

    def set_ready(self):
        from snaptex.config import APP_VERSION
        self.tray.setToolTip(f"SnapTeX v{APP_VERSION} - \u5c31\u7eea")
        self.action_recognize.setEnabled(True)

    def show_message(self, title: str, message: str):
        self.tray.showMessage(
            title, message, QSystemTrayIcon.MessageIcon.Information, 3000
        )


class SettingsDialog(QDialog):
    def __init__(self, config, parent=None):
        super().__init__(parent)
        from snaptex.config import app_data_dir, get_data_size_mb

        self.config = config
        self.setWindowTitle("SnapTeX \u8bbe\u7f6e")
        self.setMinimumWidth(400)

        layout = QFormLayout(self)

        self.device_edit = QLineEdit(config.device)
        self.device_edit.setPlaceholderText("cpu / cuda / cuda:0")
        layout.addRow("\u8ba1\u7b97\u8bbe\u5907:", self.device_edit)

        self.auto_monitor_cb = QCheckBox()
        self.auto_monitor_cb.setChecked(config.auto_monitor_clipboard)
        layout.addRow("\u81ea\u52a8\u76d1\u542c\u526a\u8d34\u677f:", self.auto_monitor_cb)

        self.auto_copy_cb = QCheckBox()
        self.auto_copy_cb.setChecked(config.auto_copy_result)
        layout.addRow("\u81ea\u52a8\u590d\u5236\u7ed3\u679c:", self.auto_copy_cb)

        layout.addRow(QLabel(""))
        layout.addRow(QLabel("<b>\u6570\u636e\u5b58\u50a8</b>"))

        dir_label = QLineEdit(str(app_data_dir()))
        dir_label.setReadOnly(True)
        dir_label.setStyleSheet("color: #555; background: #f5f5f5;")
        layout.addRow("\u6570\u636e\u76ee\u5f55:", dir_label)

        size_label = QLabel(f"{get_data_size_mb():.1f} MB")
        size_label.setStyleSheet("color: #555;")
        layout.addRow("\u5360\u7528\u7a7a\u95f4:", size_label)

        hint = QLabel(
            "\u5305\u542b\u6a21\u578b\u6587\u4ef6\u548c\u914d\u7f6e\u3002"
            "\u5378\u8f7d\u65f6\u8bf7\u901a\u8fc7\u6258\u76d8\u83dc\u5355"
            "\u201c\u6e05\u9664\u6240\u6709\u6570\u636e\u5e76\u9000\u51fa\u201d"
            "\u6765\u5f7b\u5e95\u5220\u9664\u3002"
        )
        hint.setStyleSheet("color: #888; font-size: 11px;")
        hint.setWordWrap(True)
        layout.addRow(hint)

        btn_layout = QHBoxLayout()
        save_btn = QPushButton("\u4fdd\u5b58")
        save_btn.clicked.connect(self._save)
        cancel_btn = QPushButton("\u53d6\u6d88")
        cancel_btn.clicked.connect(self.reject)
        btn_layout.addStretch()
        btn_layout.addWidget(save_btn)
        btn_layout.addWidget(cancel_btn)
        layout.addRow(btn_layout)

    def _save(self):
        self.config.device = self.device_edit.text().strip() or "cpu"
        self.config.auto_monitor_clipboard = self.auto_monitor_cb.isChecked()
        self.config.auto_copy_result = self.auto_copy_cb.isChecked()
        self.config.save()
        QMessageBox.information(
            self, "\u8bbe\u7f6e\u5df2\u4fdd\u5b58",
            "\u90e8\u5206\u8bbe\u7f6e\u9700\u8981\u91cd\u542f\u5e94\u7528\u540e\u751f\u6548\u3002"
        )
        self.accept()
