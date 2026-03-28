from __future__ import annotations

import argparse
import difflib
import json
import logging
import re
import statistics
import sys
import time
import warnings
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import transformers
from pix2text import LatexOCR
from pix2text import latex_ocr as latex_ocr_module


DEFAULT_FP32_ONNX_MODEL = "breezedeus/pix2text-mfr-1.5"
DEFAULT_PYTORCH_MODEL = "breezedeus/pix2text-mfr-1.5-pytorch"


@dataclass
class SampleResult:
    image: str
    ground_truth: str
    prediction: str
    latency_ms: float
    exact_match: bool
    similarity: float


@dataclass
class RunnerResult:
    label: str
    backend: str
    source: str
    samples: list[SampleResult]
    warning: str | None = None


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark SnapTeX M0 formula OCR accuracy on PyTorch fp32 versus ONNX int8."
    )
    parser.add_argument(
        "--test-images",
        required=True,
        help="Directory containing the formula images and manifest.json.",
    )
    parser.add_argument(
        "--model-dir",
        required=True,
        help="Local directory containing the quantized ONNX model.",
    )
    parser.add_argument(
        "--pytorch-model",
        default=DEFAULT_PYTORCH_MODEL,
        help="PyTorch checkpoint repo id or local directory. If unavailable, the script falls back to public fp32 ONNX unless --require-pytorch is set.",
    )
    parser.add_argument(
        "--fp32-onnx-model",
        default=DEFAULT_FP32_ONNX_MODEL,
        help="Public fp32 ONNX repo id or local directory used as fallback baseline.",
    )
    parser.add_argument(
        "--require-pytorch",
        action="store_true",
        help="Fail instead of falling back to fp32 ONNX when the PyTorch checkpoint is inaccessible.",
    )
    parser.add_argument(
        "--max-accuracy-drop",
        type=float,
        default=0.02,
        help="Fail if the int8 exact-match accuracy drops by more than this amount relative to the baseline.",
    )
    parser.add_argument(
        "--output-json",
        default=None,
        help="Optional path for a machine-readable benchmark result.",
    )
    return parser.parse_args()


def main() -> int:
    configure_runtime()
    args = parse_args()
    test_dir = Path(args.test_images).resolve()
    model_dir = Path(args.model_dir).resolve()
    manifest = load_manifest(test_dir / "manifest.json")

    baseline_runner, baseline_warning = load_baseline_runner(args)
    int8_runner = load_onnx_runner(
        label="ONNX int8",
        source=str(model_dir),
    )

    baseline_result = run_benchmark(baseline_runner, manifest, test_dir, baseline_warning)
    int8_result = run_benchmark(int8_runner, manifest, test_dir)

    summary_rows = build_summary_rows(baseline_result, int8_result)
    image_rows = build_image_rows(baseline_result, int8_result)

    print("\n[M0] Summary")
    print(render_table(["Model", "Backend", "Exact Match", "Mean Similarity", "Avg Latency (ms)"], summary_rows))
    print("\n[M0] Per-image predictions")
    print(render_table(["Image", "Ground Truth", baseline_result.label, int8_result.label], image_rows))

    baseline_exact = metric_exact_match(baseline_result.samples)
    int8_exact = metric_exact_match(int8_result.samples)
    exact_gap = baseline_exact - int8_exact
    baseline_similarity = metric_similarity(baseline_result.samples)
    int8_similarity = metric_similarity(int8_result.samples)
    accuracy_drop = baseline_similarity - int8_similarity
    print(
        f"\n[M0] Similarity drop: {accuracy_drop * 100:.2f} percentage points "
        f"({baseline_result.label} {baseline_similarity * 100:.2f}% -> {int8_result.label} {int8_similarity * 100:.2f}%)"
    )
    print(
        f"[M0] Exact-match gap: {exact_gap * 100:.2f} percentage points "
        f"({baseline_result.label} {baseline_exact * 100:.2f}% -> {int8_result.label} {int8_exact * 100:.2f}%)"
    )
    if baseline_result.warning:
        print(f"[M0] Baseline note: {baseline_result.warning}")

    payload = {
        "baseline": runner_to_dict(baseline_result),
        "int8": runner_to_dict(int8_result),
        "similarity_drop": accuracy_drop,
        "exact_match_gap": exact_gap,
        "threshold": args.max_accuracy_drop,
        "passed": accuracy_drop <= args.max_accuracy_drop,
    }
    if args.output_json:
        output_path = Path(args.output_json).resolve()
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"[M0] Wrote JSON report to {output_path}")

    if accuracy_drop > args.max_accuracy_drop:
        print(
            f"[M0] FAILED: similarity drop {accuracy_drop * 100:.2f} percentage points exceeds the allowed "
            f"{args.max_accuracy_drop * 100:.2f} percentage points.",
            file=sys.stderr,
        )
        return 1

    return 0


def configure_runtime() -> None:
    warnings.filterwarnings("ignore", message="Using a slow image processor as `use_fast` is unset*")
    warnings.filterwarnings("ignore", message="Could not find any ONNX files with standard file name*")
    transformers.utils.logging.set_verbosity_error()
    logging.getLogger("optimum").setLevel(logging.ERROR)
    logging.getLogger("onnxruntime").setLevel(logging.ERROR)
    latex_ocr_module.tqdm.tqdm = lambda iterable, *args, **kwargs: iterable


def load_baseline_runner(args: argparse.Namespace) -> tuple[LatexOCR, str | None]:
    try:
        runner = LatexOCR(
            model_name="mfr-1.5",
            model_backend="pytorch",
            model_dir=args.pytorch_model,
            device="cpu",
        )
        runner._snaptex_label = "PyTorch fp32"
        runner._snaptex_backend = "pytorch"
        runner._snaptex_source = str(args.pytorch_model)
        return runner, None
    except Exception as exc:  # noqa: BLE001
        if args.require_pytorch:
            raise
        warning = (
            "PyTorch fp32 baseline is unavailable in the current environment, so the benchmark used "
            f"the public fp32 ONNX checkpoint instead. Original error: {exc}"
        )
        fallback = load_onnx_runner(label="ONNX fp32 (fallback)", source=args.fp32_onnx_model)
        return fallback, warning


def load_onnx_runner(label: str, source: str) -> LatexOCR:
    runner = LatexOCR(
        model_name="mfr-1.5",
        model_backend="onnx",
        model_dir=source,
        device="cpu",
        more_model_configs={
            "provider": "CPUExecutionProvider",
            "use_cache": False,
        },
    )
    runner._snaptex_label = label
    runner._snaptex_backend = "onnx"
    runner._snaptex_source = source
    return runner


def run_benchmark(
    runner: LatexOCR,
    manifest: list[dict[str, Any]],
    test_dir: Path,
    warning: str | None = None,
) -> RunnerResult:
    label = resolve_runner_label(runner)
    backend = resolve_runner_backend(runner)
    source = resolve_runner_source(runner)
    samples: list[SampleResult] = []
    for item in manifest:
        image_path = test_dir / item["file"]
        ground_truth = item["latex"]
        started = time.perf_counter()
        raw_result = runner.recognize(str(image_path), batch_size=1)
        latency_ms = (time.perf_counter() - started) * 1000
        prediction = raw_result["text"]
        normalized_gt = normalize_latex(ground_truth)
        normalized_pred = normalize_latex(prediction)
        samples.append(
            SampleResult(
                image=item["file"],
                ground_truth=ground_truth,
                prediction=prediction,
                latency_ms=latency_ms,
                exact_match=normalized_gt == normalized_pred,
                similarity=difflib.SequenceMatcher(None, normalized_gt, normalized_pred).ratio(),
            )
        )
    return RunnerResult(
        label=label,
        backend=backend,
        source=source,
        samples=samples,
        warning=warning,
    )


def resolve_runner_label(runner: LatexOCR) -> str:
    return getattr(runner, "_snaptex_label", "unknown")


def resolve_runner_backend(runner: LatexOCR) -> str:
    return getattr(runner, "_snaptex_backend", "unknown")


def resolve_runner_source(runner: LatexOCR) -> str:
    return getattr(runner, "_snaptex_source", "unknown")


def load_manifest(path: Path) -> list[dict[str, Any]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list) or not data:
        raise ValueError(f"manifest must be a non-empty list: {path}")
    return data


def normalize_latex(text: str) -> str:
    return re.sub(r"\s+", "", text).strip()


def metric_exact_match(samples: list[SampleResult]) -> float:
    return sum(sample.exact_match for sample in samples) / len(samples)


def metric_similarity(samples: list[SampleResult]) -> float:
    return statistics.mean(sample.similarity for sample in samples)


def metric_latency(samples: list[SampleResult]) -> float:
    return statistics.mean(sample.latency_ms for sample in samples)


def build_summary_rows(*results: RunnerResult) -> list[list[str]]:
    rows: list[list[str]] = []
    for result in results:
        rows.append(
            [
                result.label,
                result.backend,
                f"{metric_exact_match(result.samples) * 100:.2f}%",
                f"{metric_similarity(result.samples) * 100:.2f}%",
                f"{metric_latency(result.samples):.1f}",
            ]
        )
    return rows


def build_image_rows(baseline: RunnerResult, int8: RunnerResult) -> list[list[str]]:
    rows: list[list[str]] = []
    for base_sample, int8_sample in zip(baseline.samples, int8.samples, strict=True):
        rows.append(
            [
                base_sample.image,
                base_sample.ground_truth,
                format_prediction(base_sample),
                format_prediction(int8_sample),
            ]
        )
    return rows


def format_prediction(sample: SampleResult) -> str:
    status = "OK" if sample.exact_match else "MISS"
    return f"{status} | {sample.prediction}"


def runner_to_dict(result: RunnerResult) -> dict[str, Any]:
    return {
        "label": result.label,
        "backend": result.backend,
        "source": result.source,
        "warning": result.warning,
        "exact_match": metric_exact_match(result.samples),
        "mean_similarity": metric_similarity(result.samples),
        "avg_latency_ms": metric_latency(result.samples),
        "samples": [
            {
                "image": sample.image,
                "ground_truth": sample.ground_truth,
                "prediction": sample.prediction,
                "latency_ms": sample.latency_ms,
                "exact_match": sample.exact_match,
                "similarity": sample.similarity,
            }
            for sample in result.samples
        ],
    }


def render_table(headers: list[str], rows: list[list[str]]) -> str:
    widths = [len(header) for header in headers]
    for row in rows:
        for index, cell in enumerate(row):
            widths[index] = max(widths[index], len(cell))

    def render_row(values: list[str]) -> str:
        return " | ".join(value.ljust(widths[index]) for index, value in enumerate(values))

    separator = "-+-".join("-" * width for width in widths)
    parts = [render_row(headers), separator]
    parts.extend(render_row(row) for row in rows)
    return "\n".join(parts)


if __name__ == "__main__":
    raise SystemExit(main())
