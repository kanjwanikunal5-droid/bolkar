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

fn copy_only(text: &str, reason: CopyReason) -> Outcome {
    match clipboard::write_text_private(text) {
        Ok(_) => Outcome::Copied { reason },
        Err(message) => Outcome::Failed { message },
    }
}

/// Inserts `text` into `target`, which was captured when recording started.
pub fn insert(text: &str, target: &Target) -> Outcome {
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
    let seq = match clipboard::write_text_private(text) {
        Ok(seq) => seq,
        Err(_) => {
            return match keys::type_text(text) {
                Ok(()) => Outcome::Typed,
                Err(message) => Outcome::Failed { message },
            }
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

    std::thread::sleep(PASTE_SETTLE);
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
