//! The floating status pill: always on top, never takes keyboard focus.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_HIDE, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

pub const PILL: &str = "pill";

#[derive(Debug, Clone, Serialize, Default)]
pub struct PillState {
    /// listening | transcribing | polishing | done | info | error | hidden
    pub state: &'static str,
    pub message: String,
    /// Input level 0..1 while listening.
    pub level: f32,
    pub seconds: u64,
    pub hands_free: bool,
    /// e.g. "Local - Large v3 Turbo (GPU)".
    pub engine: String,
}

fn pill_hwnd(app: &AppHandle) -> Option<HWND> {
    // Tauri may link a different version of the `windows` crate, so rebuild the handle
    // from its raw pointer instead of passing the type across.
    let raw = app.get_webview_window(PILL)?.hwnd().ok()?;
    Some(HWND(raw.0 as *mut _))
}

/// Marks the pill window as non-activating so showing it can never steal focus from the
/// app being dictated into.
pub fn prepare(app: &AppHandle) {
    if let Some(hwnd) = pill_hwnd(app) {
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let extra = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | extra);
        }
    }
}

/// Shows the pill at the bottom centre of the monitor that holds `target_hwnd`.
pub fn show(app: &AppHandle, target_hwnd: Option<isize>) {
    let Some(hwnd) = pill_hwnd(app) else { return };
    unsafe {
        let mut rect = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rect);
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        let anchor = target_hwnd.unwrap_or_else(crate::insert::target::foreground_hwnd);
        let (left, _top, right, bottom) =
            crate::insert::target::monitor_work_area(anchor).unwrap_or((0, 0, 1280, 720));
        let x = left + ((right - left) - width) / 2;
        let y = bottom - height - 24;
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(hwnd) = pill_hwnd(app) {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        }
    }
}

pub fn emit(app: &AppHandle, state: &PillState) {
    let _ = app.emit(PILL, state);
}
