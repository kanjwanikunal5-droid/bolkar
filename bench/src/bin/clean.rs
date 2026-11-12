//! Runs one raw transcript through the deterministic cleanup pipeline and prints the result.
//!
//!   clean "So, um, I think we should go."
//!   clean --literal "..."

use std::{env, process::ExitCode};

use app_core::dictionary::Dictionary;
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::SnippetSet;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let literal = args.iter().any(|a| a == "--literal");
    let text: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .collect();
    if text.is_empty() {
        eprintln!("usage: clean [--literal] \"raw transcript\"");
        return ExitCode::FAILURE;
    }
    let settings = Settings {
        level: if literal {
            Level::Literal
        } else {
            Level::Light
        },
        spoken_commands: true,
    };
    let out = pipeline::run(
        &text.join(" "),
        &settings,
        &Dictionary::default(),
        &SnippetSet::default(),
    );
    match serde_json::to_string_pretty(&out) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
