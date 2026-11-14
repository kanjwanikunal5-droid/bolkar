//! Applies the offline Hinglish spelling rules to every line of a transcript file.
//!
//!   hinglish-fix in.tsv > out.tsv        ("id<TAB>text" lines)

use std::{env, fs, process::ExitCode};

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: hinglish-fix in.tsv");
        return ExitCode::FAILURE;
    };
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    for line in content.lines() {
        let line = line.trim_start_matches('\u{FEFF}');
        match line.split_once('\t') {
            Some((id, text)) => println!("{id}\t{}", app_core::hinglish::normalise(text)),
            None => println!("{line}"),
        }
    }
    ExitCode::SUCCESS
}
