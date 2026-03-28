#![allow(unsafe_op_in_unsafe_fn)]

use std::{
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};

use super::CaptureBase64Result;

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, BitBlt, CreateCompatibleBitmap,
        CreateCompatibleDC, CreatePen, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint, GetDC,
        GetDIBits, GetStockObject, HBITMAP, HDC, HGDIOBJ, HOLLOW_BRUSH, IntersectClipRect,
        InvalidateRect, LineTo, MoveToEx, PAINTSTRUCT, PS_SOLID, ReleaseDC, RestoreDC, SRCCOPY,
        SaveDC, ScreenToClient, SelectObject, SetBkMode, SetTextColor, StretchDIBits,
        TRANSPARENT, TextOutW, UpdateWindow,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{
            ReleaseCapture, SetCapture, SetFocus, VK_ESCAPE,
        },
        WindowsAndMessaging::{
            CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
            DispatchMessageW, GWLP_USERDATA, GetCursorPos, GetSystemMetrics, GetWindowLongPtrW,
            IsWindow, MSG, PM_REMOVE, PeekMessageW, RegisterClassW, SM_CXSCREEN,
            SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN, SW_SHOW, SetCursor, SetForegroundWindow, SetWindowLongPtrW, ShowWindow,
            TranslateMessage, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_PAINT, WM_QUIT, WM_RBUTTONDOWN,
            WM_SETCURSOR, WM_SIZE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

#[cfg(target_os = "windows")]
const NATIVE_CAPTURE_OVERLAY_CLASS: &str = "SnapTeXNativeCaptureOverlay";

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct CaptureOverlaySelection {
    x_ratio: f64,
    y_ratio: f64,
    width_ratio: f64,
    height_ratio: f64,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct ScreenCaptureRaw {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
struct NativeCaptureOverlayContext {
    image_width: i32,
    image_height: i32,
    view_width: i32,
    view_height: i32,
    pixels_ptr: *const u8,
    pixels_len: usize,
    dark_pixels_ptr: *const u8,
    dark_pixels_len: usize,
    dragging: bool,
    start_x: i32,
    start_y: i32,
    cur_x: i32,
    cur_y: i32,
    has_cursor: bool,
    result: Option<CaptureOverlaySelection>,
    cancelled: bool,
    done: bool,
}

pub fn capture_region() -> Result<CaptureBase64Result, String> {
    #[cfg(target_os = "windows")]
    {
        let capture = capture_region_with_native_overlay_raw()?;
        let rgba = bgra_to_rgba_opaque(&capture.pixels);
        let image_b64 = encode_png_base64(capture.width, capture.height, &rgba)?;
        return Ok(CaptureBase64Result {
            image_b64,
            width: capture.width,
            height: capture.height,
            format: "png".to_string(),
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("capture overlay currently supported on Windows only".to_string())
    }
}

#[cfg(target_os = "windows")]
fn capture_region_with_native_overlay_raw() -> Result<ScreenCaptureRaw, String> {
    let raw = capture_virtual_screen_raw()?;
    let selection = run_native_capture_overlay_selection(&raw)?;
    crop_capture_raw_by_ratio(&raw, &selection)
}

#[cfg(target_os = "windows")]
fn capture_virtual_screen_raw() -> Result<ScreenCaptureRaw, String> {
    let (left, top, width, height) = virtual_screen_bounds()?;
    unsafe {
        let hdc_screen: HDC = GetDC(std::ptr::null_mut());
        if hdc_screen.is_null() {
            return Err("GetDC failed".to_string());
        }

        let hdc_mem: HDC = CreateCompatibleDC(hdc_screen);
        if hdc_mem.is_null() {
            let _ = ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return Err("CreateCompatibleDC failed".to_string());
        }

        let hbitmap: HBITMAP = CreateCompatibleBitmap(hdc_screen, width, height);
        if hbitmap.is_null() {
            let _ = DeleteDC(hdc_mem);
            let _ = ReleaseDC(std::ptr::null_mut(), hdc_screen);
            return Err("CreateCompatibleBitmap failed".to_string());
        }

        let old_obj: HGDIOBJ = SelectObject(hdc_mem, hbitmap as HGDIOBJ);
        let result = if BitBlt(hdc_mem, 0, 0, width, height, hdc_screen, left, top, SRCCOPY) != 0
        {
            let mut bitmap_info: BITMAPINFO = std::mem::zeroed();
            bitmap_info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (width as u32)
                    .saturating_mul(height as u32)
                    .saturating_mul(4),
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let byte_len = (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4);
            let mut pixels = vec![0u8; byte_len];
            let lines = GetDIBits(
                hdc_mem,
                hbitmap,
                0,
                height as u32,
                pixels.as_mut_ptr() as *mut core::ffi::c_void,
                &mut bitmap_info,
                DIB_RGB_COLORS,
            );
            if lines > 0 {
                Ok(ScreenCaptureRaw {
                    left,
                    top,
                    width,
                    height,
                    pixels,
                })
            } else {
                Err("GetDIBits failed".to_string())
            }
        } else {
            Err("BitBlt failed".to_string())
        };

        if !old_obj.is_null() {
            let _ = SelectObject(hdc_mem, old_obj);
        }
        let _ = DeleteObject(hbitmap as HGDIOBJ);
        let _ = DeleteDC(hdc_mem);
        let _ = ReleaseDC(std::ptr::null_mut(), hdc_screen);
        result
    }
}

#[cfg(target_os = "windows")]
fn run_native_capture_overlay_selection(raw: &ScreenCaptureRaw) -> Result<CaptureOverlaySelection, String> {
    if raw.width <= 0 || raw.height <= 0 {
        return Err("invalid screen size for overlay".to_string());
    }

    let mut dark_pixels = raw.pixels.clone();
    for pixel in dark_pixels.chunks_exact_mut(4) {
        pixel[0] = ((pixel[0] as u16 * 38) / 100) as u8;
        pixel[1] = ((pixel[1] as u16 * 38) / 100) as u8;
        pixel[2] = ((pixel[2] as u16 * 38) / 100) as u8;
        pixel[3] = 255;
    }

    unsafe {
        let class_name = to_wide_null(NATIVE_CAPTURE_OVERLAY_CLASS);
        let title = to_wide_null("SnapTeX Capture Overlay");
        let hinstance = GetModuleHandleW(std::ptr::null());

        let mut window_class: WNDCLASSW = std::mem::zeroed();
        window_class.style = CS_HREDRAW | CS_VREDRAW;
        window_class.lpfnWndProc = Some(native_capture_overlay_wndproc);
        window_class.hInstance = hinstance;
        window_class.hCursor = std::ptr::null_mut();
        window_class.lpszClassName = class_name.as_ptr();

        let reg = RegisterClassW(&window_class);
        if reg == 0 {
            let err = GetLastError();
            if err != ERROR_CLASS_ALREADY_EXISTS {
                return Err(format!("register native overlay class failed: {err}"));
            }
        }

        let ctx_ptr = Box::into_raw(Box::new(NativeCaptureOverlayContext {
            image_width: raw.width,
            image_height: raw.height,
            view_width: raw.width,
            view_height: raw.height,
            pixels_ptr: raw.pixels.as_ptr(),
            pixels_len: raw.pixels.len(),
            dark_pixels_ptr: dark_pixels.as_ptr(),
            dark_pixels_len: dark_pixels.len(),
            dragging: false,
            start_x: 0,
            start_y: 0,
            cur_x: 0,
            cur_y: 0,
            has_cursor: false,
            result: None,
            cancelled: false,
            done: false,
        }));

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            raw.left,
            raw.top,
            raw.width,
            raw.height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            ctx_ptr as *mut core::ffi::c_void,
        );
        if hwnd.is_null() {
            let _ = Box::from_raw(ctx_ptr);
            return Err("create native capture overlay window failed".to_string());
        }

        let mut point: POINT = std::mem::zeroed();
        if GetCursorPos(&mut point) != 0 {
            let _ = ScreenToClient(hwnd, &mut point);
            let ctx = &mut *ctx_ptr;
            ctx.cur_x = clamp_client_pos(point.x, ctx.view_width);
            ctx.cur_y = clamp_client_pos(point.y, ctx.view_height);
            ctx.has_cursor = true;
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        InvalidateRect(hwnd, std::ptr::null(), 0);

        let started = Instant::now();
        loop {
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    (*ctx_ptr).done = true;
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            if (*ctx_ptr).done {
                break;
            }
            if started.elapsed() > Duration::from_secs(120) {
                (*ctx_ptr).cancelled = true;
                (*ctx_ptr).done = true;
                break;
            }
            thread::sleep(Duration::from_millis(8));
        }

        if IsWindow(hwnd) != 0 {
            DestroyWindow(hwnd);
        }
        let ctx = Box::from_raw(ctx_ptr);
        if let Some(selection) = ctx.result {
            return Ok(selection);
        }
        if ctx.cancelled {
            return Err("region capture cancelled".to_string());
        }
        Err("region capture failed".to_string())
    }
}

#[cfg(target_os = "windows")]
fn crop_capture_raw_by_ratio(
    raw: &ScreenCaptureRaw,
    selection: &CaptureOverlaySelection,
) -> Result<ScreenCaptureRaw, String> {
    if raw.width <= 0 || raw.height <= 0 {
        return Err("invalid raw capture size".to_string());
    }

    let mut x0 = (clamp_ratio(selection.x_ratio) * raw.width as f64).floor() as i32;
    let mut y0 = (clamp_ratio(selection.y_ratio) * raw.height as f64).floor() as i32;
    let mut x1 =
        (clamp_ratio(selection.x_ratio + selection.width_ratio) * raw.width as f64).ceil() as i32;
    let mut y1 = (clamp_ratio(selection.y_ratio + selection.height_ratio) * raw.height as f64)
        .ceil() as i32;

    x0 = x0.clamp(0, raw.width.saturating_sub(1).max(0));
    y0 = y0.clamp(0, raw.height.saturating_sub(1).max(0));
    x1 = x1.clamp(x0 + 1, raw.width.max(1));
    y1 = y1.clamp(y0 + 1, raw.height.max(1));

    let crop_w = x1 - x0;
    let crop_h = y1 - y0;
    if crop_w <= 0 || crop_h <= 0 {
        return Err("invalid capture region".to_string());
    }

    let src_stride = (raw.width as usize).saturating_mul(4);
    let row_bytes = (crop_w as usize).saturating_mul(4);
    let mut out = vec![0u8; row_bytes.saturating_mul(crop_h as usize)];
    for row in 0..crop_h as usize {
        let src_y = y0 as usize + row;
        let src_start = src_y
            .saturating_mul(src_stride)
            .saturating_add((x0 as usize).saturating_mul(4));
        let src_end = src_start.saturating_add(row_bytes);
        let dst_start = row.saturating_mul(row_bytes);
        let dst_end = dst_start.saturating_add(row_bytes);
        if src_end > raw.pixels.len() || dst_end > out.len() {
            return Err("capture crop buffer overflow".to_string());
        }
        out[dst_start..dst_end].copy_from_slice(&raw.pixels[src_start..src_end]);
    }

    Ok(ScreenCaptureRaw {
        left: raw.left + x0,
        top: raw.top + y0,
        width: crop_w,
        height: crop_h,
        pixels: out,
    })
}

#[cfg(target_os = "windows")]
fn virtual_screen_bounds() -> Result<(i32, i32, i32, i32), String> {
    unsafe {
        let left = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let top = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let mut width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let mut height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if width <= 0 || height <= 0 {
            width = GetSystemMetrics(SM_CXSCREEN);
            height = GetSystemMetrics(SM_CYSCREEN);
        }
        if width <= 0 || height <= 0 {
            return Err("invalid screen size".to_string());
        }
        Ok((left, top, width, height))
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn native_capture_overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_NCCREATE => {
            let cs = lparam as *const CREATESTRUCTW;
            if cs.is_null() {
                return 0;
            }
            let ctx_ptr = (*cs).lpCreateParams as *mut NativeCaptureOverlayContext;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx_ptr as isize);
            return 1;
        }
        WM_ERASEBKGND => return 1,
        WM_SETCURSOR => {
            SetCursor(std::ptr::null_mut());
            return 1;
        }
        WM_SIZE => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                let width = (lparam as u32 & 0xffff) as i32;
                let height = ((lparam as u32 >> 16) & 0xffff) as i32;
                if width > 0 {
                    ctx.view_width = width;
                }
                if height > 0 {
                    ctx.view_height = height;
                }
                ctx.cur_x = clamp_client_pos(ctx.cur_x, ctx.view_width);
                ctx.cur_y = clamp_client_pos(ctx.cur_y, ctx.view_height);
            }
            return 0;
        }
        WM_KEYDOWN => {
            if (wparam as u16) == VK_ESCAPE {
                if let Some(ctx) = overlay_ctx_mut(hwnd) {
                    ctx.cancelled = true;
                    ctx.done = true;
                }
                DestroyWindow(hwnd);
                return 0;
            }
        }
        WM_RBUTTONDOWN => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                ctx.cancelled = true;
                ctx.done = true;
            }
            DestroyWindow(hwnd);
            return 0;
        }
        WM_LBUTTONDOWN => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                let x = clamp_client_pos(lparam_x(lparam), ctx.view_width);
                let y = clamp_client_pos(lparam_y(lparam), ctx.view_height);
                ctx.dragging = true;
                ctx.start_x = x;
                ctx.start_y = y;
                ctx.cur_x = x;
                ctx.cur_y = y;
                ctx.has_cursor = true;
                SetCapture(hwnd);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_MOUSEMOVE => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                ctx.cur_x = clamp_client_pos(lparam_x(lparam), ctx.view_width);
                ctx.cur_y = clamp_client_pos(lparam_y(lparam), ctx.view_height);
                ctx.has_cursor = true;
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            return 0;
        }
        WM_LBUTTONUP => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                ctx.cur_x = clamp_client_pos(lparam_x(lparam), ctx.view_width);
                ctx.cur_y = clamp_client_pos(lparam_y(lparam), ctx.view_height);
                ctx.has_cursor = true;
                if ctx.dragging {
                    ctx.dragging = false;
                    ReleaseCapture();

                    let left = ctx.start_x.min(ctx.cur_x);
                    let top = ctx.start_y.min(ctx.cur_y);
                    let width = (ctx.cur_x - ctx.start_x).abs();
                    let height = (ctx.cur_y - ctx.start_y).abs();
                    if width >= 2 && height >= 2 && ctx.view_width > 0 && ctx.view_height > 0 {
                        ctx.result = Some(CaptureOverlaySelection {
                            x_ratio: clamp_ratio(left as f64 / ctx.view_width as f64),
                            y_ratio: clamp_ratio(top as f64 / ctx.view_height as f64),
                            width_ratio: clamp_ratio(width as f64 / ctx.view_width as f64),
                            height_ratio: clamp_ratio(height as f64 / ctx.view_height as f64),
                        });
                        ctx.cancelled = false;
                    } else {
                        ctx.cancelled = true;
                    }
                    ctx.done = true;
                    DestroyWindow(hwnd);
                }
            }
            return 0;
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                if let Some(ctx) = overlay_ctx_mut(hwnd) {
                    paint_overlay(hdc, ctx);
                }
            }
            EndPaint(hwnd, &ps);
            return 0;
        }
        WM_DESTROY => {
            if let Some(ctx) = overlay_ctx_mut(hwnd) {
                if !ctx.done {
                    ctx.cancelled = true;
                    ctx.done = true;
                }
            }
            return 0;
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[cfg(target_os = "windows")]
unsafe fn paint_overlay(hdc: HDC, ctx: &mut NativeCaptureOverlayContext) {
    if !ctx.dark_pixels_ptr.is_null() && ctx.image_width > 0 && ctx.image_height > 0 {
        let expected = (ctx.image_width as usize)
            .saturating_mul(ctx.image_height as usize)
            .saturating_mul(4);
        if ctx.dark_pixels_len >= expected {
            let mut bitmap_info: BITMAPINFO = std::mem::zeroed();
            bitmap_info.bmiHeader = BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: ctx.image_width,
                biHeight: -ctx.image_height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                biSizeImage: (ctx.image_width as u32)
                    .saturating_mul(ctx.image_height as u32)
                    .saturating_mul(4),
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };

            let _ = StretchDIBits(
                hdc,
                0,
                0,
                ctx.view_width,
                ctx.view_height,
                0,
                0,
                ctx.image_width,
                ctx.image_height,
                ctx.dark_pixels_ptr as *const core::ffi::c_void,
                &bitmap_info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );

            if ctx.dragging && !ctx.pixels_ptr.is_null() && ctx.pixels_len >= expected {
                let left = ctx.start_x.min(ctx.cur_x);
                let top = ctx.start_y.min(ctx.cur_y);
                let right = ctx.start_x.max(ctx.cur_x);
                let bottom = ctx.start_y.max(ctx.cur_y);
                let width = (right - left).max(0);
                let height = (bottom - top).max(0);
                if width > 0 && height > 0 && ctx.view_width > 0 && ctx.view_height > 0 {
                    let saved = SaveDC(hdc);
                    if saved > 0 {
                        let _ = IntersectClipRect(hdc, left, top, right, bottom);
                        let _ = StretchDIBits(
                            hdc,
                            0,
                            0,
                            ctx.view_width,
                            ctx.view_height,
                            0,
                            0,
                            ctx.image_width,
                            ctx.image_height,
                            ctx.pixels_ptr as *const core::ffi::c_void,
                            &bitmap_info,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                        let _ = RestoreDC(hdc, saved);
                    }
                }
            }
        }
    }

    if ctx.has_cursor {
        let arm = 14;
        let x0 = (ctx.cur_x - arm).max(0);
        let x1 = (ctx.cur_x + arm).min(ctx.view_width.saturating_sub(1));
        let y0 = (ctx.cur_y - arm).max(0);
        let y1 = (ctx.cur_y + arm).min(ctx.view_height.saturating_sub(1));
        draw_line_cross(hdc, x0, x1, y0, y1, ctx.cur_x, ctx.cur_y);
    }

    if ctx.dragging {
        let left = ctx.start_x.min(ctx.cur_x);
        let top = ctx.start_y.min(ctx.cur_y);
        let right = ctx.start_x.max(ctx.cur_x);
        let bottom = ctx.start_y.max(ctx.cur_y);
        let width = right - left;
        let height = bottom - top;
        let _ = SelectObject(hdc, GetStockObject(HOLLOW_BRUSH) as HGDIOBJ);

        draw_rect_outline(hdc, left, top, right, bottom);

        if width > 0 && height > 0 {
            let text = format!("{width} x {height} | ({left}, {top})");
            let text_w = to_wide_null(&text);
            let tx = (left + 8).clamp(6, (ctx.view_width - 240).max(6));
            let mut ty = if top > 30 { top - 24 } else { bottom + 8 };
            ty = ty.clamp(6, (ctx.view_height - 24).max(6));
            let _ = SetBkMode(hdc, TRANSPARENT as i32);
            let _ = SetTextColor(hdc, 0x00FFFFFF);
            let _ = TextOutW(
                hdc,
                tx,
                ty,
                text_w.as_ptr(),
                (text_w.len().saturating_sub(1)) as i32,
            );
        }
    }
}

#[cfg(target_os = "windows")]
unsafe fn draw_line_cross(hdc: HDC, x0: i32, x1: i32, y0: i32, y1: i32, x: i32, y: i32) {
    let pen_black = CreatePen(PS_SOLID, 3, 0x000000);
    if !pen_black.is_null() {
        let old = SelectObject(hdc, pen_black as HGDIOBJ);
        let _ = MoveToEx(hdc, x0, y, std::ptr::null_mut());
        let _ = LineTo(hdc, x1, y);
        let _ = MoveToEx(hdc, x, y0, std::ptr::null_mut());
        let _ = LineTo(hdc, x, y1);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen_black as HGDIOBJ);
    }

    let pen_white = CreatePen(PS_SOLID, 1, 0x00FFFFFF);
    if !pen_white.is_null() {
        let old = SelectObject(hdc, pen_white as HGDIOBJ);
        let _ = MoveToEx(hdc, x0, y, std::ptr::null_mut());
        let _ = LineTo(hdc, x1, y);
        let _ = MoveToEx(hdc, x, y0, std::ptr::null_mut());
        let _ = LineTo(hdc, x, y1);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen_white as HGDIOBJ);
    }
}

#[cfg(target_os = "windows")]
unsafe fn draw_rect_outline(hdc: HDC, left: i32, top: i32, right: i32, bottom: i32) {
    let pen_black = CreatePen(PS_SOLID, 3, 0x000000);
    if !pen_black.is_null() {
        let old = SelectObject(hdc, pen_black as HGDIOBJ);
        let _ = MoveToEx(hdc, left, top, std::ptr::null_mut());
        let _ = LineTo(hdc, right, top);
        let _ = LineTo(hdc, right, bottom);
        let _ = LineTo(hdc, left, bottom);
        let _ = LineTo(hdc, left, top);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen_black as HGDIOBJ);
    }

    let pen_white = CreatePen(PS_SOLID, 1, 0x00FFFFFF);
    if !pen_white.is_null() {
        let old = SelectObject(hdc, pen_white as HGDIOBJ);
        let _ = MoveToEx(hdc, left, top, std::ptr::null_mut());
        let _ = LineTo(hdc, right, top);
        let _ = LineTo(hdc, right, bottom);
        let _ = LineTo(hdc, left, bottom);
        let _ = LineTo(hdc, left, top);
        let _ = SelectObject(hdc, old);
        let _ = DeleteObject(pen_white as HGDIOBJ);
    }
}

#[cfg(target_os = "windows")]
unsafe fn overlay_ctx_mut(hwnd: HWND) -> Option<&'static mut NativeCaptureOverlayContext> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut NativeCaptureOverlayContext;
    if ptr.is_null() { None } else { Some(&mut *ptr) }
}

#[cfg(target_os = "windows")]
fn bgra_to_rgba_opaque(pixels: &[u8]) -> Vec<u8> {
    let mut rgba = pixels.to_vec();
    for chunk in rgba.chunks_exact_mut(4) {
        chunk.swap(0, 2);
        chunk[3] = 255;
    }
    rgba
}

fn encode_png_base64(width: i32, height: i32, rgba: &[u8]) -> Result<String, String> {
    if width <= 0 || height <= 0 {
        return Err("capture image has invalid dimensions".to_string());
    }

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if rgba.len() < expected {
        return Err("capture image buffer too short".to_string());
    }

    let mut encoded = Vec::new();
    PngEncoder::new(&mut encoded)
        .write_image(
            &rgba[..expected],
            width as u32,
            height as u32,
            ExtendedColorType::Rgba8,
        )
        .map_err(|err| format!("encode capture png failed: {err}"))?;
    Ok(STANDARD.encode(encoded))
}

#[cfg(target_os = "windows")]
fn clamp_ratio(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(target_os = "windows")]
fn clamp_client_pos(value: i32, max: i32) -> i32 {
    let upper = max.saturating_sub(1).max(0);
    value.clamp(0, upper)
}

#[cfg(target_os = "windows")]
fn lparam_x(lparam: LPARAM) -> i32 {
    ((lparam as u32 & 0xffff) as i16) as i32
}

#[cfg(target_os = "windows")]
fn lparam_y(lparam: LPARAM) -> i32 {
    (((lparam as u32 >> 16) & 0xffff) as i16) as i32
}

#[cfg(target_os = "windows")]
fn to_wide_null(text: &str) -> Vec<u16> {
    let mut wide = text.encode_utf16().collect::<Vec<u16>>();
    wide.push(0);
    wide
}
