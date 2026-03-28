pub mod clipboard;
pub mod hotkey;
pub mod overlay;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CaptureBase64Result {
    pub image_b64: String,
    pub width: i32,
    pub height: i32,
    pub format: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotkeyStatus {
    pub registered: bool,
    pub shortcut: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotkeyTriggeredEvent {
    pub shortcut: String,
}
