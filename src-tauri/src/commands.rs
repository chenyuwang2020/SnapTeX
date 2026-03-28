use serde::Serialize;

use crate::{
    app_state::{AppState, ModelStatus},
    capture::{CaptureBase64Result, HotkeyStatus, clipboard, hotkey, overlay},
    export::{ExportBundle, build_export_formats},
};

#[derive(Debug, Clone, Serialize)]
pub struct CandidateResponse {
    pub latex: String,
    pub score: f32,
    pub warnings: Vec<String>,
    pub token_probs: Vec<f32>,
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecognitionResponse {
    pub latex: String,
    pub score: f32,
    pub warnings: Vec<String>,
    pub token_probs: Vec<f32>,
    pub tokens: Vec<String>,
    pub alternatives: Vec<CandidateResponse>,
    pub latency_ms: u64,
    pub provider: String,
    pub model_id: String,
}

#[tauri::command]
pub fn capture_region() -> Result<CaptureBase64Result, String> {
    overlay::capture_region()
}

#[tauri::command]
pub fn read_clipboard_image() -> Result<Option<CaptureBase64Result>, String> {
    clipboard::read_clipboard_image()
}

#[tauri::command]
pub fn register_hotkey(app: tauri::AppHandle, shortcut: String) -> Result<HotkeyStatus, String> {
    hotkey::register_hotkey(app, shortcut)
}

#[tauri::command]
pub fn recognize_formula(
    state: tauri::State<'_, AppState>,
    image_b64: String,
    num_beams: Option<usize>,
) -> Result<RecognitionResponse, String> {
    let num_beams = num_beams.unwrap_or(3).clamp(1, 5);
    let result = state.recognize_base64(&image_b64, num_beams)?;
    Ok(RecognitionResponse {
        latex: result.primary.latex.clone(),
        score: result.primary.score,
        warnings: result.primary.warnings.clone(),
        token_probs: result.primary.token_probs.clone(),
        tokens: result.primary.tokens.clone(),
        alternatives: result
            .alternatives
            .into_iter()
            .map(|candidate| CandidateResponse {
                latex: candidate.latex,
                score: candidate.score,
                warnings: candidate.warnings,
                token_probs: candidate.token_probs,
                tokens: candidate.tokens,
            })
            .collect(),
        latency_ms: result.latency_ms,
        provider: result.provider,
        model_id: result.model_id,
    })
}

#[tauri::command]
pub fn get_model_status(state: tauri::State<'_, AppState>) -> Result<ModelStatus, String> {
    Ok(state.status())
}

#[tauri::command]
pub fn export_formats(latex: String) -> Result<ExportBundle, String> {
    eprintln!("command export_formats: latex={latex:?}");
    let bundle = build_export_formats(&latex);
    eprintln!(
        "command export_formats done: mathml={}, typst={}",
        bundle.mathml.as_ref().map(|value| value.len()).unwrap_or(0),
        bundle.typst.as_ref().map(|value| value.len()).unwrap_or(0)
    );
    Ok(bundle)
}
