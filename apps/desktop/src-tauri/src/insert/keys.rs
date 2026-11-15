//! Synthetic key presses through `SendInput`.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
};

use super::app_rules::PasteKey;

const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_RETURN: u16 = 0x0D;
const VK_INSERT: u16 = 0x2D;
const VK_V: u16 = 0x56;
const VK_C: u16 = 0x43;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;

fn key(vk: u16, up: bool) -> INPUT {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    if vk == VK_INSERT {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::hotkey::OWN_KEY_MARK,
            },
        },
    }
}

fn unicode(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: crate::hotkey::OWN_KEY_MARK,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<(), String> {
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err("Windows blocked the simulated key presses (the target may be an admin app).".into())
    }
}

fn is_down(vk: u16) -> bool {
    (unsafe { GetAsyncKeyState(i32::from(vk)) } as u16 & 0x8000) != 0
}

/// Waits until the user has let go of Ctrl, Shift, Alt and Win. If Win were still down,
/// our Ctrl+V would become Win+Ctrl+V, a different Windows shortcut.
pub fn wait_modifiers_released(timeout: Duration) -> bool {
    let started = Instant::now();
    loop {
        let any = [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
            .into_iter()
            .any(is_down);
        if !any {
            return true;
        }
        if started.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn send_paste(paste: PasteKey) -> Result<(), String> {
    let chord: &[u16] = match paste {
        PasteKey::CtrlV => &[VK_CONTROL, VK_V],
        PasteKey::ShiftInsert => &[VK_SHIFT, VK_INSERT],
        PasteKey::CtrlShiftV => &[VK_CONTROL, VK_SHIFT, VK_V],
    };
    let mut inputs: Vec<INPUT> = chord.iter().map(|vk| key(*vk, false)).collect();
    inputs.extend(chord.iter().rev().map(|vk| key(*vk, true)));
    send(&inputs)
}

/// Ctrl+C: copies the selection in the focused app.
pub fn send_copy() -> Result<(), String> {
    send(&[
        key(VK_CONTROL, false),
        key(VK_C, false),
        key(VK_C, true),
        key(VK_CONTROL, true),
    ])
}

/// Types text directly, character by character. Newlines become the Enter key.
pub fn type_text(text: &str) -> Result<(), String> {
    for chunk in text.replace("\r\n", "\n").split_inclusive('\n') {
        let (line, newline) = match chunk.strip_suffix('\n') {
            Some(l) => (l, true),
            None => (chunk, false),
        };
        let mut inputs = Vec::with_capacity(line.len() * 2);
        // encode_utf16 yields surrogate pairs for characters outside the basic plane;
        // each unit is sent as its own down/up pair, which is what Windows expects.
        for unit in line.encode_utf16() {
            inputs.push(unicode(unit, false));
            inputs.push(unicode(unit, true));
        }
        if !inputs.is_empty() {
            send(&inputs)?;
        }
        if newline {
            send(&[key(VK_RETURN, false), key(VK_RETURN, true)])?;
        }
    }
    Ok(())
}
