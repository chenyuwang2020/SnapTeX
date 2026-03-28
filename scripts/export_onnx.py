from __future__ import annotations

import argparse
import json
import logging
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Iterable

from huggingface_hub import snapshot_download
from huggingface_hub.utils import disable_progress_bars
from onnxruntime.quantization import QuantType, quantize_dynamic


REQUIRED_ONNX_FILES = ("encoder_model.onnx", "decoder_model.onnx")
REQUIRED_METADATA_FILES = (
    "config.json",
    "generation_config.json",
    "preprocessor_config.json",
    "special_tokens_map.json",
    "tokenizer.json",
    "tokenizer_config.json",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Export the mfr-1.5 model assets for SnapTeX M0 and apply int8 dynamic quantization."
    )
    parser.add_argument(
        "--model",
        default="breezedeus/pix2text-mfr-1.5",
        help="HuggingFace model id or a local directory containing the fp32 ONNX assets.",
    )
    parser.add_argument(
        "--output",
        required=True,
        help="Output directory for the quantized model.",
    )
    parser.add_argument(
        "--quantize",
        choices=("int8",),
        default="int8",
        help="Quantization mode. Only int8 is supported for M0.",
    )
    parser.add_argument(
        "--fp32-dir",
        default=None,
        help="Optional directory to keep the intermediate fp32 model files.",
    )
    parser.add_argument(
        "--max-size-mb",
        type=float,
        default=50.0,
        help="Fail if the quantized output directory exceeds this size.",
    )
    parser.add_argument(
        "--force-download",
        action="store_true",
        help="Force re-download when the HuggingFace fallback path is used.",
    )
    return parser.parse_args()


def main() -> int:
    configure_runtime()
    args = parse_args()
    output_dir = Path(args.output).resolve()
    if args.fp32_dir:
        fp32_dir = Path(args.fp32_dir).resolve()
        fp32_dir.mkdir(parents=True, exist_ok=True)
        keep_fp32 = True
    else:
        fp32_dir = Path(tempfile.mkdtemp(prefix="snaptex-mfr-fp32-"))
        keep_fp32 = False

    try:
        source_mode = prepare_fp32_assets(
            model_ref=args.model,
            fp32_dir=fp32_dir,
            force_download=args.force_download,
        )
        build_quantized_model(
            fp32_dir=fp32_dir,
            output_dir=output_dir,
            max_size_mb=args.max_size_mb,
            quantize_mode=args.quantize,
            source_mode=source_mode,
        )
    finally:
        if not keep_fp32 and fp32_dir.exists():
            shutil.rmtree(fp32_dir, ignore_errors=True)

    return 0


def configure_runtime() -> None:
    os.environ.setdefault("HF_HUB_DISABLE_PROGRESS_BARS", "1")
    disable_progress_bars()
    logging.getLogger().setLevel(logging.ERROR)


def prepare_fp32_assets(model_ref: str, fp32_dir: Path, force_download: bool) -> str:
    if fp32_dir.exists():
        shutil.rmtree(fp32_dir)
    fp32_dir.mkdir(parents=True, exist_ok=True)

    model_path = Path(model_ref)
    if model_path.exists():
        print(f"[M0] Copying local fp32 assets from {model_path}")
        copy_local_model(model_path, fp32_dir)
        validate_fp32_dir(fp32_dir)
        return "local-directory"

    export_errors: list[str] = []
    for task_name in ("vision2seq-lm", "image-to-text"):
        try:
            run_optimum_export(model_ref=model_ref, output_dir=fp32_dir, task_name=task_name)
            validate_fp32_dir(fp32_dir)
            print(f"[M0] Exported fp32 ONNX via optimum task={task_name}")
            return f"optimum-export:{task_name}"
        except Exception as exc:  # noqa: BLE001
            export_errors.append(f"{task_name}: {exc}")
            shutil.rmtree(fp32_dir, ignore_errors=True)
            fp32_dir.mkdir(parents=True, exist_ok=True)

    print("[M0] Optimum export did not succeed. Falling back to the public fp32 ONNX assets on HuggingFace.")
    download_public_onnx_repo(model_ref=model_ref, output_dir=fp32_dir, force_download=force_download)
    validate_fp32_dir(fp32_dir)
    if export_errors:
        print("[M0] Export attempts summary:")
        for item in export_errors:
            print(f"  - {item}")
    return "huggingface-public-onnx"


def copy_local_model(source_dir: Path, target_dir: Path) -> None:
    for item in source_dir.iterdir():
        destination = target_dir / item.name
        if item.is_dir():
            shutil.copytree(item, destination, dirs_exist_ok=True)
        else:
            shutil.copy2(item, destination)


def run_optimum_export(model_ref: str, output_dir: Path, task_name: str) -> None:
    command = [
        sys.executable,
        "-m",
        "optimum.exporters.onnx",
        "--model",
        model_ref,
        "--task",
        task_name,
        str(output_dir),
    ]
    print("[M0] Running:", " ".join(command))
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode != 0:
        details = (completed.stderr or completed.stdout or "").strip()
        if details:
            details = details.splitlines()[-1]
            raise RuntimeError(f"optimum exporter exited with code {completed.returncode}: {details}")
        raise RuntimeError(f"optimum exporter exited with code {completed.returncode}")


def download_public_onnx_repo(model_ref: str, output_dir: Path, force_download: bool) -> None:
    snapshot_path = snapshot_download(
        repo_id=model_ref,
        allow_patterns=["*.onnx", "*.json", "*.txt"],
        local_dir=output_dir,
        force_download=force_download,
    )
    print(f"[M0] Downloaded fp32 ONNX assets to {snapshot_path}")


def validate_fp32_dir(fp32_dir: Path) -> None:
    missing = [name for name in REQUIRED_ONNX_FILES if not (fp32_dir / name).exists()]
    if missing:
        raise FileNotFoundError(f"fp32 model assets are incomplete, missing: {missing}")


def build_quantized_model(
    fp32_dir: Path,
    output_dir: Path,
    max_size_mb: float,
    quantize_mode: str,
    source_mode: str,
) -> None:
    if quantize_mode != "int8":
        raise ValueError(f"Unsupported quantization mode: {quantize_mode}")

    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    print("[M0] Quantizing ONNX weights with dynamic int8...")
    quantize_dynamic(
        str(fp32_dir / "encoder_model.onnx"),
        str(output_dir / "encoder_model.onnx"),
        weight_type=QuantType.QInt8,
        op_types_to_quantize=["MatMul", "Gemm"],
    )
    quantize_dynamic(
        str(fp32_dir / "decoder_model.onnx"),
        str(output_dir / "decoder_model.onnx"),
        weight_type=QuantType.QInt8,
        op_types_to_quantize=["MatMul", "Gemm"],
    )

    for name in REQUIRED_METADATA_FILES:
        source_file = fp32_dir / name
        if source_file.exists():
            shutil.copy2(source_file, output_dir / name)

    directory_size_mb = get_directory_size_mb(output_dir)
    summary = {
        "source_mode": source_mode,
        "output_dir": str(output_dir),
        "quantize_mode": quantize_mode,
        "directory_size_mb": round(directory_size_mb, 3),
        "files": collect_file_sizes(output_dir),
    }
    (output_dir / "export_summary.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    print("[M0] Quantized model files:")
    for rel_name, size_mb in summary["files"].items():
        print(f"  - {rel_name}: {size_mb:.3f} MB")
    print(f"[M0] Total output size: {directory_size_mb:.3f} MB")

    if directory_size_mb > max_size_mb:
        raise SystemExit(
            f"Quantized model directory is {directory_size_mb:.3f} MB, which exceeds the limit of {max_size_mb:.3f} MB."
        )


def collect_file_sizes(root_dir: Path) -> dict[str, float]:
    result: dict[str, float] = {}
    for path in sorted(iter_files(root_dir)):
        result[str(path.relative_to(root_dir)).replace("\\", "/")] = path.stat().st_size / (1024 * 1024)
    return result


def get_directory_size_mb(root_dir: Path) -> float:
    return sum(path.stat().st_size for path in iter_files(root_dir)) / (1024 * 1024)


def iter_files(root_dir: Path) -> Iterable[Path]:
    return (path for path in root_dir.rglob("*") if path.is_file())


if __name__ == "__main__":
    raise SystemExit(main())
