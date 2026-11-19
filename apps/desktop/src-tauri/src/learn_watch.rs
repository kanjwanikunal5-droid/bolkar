//! Learning from the user's own edits (like the best dictation apps do).
//!
//! After a dictation is typed into an app, the text box it went into is read again for up to
//! 90 seconds (Windows UI Automation, the same interface screen readers use). If the user
//! corrected a word there ("Henglish" -> "Hinglish"), the correction is learned:
//! - a misheard form that is not an ordinary word is replaced automatically from then on;
//! - a correction of an ordinary word ("cloud" -> "Claude") only steers the recogniser, so a
//!   real "cloud" is never changed.
//!
//! Only the same app and text box are read, never a password box; nothing but the learned
//! word pairs is kept. Settings > Writing > "Learn from my corrections" switches it off.

use std::time::{Duration, Instant};

use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationTextPattern, IUIAutomationValuePattern,
    UIA_TextPatternId, UIA_ValuePatternId,
};

/// How long after a dictation its text box is watched for corrections.
pub const WATCH_FOR: Duration = Duration::from_secs(90);
/// How often it is read while watching.
pub const READ_EVERY: Duration = Duration::from_secs(4);

pub struct Watch {
    pub pid: u32,
    pub inserted: String,
    pub until: Instant,
    /// The latest corrected version of the inserted text seen in the box.
    pub corrected: Option<String>,
}

impl Watch {
    pub fn new(pid: u32, inserted: &str) -> Self {
        Self {
            pid,
            inserted: inserted.to_string(),
            until: Instant::now() + WATCH_FOR,
            corrected: None,
        }
    }
}

/// The text of the focused text box, if it belongs to process `pid` and is not a password box.
pub fn focused_text(pid: u32) -> Option<String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let ua: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let el = ua.GetFocusedElement().ok()?;
        if el.CurrentIsPassword().ok()?.as_bool() {
            return None;
        }
        if el.CurrentProcessId().ok()? as u32 != pid {
            return None;
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
            if let Ok(r) = p.DocumentRange() {
                if let Ok(t) = r.GetText(20_000) {
                    return Some(t.to_string());
                }
            }
        }
        if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
            if let Ok(v) = p.CurrentValue() {
                return Some(v.to_string());
            }
        }
        None
    }
}

/// One look at the text box: updates `watch.corrected` when the dictation is there with a
/// change. Returns false when watching can stop (the time is up).
pub fn look(watch: &mut Watch) -> bool {
    if Instant::now() > watch.until {
        return false;
    }
    if let Some(field) = focused_text(watch.pid) {
        if let Some(region) = app_core::learn::find_edited(&watch.inserted, &field) {
            if region != watch.inserted {
                watch.corrected = Some(region);
            }
        }
    }
    true
}
