//! Clipboard access: snapshot, private write (kept out of Win+V history and cloud sync),
//! and restore.

use std::time::Duration;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;

/// Everything on the clipboard that can be put back faithfully.
#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    items: Vec<(u32, Vec<u8>)>,
}

#[cfg(test)]
impl Snapshot {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

struct Guard;

impl Guard {
    /// Another program may hold the clipboard for a moment: 10 tries, 20 ms apart.
    fn open() -> Result<Self, String> {
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return Ok(Guard);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Err("The clipboard is busy (another app is holding it).".into())
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

fn registered(name: PCWSTR) -> u32 {
    unsafe { RegisterClipboardFormatW(name) }
}

/// Formats that are plain memory blocks and safe to copy and restore. Delayed-render and
/// GDI-handle formats are skipped.
fn restorable_formats() -> Vec<u32> {
    vec![
        CF_UNICODETEXT,
        CF_DIB,
        CF_DIBV5,
        CF_HDROP,
        registered(w!("HTML Format")),
        registered(w!("Rich Text Format")),
        registered(w!("PNG")),
    ]
}

unsafe fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let global = HGLOBAL(handle.0);
    let size = GlobalSize(global);
    if size == 0 {
        return None;
    }
    let ptr = GlobalLock(global) as *const u8;
    if ptr.is_null() {
        return None;
    }
    let bytes = std::slice::from_raw_parts(ptr, size).to_vec();
    let _ = GlobalUnlock(global);
    Some(bytes)
}

unsafe fn set_bytes(format: u32, bytes: &[u8]) -> Result<(), String> {
    let global = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| e.to_string())?;
    let ptr = GlobalLock(global) as *mut u8;
    if ptr.is_null() {
        return Err("could not lock clipboard memory".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
    let _ = GlobalUnlock(global);
    // On success the system owns the memory.
    SetClipboardData(format, Some(HANDLE(global.0)))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub fn snapshot() -> Result<Snapshot, String> {
    let _guard = Guard::open()?;
    let wanted = restorable_formats();
    let mut items = Vec::new();
    unsafe {
        let mut format = EnumClipboardFormats(0);
        while format != 0 {
            if wanted.contains(&format) {
                if let Ok(handle) = GetClipboardData(format) {
                    if let Some(bytes) = read_global(handle) {
                        items.push((format, bytes));
                    }
                }
            }
            format = EnumClipboardFormats(format);
        }
    }
    Ok(Snapshot { items })
}

fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Wraps an HTML fragment in the Windows "HTML Format" clipboard header (byte offsets of the
/// HTML and of the fragment, all in UTF-8).
pub fn cf_html(fragment: &str) -> Vec<u8> {
    const PRE: &str = "<html><body>\r\n<!--StartFragment-->";
    const POST: &str = "<!--EndFragment-->\r\n</body></html>";
    let header = |a: usize, b: usize, c: usize, d: usize| {
        format!(
            "Version:0.9\r\nStartHTML:{a:010}\r\nEndHTML:{b:010}\r\nStartFragment:{c:010}\r\nEndFragment:{d:010}\r\n"
        )
    };
    let len = header(0, 0, 0, 0).len();
    let start_frag = len + PRE.len();
    let end_frag = start_frag + fragment.len();
    let end_html = end_frag + POST.len();
    let mut out = header(len, end_html, start_frag, end_frag).into_bytes();
    out.extend_from_slice(PRE.as_bytes());
    out.extend_from_slice(fragment.as_bytes());
    out.extend_from_slice(POST.as_bytes());
    out.push(0);
    out
}

/// Puts `text` on the clipboard marked so that clipboard history (Win+V) and cloud clipboard
/// ignore it. Returns the clipboard sequence number right after the write.
pub fn write_text_private(text: &str) -> Result<u32, String> {
    write_private(text, None)
}

/// Like [`write_text_private`], with an optional HTML version (Word and Google Docs paste
/// HTML lists as real lists; plain-text apps use the text).
pub fn write_private(text: &str, html: Option<&str>) -> Result<u32, String> {
    let guard = Guard::open()?;
    unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        set_bytes(CF_UNICODETEXT, &utf16_bytes(text))?;
        if let Some(fragment) = html {
            set_bytes(registered(w!("HTML Format")), &cf_html(fragment))?;
        }
        // Documented by Microsoft: presence of this format excludes the content from
        // clipboard history and from cloud sync. A failure is not fatal (the text is still
        // inserted) but is logged, because the text may then appear in Win+V history.
        let zero = 0u32.to_le_bytes();
        let failed: Vec<&str> = [
            (
                "ExcludeClipboardContentFromMonitorProcessing",
                set_bytes(
                    registered(w!("ExcludeClipboardContentFromMonitorProcessing")),
                    &[0],
                )
                .is_err(),
            ),
            (
                "CanIncludeInClipboardHistory",
                set_bytes(registered(w!("CanIncludeInClipboardHistory")), &zero).is_err(),
            ),
            (
                "CanUploadToCloudClipboard",
                set_bytes(registered(w!("CanUploadToCloudClipboard")), &zero).is_err(),
            ),
        ]
        .into_iter()
        .filter_map(|(name, err)| err.then_some(name))
        .collect();
        if !failed.is_empty() {
            crate::diag::log(
                "clipboard",
                &format!(
                    "privacy marker(s) not set, the text may appear in Win+V history: {failed:?}"
                ),
            );
        }
    }
    // The sequence number only settles once the clipboard is closed, so close first.
    drop(guard);
    Ok(sequence())
}

/// Reads the clipboard's text, if any.
pub fn read_text() -> Option<String> {
    let _guard = Guard::open().ok()?;
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT).ok()?;
        let bytes = read_global(handle)?;
        let wide: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes(*b))
            .take_while(|c| *c != 0)
            .collect();
        Some(String::from_utf16_lossy(&wide))
    }
}

/// True when the clipboard currently carries the history-exclusion marker.
#[cfg(test)]
pub fn has_exclusion_marker() -> bool {
    let Ok(_guard) = Guard::open() else {
        return false;
    };
    let marker = registered(w!("ExcludeClipboardContentFromMonitorProcessing"));
    unsafe {
        let mut format = EnumClipboardFormats(0);
        while format != 0 {
            if format == marker {
                return true;
            }
            format = EnumClipboardFormats(format);
        }
    }
    false
}

/// Puts a snapshot back. An empty snapshot clears the clipboard.
pub fn restore(snapshot: &Snapshot) -> Result<(), String> {
    let _guard = Guard::open()?;
    unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        for (format, bytes) in &snapshot.items {
            set_bytes(*format, bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // The clipboard is one shared system resource: run these tests one at a time, and put
    // the user's clipboard back afterwards.
    static LOCK: Mutex<()> = Mutex::new(());

    /// GitHub's build machines have no interactive desktop, so the real clipboard tests
    /// only run on a real Windows session (the owner's laptop).
    fn on_ci() -> bool {
        std::env::var_os("GITHUB_ACTIONS").is_some()
    }

    #[test]
    fn html_header_offsets_point_at_the_fragment() {
        let bytes = cf_html("<ul><li>a & b</li></ul>");
        let s = String::from_utf8(bytes[..bytes.len() - 1].to_vec()).unwrap();
        let num = |key: &str| -> usize {
            let at = s.find(key).unwrap() + key.len();
            s[at..at + 10].parse().unwrap()
        };
        assert_eq!(
            &s[num("StartFragment:")..num("EndFragment:")],
            "<ul><li>a & b</li></ul>"
        );
        assert!(s[num("StartHTML:")..].starts_with("<html>"));
        assert_eq!(num("EndHTML:"), s.len());
    }

    #[test]
    fn private_write_then_restore_round_trip() {
        if on_ci() {
            return;
        }
        let _l = LOCK.lock().unwrap();
        let original = snapshot().expect("snapshot");

        // Start from known text so the restore can be checked.
        let _guard_free = write_text_private("previous clipboard ☃ नमस्ते").unwrap();
        let before = snapshot().unwrap();
        assert!(!before.is_empty());

        let seq = write_text_private("dictated text").unwrap();
        assert_eq!(read_text().as_deref(), Some("dictated text"));
        assert!(
            has_exclusion_marker(),
            "dictation must carry the history-exclusion marker"
        );
        assert_eq!(sequence(), seq, "nothing else touched the clipboard");

        restore(&before).unwrap();
        assert_eq!(read_text().as_deref(), Some("previous clipboard ☃ नमस्ते"));
        assert_ne!(sequence(), seq);

        restore(&original).unwrap();
    }

    #[test]
    fn binary_formats_survive_a_round_trip() {
        if on_ci() {
            return;
        }
        let _l = LOCK.lock().unwrap();
        let original = snapshot().expect("snapshot");

        // A minimal 1x1 32-bit device-independent bitmap.
        let mut dib = vec![0u8; 40 + 4];
        dib[0] = 40; // biSize
        dib[4] = 1; // biWidth
        dib[8] = 1; // biHeight
        dib[12] = 1; // biPlanes
        dib[14] = 32; // biBitCount
        dib[40..44].copy_from_slice(&[0x11, 0x22, 0x33, 0xFF]);
        {
            let _g = Guard::open().unwrap();
            unsafe {
                EmptyClipboard().unwrap();
                set_bytes(CF_DIB, &dib).unwrap();
            }
        }
        let image = snapshot().unwrap();
        write_text_private("dictated").unwrap();
        restore(&image).unwrap();
        let again = snapshot().unwrap();
        let restored = again
            .items
            .iter()
            .find(|(f, _)| *f == CF_DIB)
            .map(|(_, b)| b.clone());
        assert_eq!(restored.as_deref().map(|b| &b[..44]), Some(&dib[..]));

        restore(&original).unwrap();
    }
}
