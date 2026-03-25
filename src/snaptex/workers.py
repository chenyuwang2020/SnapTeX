"""Background workers for model loading and OCR recognition."""

import logging

import numpy as np
from PIL import Image
from PySide6.QtCore import QThread, Signal, QMutex, QWaitCondition

logger = logging.getLogger(__name__)


class ModelLoader(QThread):
    """Loads LatexOCR (formula recognition) in a background thread."""

    progress = Signal(str)
    finished_ok = Signal(object)
    error = Signal(str)

    def __init__(self, config):
        super().__init__()
        self.config = config

    def run(self):
        try:
            self.progress.emit("\u6b63\u5728\u52a0\u8f7d\u516c\u5f0f\u8bc6\u522b\u6a21\u578b...")
            from pix2text.latex_ocr import LatexOCR

            latex_ocr = LatexOCR(device=self.config.device)
            self.finished_ok.emit(latex_ocr)
        except Exception as e:
            logger.error("Model loading failed", exc_info=True)
            self.error.emit(str(e))


def _recognize_text_rapid_ocr(pil_image) -> str:
    """Use RapidOCR (PaddleOCR ONNX) for high-quality text recognition."""
    from rapidocr_onnxruntime import RapidOCR

    ocr = RapidOCR()
    img = pil_image.convert("RGB")
    result, _ = ocr(np.array(img))
    if not result:
        return ""
    lines = [line[1] for line in result]
    return "\n".join(lines)


class RecognitionWorker(QThread):
    """Runs OCR recognition in a background thread."""

    result_ready = Signal(str)
    error_occurred = Signal(str)
    processing_started = Signal()

    def __init__(self):
        super().__init__()
        self._latex_ocr = None
        self._image = None
        self._mode = "formula"
        self._mutex = QMutex()
        self._condition = QWaitCondition()
        self._running = True

    def set_model(self, latex_ocr):
        self._latex_ocr = latex_ocr

    def recognize(self, pil_image, mode="formula"):
        self._mutex.lock()
        self._image = pil_image
        self._mode = mode
        self._condition.wakeOne()
        self._mutex.unlock()

    def stop(self):
        self._running = False
        self._mutex.lock()
        self._condition.wakeOne()
        self._mutex.unlock()

    def run(self):
        while self._running:
            self._mutex.lock()
            if self._image is None:
                self._condition.wait(self._mutex)
            image = self._image
            mode = self._mode
            self._image = None
            self._mutex.unlock()

            if image is None:
                continue

            self.processing_started.emit()
            try:
                if mode == "text":
                    result = _recognize_text_rapid_ocr(image)
                else:
                    if self._latex_ocr is None:
                        self.error_occurred.emit(
                            "\u516c\u5f0f\u8bc6\u522b\u6a21\u578b\u5c1a\u672a\u52a0\u8f7d\u5b8c\u6210"
                        )
                        continue
                    out = self._latex_ocr.recognize(image, return_text=True)
                    result = out if isinstance(out, str) else out.get("text", str(out))
                self.result_ready.emit(str(result))
            except Exception as e:
                self.error_occurred.emit(str(e))
