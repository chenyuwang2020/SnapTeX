#![allow(unsafe_op_in_unsafe_fn)]

use std::{
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
    thread,
    time::Duration,
};

use tauri::{AppHandle, Emitter};

use super::{HotkeyStatus, HotkeyTriggeredEvent};

const HOTKEY_ID: i32 = 0x4C53;
const HOTKEY_MSG_RELOAD: u32 = 0x8000 + 71;
const HOTKEY_MSG_STOP: u32 = 0x8000 + 72;
const HK_MOD_ALT: u32 = 0x0001;
const HK_MOD_CONTROL: u32 = 0x0002;
const HK_MOD_SHIFT: u32 = 0x0004;
const HK_MOD_WIN: u32 = 0x0008;
const HK_MOD_NOREPEAT: u32 = 0x4000;

#[derive(Debug, Clone)]
struct HotkeySpec {
    modifiers: u32,
    vk: u32,
    label: String,
}

static HOTKEY_THREAD_ID: AtomicU32 = AtomicU32::new(0);
static HOTKEY_PENDING_SPEC: OnceLock<Mutex<Option<HotkeySpec>>> = OnceLock::new();
static HOTKEY_STARTED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::GetLastError,
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey},
        WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
            TranslateMessage, WM_HOTKEY,
        },
    },
};

fn pending_spec_slot() -> &'static Mutex<Option<HotkeySpec>> {
    HOTKEY_PENDING_SPEC.get_or_init(|| Mutex::new(None))
}

fn hotkey_log(message: impl AsRef<str>) {
    eprintln!("{}", message.as_ref());
}

pub fn start_hotkey_thread(app: AppHandle, default_shortcut: &str) -> Result<(), String> {
    let default_spec = parse_hotkey_shortcut(default_shortcut)?;
    if let Ok(mut guard) = pending_spec_slot().lock() {
        *guard = Some(default_spec);
    }

    if HOTKEY_STARTED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        thread::Builder::new()
            .name("snaptex-hotkey".to_string())
            .spawn(move || unsafe {
                let mut msg: MSG = std::mem::zeroed();
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
                let tid = GetCurrentThreadId();
                HOTKEY_THREAD_ID.store(tid, Ordering::SeqCst);
                hotkey_log(format!("hotkey thread started: tid={tid}"));

                let mut active: Option<HotkeySpec> = None;
                if let Ok(mut guard) = pending_spec_slot().lock() {
                    if let Some(spec) = guard.take() {
                        let ok = apply_hotkey_on_thread(&spec, &mut active);
                        if ok {
                            hotkey_log(format!("hotkey registered: {}", spec.label));
                        } else {
                            hotkey_log(format!("hotkey registration failed: {}", spec.label));
                        }
                    }
                }

                loop {
                    let ret = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                    if ret <= 0 {
                        hotkey_log(format!("hotkey thread exiting: GetMessageW={ret}"));
                        break;
                    }
                    match msg.message {
                        WM_HOTKEY => {
                            if msg.wParam as i32 == HOTKEY_ID {
                                let label = active
                                    .as_ref()
                                    .map(|spec| spec.label.clone())
                                    .unwrap_or_default();
                                hotkey_log(format!("hotkey triggered: {label}"));
                                let _ = app.emit(
                                    "global-hotkey-triggered",
                                    HotkeyTriggeredEvent { shortcut: label },
                                );
                            }
                        }
                        HOTKEY_MSG_RELOAD => {
                            if let Ok(mut guard) = pending_spec_slot().lock() {
                                if let Some(spec) = guard.take() {
                                    let ok = apply_hotkey_on_thread(&spec, &mut active);
                                    if ok {
                                        hotkey_log(format!("hotkey registered: {}", spec.label));
                                    } else {
                                        hotkey_log(format!("hotkey registration failed: {}", spec.label));
                                    }
                                }
                            }
                        }
                        HOTKEY_MSG_STOP => {
                            hotkey_log("hotkey thread stop requested");
                            break;
                        }
                        _ => {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                }

                if active.is_some() {
                    let _ = UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
                }
                HOTKEY_THREAD_ID.store(0, Ordering::SeqCst);
                HOTKEY_STARTED.store(false, Ordering::SeqCst);
                hotkey_log("hotkey thread stopped");
            })
            .map(|_| ())
            .map_err(|err| format!("start hotkey thread failed: {err}"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        HOTKEY_STARTED.store(false, Ordering::SeqCst);
        Err("global hotkey currently supported on Windows only".to_string())
    }
}

pub fn register_hotkey(app: AppHandle, shortcut: String) -> Result<HotkeyStatus, String> {
    let spec = parse_hotkey_shortcut(&shortcut)?;
    let label = spec.label.clone();

    #[cfg(target_os = "windows")]
    {
        if HOTKEY_THREAD_ID.load(Ordering::SeqCst) == 0 {
            hotkey_log("register_hotkey: thread not running yet, starting it now");
            start_hotkey_thread(app, &shortcut)?;
            wait_for_hotkey_thread_ready()?;
        }

        if let Ok(mut guard) = pending_spec_slot().lock() {
            *guard = Some(spec);
        }
        post_hotkey_thread_message(HOTKEY_MSG_RELOAD)?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        return Err("global hotkey currently supported on Windows only".to_string());
    }

    Ok(HotkeyStatus {
        registered: true,
        shortcut: label,
    })
}

#[cfg(target_os = "windows")]
fn wait_for_hotkey_thread_ready() -> Result<(), String> {
    for _ in 0..20 {
        if HOTKEY_THREAD_ID.load(Ordering::SeqCst) != 0 {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(15));
    }
    Err("hotkey thread did not become ready in time".to_string())
}

#[cfg(target_os = "windows")]
unsafe fn apply_hotkey_on_thread(spec: &HotkeySpec, active: &mut Option<HotkeySpec>) -> bool {
    if active.is_some() {
        let _ = UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID);
    }
    let ok = RegisterHotKey(
        std::ptr::null_mut(),
        HOTKEY_ID,
        spec.modifiers as HOT_KEY_MODIFIERS,
        spec.vk,
    );
    if ok != 0 {
        *active = Some(spec.clone());
        true
    } else {
        let err = GetLastError();
        hotkey_log(format!(
            "RegisterHotKey failed: shortcut={} modifiers={} vk={} error={err}",
            spec.label, spec.modifiers, spec.vk
        ));
        *active = None;
        false
    }
}

#[cfg(target_os = "windows")]
fn post_hotkey_thread_message(message: u32) -> Result<(), String> {
    let tid = HOTKEY_THREAD_ID.load(Ordering::SeqCst);
    if tid == 0 {
        return Err("hotkey thread not running".to_string());
    }

    unsafe {
        if PostThreadMessageW(tid, message, 0, 0) == 0 {
            return Err("post hotkey thread message failed".to_string());
        }
    }
    Ok(())
}

fn parse_hotkey_shortcut(shortcut: &str) -> Result<HotkeySpec, String> {
    let raw = shortcut.trim();
    if raw.is_empty() {
        return Err("hotkey empty".to_string());
    }

    let mut modifiers = HK_MOD_NOREPEAT;
    let mut vk = None;
    let mut normalized = Vec::new();
    let parts = raw
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err("hotkey invalid".to_string());
    }

    for part in parts {
        let token = part.to_ascii_uppercase();
        match token.as_str() {
            "CTRL" | "CONTROL" => {
                modifiers |= HK_MOD_CONTROL;
                if !normalized.iter().any(|item| item == "Ctrl") {
                    normalized.push("Ctrl".to_string());
                }
            }
            "SHIFT" => {
                modifiers |= HK_MOD_SHIFT;
                if !normalized.iter().any(|item| item == "Shift") {
                    normalized.push("Shift".to_string());
                }
            }
            "ALT" => {
                modifiers |= HK_MOD_ALT;
                if !normalized.iter().any(|item| item == "Alt") {
                    normalized.push("Alt".to_string());
                }
            }
            "WIN" | "META" | "CMD" | "SUPER" => {
                modifiers |= HK_MOD_WIN;
                if !normalized.iter().any(|item| item == "Win") {
                    normalized.push("Win".to_string());
                }
            }
            _ => {
                if vk.is_some() {
                    return Err(format!("hotkey invalid: multiple key parts ({part})"));
                }
                vk = parse_vk(&token);
                if vk.is_none() {
                    return Err(format!("hotkey invalid key: {part}"));
                }
                normalized.push(part.to_ascii_uppercase());
            }
        }
    }

    let vk = vk.ok_or_else(|| "hotkey missing key".to_string())?;
    if modifiers == HK_MOD_NOREPEAT {
        return Err("hotkey requires at least one modifier (Ctrl/Alt/Shift/Win)".to_string());
    }

    Ok(HotkeySpec {
        modifiers,
        vk,
        label: normalized.join("+"),
    })
}

fn parse_vk(token: &str) -> Option<u32> {
    if token.len() == 1 {
        let ch = token.chars().next()?;
        if ch.is_ascii_alphabetic() || ch.is_ascii_digit() {
            return Some(ch as u32);
        }
    }

    if let Some(number) = token.strip_prefix('F') {
        if let Ok(value) = number.parse::<u32>() {
            if (1..=24).contains(&value) {
                return Some(0x70 + (value - 1));
            }
        }
    }

    match token {
        "SPACE" => Some(0x20),
        "TAB" => Some(0x09),
        "ENTER" | "RETURN" => Some(0x0D),
        "ESC" | "ESCAPE" => Some(0x1B),
        "UP" => Some(0x26),
        "DOWN" => Some(0x28),
        "LEFT" => Some(0x25),
        "RIGHT" => Some(0x27),
        _ => None,
    }
}
