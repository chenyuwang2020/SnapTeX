# SnapTeX

Screenshot -> LaTeX formula recognition, powered by ONNX Runtime.  
截图即识别，轻量级公式 OCR 桌面工具。

## Features / 功能

- Screenshot capture (`Alt+Q` hotkey) -> automatic formula recognition
- Beam search with multiple candidates and confidence visualization
- MathLive visual editor + LaTeX source editor (bidirectional sync)
- Export to LaTeX / MathML / Typst / Markdown
- Batch mode for capturing multiple formulas
- ~30MB installer, offline-capable, GPU accelerated via DirectML

## Requirements / 环境要求

- Windows 10/11 (x64)
- [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually pre-installed on Windows 10+)

## Install / 安装

Download `SnapTeX_2.0.0_x64-setup.exe` from [Releases](../../releases) and run the installer.

## Development / 开发

```bash
# Prerequisites: Rust toolchain, cargo-tauri, VS Build Tools
cd src-tauri
cargo tauri dev
```

Model files are not included in the repository. Place the ONNX model in `models/mfr-1.5-int8/`.

## License / 许可证

MIT
