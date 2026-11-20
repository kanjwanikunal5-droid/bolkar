//! Global shortcuts through a low-level keyboard hook (`WH_KEYBOARD_LL`).
//!
//! `RegisterHotKey` cannot report key *release*, which push-to-talk needs, so it is not used.
//! The hook runs on its own high-priority thread with its own message loop; the callback only
//! updates the state machine and posts to a channel, so it returns in well under a millisecond.
//!
//! Windows skips a low-level hook that answers too slowly and, after repeated timeouts,
//! removes it without telling anyone (shortcuts then stop working until restart). So the hook
//! thread runs at time-critical priority, never touches the disk, and re-attaches the hook
//! every 20 seconds while no shortcut key is held.

pub mod machine;

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetTimer, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
    LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN, WM_TIMER,
};

pub use machine::Action;
use machine::Machine;

struct Hook {
    machine: Machine,
    tx: Sender<Action>,
}

static HOOK: OnceLock<Mutex<Hook>> = OnceLock::new();

/// How often the hook is re-attached (only while no shortcut key is held).
const REATTACH_MS: u32 = 20_000;

/// Test runs only (own data folder + APP_TEST_INJECTED_KEYS=1): accept key presses sent by
/// software, so an automated test can press Ctrl+Win. Never true in normal use.
static ACCEPT_INJECTED: OnceLock<bool> = OnceLock::new();

fn accept_injected() -> bool {
    *ACCEPT_INJECTED.get_or_init(|| {
        std::env::var_os("APP_DATA_DIR").is_some()
            && std::env::var("APP_TEST_INJECTED_KEYS").is_ok_and(|v| v == "1")
    })
}

/// Test runs only (own data folder + APP_TEST_LOG): raw key events, written by another thread
/// so the hook itself never waits on the disk.
static TRACE: OnceLock<Option<Mutex<Sender<String>>>> = OnceLock::new();

fn trace(line: String) {
    let tx = TRACE.get_or_init(|| {
        let path = std::env::var_os("APP_TEST_LOG")?;
        std::env::var_os("APP_DATA_DIR")?;
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            use std::io::Write;
            for line in rx {
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                {
                    let _ = writeln!(f, "{line}");
                }
            }
        });
        Some(Mutex::new(tx))
    });
    if let Some(tx) = tx.as_ref().and_then(|t| t.try_lock().ok()) {
        let _ = tx.send(line);
    }
}

/// An unassigned virtual key. Sending it while Win is held makes Windows treat the Win
/// press as part of a shortcut, so releasing Win does not open the Start menu.
const MASK_KEY: u16 = 0xE8;

/// Marks key presses this app sends itself (see `insert::keys`).
pub const OWN_KEY_MARK: usize = 0x5052_4B59;

fn send_mask_key() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(MASK_KEY),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: OWN_KEY_MARK,
            },
        },
    };
    let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
    unsafe {
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let injected = info.flags.contains(LLKHF_INJECTED);
        // Keys this app sends itself (paste, mask key) are always ignored; keys injected by
        // other software only count in a test run.
        let ours = info.dwExtraInfo == OWN_KEY_MARK;
        if accept_injected() {
            trace(format!(
                "key vk={:#x} msg={:#x} injected={injected}",
                info.vkCode, wparam.0
            ));
        }
        if !ours && (!injected || accept_injected()) {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            // Never block inside a low-level hook (Windows drops a hook that is too slow): a
            // few quick tries, and if the state is busy the key simply passes through.
            let decision = match HOOK.get().and_then(try_lock_briefly) {
                Some(mut hook) => {
                    let d = hook.machine.on_key(info.vkCode, down, info.time);
                    if let Some(action) = d.action {
                        let _ = hook.tx.send(action);
                    }
                    d
                }
                None => Default::default(),
            };
            if decision.mask_start_menu {
                send_mask_key();
            }
            if decision.swallow {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

/// The hook state without waiting: up to ~200 quick tries (well under a millisecond).
fn try_lock_briefly<T>(m: &std::sync::Mutex<T>) -> Option<std::sync::MutexGuard<'_, T>> {
    for _ in 0..200 {
        match m.try_lock() {
            Ok(g) => return Some(g),
            Err(std::sync::TryLockError::WouldBlock) => std::hint::spin_loop(),
            Err(std::sync::TryLockError::Poisoned(_)) => return None,
        }
    }
    None
}

/// No shortcut in progress and no modifier held, so the hook can be swapped safely.
fn keys_quiet() -> bool {
    HOOK.get()
        .and_then(|h| h.try_lock().ok())
        .is_some_and(|h| h.machine.quiet())
}

/// Tells the state machine a recording ended for a non-keyboard reason.
pub fn force_idle() {
    if let Some(mut hook) = HOOK.get().and_then(|h| h.lock().ok()) {
        hook.machine.force_idle();
    }
}

/// Starts the hook thread. Actions arrive on `tx`.
pub fn start(tx: Sender<Action>) -> Result<(), String> {
    if HOOK
        .set(Mutex::new(Hook {
            machine: Machine::new(),
            tx,
        }))
        .is_err()
    {
        return Err("keyboard hook already started".into());
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("keyboard-hook".into())
        .spawn(move || unsafe {
            let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
                Ok(first) => {
                    let mut hook = first;
                    let _ = ready_tx.send(Ok(()));
                    trace("hook installed".into());
                    let _ = SetTimer(None, 0, REATTACH_MS, None);
                    let mut msg = MSG::default();
                    // The hook is serviced by this thread's message loop.
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                        if msg.message == WM_TIMER && keys_quiet() {
                            if let Ok(fresh) =
                                SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)
                            {
                                let _ = UnhookWindowsHookEx(hook);
                                hook = fresh;
                                trace("hook re-attached".into());
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("could not install keyboard hook: {e}")));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv().map_err(|e| e.to_string())?
}
