//! Puts dictated text into the app the user was in.
//!
//! Default method: copy privately -> send the app's paste key -> restore the old clipboard.
//! Nothing is pasted if the user switched windows meanwhile, or if the target runs as
//! administrator and we do not (Windows blocks that); the text is then left on the clipboard.

pub mod app_rules;
pub mod clipboard;
pub mod keys;
pub mod target;

use std::time::Duration;

use serde::Serialize;

pub use target::Target;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Outcome {
    /// Pasted into the target. `clipboard_restored` is false when something else changed
    /// the clipboard during the paste, in which case it was left alone.
    Pasted {
        method: &'static str,
        clipboard_restored: bool,
    },
    /// Typed directly, without the clipboard.
    Typed,
    /// Not pasted; the text is on the clipboard for a manual Ctrl+V.
    Copied {
        reason: CopyReason,
    },
    Failed {
        message: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CopyReason {
    FocusChanged,
    ElevatedTarget,
    KeysBlocked,
}

impl Outcome {
    pub fn inserted(&self) -> bool {
        matches!(self, Outcome::Pasted { .. } | Outcome::Typed)
    }

    /// Short message for the pill.
    pub fn message(&self) -> String {
        match self {
            Outcome::Pasted { .. } | Outcome::Typed => "Inserted".into(),
            Outcome::Copied {
                reason: CopyReason::FocusChanged,
            } => "Copied - focus changed. Press Ctrl+V".into(),
            Outcome::Copied {
                reason: CopyReason::ElevatedTarget,
            } => "This app runs as admin. Text copied; press Ctrl+V".into(),
            Outcome::Copied {
                reason: CopyReason::KeysBlocked,
            } => "Copied. Press Ctrl+V".into(),
            Outcome::Failed { message } => message.clone(),
        }
    }

    pub fn method(&self) -> &'static str {
        match self {
            Outcome::Pasted { method, .. } => method,
            Outcome::Typed => "typed",
            Outcome::Copied { .. } => "copied",
            Outcome::Failed { .. } => "failed",
        }
    }
}

/// How long the target gets to read the clipboard before it is restored.
const PASTE_SETTLE: Duration = Duration::from_millis(400);
/// Word converts pasted HTML slowly and reads the clipboard late; restoring the old clipboard
/// before it has read the list makes the paste fail silently (measured: 3 of 10 runs at 400 ms).
const PASTE_SETTLE_RICH: Duration = Duration::from_millis(1500);

fn copy_only(text: &str, reason: CopyReason) -> Outcome {
    match clipboard::write_text_private(text) {
        Ok(_) => Outcome::Copied { reason },
        Err(message) => Outcome::Failed { message },
    }
}

/// Inserts `text` into `target`, which was captured when recording started.
pub fn insert(text: &str, target: &Target) -> Outcome {
    insert_rich(text, None, target)
}

/// Copies the selected text of `target` (which must be in front) and puts the user's
/// clipboard back. `Ok("")` when nothing is selected.
pub fn copy_selection(target: &Target) -> Result<String, String> {
    keys::wait_modifiers_released(Duration::from_millis(1500));
    if target::foreground_hwnd() != target.hwnd {
        return Err("The window changed before the selection could be read.".into());
    }
    let snapshot = clipboard::snapshot()?;
    let before = clipboard::sequence();
    // An app that has only just come to the front may miss the first Ctrl+C: try twice.
    for _ in 0..2 {
        if target::foreground_hwnd() != target.hwnd {
            break;
        }
        keys::send_copy()?;
        // Wait for the app to put its copy on the clipboard.
        let started = std::time::Instant::now();
        while clipboard::sequence() == before && started.elapsed() < Duration::from_millis(700) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if clipboard::sequence() != before {
            break;
        }
    }
    let text = if clipboard::sequence() == before {
        String::new() // nothing selected: the app did not copy anything
    } else {
        clipboard::read_text().unwrap_or_default()
    };
    let _ = clipboard::restore(&snapshot);
    Ok(text.replace("\r\n", "\n"))
}

/// Inserts `text` (and, for apps that read it, `html`) into `target`.
pub fn insert_rich(text: &str, html: Option<&str>, target: &Target) -> Outcome {
    if text.is_empty() {
        return Outcome::Failed {
            message: "Nothing to insert".into(),
        };
    }
    if target::foreground_hwnd() != target.hwnd {
        return copy_only(text, CopyReason::FocusChanged);
    }
    if target::is_elevated(target.pid) && !target::self_is_elevated() {
        return copy_only(text, CopyReason::ElevatedTarget);
    }
    let rule = app_rules::rule_for(&target.process, &target.title);

    // The user has just released the shortcut; make sure the modifiers are really up.
    keys::wait_modifiers_released(Duration::from_millis(1500));
    if target::foreground_hwnd() != target.hwnd {
        return copy_only(text, CopyReason::FocusChanged);
    }

    let snapshot = match clipboard::snapshot() {
        Ok(s) => s,
        // Clipboard locked by another app: type the text instead.
        Err(_) => {
            return match keys::type_text(text) {
                Ok(()) => Outcome::Typed,
                Err(message) => Outcome::Failed { message },
            }
        }
    };
    let seq = match clipboard::write_private(text, html) {
        Ok(seq) => seq,
        Err(e) => {
            // The write may have failed after the clipboard was emptied: put the user's
            // previous clipboard back before typing the text instead.
            crate::diag::log("clipboard", &format!("write failed, typing instead: {e}"));
            let _ = clipboard::restore(&snapshot);
            return match keys::type_text(text) {
                Ok(()) => Outcome::Typed,
                Err(message) => Outcome::Failed { message },
            };
        }
    };
    // Last check, immediately before the key press: another window may have jumped in
    // front while the clipboard was being prepared. The text then stays on the clipboard.
    if target::foreground_hwnd() != target.hwnd {
        return Outcome::Copied {
            reason: CopyReason::FocusChanged,
        };
    }
    if keys::send_paste(rule.paste_key).is_err() {
        // Text stays on the clipboard so nothing is lost.
        return Outcome::Copied {
            reason: CopyReason::KeysBlocked,
        };
    }

    std::thread::sleep(if html.is_some() {
        PASTE_SETTLE_RICH
    } else {
        PASTE_SETTLE
    });
    // Restore only if the clipboard is still exactly what we wrote. If the user or the
    // target changed it in the meantime, their content wins.
    let clipboard_restored = clipboard::sequence() == seq && clipboard::restore(&snapshot).is_ok();
    let method = match rule.paste_key {
        app_rules::PasteKey::CtrlV => "clipboard:ctrl+v",
        app_rules::PasteKey::ShiftInsert => "clipboard:shift+insert",
        app_rules::PasteKey::CtrlShiftV => "clipboard:ctrl+shift+v",
    };
    Outcome::Pasted {
        method,
        clipboard_restored,
    }
}
