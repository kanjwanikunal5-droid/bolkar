//! Global shortcuts through a low-level keyboard hook (`WH_KEYBOARD_LL`).
//!
//! `RegisterHotKey` cannot report key *release*, which push-to-talk needs, so it is not used.
//! The hook runs on its own thread with its own message loop; the callback only updates the
//! state machine and posts to a channel, so it returns in well under a millisecond.

pub mod machine;

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

pub use machine::Action;
use machine::Machine;

struct Hook {
    machine: Machine,
    tx: Sender<Action>,
}

static HOOK: OnceLock<Mutex<Hook>> = OnceLock::new();

/// An unassigned virtual key. Sending it while Win is held makes Windows treat the Win
/// press as part of a shortcut, so releasing Win does not open the Start menu.
const MASK_KEY: u16 = 0xE8;

fn send_mask_key() {
    let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(MASK_KEY),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
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
        // Ignore keys injected by software (including our own paste and mask keys).
        if !info.flags.contains(LLKHF_INJECTED) {
            let down = matches!(wparam.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
            let decision = match HOOK.get().and_then(|h| h.lock().ok()) {
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
            match SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0) {
                Ok(_hook) => {
                    let _ = ready_tx.send(Ok(()));
                    let mut msg = MSG::default();
                    // The hook is serviced by this thread's message loop.
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("could not install keyboard hook: {e}")));
                }
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv().map_err(|e| e.to_string())?
}
