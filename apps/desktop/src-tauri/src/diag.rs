//! Diagnostics log: a small text file on this computer that says what happened (versions,
//! steps, timings, error messages), never what was said. No audio, no transcripts, no
//! meeting titles, no API keys: callers pass categories, numbers and error texts only, and
//! anything that looks like a key is removed before writing.
//!
//! On by default (Settings > Privacy > Diagnostics log), 1 MB at most (one older file is
//! kept), and the user can open, export or delete it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use regex::Regex;

const MAX_BYTES: u64 = 1024 * 1024;

static DIR: OnceLock<PathBuf> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(true);
static WRITE: Mutex<()> = Mutex::new(());

static SECRET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(gsk_[A-Za-z0-9]{8,}|sk-[A-Za-z0-9_-]{8,}|AIza[0-9A-Za-z_-]{20,}|bearer\s+[A-Za-z0-9._-]{8,})")
        .unwrap()
});

pub fn init(data_dir: &Path, enabled: bool) {
    let _ = DIR.set(data_dir.join("logs"));
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

pub fn dir() -> Option<&'static PathBuf> {
    DIR.get()
}

pub fn file() -> Option<PathBuf> {
    DIR.get().map(|d| d.join("diagnostics.log"))
}

/// Removes anything that looks like an API key or bearer token.
pub fn redact(s: &str) -> String {
    SECRET.replace_all(s, "[removed]").into_owned()
}

/// One line: time, kind, message. Never pass dictated text, transcripts or titles here.
pub fn log(kind: &str, message: &str) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some(path) = file() else { return };
    let _guard = WRITE.lock();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(&path, path.with_file_name("diagnostics.1.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let line = redact(&message.replace(['\r', '\n'], " "));
        let _ = writeln!(f, "{} [{kind}] {line}", chrono::Utc::now().to_rfc3339());
    }
}

/// The last `max_lines` lines of the log (both files).
pub fn tail(max_lines: usize) -> Vec<String> {
    let Some(path) = file() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    for p in [path.with_file_name("diagnostics.1.log"), path] {
        if let Ok(text) = std::fs::read_to_string(p) {
            lines.extend(text.lines().map(str::to_string));
        }
    }
    let skip = lines.len().saturating_sub(max_lines);
    lines.split_off(skip)
}

/// Deletes both log files. Returns an error naming a file that could not be deleted.
pub fn delete() -> Result<(), String> {
    let Some(path) = file() else { return Ok(()) };
    let _guard = WRITE.lock();
    for p in [path.with_file_name("diagnostics.1.log"), path] {
        match std::fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", p.display())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_tokens_are_removed() {
        let s = redact("failed with key gsk_abcdefghijklmnop1234 and Bearer abc.def-123456789 ok");
        assert!(!s.contains("gsk_abc") && !s.contains("abc.def"), "{s}");
        assert_eq!(s.matches("[removed]").count(), 2);
        assert_eq!(
            redact("model large-v3 in 412 ms"),
            "model large-v3 in 412 ms"
        );
    }
}
