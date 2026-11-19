//! Compares cloud speech-recognition settings on a folder of recordings (developer tool).
//!
//!   asr-eval <variant> <out.tsv> <folder> [--vocab "word, word, ..."]
//!
//! Variants: `hi` (the app's Hinglish request: Hindi + Roman example prompt), `en` (English +
//! glossary), `auto` (no fixed language, mixed example + glossary), `auto-plain` (no language,
//! no prompt). Leading exact zeros (a microphone that had not started) are cut first, as the
//! app does since 0.4.3. Output: "id<TAB>text" lines for `wer`. The key is read from Windows
//! Credential Manager. Requests are paced to stay inside the free tier's per-minute limit.

use std::path::Path;
use std::time::Duration;

use app_asr::audio;
use app_desktop_lib::{credentials, providers};

const MIXED_EXAMPLE: &str = "Okay, so the meeting is at 4 PM and I will send the report by Friday. Haan bhai, main abhi kaam kar raha hoon, baad mein call karunga. Mujhe yeh topic revision style mein samjhao, okay?";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (variant, out, dir) = (&args[0], &args[1], Path::new(&args[2]));
    let vocab = args
        .iter()
        .position(|a| a == "--vocab")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_default();
    let glossary = if vocab.is_empty() {
        String::new()
    } else {
        format!(" Glossary: {vocab}.")
    };
    let key = credentials::load("groq").expect("no Groq key in Credential Manager");
    let preset = providers::preset("groq").expect("groq preset");
    let cfg = providers::Config {
        provider: "groq".into(),
        base_url: preset.base_url.to_string(),
        model: "whisper-large-v3".into(),
        key: Some(key),
    };
    let (lang, prompt): (Option<&str>, String) = match variant.as_str() {
        "hi" => (
            Some("hi"),
            format!("{} Kunal Kanjwani.", app_core::llm::ROMAN_HINGLISH_EXAMPLE),
        ),
        "en" => (Some("en"), format!("Kunal Kanjwani.{glossary}")),
        "auto" => (None, format!("{MIXED_EXAMPLE} Kunal Kanjwani.{glossary}")),
        "auto-plain" => (None, String::new()),
        other => panic!("unknown variant {other}"),
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    files.sort();
    let mut lines = Vec::new();
    for f in &files {
        let mut samples = audio::read_audio_16k_mono(f).expect("audio");
        let zeros = samples
            .iter()
            .position(|s| *s != 0.0)
            .unwrap_or(samples.len());
        samples.drain(..zeros);
        let id = f.file_stem().unwrap().to_string_lossy().to_string();
        if samples.len() < 8000 {
            lines.push(format!("{id}\t"));
            continue;
        }
        let body = app_asr::opus::encode_ogg_opus(&samples).expect("opus");
        let mut text = String::new();
        for attempt in 0..4 {
            match providers::transcribe(
                &cfg,
                body.clone(),
                lang,
                Some(prompt.trim()).filter(|p| !p.is_empty()),
                Duration::from_secs(30),
            ) {
                Ok((t, _)) => {
                    text = t.replace(['\t', '\n', '\r'], " ").trim().to_string();
                    break;
                }
                Err(providers::CallError::RateLimited) => {
                    std::thread::sleep(Duration::from_secs(20 * (attempt + 1)))
                }
                Err(e) => {
                    eprintln!("{id}: {e}");
                    break;
                }
            }
        }
        eprintln!("{id}: {text}");
        lines.push(format!("{id}\t{text}"));
        std::fs::write(out, lines.join("\n") + "\n").expect("write");
        // About 17 requests a minute: inside the free tier's 20 per minute.
        std::thread::sleep(Duration::from_millis(3500));
    }
}
