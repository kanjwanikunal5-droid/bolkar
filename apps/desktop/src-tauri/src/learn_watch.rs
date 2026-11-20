//! Learning from the user's own edits.
//!
//! After a dictation is typed into an app, that exact text box is read again for up to 90
//! seconds (Windows UI Automation, the interface screen readers use). If the user corrected a
//! word there ("Henglish" -> "Hinglish"), the correction is learned:
//! - a misheard form that is not an ordinary word is replaced automatically from then on;
//! - a correction of an ordinary word ("cloud" -> "Claude") only steers the recogniser, so a
//!   real "cloud" is never changed.
//!
//! Scope (audit of 0.4.3, 19 Nov 2026): the window and the text box are pinned when the
//! dictation is typed. Watching stops as soon as another window is in front or another box has
//! the focus, so no other box is ever read. Only a small stretch around the cursor is read
//! (never the whole document), never a password box, and the text read is not stored: only
//! the learned word pairs are. Apps whose text box cannot be pinned are not watched.
//! Settings > Writing > "Learn from my corrections" switches it off.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationValuePattern, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_TextPatternId, UIA_ValuePatternId,
};

/// How long after a dictation its text box is watched for corrections.
pub const WATCH_FOR: Duration = Duration::from_secs(90);
/// How often it is read while watching.
pub const READ_EVERY: Duration = Duration::from_secs(4);
/// Extra characters read on each side of the dictation's length around the cursor.
const MARGIN: usize = 120;
/// A one-line box whose whole value is longer than this is not read.
const MAX_VALUE: usize = 4000;

pub struct Watch {
    pub pid: u32,
    /// The window the dictation was typed into.
    pub hwnd: isize,
    pub inserted: String,
    pub until: Instant,
    /// The latest corrected version of the inserted text seen in the box.
    pub corrected: Option<String>,
}

impl Watch {
    /// Pins the focused text box of the window in front (it must be the window the dictation
    /// went into, in process `pid`). None when there is no box that can be pinned: the
    /// dictation is then not watched.
    pub fn start(pid: u32, hwnd: isize, inserted: &str) -> Option<Self> {
        if crate::insert::target::foreground_hwnd() != hwnd {
            return None;
        }
        with_com(|com| {
            let el = unsafe { com.ua.as_ref()?.GetFocusedElement() }.ok()?;
            if !is_readable(&el, pid) {
                return None;
            }
            com.pinned = Some(el);
            Some(())
        })?;
        Some(Self {
            pid,
            hwnd,
            inserted: inserted.to_string(),
            until: Instant::now() + WATCH_FOR,
            corrected: None,
        })
    }
}

/// UI Automation for this thread: COM is set up once and closed when the thread ends.
struct Com {
    ua: Option<IUIAutomation>,
    pinned: Option<IUIAutomationElement>,
    /// CoInitializeEx succeeded (S_OK or S_FALSE) and must be balanced.
    owns_com: bool,
}

impl Drop for Com {
    fn drop(&mut self) {
        // Release the interfaces before COM is closed.
        self.pinned = None;
        self.ua = None;
        if self.owns_com {
            unsafe { CoUninitialize() };
        }
    }
}

thread_local! {
    static COM: RefCell<Option<Com>> = const { RefCell::new(None) };
}

fn with_com<T>(f: impl FnOnce(&mut Com) -> Option<T>) -> Option<T> {
    COM.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            // RPC_E_CHANGED_MODE (COM already set up differently on this thread) is not ours
            // to close; UI Automation still works.
            let owns_com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
            let ua: Option<IUIAutomation> =
                unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok();
            *slot = Some(Com {
                ua,
                pinned: None,
                owns_com,
            });
        }
        f(slot.as_mut()?)
    })
}

fn is_readable(el: &IUIAutomationElement, pid: u32) -> bool {
    unsafe {
        el.CurrentIsPassword()
            .map(|b| !b.as_bool())
            .unwrap_or(false)
            && el
                .CurrentProcessId()
                .map(|p| p as u32 == pid)
                .unwrap_or(false)
    }
}

/// What one look found.
enum Seen {
    /// The pinned box, a small stretch of its text.
    Text(String),
    /// The pinned box, nothing readable this time.
    Nothing,
    /// Another window or box has the focus: stop watching.
    Moved,
}

/// Whether the same window and box still have the focus. Pure decision, tested below.
fn still_pinned(front: isize, pinned_window: isize, same_box: Option<bool>) -> bool {
    front == pinned_window && same_box == Some(true)
}

fn read_pinned(watch: &Watch) -> Seen {
    let front = crate::insert::target::foreground_hwnd();
    with_com(|com| {
        let ua = com.ua.as_ref()?;
        let pinned = com.pinned.as_ref()?;
        let el = unsafe { ua.GetFocusedElement() }.ok();
        let same = el
            .as_ref()
            .map(|e| unsafe { ua.CompareElements(pinned, e) }.is_ok_and(|b| b.as_bool()));
        if !still_pinned(front, watch.hwnd, same) {
            return Some(Seen::Moved);
        }
        let el = el?;
        if !is_readable(&el, watch.pid) {
            return Some(Seen::Moved);
        }
        let around = watch.inserted.chars().count() + MARGIN;
        unsafe {
            if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
                // A stretch around the cursor, not the document.
                let range = p
                    .GetSelection()
                    .ok()
                    .filter(|s| s.Length().unwrap_or(0) > 0)
                    .and_then(|s| s.GetElement(0).ok())
                    .and_then(|r| r.Clone().ok());
                if let Some(r) = range {
                    let back = -(around as i32);
                    let _ = r.MoveEndpointByUnit(
                        TextPatternRangeEndpoint_Start,
                        TextUnit_Character,
                        back,
                    );
                    let _ = r.MoveEndpointByUnit(
                        TextPatternRangeEndpoint_End,
                        TextUnit_Character,
                        around as i32,
                    );
                    if let Ok(t) = r.GetText((2 * around) as i32) {
                        return Some(Seen::Text(t.to_string()));
                    }
                }
                return Some(Seen::Nothing);
            }
            if let Ok(p) = el.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) {
                if let Ok(v) = p.CurrentValue() {
                    let v = v.to_string();
                    if v.chars().count() <= MAX_VALUE {
                        return Some(Seen::Text(v));
                    }
                }
            }
        }
        Some(Seen::Nothing)
    })
    .unwrap_or(Seen::Moved)
}

/// One look at the pinned text box: updates `watch.corrected` when the dictation is there
/// with a change. Returns false when watching stops (time up, or the focus moved elsewhere).
pub fn look(watch: &mut Watch) -> bool {
    if Instant::now() > watch.until {
        return false;
    }
    match read_pinned(watch) {
        Seen::Moved => false,
        Seen::Nothing => true,
        Seen::Text(field) => {
            if let Some(region) = app_core::learn::find_edited(&watch.inserted, &field) {
                if region != watch.inserted {
                    watch.corrected = Some(region);
                }
            }
            true
        }
    }
}

/// Stops watching and lets go of the pinned box.
pub fn release() {
    let _ = with_com(|com| {
        com.pinned = None;
        Some(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watching_stops_when_another_window_or_box_has_the_focus() {
        assert!(still_pinned(10, 10, Some(true)));
        assert!(!still_pinned(11, 10, Some(true)), "another window in front");
        assert!(
            !still_pinned(10, 10, Some(false)),
            "another box in the same window"
        );
        assert!(!still_pinned(10, 10, None), "no focused element");
    }

    #[test]
    fn com_is_set_up_once_per_thread() {
        // Many looks on one thread: one CoInitializeEx, closed when the thread ends.
        std::thread::spawn(|| {
            for _ in 0..50 {
                let _ = with_com(|_| Some(()));
            }
            COM.with(|c| assert!(c.borrow().is_some()));
        })
        .join()
        .unwrap();
    }
}
