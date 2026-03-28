use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::inference::recognizer::{RecognitionResult, Recognizer, TrOCRRecognizer};

#[derive(Clone)]
pub struct AppState {
    model: Arc<Mutex<ModelSlot>>,
}

enum ModelSlot {
    Loading,
    Ready(ModelRuntime),
    Failed(String),
}

struct ModelRuntime {
    recognizer: TrOCRRecognizer,
    model_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelStatus {
    pub status: String,
    pub model_id: Option<String>,
    pub model_dir: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SnaptexConfig {
    model_dir: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            model: Arc::new(Mutex::new(ModelSlot::Loading)),
        }
    }

    pub fn start_model_loader(&self, app: AppHandle) {
        let state = self.clone();
        thread::Builder::new()
            .name("snaptex-model-loader".to_string())
            .spawn(move || {
                eprintln!("model loader: started");
                let loaded = load_runtime();
                match loaded {
                    Ok(runtime) => {
                        let status = ModelStatus {
                            status: "ready".to_string(),
                            model_id: Some(runtime.recognizer.metadata().id.clone()),
                            model_dir: Some(runtime.model_dir.display().to_string()),
                            error: None,
                        };
                        if let Ok(mut guard) = state.model.lock() {
                            *guard = ModelSlot::Ready(runtime);
                        }
                        eprintln!(
                            "model loader: ready id={} dir={}",
                            status.model_id.clone().unwrap_or_default(),
                            status.model_dir.clone().unwrap_or_default()
                        );
                        let _ = app.emit("model-ready", status);
                    }
                    Err(err) => {
                        if let Ok(mut guard) = state.model.lock() {
                            *guard = ModelSlot::Failed(err.clone());
                        }
                        eprintln!("model loader: failed {err}");
                        let _ = app.emit(
                            "model-error",
                            ModelStatus {
                                status: "error".to_string(),
                                model_id: None,
                                model_dir: None,
                                error: Some(err),
                            },
                        );
                    }
                }
            })
            .expect("spawn model loader thread");
    }

    pub fn status(&self) -> ModelStatus {
        match self.model.lock() {
            Ok(guard) => match &*guard {
                ModelSlot::Loading => ModelStatus {
                    status: "loading".to_string(),
                    model_id: None,
                    model_dir: None,
                    error: None,
                },
                ModelSlot::Ready(runtime) => ModelStatus {
                    status: "ready".to_string(),
                    model_id: Some(runtime.recognizer.metadata().id.clone()),
                    model_dir: Some(runtime.model_dir.display().to_string()),
                    error: None,
                },
                ModelSlot::Failed(err) => ModelStatus {
                    status: "error".to_string(),
                    model_id: None,
                    model_dir: None,
                    error: Some(err.clone()),
                },
            },
            Err(_) => ModelStatus {
                status: "error".to_string(),
                model_id: None,
                model_dir: None,
                error: Some("model state lock poisoned".to_string()),
            },
        }
    }

    pub fn recognize_base64(&self, image_b64: &str, num_beams: usize) -> Result<RecognitionResult, String> {
        let image_bytes = decode_image_b64(image_b64)?;
        let mut model_guard = self
            .model
            .lock()
            .map_err(|_| "model state lock poisoned".to_string())?;

        let result = match &mut *model_guard {
            ModelSlot::Loading => Err("模型尚未就绪".to_string()),
            ModelSlot::Failed(err) => Err(format!("模型加载失败: {err}")),
            ModelSlot::Ready(runtime) => runtime
                .recognizer
                .infer_image_bytes(&image_bytes, num_beams)
                .map_err(|err| err.to_string()),
        }?;
        drop(model_guard);

        Ok(result)
    }
}

fn load_runtime() -> Result<ModelRuntime, String> {
    let model_dir = resolve_model_dir()?;
    let mut recognizer =
        TrOCRRecognizer::from_model_dir(&model_dir).map_err(|err| err.to_string())?;
    recognizer.warmup().map_err(|err| err.to_string())?;
    Ok(ModelRuntime {
        recognizer,
        model_dir,
    })
}

fn decode_image_b64(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    let payload = trimmed
        .split_once(',')
        .map(|(_, b64)| b64)
        .unwrap_or(trimmed);
    STANDARD
        .decode(payload)
        .map_err(|err| format!("decode image base64 failed: {err}"))
}

fn resolve_model_dir() -> Result<PathBuf, String> {
    if let Ok(value) = env::var("SNAPTEX_MODEL_DIR") {
        let path = PathBuf::from(value.trim());
        if is_valid_model_dir(&path) {
            return Ok(path);
        }
    }

    for base in candidate_bases() {
        if let Some(configured) = read_model_dir_from_config(&base) {
            if is_valid_model_dir(&configured) {
                return Ok(configured);
            }
        }

        let candidate = base.join("models").join("mfr-1.5-int8");
        if is_valid_model_dir(&candidate) {
            return Ok(candidate);
        }
    }

    Err("未找到 models/mfr-1.5-int8 模型目录；可通过环境变量 SNAPTEX_MODEL_DIR 或 snaptex.config.json 指定".to_string())
}

fn candidate_bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let resources_sibling = exe_dir.join("resources");
            if resources_sibling.is_dir() {
                bases.push(resources_sibling);
            }
            if let Some(parent) = exe_dir.parent() {
                let resources_parent = parent.join("resources");
                if resources_parent.is_dir() {
                    bases.push(resources_parent);
                }
            }
        }
    }
    if let Ok(cwd) = env::current_dir() {
        push_with_ancestors(&mut bases, cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            push_with_ancestors(&mut bases, parent.to_path_buf());
        }
    }
    bases
}

fn push_with_ancestors(bases: &mut Vec<PathBuf>, start: PathBuf) {
    let mut current = Some(start);
    while let Some(path) = current {
        if !bases.iter().any(|existing| existing == &path) {
            bases.push(path.clone());
        }
        current = path.parent().map(|parent| parent.to_path_buf());
    }
}

fn read_model_dir_from_config(base: &Path) -> Option<PathBuf> {
    for file_name in ["snaptex.config.json", "snaptex-v2.config.json"] {
        let path = base.join(file_name);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path).ok()?;
        let config = serde_json::from_str::<SnaptexConfig>(&text).ok()?;
        let raw_dir = config.model_dir?;
        let configured = PathBuf::from(raw_dir.trim());
        let resolved = if configured.is_absolute() {
            configured
        } else {
            base.join(configured)
        };
        return Some(resolved);
    }
    None
}

fn is_valid_model_dir(path: &Path) -> bool {
    path.join("model_manifest.json").exists()
}
