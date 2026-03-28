#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env,
    path::{Path, PathBuf},
    process,
    time::Instant,
};

mod capture;
mod app_state;
mod commands;
mod export;
mod inference;

use app_state::AppState;
use inference::recognizer::{Recognizer, TrOCRRecognizer};
use tauri::Manager;

type AppResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct CliArgs {
    image: PathBuf,
    model_dir: PathBuf,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("snaptex-v2 error: {err}");
        process::exit(1);
    }
}

fn run() -> AppResult<()> {
    if let Some(args) = parse_cli_args()? {
        return run_cli(args);
    }

    let _ = ort::init().with_name("snaptex-v2").commit();
    let app_state = AppState::new();
    tauri::Builder::default()
        .manage(app_state.clone())
        .setup(move |app| {
            let handle = app.handle().clone();
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
            }
            eprintln!("setup: main window ready");
            capture::clipboard::start_monitor(handle.clone()).map_err(std::io::Error::other)?;
            capture::hotkey::start_hotkey_thread(handle, "Alt+Q")
                .map_err(std::io::Error::other)?;
            app_state.start_model_loader(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::capture_region,
            commands::read_clipboard_image,
            commands::register_hotkey,
            commands::recognize_formula,
            commands::get_model_status,
            commands::export_formats,
        ])
        .run(tauri::generate_context!())
        .map_err(|err| std::io::Error::other(format!("tauri run failed: {err}")))?;
    Ok(())
}

fn run_cli(args: CliArgs) -> AppResult<()> {
    let _ = ort::init().with_name("snaptex-v2").commit();
    let mut recognizer = TrOCRRecognizer::from_model_dir(&args.model_dir)?;
    recognizer.warmup()?;

    let started = Instant::now();
    let result = recognizer.infer_image(&args.image)?;
    let total_ms = started.elapsed().as_millis();

    println!("{}", result.primary.latex);
    eprintln!(
        "latency_ms={} provider={} model_id={}",
        total_ms, result.provider, result.model_id
    );
    Ok(())
}

fn parse_cli_args() -> AppResult<Option<CliArgs>> {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    let has_cli_flags = raw_args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--image" | "--model-dir" | "--help" | "-h"));
    if !has_cli_flags {
        return Ok(None);
    }

    let mut image = None;
    let mut model_dir = None;
    let mut args = raw_args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--image" => image = args.next().map(PathBuf::from),
            "--model-dir" => model_dir = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            other => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown argument: {other}"),
                )
                .into())
            }
        }
    }

    Ok(Some(CliArgs {
        image: require_path(image, "--image")?,
        model_dir: require_path(model_dir, "--model-dir")?,
    }))
}

fn require_path(value: Option<PathBuf>, flag: &str) -> AppResult<PathBuf> {
    let path = value.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("missing required {flag}"))
    })?;
    if !Path::new(&path).exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("path does not exist: {}", path.display()),
        )
        .into());
    }
    Ok(path)
}

fn print_usage() {
    eprintln!("Usage: snaptex-v2 --image <path> --model-dir <path>");
}
