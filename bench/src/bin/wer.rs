//! Scores recogniser output against reference transcripts.
//!
//!   wer --ref refs.tsv --hyp hyps.tsv [--json out.json]
//!
//! Both files hold one "id<TAB>text" line per recording.

use std::{env, fs, process::ExitCode};

use app_bench::{parse_tsv, percent, score};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let (Some(ref_path), Some(hyp_path)) = (arg(&args, "--ref"), arg(&args, "--hyp")) else {
        return Err("usage: wer --ref refs.tsv --hyp hyps.tsv [--json out.json]".to_string());
    };
    let references =
        parse_tsv(&fs::read_to_string(&ref_path).map_err(|e| format!("{ref_path}: {e}"))?)
            .map_err(|e| format!("{ref_path}: {e}"))?;
    let hypotheses =
        parse_tsv(&fs::read_to_string(&hyp_path).map_err(|e| format!("{hyp_path}: {e}"))?)
            .map_err(|e| format!("{hyp_path}: {e}"))?;
    if references.is_empty() {
        return Err(format!("{ref_path}: no reference lines"));
    }

    let report = score(&references, &hypotheses);

    println!("{:<24} {:>9} {:>9} {:>6}", "id", "WER", "CER", "words");
    for u in &report.utterances {
        println!(
            "{:<24} {:>9} {:>9} {:>6}",
            u.id,
            percent(u.wer),
            percent(u.cer),
            u.words.reference_len
        );
    }
    println!("{}", "-".repeat(51));
    println!(
        "{:<24} {:>9} {:>9} {:>6}",
        "ALL (pooled)",
        percent(report.wer),
        percent(report.cer),
        report.words.reference_len
    );
    println!(
        "substitutions {} | deletions {} | insertions {}",
        report.words.substitutions, report.words.deletions, report.words.insertions
    );
    if !report.missing_hypotheses.is_empty() {
        println!(
            "WARNING: no output for {} recording(s), scored as empty: {:?}",
            report.missing_hypotheses.len(),
            report.missing_hypotheses
        );
    }
    if !report.extra_hypotheses.is_empty() {
        println!(
            "NOTE: {} hypothesis id(s) not in the reference file were ignored",
            report.extra_hypotheses.len()
        );
    }

    if let Some(out) = arg(&args, "--json") {
        let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        fs::write(&out, json).map_err(|e| format!("{out}: {e}"))?;
        println!("wrote {out}");
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
