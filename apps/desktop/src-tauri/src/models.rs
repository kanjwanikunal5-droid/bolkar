//! Speech model catalogue and downloader (resumable, checksum-verified, fixed addresses).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub file: &'static str,
    pub size_mb: u32,
    /// Rough memory needed while loaded.
    pub ram_mb: u32,
    pub sha256: &'static str,
    pub note: &'static str,
}

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/";

/// Checksums recorded from the files downloaded on 12 November 2026 (large-v3: 14 November
/// 2026, matches the SHA-256 published on Hugging Face).
pub const CATALOGUE: &[ModelInfo] = &[
    ModelInfo {
        id: "large-v3",
        label: "Large v3 (best for Hinglish, with AI clean-up on)",
        file: "ggml-large-v3-q5_0.bin",
        size_mb: 1031,
        ram_mb: 2600,
        sha256: "d75795ecff3f83b5faa89d1900604ad8c780abd5739fae406de19f23ecd98ad1",
        note: "About 3 times slower than Turbo (about 1 s per dictation on this laptop's graphics card). Writes Hinglish in Hindi letters; AI clean-up turns it into Roman letters, so turn AI clean-up on.",
    },
    ModelInfo {
        id: "large-v3-turbo",
        label: "Large v3 Turbo (best accuracy)",
        file: "ggml-large-v3-turbo-q5_0.bin",
        size_mb: 547,
        ram_mb: 1500,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        note: "Needs a dedicated graphics card to be quick.",
    },
    ModelInfo {
        id: "small",
        label: "Small (balanced)",
        file: "ggml-small-q5_1.bin",
        size_mb: 181,
        ram_mb: 700,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        note: "",
    },
    ModelInfo {
        id: "base",
        label: "Base (fastest, least accurate)",
        file: "ggml-base-q5_1.bin",
        size_mb: 57,
        ram_mb: 350,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        note: "",
    },
];

pub fn find(id: &str) -> Option<&'static ModelInfo> {
    CATALOGUE.iter().find(|m| m.id == id)
}

pub fn path_for(dir: &Path, model: &ModelInfo) -> PathBuf {
    dir.join(model.file)
}

pub fn is_installed(dir: &Path, model: &ModelInfo) -> bool {
    path_for(dir, model).is_file()
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Checks a finished download and moves it into place. A wrong checksum deletes the file.
pub fn verify_and_install(
    partial: &Path,
    dest: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    let actual = sha256_file(partial)?;
    if actual != expected_sha256 {
        let _ = fs::remove_file(partial);
        return Err(
            "The downloaded model is damaged (checksum mismatch). Please download it again.".into(),
        );
    }
    fs::rename(partial, dest).map_err(|e| e.to_string())
}

/// Downloads a model into `dir`. An interrupted download resumes from the `.part` file.
/// `progress(done_bytes, total_bytes)` is called as data arrives.
pub fn download(
    dir: &Path,
    model: &ModelInfo,
    progress: &dyn Fn(u64, u64),
) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let dest = path_for(dir, model);
    if dest.is_file() {
        return Ok(dest);
    }
    let partial = dir.join(format!("{}.part", model.file));
    let have = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);

    let client = reqwest::blocking::Client::builder()
        .timeout(None)
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client.get(format!("{BASE_URL}{}", model.file));
    if have > 0 {
        request = request.header("Range", format!("bytes={have}-"));
    }
    let mut response = request
        .send()
        .map_err(|e| format!("Download failed: {e}. Check the internet connection."))?;
    let status = response.status();
    if !(status.is_success()) {
        return Err(format!("Download failed (server answered {status})."));
    }
    // 206 = the server honoured the resume; 200 = it is sending the whole file again.
    let resumed = status.as_u16() == 206;
    let total = response.content_length().unwrap_or(0) + if resumed { have } else { 0 };
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&partial)
        .map_err(|e| e.to_string())?;
    let mut done = if resumed { have } else { 0 };
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|e| format!("Download interrupted: {e}. Try again to resume."))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|e| format!("Could not write the model file: {e}"))?;
        done += n as u64;
        progress(done, total);
    }
    file.flush().map_err(|e| e.to_string())?;
    drop(file);
    verify_and_install(&partial, &dest, model.sha256)?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("app-desktop-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn catalogue_is_consistent() {
        assert!(find("large-v3-turbo").is_some());
        assert!(find("nope").is_none());
        for m in CATALOGUE {
            assert_eq!(m.sha256.len(), 64);
            assert!(m.file.ends_with(".bin"));
        }
    }

    #[test]
    fn good_file_is_installed_and_corrupt_file_is_rejected() {
        let dir = temp_dir("verify");
        let part = dir.join("m.part");
        fs::write(&part, b"hello").unwrap();
        // sha256("hello")
        let good = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        verify_and_install(&part, &dir.join("m.bin"), good).unwrap();
        assert!(dir.join("m.bin").is_file());

        fs::write(&part, b"hellp").unwrap();
        assert!(verify_and_install(&part, &dir.join("m2.bin"), good).is_err());
        assert!(!part.exists(), "a damaged download must be removed");
        assert!(!dir.join("m2.bin").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
