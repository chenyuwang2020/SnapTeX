"""Clipboard monitoring for image detection."""

from PIL import Image
from PySide6.QtCore import QObject, Signal
from PySide6.QtGui import QClipboard, QImage


class ClipboardMonitor(QObject):
    """Monitors the system clipboard for new images."""

    image_detected = Signal(object)

    def __init__(self, clipboard: QClipboard):
        super().__init__()
        self._clipboard = clipboard
        self._enabled = False
        self._ignore_next = False
        self._clipboard.dataChanged.connect(self._on_clipboard_changed)

    def enable(self):
        self._enabled = True

    def disable(self):
        self._enabled = False

    def set_ignore_next(self):
        """Call before writing to clipboard to prevent re-triggering."""
        self._ignore_next = True

    def grab_image_now(self):
        """Manually grab the current clipboard image."""
        pil_image = self._extract_image()
        if pil_image is not None:
            self.image_detected.emit(pil_image)
            return True
        return False

    def _on_clipboard_changed(self):
        if self._ignore_next:
            self._ignore_next = False
            return
        if not self._enabled:
            return

        pil_image = self._extract_image()
        if pil_image is not None:
            self.image_detected.emit(pil_image)

    def _extract_image(self):
        mime_data = self._clipboard.mimeData()
        if mime_data is None or not mime_data.hasImage():
            return None
        qimage = self._clipboard.image()
        if qimage.isNull():
            return None
        return self._qimage_to_pil(qimage)

    @staticmethod
    def _qimage_to_pil(qimage: QImage) -> Image.Image:
        qimage = qimage.convertToFormat(QImage.Format.Format_RGBA8888)
        width = qimage.width()
        height = qimage.height()
        ptr = qimage.bits()
        raw_bytes = bytes(ptr)
        return Image.frombytes("RGBA", (width, height), raw_bytes).convert("RGB")
