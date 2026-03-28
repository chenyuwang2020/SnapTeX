console.log("main.js loaded");
console.log("__TAURI__ available:", !!window.__TAURI__);
console.log("MathLive available:", !!window.MathfieldElement);

document.addEventListener("DOMContentLoaded", () => {
  void bootstrap();
});

async function bootstrap() {
  console.log("DOMContentLoaded fired");

  const tauri = window.__TAURI__;
  const captureButton = document.getElementById("capture-button");
  const batchButton = document.getElementById("batch-button");
  const copyButton = document.getElementById("copy-button");
  const copyEditorButton = document.getElementById("copy-editor-button");
  const keyboardButton = document.getElementById("keyboard-button");
  const formatSelect = document.getElementById("format-select");
  const exportText = document.getElementById("export-text");
  const exportNote = document.getElementById("export-note");
  const exportLabel = document.getElementById("export-label");
  const previewImage = document.getElementById("preview-image");
  const placeholder = document.getElementById("placeholder");
  const statusBar = document.getElementById("status");
  const statusSpinner = document.getElementById("status-spinner");
  const metaBar = document.getElementById("meta");
  const resultMeta = document.getElementById("result-meta");
  const katexPanel = document.getElementById("katex-panel");
  const renderEmpty = document.getElementById("render-empty");
  const mathEditor = document.getElementById("math-editor");
  const editorLatex = document.getElementById("editor-latex");
  const editorMeta = document.getElementById("editor-meta");
  const batchEditMeta = document.getElementById("batch-edit-meta");
  const copyToast = document.getElementById("copy-toast");
  const footerLeft = document.getElementById("footer-left");
  const footerRight = document.getElementById("footer-right");
  const candidatesPanel = document.getElementById("candidates-panel");
  const candidatesMeta = document.getElementById("candidates-meta");
  const candidatesList = document.getElementById("candidates-list");
  const confidenceBar = document.getElementById("confidence-bar");
  const confidenceTokens = document.getElementById("confidence-tokens");
  const batchStrip = document.getElementById("batch-strip");
  const batchCount = document.getElementById("batch-count");
  const batchList = document.getElementById("batch-list");
  const batchExportButton = document.getElementById("batch-export-button");

  let modelReady = false;
  let currentImageB64 = null;
  let currentExports = emptyExports();
  let currentCandidates = [];
  let currentSelectedCandidateIndex = 0;
  let copyToastTimer = null;
  let recognitionToken = 0;
  let exportToken = 0;
  let lastImageSource = "";
  let exportDebounce = null;
  let suppressEditorInput = false;
  let suppressLatexInput = false;
  let batchMode = false;
  let batchItems = [];
  let nextBatchId = 1;
  let editingBatchItemId = null;

  const formatLabels = {
    latex: "LaTeX",
    mathml: "MathML",
    typst: "Typst",
    markdown: "Markdown",
  };

  function emptyExports() {
    return {
      latex: "",
      markdown: "",
      mathml: null,
      typst: null,
    };
  }

  function setStatus(text, { loading = false } = {}) {
    statusBar.textContent = text;
    statusSpinner.style.display = loading ? "inline-block" : "none";
  }

  function setMeta(text) {
    metaBar.textContent = text;
  }

  function setResultMeta(text) {
    resultMeta.textContent = text;
    footerRight.textContent = text;
  }

  function showCopyToast(text) {
    copyToast.textContent = text;
    copyToast.classList.add("visible");
    window.clearTimeout(copyToastTimer);
    copyToastTimer = window.setTimeout(() => {
      copyToast.classList.remove("visible");
    }, 1800);
  }

  function selectedFormat() {
    return formatSelect.value || "latex";
  }

  function getMathfieldElementClass() {
    return window.MathfieldElement || null;
  }

  function configureMathLive() {
    const MathfieldElement = getMathfieldElementClass();
    if (!MathfieldElement) {
      console.error("MathfieldElement is unavailable");
      editorMeta.textContent = "MathLive 资源未加载";
      return;
    }
    MathfieldElement.fontsDirectory = "./assets/mathlive/fonts";
    MathfieldElement.soundsDirectory = "./assets/mathlive/sounds";
    mathEditor.mathVirtualKeyboardPolicy = "manual";
    mathEditor.setAttribute("math-virtual-keyboard-policy", "manual");
    editorMeta.textContent = "识别完成后会自动写入编辑器";

    if (window.mathVirtualKeyboard?.addEventListener) {
      window.mathVirtualKeyboard.addEventListener("virtual-keyboard-toggle", () => {
        keyboardButton.textContent = window.mathVirtualKeyboard.visible
          ? "隐藏数学键盘"
          : "显示数学键盘";
      });
    }
  }

  function getEditorLatex() {
    if (!mathEditor) {
      return "";
    }
    if (typeof mathEditor.getValue === "function") {
      return `${mathEditor.getValue() || ""}`.trim();
    }
    return `${mathEditor.value || ""}`.trim();
  }

  function setEditorLatex(latex) {
    suppressEditorInput = true;
    try {
      if (typeof mathEditor.setValue === "function") {
        mathEditor.setValue(latex || "", { silenceNotifications: true });
      } else {
        mathEditor.value = latex || "";
      }
    } finally {
      suppressEditorInput = false;
    }
    syncEditorLatexMirror();
  }

  function syncEditorLatexMirror() {
    suppressLatexInput = true;
    editorLatex.value = getEditorLatex();
    suppressLatexInput = false;
  }

  function renderKatex(latex) {
    if (!latex) {
      katexPanel.innerHTML = "";
      renderEmpty.style.display = "block";
      renderEmpty.textContent = "尚未开始识别";
      return;
    }

    if (!window.katex || typeof window.katex.renderToString !== "function") {
      renderEmpty.style.display = "block";
      renderEmpty.textContent = "KaTeX 资源未加载";
      katexPanel.innerHTML = "";
      return;
    }

    try {
      const html = window.katex.renderToString(latex, {
        displayMode: true,
        throwOnError: false,
      });
      katexPanel.innerHTML = html;
      renderEmpty.style.display = "none";
    } catch (error) {
      katexPanel.innerHTML = "";
      renderEmpty.style.display = "block";
      renderEmpty.textContent = `KaTeX 渲染失败: ${error}`;
    }
  }

  function renderFormulaHtml(latex) {
    if (!window.katex || !latex) {
      return `<div class="hint">${escapeHtml(latex || "暂无公式")}</div>`;
    }

    try {
      return window.katex.renderToString(latex, {
        displayMode: true,
        throwOnError: false,
      });
    } catch (error) {
      return `<div class="hint">${escapeHtml(latex)}</div>`;
    }
  }

  function escapeHtml(text) {
    return `${text ?? ""}`
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function getExportValue(format) {
    const key = format || selectedFormat();
    if (key === "latex") {
      return currentExports.latex || getEditorLatex();
    }
    if (key === "markdown") {
      return currentExports.markdown || "";
    }
    return currentExports[key] || "";
  }

  function updateExportView() {
    const format = selectedFormat();
    const value = getExportValue(format);
    exportLabel.textContent = `导出格式 · ${formatLabels[format] || format}`;
    footerLeft.textContent = `当前导出：${formatLabels[format] || format}`;

    if (value) {
      exportText.value = value;
      exportNote.textContent = `${formatLabels[format] || format} 导出已生成，自动复制和手动复制都会使用这个格式。`;
      return;
    }

    exportText.value = "";
    if (!getEditorLatex()) {
      exportNote.textContent = "识别完成后，这里会显示基于 MathLive 当前内容生成的导出结果。";
      return;
    }

    exportNote.textContent = `${formatLabels[format] || format} 当前不可用。`;
  }

  function findBatchItem(id) {
    return batchItems.find((item) => item.id === id) || null;
  }

  function syncCurrentEditorToBatchItem() {
    if (editingBatchItemId == null) {
      return;
    }
    const item = findBatchItem(editingBatchItemId);
    if (!item) {
      return;
    }
    item.latex = getEditorLatex();
    item.exports = { ...currentExports };
  }

  async function refreshExports(reason) {
    const latex = getEditorLatex();
    syncEditorLatexMirror();
    if (!latex) {
      currentExports = emptyExports();
      updateExportView();
      syncCurrentEditorToBatchItem();
      return;
    }

    const token = ++exportToken;
    console.log("export_formats start", { reason, latex });
    try {
      const bundle = await tauri.core.invoke("export_formats", { latex });
      if (token !== exportToken) {
        return;
      }
      currentExports = bundle;
      updateExportView();
      syncCurrentEditorToBatchItem();
      if (batchMode) {
        renderBatchItems();
      }
    } catch (error) {
      if (token !== exportToken) {
        return;
      }
      console.error("export_formats failed", error);
      currentExports = emptyExports();
      updateExportView();
      exportNote.textContent = `导出失败: ${error ?? "未知错误"}`;
    }
  }

  async function copyText(text, successText) {
    if (!text) {
      showCopyToast("没有可复制的内容");
      return false;
    }

    try {
      await navigator.clipboard.writeText(text);
      showCopyToast(successText);
      return true;
    } catch (error) {
      console.error("navigator.clipboard.writeText failed", error);
      showCopyToast("复制失败");
      return false;
    }
  }

  async function copyCurrentFormat() {
    const format = selectedFormat();
    const content = getExportValue(format);
    const formatName = formatLabels[format] || format;
    const copied = await copyText(content, `已复制 ${formatName}`);
    if (!copied) {
      return;
    }
    setStatus(`已复制 ${formatName}`);
  }

  async function copyEditorLatex() {
    const latex = getEditorLatex();
    const copied = await copyText(latex, "已复制 LaTeX");
    if (!copied) {
      return;
    }
    setStatus("已复制编辑器 LaTeX");
  }

  function buildCandidatesFromResult(result) {
    return [
      {
        latex: result.latex,
        score: result.score,
        warnings: result.warnings || [],
        token_probs: result.token_probs || [],
        tokens: result.tokens || [],
      },
      ...(result.alternatives || []),
    ];
  }

  function hideCandidateViews() {
    candidatesPanel.classList.add("hidden");
    confidenceBar.classList.add("hidden");
  }

  function tokenColor(probability) {
    if (probability > 0.95) {
      return "rgba(14, 159, 110, 0.18)";
    }
    if (probability >= 0.7) {
      return "rgba(245, 158, 11, 0.24)";
    }
    return "rgba(239, 68, 68, 0.3)";
  }

  function renderConfidence(candidate) {
    if (!candidate || !candidate.token_probs?.length || !candidate.tokens?.length) {
      confidenceBar.classList.add("hidden");
      confidenceTokens.innerHTML = "";
      return;
    }

    const allHigh = candidate.token_probs.every((probability) => probability > 0.95);
    if (allHigh) {
      confidenceBar.classList.add("hidden");
      confidenceTokens.innerHTML = "";
      return;
    }

    confidenceBar.classList.remove("hidden");
    confidenceTokens.innerHTML = "";
    candidate.tokens.forEach((token, index) => {
      const probability = candidate.token_probs[index] ?? 0;
      const span = document.createElement("span");
      span.className = "confidence-token";
      span.textContent = token;
      span.style.backgroundColor = tokenColor(probability);
      span.title = `token: ${token}, prob: ${(probability * 100).toFixed(1)}%`;
      confidenceTokens.appendChild(span);
    });
  }

  function selectCandidate(index, { fromManualEdit = false } = {}) {
    const candidate = currentCandidates[index];
    if (!candidate) {
      return;
    }

    currentSelectedCandidateIndex = index;
    setEditorLatex(candidate.latex);
    renderKatex(candidate.latex);
    void refreshExports(fromManualEdit ? "manual-candidate" : "candidate-select");

    if (!fromManualEdit) {
      renderCandidates();
      renderConfidence(candidate);
    }
  }

  function renderCandidates() {
    if (currentCandidates.length <= 1) {
      candidatesPanel.classList.add("hidden");
      candidatesList.innerHTML = "";
      return;
    }

    candidatesPanel.classList.remove("hidden");
    candidatesMeta.textContent = `共 ${currentCandidates.length} 个候选，点击可切换`;
    candidatesList.innerHTML = currentCandidates
      .map((candidate, index) => {
        const warnings = (candidate.warnings || []).join(" · ");
        return `
          <button type="button" class="candidate-card ${index === currentSelectedCandidateIndex ? "selected" : ""}" data-candidate-index="${index}">
            <div class="candidate-top">
              <span class="candidate-index">候选 #${index + 1}</span>
              <span class="candidate-score">${(Number(candidate.score || 0) * 100).toFixed(1)}%</span>
            </div>
            <div class="candidate-render">${renderFormulaHtml(candidate.latex)}</div>
            <div class="hint">${escapeHtml(candidate.latex)}</div>
            ${warnings ? `<div class="candidate-warning">${escapeHtml(warnings)}</div>` : ""}
          </button>
        `;
      })
      .join("");

    candidatesList.querySelectorAll("[data-candidate-index]").forEach((element) => {
      element.addEventListener("click", () => {
        const index = Number(element.getAttribute("data-candidate-index"));
        selectCandidate(index);
      });
    });
  }

  async function runRecognition(imageB64, numBeams, reason) {
    console.log("recognize_formula start", { reason, numBeams });
    return tauri.core.invoke("recognize_formula", {
      imageB64,
      numBeams,
    });
  }

  async function applySingleRecognitionResult(result, reason) {
    currentCandidates = buildCandidatesFromResult(result);
    currentSelectedCandidateIndex = 0;

    setEditorLatex(result.latex);
    renderKatex(result.latex);
    renderCandidates();
    renderConfidence(currentCandidates[0]);
    await refreshExports("recognition");

    const format = selectedFormat();
    const content = getExportValue(format);
    const formatName = formatLabels[format] || format;
    const copied = await copyText(content, `已复制 ${formatName}`);
    const scoreText = Number.isFinite(result.score)
      ? `score ${(result.score * 100).toFixed(1)}%`
      : "score -";
    setResultMeta(`${result.model_id} · ${result.provider} · ${scoreText}`);
    setStatus(`识别完成 · ${result.latency_ms}ms · ${copied ? "已复制到剪贴板" : "复制失败"}`);
    editorMeta.textContent = "识别完成，可在候选和编辑器之间切换审查";
    batchEditMeta.textContent = editingBatchItemId != null ? "当前正在编辑题集中的一道题" : "";
  }

  function showImage(payload, sourceLabel) {
    if (!payload || !payload.image_b64) {
      return;
    }
    currentImageB64 = payload.image_b64;
    lastImageSource = sourceLabel;
    previewImage.src = `data:image/png;base64,${payload.image_b64}`;
    previewImage.style.display = "block";
    placeholder.style.display = "none";
    setMeta(`${payload.width} x ${payload.height} · ${sourceLabel}`);
  }

  function updateBatchModeUi() {
    batchButton.textContent = batchMode ? "退出题集" : "题集模式";
    batchButton.classList.toggle("active", batchMode);
    batchButton.classList.toggle("secondary", !batchMode);
    batchStrip.classList.toggle("hidden", !batchMode);
  }

  function renderBatchItems() {
    batchCount.textContent = `${batchItems.length}`;
    batchList.innerHTML = batchItems
      .map((item, index) => {
        return `
          <article class="batch-item ${item.id === editingBatchItemId ? "active" : ""}" data-batch-open="${item.id}">
            <div class="batch-item-top">
              <span class="batch-badge">#${index + 1}</span>
              <button type="button" class="danger" data-batch-delete="${item.id}">删除</button>
            </div>
            <img class="batch-thumb" alt="题目缩略图" src="data:image/png;base64,${item.imageB64}" />
            <div class="batch-render">${renderFormulaHtml(item.latex)}</div>
            <div class="batch-latex" title="点击加载到上方编辑区">${escapeHtml(item.latex)}</div>
          </article>
        `;
      })
      .join("");

    batchList.querySelectorAll("[data-batch-open]").forEach((element) => {
      element.addEventListener("click", () => {
        openBatchEditor(Number(element.getAttribute("data-batch-open")));
      });
    });

    batchList.querySelectorAll("[data-batch-delete]").forEach((element) => {
      element.addEventListener("click", (event) => {
        event.stopPropagation();
        batchDeleteItem(Number(element.getAttribute("data-batch-delete")));
      });
    });
  }

  function enterBatchMode() {
    batchMode = true;
    batchItems = [];
    nextBatchId = 1;
    editingBatchItemId = null;
    renderBatchItems();
    updateBatchModeUi();
    batchEditMeta.textContent = "";
    setStatus("题集模式已开启");
    setResultMeta("题集模式 · 新截图会追加到底部列表");
  }

  function exitBatchMode() {
    batchMode = false;
    batchItems = [];
    editingBatchItemId = null;
    renderBatchItems();
    updateBatchModeUi();
    batchEditMeta.textContent = "";
    setStatus("就绪 - 截图或按 Alt+Q 开始识别");
  }

  function openBatchEditor(id) {
    const item = findBatchItem(id);
    if (!item) {
      return;
    }
    editingBatchItemId = id;
    renderBatchItems();
    showImage(
      {
        image_b64: item.imageB64,
        width: item.width,
        height: item.height,
      },
      "题集编辑"
    );
    currentExports = item.exports || emptyExports();
    currentCandidates = [];
    hideCandidateViews();
    setEditorLatex(item.latex);
    renderKatex(item.latex);
    updateExportView();
    batchEditMeta.textContent = `正在编辑题集中的第 ${batchItems.findIndex((candidate) => candidate.id === id) + 1} 题`;
    setStatus("题集单题编辑中");
  }

  function batchDeleteItem(id) {
    const removedIndex = batchItems.findIndex((item) => item.id === id);
    batchItems = batchItems.filter((item) => item.id !== id);

    if (editingBatchItemId === id) {
      const nextItem = batchItems[Math.min(removedIndex, batchItems.length - 1)] || null;
      editingBatchItemId = null;
      if (nextItem) {
        openBatchEditor(nextItem.id);
        return;
      }
      batchEditMeta.textContent = "";
    }

    renderBatchItems();
  }

  function appendCurrentResultToBatch(payload, result) {
    const item = {
      id: nextBatchId++,
      imageB64: payload.image_b64,
      width: payload.width,
      height: payload.height,
      latex: getEditorLatex(),
      originalLatex: result.latex,
      exports: { ...currentExports },
    };
    batchItems.push(item);
    editingBatchItemId = item.id;
    renderBatchItems();
    batchEditMeta.textContent = `当前正在编辑题集中的第 ${batchItems.length} 题`;
  }

  function buildBatchExportText(format) {
    return batchItems
      .map((item, index) => {
        const value =
          format === "latex"
            ? item.latex
            : format === "markdown"
              ? item.exports?.markdown || ""
              : item.exports?.[format] || "";
        const header = format === "latex" ? `% #${index + 1}` : `#${index + 1}`;
        return `${header}\n${value}`;
      })
      .join("\n\n");
  }

  async function batchExportAll() {
    if (!batchItems.length) {
      showCopyToast("题集中还没有内容");
      return;
    }

    const input = window.prompt("导出格式（latex / mathml / typst / markdown）", "latex");
    if (!input) {
      return;
    }
    const format = input.trim().toLowerCase();
    if (!["latex", "mathml", "typst", "markdown"].includes(format)) {
      showCopyToast("导出格式无效");
      return;
    }

    const text = buildBatchExportText(format);
    const copied = await copyText(text, `已复制 ${batchItems.length} 道题`);
    if (!copied) {
      return;
    }
    setStatus(`已复制 ${batchItems.length} 道题的 ${formatLabels[format] || format}`);
  }

  async function handleIncomingImage(payload, sourceLabel) {
    showImage(payload, sourceLabel);
    if (!modelReady) {
      setStatus("模型尚未就绪");
      setResultMeta(`${sourceLabel} · 等待模型加载`);
      return;
    }

    if (batchMode) {
      syncCurrentEditorToBatchItem();
      editingBatchItemId = null;
      renderBatchItems();
      const token = ++recognitionToken;
      setStatus("题集模式 · 正在识别...", { loading: true });
      setResultMeta(`${sourceLabel} · 正在推理`);
      try {
        const result = await runRecognition(payload.image_b64, 1, sourceLabel);
        if (token !== recognitionToken) {
          return;
        }
        await applySingleRecognitionResult(result, sourceLabel);
        appendCurrentResultToBatch(payload, result);
      } catch (error) {
        if (token !== recognitionToken) {
          return;
        }
        const message = `${error ?? ""}`;
        console.error("recognize_formula failed", error);
        setStatus(`识别失败: ${message || "未知错误"}`);
        setResultMeta(`${sourceLabel} · 识别失败`);
      } finally {
        if (token === recognitionToken && modelReady) {
          statusSpinner.style.display = "none";
        }
      }
      return;
    }

    await recognizeCurrentImage(sourceLabel);
  }

  async function captureRegion(sourceLabel = "截图") {
    setStatus(batchMode ? "题集模式 · 截图中" : "截图中");
    setResultMeta(`${sourceLabel} · 等待选区`);
    try {
      const payload = await tauri.core.invoke("capture_region");
      await handleIncomingImage(payload, sourceLabel);
    } catch (error) {
      const message = `${error ?? ""}`;
      if (message.includes("cancelled")) {
        setStatus("已取消");
        setResultMeta("截图已取消");
        return;
      }
      setStatus(`截图失败: ${message || "未知错误"}`);
      setResultMeta("截图失败");
    }
  }

  async function recognizeCurrentImage(reason) {
    if (!currentImageB64) {
      return;
    }

    if (!modelReady) {
      setStatus("模型尚未就绪");
      setResultMeta("模型尚未就绪");
      return;
    }

    const token = ++recognitionToken;
    setStatus("正在识别...", { loading: true });
    setResultMeta(`${reason} · 正在推理`);

    try {
      const result = await runRecognition(currentImageB64, 3, reason);
      if (token !== recognitionToken) {
        return;
      }
      await applySingleRecognitionResult(result, reason);
    } catch (error) {
      if (token !== recognitionToken) {
        return;
      }
      const message = `${error ?? ""}`;
      console.error("recognize_formula failed", error);
      setStatus(`识别失败: ${message || "未知错误"}`);
      setResultMeta(`${reason} · 识别失败`);
    } finally {
      if (token === recognitionToken && modelReady) {
        statusSpinner.style.display = "none";
      }
    }
  }

  function applyModelStatus(status) {
    if (!status) {
      return;
    }
    if (status.status === "ready") {
      modelReady = true;
      setStatus("就绪 - 截图或按 Alt+Q 开始识别");
      setResultMeta(status.model_id || "模型已就绪");
      if (currentImageB64 && !batchMode) {
        void recognizeCurrentImage(lastImageSource || "当前图片");
      }
      return;
    }

    if (status.status === "error") {
      modelReady = false;
      setStatus(`模型加载失败: ${status.error || "未知错误"}`);
      setResultMeta("模型不可用");
      return;
    }

    modelReady = false;
    setStatus("模型加载中...", { loading: true });
    setResultMeta("模型加载中...");
  }

  if (!tauri?.core || !tauri?.event) {
    setStatus("Tauri API 不可用");
    return;
  }

  configureMathLive();

  const { listen } = tauri.event;
  const { invoke } = tauri.core;
  const listenOptions = { target: { kind: "Any" } };

  captureButton.addEventListener("click", () => {
    void captureRegion(batchMode ? "题集截图" : "截图");
  });

  batchButton.addEventListener("click", () => {
    if (batchMode) {
      exitBatchMode();
      return;
    }
    enterBatchMode();
  });

  copyButton.addEventListener("click", () => {
    void copyCurrentFormat();
  });

  copyEditorButton.addEventListener("click", () => {
    void copyEditorLatex();
  });

  keyboardButton.addEventListener("click", () => {
    if (!window.mathVirtualKeyboard) {
      return;
    }
    mathEditor.focus();
    if (window.mathVirtualKeyboard.visible) {
      window.mathVirtualKeyboard.hide({ animate: true });
    } else {
      window.mathVirtualKeyboard.show({ animate: true });
      window.mathVirtualKeyboard.update?.(mathEditor);
    }
    keyboardButton.textContent = window.mathVirtualKeyboard.visible
      ? "隐藏数学键盘"
      : "显示数学键盘";
  });

  formatSelect.addEventListener("change", () => {
    updateExportView();
  });

  batchExportButton.addEventListener("click", () => {
    void batchExportAll();
  });

  mathEditor.addEventListener("input", () => {
    if (suppressEditorInput) {
      return;
    }
    const latex = getEditorLatex();
    renderKatex(latex);
    syncEditorLatexMirror();
    editorMeta.textContent = "已根据编辑器更新预览与导出";
    hideCandidateViews();
    syncCurrentEditorToBatchItem();
    if (batchMode) {
      renderBatchItems();
    }
    window.clearTimeout(exportDebounce);
    exportDebounce = window.setTimeout(() => {
      void refreshExports("mathlive-edit");
    }, 180);
  });

  editorLatex.addEventListener("input", () => {
    if (suppressLatexInput) {
      return;
    }
    const latex = editorLatex.value.trim();
    suppressEditorInput = true;
    try {
      if (typeof mathEditor.setValue === "function") {
        mathEditor.setValue(latex, { silenceNotifications: true });
      } else {
        mathEditor.value = latex;
      }
    } finally {
      suppressEditorInput = false;
    }
    renderKatex(latex);
    hideCandidateViews();
    syncCurrentEditorToBatchItem();
    if (batchMode) {
      renderBatchItems();
    }
    window.clearTimeout(exportDebounce);
    exportDebounce = window.setTimeout(() => {
      void refreshExports("latex-source-edit");
    }, 180);
  });

  await listen(
    "global-hotkey-triggered",
    async () => {
      await captureRegion(batchMode ? "题集热键" : "热键");
    },
    listenOptions
  );

  await listen(
    "clipboard-image",
    async (eventPayload) => {
      await handleIncomingImage(eventPayload.payload, batchMode ? "题集剪贴板" : "剪贴板");
    },
    listenOptions
  );

  await listen(
    "model-ready",
    (eventPayload) => {
      applyModelStatus(eventPayload.payload);
    },
    listenOptions
  );

  await listen(
    "model-error",
    (eventPayload) => {
      applyModelStatus(eventPayload.payload);
    },
    listenOptions
  );

  syncEditorLatexMirror();
  updateExportView();
  updateBatchModeUi();

  try {
    const modelStatus = await invoke("get_model_status");
    applyModelStatus(modelStatus);
  } catch (error) {
    console.error("get_model_status failed", error);
    setStatus("无法获取模型状态");
  }

  try {
    await invoke("register_hotkey", { shortcut: "Alt+Q" });
  } catch (error) {
    console.error("register_hotkey failed", error);
  }

  try {
    const payload = await invoke("read_clipboard_image");
    if (payload && !batchMode) {
      await handleIncomingImage(payload, "剪贴板");
    }
  } catch (error) {
    console.error("read_clipboard_image failed", error);
  }
}
