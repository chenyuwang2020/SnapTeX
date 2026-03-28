use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    thread,
    time::Duration,
};

use arboard::{Clipboard, Error as ClipboardError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use tauri::{AppHandle, Emitter};

use super::CaptureBase64Result;

const CLIPBOARD_POLL_INTERVAL_MS: u64 = 500;

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber;

pub fn start_monitor(app: AppHandle) -> Result<(), String> {
    thread::Builder::new()
        .name("snaptex-clipboard".to_string())
        .spawn(move || {
            let mut last_hash = None;
            let mut last_sequence = 0u32;
            eprintln!("clipboard monitor started");
            loop {
                let sequence = clipboard_sequence_number();
                match read_clipboard_image_with_hash() {
                    Ok(Some((hash, payload))) => {
                        eprintln!(
                            "clipboard poll: seq={} has_image=true width={} height={}",
                            sequence, payload.width, payload.height
                        );
                        let is_new = sequence != last_sequence || last_hash != Some(hash);
                        if is_new {
                            eprintln!("clipboard: new image detected, emitting event");
                            last_sequence = sequence;
                            last_hash = Some(hash);
                            let _ = app.emit("clipboard-image", payload);
                        }
                    }
                    Ok(None) => {
                        eprintln!("clipboard poll: seq={} has_image=false", sequence);
                        last_sequence = sequence;
                        last_hash = None;
                    }
                    Err(err) => {
                        eprintln!("clipboard poll error: {err}");
                    }
                }
                thread::sleep(Duration::from_millis(CLIPBOARD_POLL_INTERVAL_MS));
            }
        })
        .map(|_| ())
        .map_err(|err| format!("start clipboard monitor failed: {err}"))
}

pub fn read_clipboard_image() -> Result<Option<CaptureBase64Result>, String> {
    read_clipboard_image_with_hash().map(|payload| payload.map(|(_, image)| image))
}

fn read_clipboard_image_with_hash() -> Result<Option<(u64, CaptureBase64Result)>, String> {
    let mut clipboard = Clipboard::new().map_err(|err| format!("init clipboard failed: {err}"))?;
    let image = match clipboard.get_image() {
        Ok(image) => image,
        Err(ClipboardError::ContentNotAvailable) => return Ok(None),
        Err(err) => return Err(format!("read clipboard image failed: {err}")),
    };

    let width = i32::try_from(image.width).map_err(|_| "clipboard image width overflow".to_string())?;
    let height =
        i32::try_from(image.height).map_err(|_| "clipboard image height overflow".to_string())?;
    let rgba = image.bytes.into_owned();
    let hash = hash_image_payload(width, height, &rgba);
    let image_b64 = encode_png_base64(width, height, &rgba)?;

    Ok(Some((
        hash,
        CaptureBase64Result {
            image_b64,
            width,
            height,
            format: "png".to_string(),
        },
    )))
}

fn hash_image_payload(width: i32, height: i32, pixels: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    pixels.hash(&mut hasher);
    hasher.finish()
}

fn encode_png_base64(width: i32, height: i32, rgba: &[u8]) -> Result<String, String> {
    if width <= 0 || height <= 0 {
        return Err("clipboard image has invalid dimensions".to_string());
    }

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if rgba.len() < expected {
        return Err("clipboard image buffer too short".to_string());
    }

    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(
            &rgba[..expected],
            width as u32,
            height as u32,
            ExtendedColorType::Rgba8,
        )
        .map_err(|err| format!("encode clipboard png failed: {err}"))?;
    Ok(STANDARD.encode(encoded))
}

fn clipboard_sequence_number() -> u32 {
    #[cfg(target_os = "windows")]
    unsafe {
        GetClipboardSequenceNumber()
    }

    #[cfg(not(target_os = "windows"))]
    {
        0
    }
}
