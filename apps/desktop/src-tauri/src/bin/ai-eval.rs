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
                    )
                }) {
                    Ok(text) if loose(&text) == loose(&case.expected) => {
                        passed += 1;
                        "pass".into()
                    }
                    Ok(text) => format!("DIFFERENT got {text:?}"),
                    Err(why) => {
                        caught += 1;
                        format!("REJECTED ({why}) -> rules used")
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
            std::thread::sleep(Duration::from_millis(2200));
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
        "cases {total} | match {passed} ({:.1}%) | rejected by checks {caught} | errors {errors}",
        passed as f64 * 100.0 / total.max(1) as f64
    );
    println!(
        "latency median {} ms | p90 {} ms | over 2500 ms: {}",
        pct(50),
        pct(90),
        latencies.iter().filter(|l| **l > 2500).count()
    );
}
