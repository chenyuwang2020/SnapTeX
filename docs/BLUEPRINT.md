# SnapTeX 2.0 — 极高颗粒度实施蓝图

> **基于真实源码分析，面向 Codex 的工程指令文档**
> **生成时间**: 2026-03-27
> **生成者**: Sisyphus (Claude Opus) 基于 4 个竞品仓库的深度源码分析 + Oracle 架构咨询 + Librarian 技术调研

---

## Part 1: 竞品深度剖析（基于源码）

### 1.1 LaTeX-OCR (lukas-blecher)

| 维度 | 分析 |
|------|------|
| **模型架构** | 自定义 CustomVisionTransformer：ResNetV2 backbone (layers=[2,3,3]) → HybridEmbed → ViT encoder → Transformer decoder。额外的 image_resizer 模型做自适应分辨率 |
| **推理引擎** | 纯 PyTorch，无 ONNX 支持 |
| **交互流程** | CLI REPL + PyQt5 GUI。剪贴板 ImageGrab.grabclipboard() → resize/pad → inference → clipboard.copy(pred) |
| **优势** | image_resizer 做自适应分辨率，对不同分辨率截图鲁棒性更好 |
| **痛点** | (1) 仅单公式，无布局分析；(2) 纯 PyTorch ≈ 400MB+；(3) 后处理粗糙；(4) 无导出格式（仅 LaTeX 字符串）|

### 1.2 Pix2Text (breezedeus) — 上游库

| 维度 | 分析 |
|------|------|
| **模型架构** | 三层管线：LayoutParser(DocYolo) → TextFormulaOCR(CnOCR + LatexOCR) → TableOCR。公式引擎用 TrOCRProcessor + VisionEncoderDecoderModel（模型名 mfr-1.5）|
| **推理引擎** | **已支持 ONNX**：ORTModelForVision2Seq via optimum。model_backend='onnx' 参数切换 |
| **ONNX 模型实际尺寸** | encoder_model.onnx = 87.5MB，decoder_model.onnx = 32MB，tokenizer.json = 113KB。**总计 ~119.5MB（fp32）** |
| **后处理** | 完整的 post_process()：remove_redundant_script → remove_trailing_whitespace → replace_illegal_symbols → remove_empty_text → fix_latex（\left/\right 配对修复）→ remove_unnecessary_spaces |
| **优势** | (1) 全管线：公式+文字+表格+PDF+页面布局；(2) ONNX 已就绪；(3) 后处理最成熟 |
| **痛点** | (1) 依赖链巨大：torch, torchvision, cnstd, cnocr 等；(2) ONNX 的 use_cache=False 是强制的（optimum 对 TrOCR past_key_values 导出 broken）；(3) 无桌面 GUI |

### 1.3 LaTeXSnipper（你的前作）

| 维度 | 分析 |
|------|------|
| **架构** | Python 后端 (PyQt6 + QWebEngineView) + Tauri Rust 前端混合架构。daemon_server.py 运行为 TCP JSON-RPC |
| **推理** | model.py 的 ModelWrapper 通过子进程调用 pix2text，有 resident worker 和 one-shot 模式 |
| **导出** | latex_export_formats.py 已实现：LaTeX → MathML (latex2mathml) → OMML (MML2OMML.XSL XSLT)。支持 mml/m 前缀命名空间 |
| **交互** | 原生 Win32 截图覆盖层（commands.rs 中 NativeCaptureOverlayContext），全局热键（RegisterHotKey Win32 API）|
| **优势** | (1) Tauri + Rust 原生截图已实现；(2) MathML/OMML 导出已完成；(3) 2.0 Plan 有 MathLive 设计；(4) Daemon 架构解耦了 UI 和推理 |
| **痛点** | (1) main.py ~1500+ 行单体；(2) PyInstaller + 嵌入 python311 极臃肿；(3) 双进程通信复杂 |

### 1.4 SnapTeX（当前项目）

| 维度 | 分析 |
|------|------|
| **架构** | 干净的 MVC：app.py → controller.py → workers.py → ui.py |
| **推理** | ModelLoader 线程加载 pix2text LatexOCR，RecognitionWorker 线程执行识别。文字 OCR 用 rapidocr_onnxruntime |
| **UI** | PySide6，KaTeX 本地渲染预览，剪贴板监听 |
| **优势** | (1) 代码最干净；(2) 开箱即用 UX |
| **致命痛点** | (1) PyInstaller + torch = ~500MB+ 分发包；(2) 仅 LaTeX 输出；(3) 无截图工具；(4) 无公式编辑/校正 |

---

## Part 2: 差异化杀手功能

### 已确认功能
1. **MathML 原生导出** → 适配 Word/Office
2. **Typst 导出** → 新一代轻量排版
3. **Markdown 实时渲染** → 学术写作

### 新增 5 个核心差异化功能

| # | 功能名 | 直击痛点 | 技术方案 |
|---|--------|---------|---------|
| **F1** | **歧义审查 (Ambiguity Review)** | OCR 常混淆 l/1, x/×, O/0，用户不知道哪里错了 | 输出 top-3 候选 + 置信度热力图，高亮不确定区域 |
| **F2** | **MathLive 可视化校正器** | 不懂 LaTeX 的用户无法修改识别结果 | 嵌入 MathLive math-field 作为主编辑面 |
| **F3** | **个人纠错记忆 (Correction Memory)** | 同一用户反复遇到相同识别错误 | 本地 SQLite 存储修正历史，后续识别时 rerank |
| **F4** | **题集模式 (Problem-Set Mode)** | 数学竞赛学生需要批量抓取试卷多道题 | 连续截图队列 + 自动编号 + 批量导出 |
| **F5** | **一键粘贴为 Word 公式** | 国内用户用 Word 但 Word 不认 LaTeX | LaTeX → MathML → OMML → 剪贴板 RTF/OMML |

---

## Part 3: 技术栈重构决策

### 3.1 推理引擎：ONNX Runtime（唯一正确选择）

**决策**：丢弃 PyTorch，全面转向 ONNX Runtime。

| 方案 | 分发大小 | TrOCR 支持 | 理由 |
|------|---------|-----------|------|
| **ONNX Runtime (CPU)** ✅ | onnxruntime.dll ~9MB | 完整支持 | 唯一满足 <80MB + transformer seq2seq |
| NCNN | ~2MB | ❌ transformer 生成极弱 | 不适合 encoder-decoder 自回归 |
| OpenVINO | ~50MB | 部分 | Intel 优先，非 Windows 首选 |
| TensorRT | ~200MB+ | NVIDIA only | 仅可选加速包 |

**模型尺寸预算**（HuggingFace 实际文件）：

| 格式 | 大小 |
|------|------|
| PyTorch (fp32 + framework) | ~400MB |
| ONNX fp32 (encoder + decoder) | ~119.5MB |
| ONNX int8 量化 | ~37MB (预估) |

**Provider 分层**：
- **Base**: ONNX Runtime CPU (~9MB) + int8 模型 (~37MB) + Tauri (~12MB) = **~58MB**
- **GPU 加速包（可选）**: DirectML (~120MB)
- **NVIDIA 加速包（可选）**: CUDA provider

### 3.2 桌面框架：Tauri + Rust 原生 ONNX（无 Python）

**决策**：Tauri 2.0 + Rust ort crate，完全移除 Python。

| 方案 | 分发大小 | 开发速度 | 维护成本 |
|------|---------|---------|---------|
| **Tauri + Rust + ort** ✅ | ~58MB | 中等 | 低（单进程）|
| Tauri + Python sidecar | ~90MB+ | 快 | 高（双进程）|
| Electron + onnxruntime-node | ~150MB+ | 快 | 中 |

**关键依据**：
- ort crate 2.0.0-rc.12 已有 phi-3-vision 的 encoder-decoder 完整示例
- TrOCR 的 use_cache=False 强制路径反而简化了 Rust 实现（无需 past_key_values）
- ort 的 copy-dylibs feature 仅拷贝 onnxruntime.dll (~9MB)

### 3.3 关键技术风险

| 风险 | 等级 | 缓解方案 |
|------|------|---------|
| int8 量化后精度下降 >2% | 高 | **M0 阶段先验证**，不合格则换模型 |
| use_cache=False 导致解码慢 | 中 | 公式一般 <100 tokens，可接受 |
| MathML/OMML Rust 实现复杂 | 中 | 可先内嵌 latex2mathml WASM |

---

## Part 4: 全局架构与数据流

```
┌──────────────────────────────────────────────────────────────────┐
│                      Tauri 2.0 (Rust Host)                       │
│                                                                  │
│  ┌─────────────┐  ┌──────────────┐  ┌─────────────────────────┐ │
│  │ Screenshot   │  │ Clipboard    │  │ Global Hotkey           │ │
│  │ Overlay      │  │ Monitor      │  │ (RegisterHotKey Win32)  │ │
│  │ (Win32 GDI)  │  │ (arboard)    │  │                         │ │
│  └──────┬───────┘  └──────┬───────┘  └───────────┬─────────────┘ │
│         │                 │                       │               │
│         └─────────────────┴───────────┬───────────┘               │
│                                       │ Image (RGBA bytes)        │
│                                       ▼                           │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │                    Pipeline Engine (Rust)                  │   │
│  │                                                           │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌─────────────────┐  │   │
│  │  │ Preprocessor │→│ ORT Session   │→│ Decoder Loop    │  │   │
│  │  │ (resize,     │  │ (encoder.onnx│  │ (autoregressive │  │   │
│  │  │  normalize,  │  │  → encoder   │  │  token gen,     │  │   │
│  │  │  pad)        │  │  features)   │  │  decoder.onnx)  │  │   │
│  │  └─────────────┘  └──────────────┘  └───────┬─────────┘  │   │
│  │                                              │             │   │
│  │  ┌─────────────┐  ┌──────────────┐  ┌───────▼─────────┐  │   │
│  │  │ PostProcess  │←│ Tokenizer    │←│ Token IDs       │  │   │
│  │  │ (fix_latex,  │  │ (decode)     │  │                 │  │   │
│  │  │  rerank)     │  │              │  │                 │  │   │
│  │  └──────┬──────┘  └──────────────┘  └─────────────────┘  │   │
│  │         │ RecognitionResult { latex, score, alternatives } │   │
│  └─────────┼─────────────────────────────────────────────────┘   │
│            │                                                      │
│            ▼                                                      │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │                   Export Adapters (Rust)                    │   │
│  │  ┌──────┐ ┌────────┐ ┌──────┐ ┌──────┐ ┌────────┐       │   │
│  │  │LaTeX │ │MathML  │ │OMML  │ │Typst │ │Markdown│       │   │
│  │  └──────┘ └────────┘ └──────┘ └──────┘ └────────┘       │   │
│  └───────────────────────────────────────────────────────────┘   │
│            │                                                      │
│            ▼ Tauri Commands (invoke)                              │
│  ┌───────────────────────────────────────────────────────────┐   │
│  │              Web Frontend (Tauri WebView)                  │   │
│  │  ┌───────────────┐  ┌──────────────┐  ┌───────────────┐  │   │
│  │  │ Result Panel   │  │ MathLive     │  │ Settings      │  │   │
│  │  │ (KaTeX render, │  │ Editor       │  │               │  │   │
│  │  │  confidence)   │  │ (correction) │  │               │  │   │
│  │  └───────────────┘  └──────────────┘  └───────────────┘  │   │
│  └───────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

---

## Part 5: 目录与文件结构

```
snaptex-v2/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   └── src/
│       ├── main.rs                     # Tauri 入口
│       ├── commands.rs                 # Tauri command 定义
│       │
│       ├── capture/
│       │   ├── mod.rs
│       │   ├── overlay.rs              # Win32 原生截图覆盖层
│       │   ├── clipboard.rs            # arboard 剪贴板监听
│       │   └── hotkey.rs               # 全局热键
│       │
│       ├── inference/
│       │   ├── mod.rs
│       │   ├── recognizer.rs           # trait Recognizer + TrOCRRecognizer
│       │   ├── provider.rs             # ExecutionProvider: CPU/DirectML/CUDA
│       │   ├── decoder_loop.rs         # 自回归解码（参考 ort phi-3-vision）
│       │   ├── preprocessor.rs         # 图片 resize/normalize/pad
│       │   ├── tokenizer.rs            # tokenizers crate 封装
│       │   ├── postprocessor.rs        # LaTeX 后处理（移植自 Pix2Text）
│       │   ├── model_registry.rs       # 模型清单管理
│       │   └── job_queue.rs            # 任务队列 + 批处理
│       │
│       ├── export/
│       │   ├── mod.rs
│       │   ├── latex.rs
│       │   ├── mathml.rs               # LaTeX → MathML
│       │   ├── omml.rs                 # MathML → OMML
│       │   ├── typst.rs                # LaTeX → Typst
│       │   ├── markdown.rs
│       │   └── clipboard_formats.rs    # 多格式剪贴板写入
│       │
│       ├── correction/
│       │   ├── mod.rs
│       │   ├── memory.rs               # SQLite 修正历史
│       │   └── reranker.rs             # 候选重排
│       │
│       └── config/
│           ├── mod.rs
│           └── settings.rs
│
├── dist/                               # Web 前端
│   ├── index.html
│   ├── main.js
│   ├── styles.css
│   └── assets/
│       ├── katex/
│       └── mathlive/
│
├── models/
│   └── mfr-1.5-int8/
│       ├── model_manifest.json
│       ├── encoder.onnx               # ~25MB (int8)
│       ├── decoder.onnx               # ~12MB (int8)
│       └── tokenizer.json             # ~113KB
│
└── scripts/
    ├── export_onnx.py                  # 导出 + 量化脚本
    └── benchmark.py                    # 精度基准测试
```

---

## Part 6: 核心模块接口定义

### 6.1 inference/recognizer.rs

```rust
pub struct ModelMetadata {
    pub id: String,                    // "mfr-1.5-int8"
    pub task: String,                  // "formula_ocr"
    pub image_size: (u32, u32),        // (384, 384)
    pub max_length: usize,             // 256
    pub bos_token_id: i64,
    pub eos_token_id: i64,
    pub pad_token_id: i64,
}

pub struct Candidate {
    pub latex: String,
    pub score: f32,
    pub warnings: Vec<String>,
}

pub struct RecognitionResult {
    pub primary: Candidate,
    pub alternatives: Vec<Candidate>,
    pub latency_ms: u64,
    pub provider: String,
    pub model_id: String,
}

pub struct InferenceJob {
    pub image: Vec<u8>,                // RGBA 像素
    pub width: u32,
    pub height: u32,
    pub top_k: usize,
    pub timeout_ms: u64,
}

/// 核心识别器 trait
pub trait Recognizer: Send + Sync {
    fn metadata(&self) -> &ModelMetadata;
    fn warmup(&mut self) -> Result<(), InferenceError>;
    fn infer(&self, job: InferenceJob) -> Result<RecognitionResult, InferenceError>;
    fn infer_batch(&self, jobs: Vec<InferenceJob>) -> Result<Vec<RecognitionResult>, InferenceError>;
}
```

### 6.2 inference/decoder_loop.rs

```rust
/// 自回归解码循环 (use_cache=False 模式)
///
/// 1. encoder_session.run(pixel_values) → encoder_hidden_states
/// 2. loop {
///      decoder_session.run(decoder_input_ids, encoder_hidden_states) → logits
///      next_token = argmax(logits[:, -1, :])
///      if next_token == eos_token_id { break }
///      decoder_input_ids = concat(decoder_input_ids, next_token)
///    }
/// 3. tokenizer.decode(all_token_ids) → raw_latex
pub fn autoregressive_decode(
    encoder_session: &ort::Session,
    decoder_session: &ort::Session,
    pixel_values: &ndarray::Array4<f32>,
    config: &ModelMetadata,
    tokenizer: &tokenizers::Tokenizer,
) -> Result<Vec<Candidate>, InferenceError> {
    todo!()
}
```

### 6.3 inference/postprocessor.rs

```rust
/// LaTeX 后处理器（移植自 Pix2Text latex_ocr.py）
/// 处理链：
/// 1. remove_redundant_script
/// 2. remove_trailing_whitespace
/// 3. replace_illegal_symbols
/// 4. remove_empty_text
/// 5. fix_latex (\left/\right 配对)
/// 6. remove_unnecessary_spaces
pub struct LatexPostProcessor;

impl LatexPostProcessor {
    pub fn process(&self, raw: &str) -> String { todo!() }
    fn fix_left_right_pairs(&self, latex: &str) -> String { todo!() }
}
```

### 6.4 export/mathml.rs

```rust
pub fn latex_to_mathml(latex: &str) -> Result<String, ExportError> { todo!() }
pub fn mathml_with_prefix(mathml: &str, prefix: &str) -> String { todo!() }
pub fn latex_to_omml(latex: &str) -> Result<String, ExportError> { todo!() }

pub struct ExportBundle {
    pub latex: String,
    pub markdown: String,
    pub mathml: Option<String>,
    pub omml: Option<String>,
    pub typst: Option<String>,
}

pub fn build_export_formats(latex: &str) -> ExportBundle { todo!() }
```

### 6.5 commands.rs

```rust
#[tauri::command]
pub fn capture_region() -> Result<CaptureResult, String> { todo!() }

#[tauri::command]
pub fn read_clipboard_image() -> Result<Option<ImageData>, String> { todo!() }

#[tauri::command]
pub async fn recognize_formula(
    state: tauri::State<'_, AppState>,
    image_b64: String,
    top_k: Option<usize>,
) -> Result<RecognitionResult, String> { todo!() }

#[tauri::command]
pub fn export_formats(latex: String) -> Result<ExportBundle, String> { todo!() }

#[tauri::command]
pub fn copy_to_clipboard(content: String, format: String) -> Result<(), String> { todo!() }

#[tauri::command]
pub fn copy_as_word_formula(latex: String) -> Result<(), String> { todo!() }

#[tauri::command]
pub fn register_hotkey(shortcut: String) -> Result<HotkeyStatus, String> { todo!() }

#[tauri::command]
pub fn save_correction(original: String, corrected: String) -> Result<(), String> { todo!() }

#[tauri::command]
pub fn get_model_status(state: tauri::State<'_, AppState>) -> Result<ModelStatus, String> { todo!() }
```

### 6.6 model_manifest.json

```json
{
  "id": "mfr-1.5-int8",
  "task": "formula_ocr",
  "architecture": "vision_encoder_decoder",
  "inputs": {
    "image_size": [384, 384],
    "pixel_format": "rgb",
    "normalize_mean": [0.5, 0.5, 0.5],
    "normalize_std": [0.5, 0.5, 0.5]
  },
  "files": {
    "encoder": "encoder.onnx",
    "decoder": "decoder.onnx",
    "tokenizer": "tokenizer.json"
  },
  "generation": {
    "bos_token_id": 0,
    "eos_token_id": 2,
    "pad_token_id": 1,
    "max_length": 256,
    "num_beams": 1
  },
  "providers": ["cpu", "directml", "cuda"]
}
```

---

## Part 7: 实施里程碑（含人类验证检查点）

> **每个里程碑结束时都有明确的「人类验证」步骤。**
> Codex 完成每个里程碑后，必须告知用户如何运行和验证，然后**等待用户确认**再进入下一阶段。

---

### M0: 模型验证（Python 环境，3天）

**目标**: 验证 mfr-1.5 ONNX int8 量化后精度可接受

**Codex 任务**:
1. 编写 `scripts/export_onnx.py`：从 HuggingFace 导出 + int8 量化
2. 编写 `scripts/benchmark.py`：对比 PyTorch fp32 vs ONNX int8 的识别精度
3. 准备 10 张测试公式截图

**人类验证检查点 M0**:
```bash
cd SnapTeX
python scripts/export_onnx.py --model breezedeus/pix2text-mfr-1.5 --output ./models/mfr-1.5-int8/ --quantize int8
python scripts/benchmark.py --test-images ./test_images/ --model-dir ./models/mfr-1.5-int8/
```
- 验证：int8 精度损失 <2%（benchmark 输出对比表）
- 验证：模型文件总大小 <50MB
- 若精度不可接受 → 停止，换模型后重新验证

---

### M1: Rust 推理 MVP（1周）

**目标**: 命令行输入图片，输出 LaTeX

**Codex 任务**:
1. `cargo init snaptex-v2` + 配置 Cargo.toml (ort, image, tokenizers, ndarray)
2. 实现 `inference/preprocessor.rs`
3. 实现 `inference/decoder_loop.rs`
4. 实现 `inference/tokenizer.rs`
5. 实现 `inference/postprocessor.rs`
6. 编写 `src/main.rs` 的 CLI 模式

**人类验证检查点 M1**:
```bash
cd snaptex-v2
cargo build --release
./target/release/snaptex-v2 --image test.png
# 应输出类似: \frac{1}{2} + \sum_{i=1}^{n} x_i
```
- 验证：输出正确 LaTeX
- 验证：推理时间 <3 秒（CPU）
- 验证：二进制 + 模型 <80MB

---

### M2: Tauri 壳 + 截图（1周）

**目标**: 有窗口，能截图，截图后显示在窗口中

**Codex 任务**:
1. 配置 Tauri 2.0 项目结构
2. 移植 LaTeXSnipper 的 Win32 截图覆盖层到 `capture/overlay.rs`
3. 实现 `capture/clipboard.rs` (arboard)
4. 实现 `capture/hotkey.rs` (RegisterHotKey)
5. 编写最小前端 (index.html + main.js)：显示截图 + "识别"按钮

**人类验证检查点 M2**:
```bash
cargo tauri dev
```
- 验证：窗口正常启动
- 验证：按全局热键 → 截图覆盖层出现 → 框选区域 → 图片显示在窗口中
- 验证：复制图片到剪贴板 → 窗口自动显示图片

---

### M3: 端到端集成（3天）

**目标**: 截图 → 识别 → 渲染 → 自动复制

**Codex 任务**:
1. 连接 M1 推理引擎到 M2 的 Tauri commands
2. 前端添加 KaTeX 渲染面板
3. 识别结果自动复制到剪贴板
4. 添加加载状态（模型 warmup 进度）

**人类验证检查点 M3**:
```bash
cargo tauri dev
```
- 验证：截图 → 自动识别 → KaTeX 渲染公式 → LaTeX 已在剪贴板
- 验证：打开 Word/Typora，Ctrl+V 粘贴 LaTeX 文本正确
- 验证：整个流程 <3 秒
- **这是第一个"可用产品"节点——功能等同当前 SnapTeX v1**

---

### M4: 导出格式（1周）

**目标**: LaTeX/MathML/OMML/Typst/Markdown 全格式导出

**Codex 任务**:
1. 实现 `export/mathml.rs`
2. 实现 `export/omml.rs`（内嵌 MML2OMML.XSL）
3. 实现 `export/typst.rs`
4. 实现 `export/clipboard_formats.rs`（OMML → Word 粘贴）
5. 前端添加格式选择下拉

**人类验证检查点 M4**:
```bash
cargo tauri dev
```
- 验证：截图识别后，切换到 MathML 格式 → 复制 → 检查输出
- 验证：切换到 OMML → 粘贴到 Word → **公式正确渲染为 Word 原生公式**
- 验证：Typst 格式输出语法正确
- 验证：Markdown $$ 包装正确

---

### M5: MathLive 编辑器（1周）

**目标**: 识别结果进入 MathLive 可视化编辑器

**Codex 任务**:
1. 打包 MathLive 到 dist/assets/mathlive/
2. 前端实现 MathLive math-field 组件
3. Tauri bridge：setLatex / getLatex / focusMathField
4. 识别结果 → 自动填入 MathLive → 用户可视化编辑 → 导出

**人类验证检查点 M5**:
```bash
cargo tauri dev
```
- 验证：截图识别后，公式出现在 MathLive 编辑器中
- 验证：可以用鼠标/键盘修改公式（如修改分数分子）
- 验证：修改后点击复制 → 拿到修改后的 LaTeX
- 验证：虚拟数学键盘可用

---

### M6: 差异化功能（2周）

**目标**: 歧义审查 + 纠错记忆 + 题集模式

**Codex 任务**:
1. 歧义审查：top-K 候选展示 + 置信度高亮
2. correction/memory.rs：SQLite 修正存储
3. correction/reranker.rs：基于历史 rerank
4. 题集模式：连续截图队列 + 批量导出

**人类验证检查点 M6**:
```bash
cargo tauri dev
```
- 验证：识别不确定的公式时，能看到多个候选和高亮
- 验证：修正一个错误后，下次遇到相同模式时排名更高
- 验证：题集模式能连续截图 3 道题并批量导出

---

### M7: 打包发布（3天）

**目标**: 生成 <80MB 安装包

**Codex 任务**:
1. 配置 tauri.conf.json bundle
2. NSIS/Inno Setup 安装脚本
3. 首次启动引导（模型下载 or 内嵌）
4. 自动更新机制

**人类验证检查点 M7**:
```bash
cargo tauri build
```
- 验证：安装包大小 <80MB
- 验证：在干净 Windows 机器安装 → 运行 → 截图识别正常
- 验证：卸载干净

---

## 附录 A: 关键技术参考

| 资源 | 链接/位置 |
|------|----------|
| ort crate phi-3-vision 示例 | github.com/pykeio/ort/examples/phi-3-vision |
| Pix2Text mfr-1.5 ONNX 文件 | huggingface.co/breezedeus/pix2text-mfr-1.5 |
| LaTeXSnipper Win32 截图代码 | LaTeXSnipper/apps/tauri-client/src-tauri/src/commands.rs |
| LaTeXSnipper 导出格式代码 | LaTeXSnipper/src/backend/latex_export_formats.py |
| Pix2Text 后处理代码 | Pix2Text/pix2text/latex_ocr.py (post_process 函数) |
| Tauri + Python sidecar 参考 | github.com/dieharders/example-tauri-v2-python-server-sidecar |

## 附录 B: Cargo.toml 核心依赖

```toml
[dependencies]
tauri = { version = "2", features = ["shell-open"] }
ort = { version = "2.0.0-rc.12", features = ["download-binaries", "copy-dylibs"] }
tokenizers = "0.21"
image = "0.25"
ndarray = "0.16"
arboard = "3"
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## 附录 C: 模型导出命令

```bash
# 步骤 1: 导出 ONNX
pip install optimum[onnxruntime] onnxruntime
python -m optimum.exporters.onnx \
  --model breezedeus/pix2text-mfr-1.5 \
  --task vision2seq-lm \
  ./models/mfr-1.5-fp32/

# 步骤 2: int8 量化
python -c "
from onnxruntime.quantization import quantize_dynamic, QuantType
quantize_dynamic('models/mfr-1.5-fp32/encoder_model.onnx', 'models/mfr-1.5-int8/encoder.onnx', weight_type=QuantType.QInt8)
quantize_dynamic('models/mfr-1.5-fp32/decoder_model.onnx', 'models/mfr-1.5-int8/decoder.onnx', weight_type=QuantType.QInt8)
"

# 步骤 3: 复制 tokenizer
cp models/mfr-1.5-fp32/tokenizer.json models/mfr-1.5-int8/
```
