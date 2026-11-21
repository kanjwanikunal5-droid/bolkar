//! Long dictations: whole-file recognition (what the app did up to 0.4.5) against recognition
//! in pieces of at most ~29 s cut at quiet moments, sent in parallel (developer tool).
//!
//!   chunk-eval <folder-of-wavs> <out-folder> [--lang en|hi|auto]
//!
//! Writes `<id>.full.txt` and `<id>.pieces.txt` and prints, per file: seconds, words and time
//! for both ways. The Groq key is read from Windows Credential Manager. Leading exact zeros are
//! cut first, as the app does.

use std::path::Path;
use std::time::{Duration, Instant};

use app_asr::audio;
use app_desktop_lib::{credentials, meeting, providers};

const MIXED_EXAMPLE: &str = "Okay, so the meeting is at 4 PM and I will send the report by Friday. Haan bhai, main abhi kaam kar raha hoon, baad mein call karunga. Mujhe yeh topic revision style mein samjhao, okay?";

fn recognise(cfg: &providers::Config, samples: &[f32], lang: Option<&str>) -> String {
    let body = app_asr::opus::encode_ogg_opus(samples).expect("opus");
    let prompt = format!("{MIXED_EXAMPLE} Kunal Kanjwani.");
    for attempt in 0..5 {
        match providers::transcribe(
            cfg,
            body.clone(),
            lang,
            Some(&prompt),
            Duration::from_secs(60),
        ) {
            Ok((t, _)) => return t.split_whitespace().collect::<Vec<_>>().join(" "),
            Err(providers::CallError::RateLimited) => {
                std::thread::sleep(Duration::from_secs(10 + 10 * attempt))
            }
            Err(e) => {
                eprintln!("  error: {e}");
                std::thread::sleep(Duration::from_secs(3));
            }
        }
    }
    String::new()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (dir, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let lang = match args
        .iter()
        .position(|a| a == "--lang")
        .and_then(|i| args.get(i + 1))
    {
        Some(l) if l == "en" || l == "hi" => Some(l.clone()),
        _ => None,
    };
    std::fs::create_dir_all(out).expect("out");
    let key = credentials::load("groq").expect("no Groq key in Credential Manager");
    let preset = providers::preset("groq").expect("groq preset");
    let cfg = providers::Config {
        provider: "groq".into(),
        base_url: preset.base_url.to_string(),
        model: "whisper-large-v3".into(),
        key: Some(key),
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    files.sort();
    println!("id\tsecs\tfull_words\tpiece_words\tpieces\tfull_ms\tpieces_ms");
    for f in &files {
        let id = f.file_stem().unwrap().to_string_lossy().to_string();
        let mut s = audio::read_audio_16k_mono(f).expect("audio");
        let z = s.iter().position(|x| *x != 0.0).unwrap_or(s.len());
        s.drain(..z);
        let secs = s.len() as f64 / 16000.0;
        let t0 = Instant::now();
        let full = recognise(&cfg, &s, lang.as_deref());
        let full_ms = t0.elapsed().as_millis();
        std::thread::sleep(Duration::from_secs(4));
        let pieces = meeting::split_for_recognition(&s);
        let t1 = Instant::now();
        let texts: Vec<String> = std::thread::scope(|sc| {
            let handles: Vec<_> = pieces
                .iter()
                .map(|(_, p)| {
                    let (cfg, lang) = (&cfg, lang.as_deref());
                    sc.spawn(move || recognise(cfg, p, lang))
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_default())
                .collect()
        });
        let pieces_ms = t1.elapsed().as_millis();
        let joined = texts.join(" ");
        std::fs::write(out.join(format!("{id}.full.txt")), &full).ok();
        std::fs::write(out.join(format!("{id}.pieces.txt")), texts.join("\n|\n")).ok();
        println!(
            "{id}\t{secs:.0}\t{}\t{}\t{}\t{full_ms}\t{pieces_ms}",
            full.split_whitespace().count(),
            joined.split_whitespace().count(),
            pieces.len()
        );
        std::thread::sleep(Duration::from_secs(8));
    }
}
