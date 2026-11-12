//! Shared code for the benchmark command-line tools.

use std::collections::BTreeMap;

use app_core::metrics::{char_errors, word_errors, EditCounts};
use serde::Serialize;

/// Parses "id<TAB>text" lines. Blank lines and lines starting with '#' are skipped.
pub fn parse_tsv(content: &str) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for (n, line) in content.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (id, text) = line
            .split_once('\t')
            .ok_or_else(|| format!("line {}: expected 'id<TAB>text'", n + 1))?;
        let id = id.trim().trim_start_matches('\u{FEFF}').to_string();
        if map.insert(id.clone(), text.trim().to_string()).is_some() {
            return Err(format!("line {}: duplicate id '{id}'", n + 1));
        }
    }
    Ok(map)
}

#[derive(Debug, Serialize)]
pub struct UtteranceScore {
    pub id: String,
    pub wer: Option<f64>,
    pub cer: Option<f64>,
    pub words: EditCounts,
    pub chars: EditCounts,
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// Corpus-level rates: total errors / total reference tokens (not a mean of means).
    pub wer: Option<f64>,
    pub cer: Option<f64>,
    pub words: EditCounts,
    pub chars: EditCounts,
    pub utterances: Vec<UtteranceScore>,
    /// Reference ids with no hypothesis. They are scored as empty output (all deletions).
    pub missing_hypotheses: Vec<String>,
    /// Hypothesis ids that are not in the reference file. They are ignored.
    pub extra_hypotheses: Vec<String>,
}

pub fn score(
    references: &BTreeMap<String, String>,
    hypotheses: &BTreeMap<String, String>,
) -> Report {
    let mut words = EditCounts::default();
    let mut chars = EditCounts::default();
    let mut utterances = Vec::new();
    let mut missing = Vec::new();

    for (id, reference) in references {
        let hypothesis = match hypotheses.get(id) {
            Some(h) => h.as_str(),
            None => {
                missing.push(id.clone());
                ""
            }
        };
        let w = word_errors(reference, hypothesis);
        let c = char_errors(reference, hypothesis);
        words.add(&w);
        chars.add(&c);
        utterances.push(UtteranceScore {
            id: id.clone(),
            wer: w.rate(),
            cer: c.rate(),
            words: w,
            chars: c,
        });
    }
    let extra = hypotheses
        .keys()
        .filter(|k| !references.contains_key(*k))
        .cloned()
        .collect();

    Report {
        wer: words.rate(),
        cer: chars.rate(),
        words,
        chars,
        utterances,
        missing_hypotheses: missing,
        extra_hypotheses: extra,
    }
}

pub fn percent(rate: Option<f64>) -> String {
    rate.map_or_else(|| "n/a".to_string(), |r| format!("{:.2}%", r * 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_rate_is_pooled_not_averaged() {
        let refs = parse_tsv("a\tone two three four five six seven eight\nb\thello\n").unwrap();
        let hyps = parse_tsv("a\tone two three four five six seven eight\nb\tgoodbye\n").unwrap();
        let report = score(&refs, &hyps);
        // 1 error over 9 reference words (a mean of per-utterance rates would say 50%).
        assert!((report.wer.unwrap() - 1.0 / 9.0).abs() < 1e-9);
        assert_eq!(report.utterances.len(), 2);
    }

    #[test]
    fn missing_hypothesis_counts_as_all_deleted() {
        let refs = parse_tsv("a\tone two\nb\tthree four\n").unwrap();
        let hyps = parse_tsv("a\tone two\nzzz\tignored\n").unwrap();
        let report = score(&refs, &hyps);
        assert_eq!(report.missing_hypotheses, ["b"]);
        assert_eq!(report.extra_hypotheses, ["zzz"]);
        assert!((report.wer.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn tsv_errors_are_reported() {
        assert!(parse_tsv("no tab here").is_err());
        assert!(parse_tsv("a\tx\na\ty").is_err());
        assert_eq!(
            parse_tsv("# comment\n\n\u{FEFF}a\tx\r\n")
                .unwrap()
                .get("a")
                .map(String::as_str),
            Some("x")
        );
    }
}
