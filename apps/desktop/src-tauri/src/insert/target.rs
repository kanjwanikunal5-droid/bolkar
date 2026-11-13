//! The window the user is dictating into.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

#[derive(Debug, Clone)]
pub struct Target {
    pub hwnd: isize,
    pub pid: u32,
    /// Executable file name, e.g. "chrome.exe". Empty when it could not be read.
    pub process: String,
    pub title: String,
}

fn token_is_elevated(process: HANDLE) -> Option<bool> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut elevation = TOKEN_ELEVATION::default();
        let mut returned = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        );
        let _ = CloseHandle(token);
        ok.ok()?;
        Some(elevation.TokenIsElevated != 0)
    }
}

/// Is this app itself running as administrator?
pub fn self_is_elevated() -> bool {
    token_is_elevated(unsafe { GetCurrentProcess() }).unwrap_or(false)
}

/// Is the target process running as administrator? If the process cannot even be opened,
/// it is treated as elevated, because we will not be able to type into it either.
pub fn is_elevated(pid: u32) -> bool {
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let elevated = token_is_elevated(handle).unwrap_or(true);
                let _ = CloseHandle(handle);
                elevated
            }
            Err(_) => true,
        }
    }
}

fn process_name(pid: u32) -> String {
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return String::new();
        };
        let mut buffer = [0u16; 1024];
        let mut len = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        if ok.is_err() {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buffer[..len as usize]);
        full.rsplit(['\\', '/']).next().unwrap_or("").to_string()
    }
}

pub fn foreground_hwnd() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Reads the current foreground window. `None` when there is none (for example the desktop
/// is locked).
pub fn foreground() -> Option<Target> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let mut title = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut title).max(0) as usize;
        Some(Target {
            hwnd: hwnd.0 as isize,
            pid,
            process: process_name(pid),
            title: String::from_utf16_lossy(&title[..n]),
        })
    }
}

/// Work area (excluding the taskbar) of the monitor showing `hwnd`: (left, top, right, bottom).
pub fn monitor_work_area(hwnd: isize) -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let monitor = MonitorFromWindow(HWND(hwnd as *mut _), MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let RECT {
            left,
            top,
            right,
            bottom,
        } = info.rcWork;
        Some((left, top, right, bottom))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_can_be_inspected() {
        let pid = std::process::id();
        assert!(!process_name(pid).is_empty());
        // The test runner is a normal process, so both views must agree.
        assert_eq!(is_elevated(pid), self_is_elevated());
    }

    #[test]
    fn unopenable_process_counts_as_elevated() {
        // PID 4 is the System process; a normal user cannot open it.
        if !self_is_elevated() {
            assert!(is_elevated(4));
        }
    }
}
