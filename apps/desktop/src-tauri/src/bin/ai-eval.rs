//! Measures the AI clean-up path on the golden fixtures.
//!
//!   ai-eval <provider> [model]       e.g.  ai-eval groq llama-3.1-8b-instant
//!
//! The API key is read from Windows Credential Manager, where the app stores it after the
//! user saves it in Settings. Only the fixture sentences (no personal data) are sent.
//! A case "passes" when the accepted answer matches the expected text after ignoring case,
//! punctuation and spacing. Answers the validator rejects fall back to the rules, exactly as
//! in the app, and are counted separately: they are inventions that were caught.

use std::time::Duration;

use app_core::dictionary::{Dictionary, Term};
use app_core::llm::{self, AppCategory};
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::{Snippet, SnippetSet};
use app_desktop_lib::{credentials, providers};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: String,
    #[serde(default)]
    level: Option<Level>,
    #[serde(default)]
    spoken_commands: Option<bool>,
    #[serde(default)]
    dictionary: Vec<Term>,
    #[serde(default)]
    snippets: Vec<Snippet>,
    input: String,
    expected: String,
}

#[derive(Deserialize)]
struct File {
    cases: Vec<Case>,
}

fn loose(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let provider = args.first().cloned().unwrap_or_else(|| "groq".into());
    let preset = providers::preset(&provider).expect("unknown provider");
    let cfg = providers::Config {
        provider: provider.clone(),
        base_url: preset.base_url.to_string(),
        model: args
            .get(1)
            .cloned()
            .unwrap_or_else(|| preset.model.to_string()),
        key: credentials::load(&provider),
    };
    if preset.needs_key && cfg.key.is_none() {
        eprintln!(
            "No key saved for '{provider}'. Save it in the app (Settings > AI clean-up) first."
        );
        std::process::exit(2);
    }
    // --asr file.wav [file2.wav ...]: cloud speech-to-text speed check on test recordings.
    if let Some(pos) = args.iter().position(|a| a == "--asr") {
        for path in &args[pos + 1..] {
            let samples =
                app_asr::audio::read_wav_16k_mono(std::path::Path::new(path)).expect("wav");
            let audio_ms = samples.len() as u64 * 1000 / 16_000;
            for run in 1..=3 {
                match providers::transcribe(
                    &cfg,
                    providers::wav_bytes(&samples),
                    Some("en"),
                    None,
                    Duration::from_secs(30),
                ) {
                    Ok((text, took)) => println!(
                        "{path} audio {audio_ms} ms | run {run}: {} ms | {text}",
                        took.as_millis()
                    ),
                    Err(e) => println!("{path} run {run}: error {e}"),
                }
                std::thread::sleep(Duration::from_millis(3500));
            }
        }
        return;
    }
    // --translit file.tsv: converts each line's text to Roman Hinglish with the chat model.
    if let Some(pos) = args.iter().position(|a| a == "--translit") {
        let content = std::fs::read_to_string(&args[pos + 1]).expect("tsv");
        for line in content.lines() {
            let Some((id, text)) = line.trim_start_matches('\u{FEFF}').split_once('\t') else {
                continue;
            };
            let system = app_core::llm::transliterate_prompt();
            let user = serde_json::json!({ "text": text }).to_string();
            match providers::chat(&cfg, &system, &user, Duration::from_secs(20)) {
                Ok(a) => {
                    let out = app_core::llm::parse_transliteration(&a.content, text)
                        .unwrap_or_else(|e| format!("[rejected: {e}] {text}"));
                    println!("{id}\t{out}");
                }
                Err(e) => eprintln!("{id}: {e}"),
            }
            std::thread::sleep(Duration::from_millis(6000));
        }
        return;
    }
    // --asr-dir folder [lang] [prompt]: cloud recognition of every WAV, "id<TAB>text" lines.
    if let Some(pos) = args.iter().position(|a| a == "--asr-dir") {
        let dir = std::path::Path::new(&args[pos + 1]);
        let lang = args
            .get(pos + 2)
            .filter(|l| !l.is_empty())
            .map(String::as_str);
        let prompt = args.get(pos + 3).map(String::as_str);
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .expect("folder")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "wav"))
            .collect();
        files.sort();
        for f in files {
            let samples = app_asr::audio::read_wav_16k_mono(&f).expect("wav");
            match providers::transcribe(
                &cfg,
                providers::wav_bytes(&samples),
                lang,
                prompt,
                Duration::from_secs(30),
            ) {
                Ok((text, _)) => println!(
                    "{}\t{}",
                    f.file_stem().unwrap().to_string_lossy(),
                    text.replace(['\t', '\n'], " ")
                ),
                Err(e) => eprintln!("{}: {e}", f.display()),
            }
            std::thread::sleep(Duration::from_millis(3200));
        }
        return;
    }
    if args.iter().any(|a| a == "--models") {
        match providers::list_models(&cfg, Duration::from_secs(20)) {
            Ok(ids) => ids.iter().for_each(|id| println!("{id}")),
            Err(e) => eprintln!("could not list models: {e}"),
        }
        return;
    }
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../spec/fixtures");
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(dir).expect("fixtures") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "json") {
            let f: File = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            cases.extend(f.cases);
        }
    }
    // LITERAL cases never use AI.
    cases.retain(|c| c.level != Some(Level::Literal));
    cases.sort_by(|a, b| a.id.cmp(&b.id));
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .and_then(|v| v.parse::<usize>().ok())
    };
    // --limit N: an even sample of N cases across all categories.
    if let Some(limit) = flag("--limit").filter(|l| *l < cases.len()) {
        let step = cases.len() as f64 / limit as f64;
        cases = (0..limit)
            .map(|i| cases.swap_remove(((i as f64 * step) as usize).min(cases.len() - 1)))
            .collect();
    }
    let pause = Duration::from_millis(flag("--sleep").unwrap_or(2200) as u64);

    let (mut passed, mut caught, mut errors) = (0usize, 0usize, 0usize);
    let mut latencies = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let settings = Settings {
            level: case.level.unwrap_or(Level::Light),
            spoken_commands: case.spoken_commands.unwrap_or(true),
        };
        let dictionary = Dictionary::new(case.dictionary.clone());
        let snippets = SnippetSet::new(case.snippets.clone());
        let prepared = pipeline::prepare(&case.input, &settings, &dictionary, &snippets);
        let rules_text = pipeline::finish(&prepared, &settings).text;
        let req = llm::build_cleanup(
            &prepared,
            settings.level,
            AppCategory::Docs,
            "en",
            dictionary.terms(),
        );
        let outcome = match providers::chat(&cfg, &req.system, &req.user, Duration::from_secs(20)) {
            Err(e) => {
                errors += 1;
                format!("ERROR {e}")
            }
            Ok(answer) => {
                latencies.push(answer.elapsed.as_millis() as u64);
                match llm::parse_reply(&answer.content).and_then(|r| {
                    llm::accept(
                        &r,
                        &prepared,
                        &case.input,
                        dictionary.terms(),
                        settings.level,
                        &rules_text,
                    )
                }) {
                    Ok(text) if loose(&text) == loose(&case.expected) => {
                        passed += 1;
                        "pass".into()
                    }
                    Ok(text) => format!("DIFFERENT got {text:?}"),
                    Err(why) => {
                        // What the app really does: fall back to the rules.
                        caught += 1;
                        if loose(&rules_text) == loose(&case.expected) {
                            passed += 1;
                            "pass".into()
                        } else {
                            format!("REJECTED ({why}) -> rules used: {rules_text:?}")
                        }
                    }
                }
            }
        };
        if outcome != "pass" {
            println!(
                "[{}] {}\n    expected {:?}",
                case.id, outcome, case.expected
            );
        }
        if i + 1 < cases.len() {
            // Stay well inside free-tier request limits.
            std::thread::sleep(pause);
        }
    }
    latencies.sort_unstable();
    let pct = |p: usize| {
        latencies
            .get((latencies.len() * p / 100).min(latencies.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    };
    let total = cases.len();
    println!();
    println!("provider {} model {}", cfg.provider, cfg.model);
    println!(
        "cases {total} | final text matches {passed} ({:.1}%) | AI answers rejected (rules used) {caught} | errors {errors}",
        passed as f64 * 100.0 / total.max(1) as f64
    );
    println!(
        "latency median {} ms | p90 {} ms | over 2500 ms: {}",
        pct(50),
        pct(90),
        latencies.iter().filter(|l| **l > 2500).count()
    );
}
