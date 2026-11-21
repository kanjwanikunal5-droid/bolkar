//! Personal dictionary: write a term the way the user wants it spelled.
//!
//! Stage 0 matches the term itself and its listed spoken variants, ignoring case. It never
//! replaces a different word just because it sounds similar.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::pipeline::tokens::is_word_char;
use crate::pipeline::{Change, ChangeKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Term {
    /// The exact spelling to write, e.g. "EBITDA".
    pub term: String,
    /// Other ways the recogniser may write it, e.g. ["e bit da"].
    #[serde(default)]
    pub spoken_variants: Vec<String>,
    /// When false the term is only a hint to the recogniser and the AI clean-up; it is never
    /// forced into the text. Used for names that are also everyday words ("Cursor",
    /// "Excel"), so "move the cursor" stays as it is.
    #[serde(default = "yes")]
    pub auto_replace: bool,
}

fn yes() -> bool {
    true
}

impl Term {
    pub fn new(term: &str) -> Self {
        Self {
            term: term.to_string(),
            spoken_variants: Vec::new(),
            auto_replace: true,
        }
    }

    pub fn hint_only(term: &str) -> Self {
        Self {
            auto_replace: false,
            ..Self::new(term)
        }
    }

    pub fn with_variants(term: &str, variants: &[&str]) -> Self {
        Self {
            spoken_variants: variants.iter().map(|v| v.to_string()).collect(),
            ..Self::new(term)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    /// Spoken phrase (lower-case words joined by single spaces) -> term to write.
    phrases: HashMap<String, String>,
    /// Most words in any spoken phrase.
    max_words: usize,
    terms: Vec<String>,
}

impl Dictionary {
    pub fn new(terms: Vec<Term>) -> Self {
        let mut pairs: Vec<(String, String)> = Vec::new();
        let all: Vec<String> = terms.iter().map(|t| t.term.clone()).collect();
        for t in terms.iter().filter(|t| t.auto_replace) {
            pairs.push((t.term.clone(), t.term.clone()));
            for v in &t.spoken_variants {
                pairs.push((v.clone(), t.term.clone()));
            }
        }
        // One lookup table instead of one search pattern per word: measured 21 Nov 2026, 5,000
        // terms took 1.31 s to build and 117 ms to apply as patterns on every dictation.
        // Longest spoken form first, so "power bi desktop" wins over "power bi"; for the same
        // spoken form the first term listed wins.
        pairs.sort_by_key(|p| std::cmp::Reverse(p.0.len()));
        let mut phrases = HashMap::new();
        let mut max_words = 0;
        for (spoken, term) in pairs {
            let words = phrase_words(&spoken);
            if words.is_empty() {
                continue;
            }
            max_words = max_words.max(words.len());
            phrases.entry(words.join(" ")).or_insert(term);
        }
        Self {
            phrases,
            max_words,
            terms: all,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Every term's spelling, including hint-only terms.
    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Vocabulary hint for the speech recogniser. Whisper reads only about the last 224
    /// tokens of its prompt, so the list is capped at `max_chars` (terms are given in
    /// priority order: the first ones are kept).
    ///
    /// The hint is written as a punctuated sentence ("Glossary: A, B, C."). Whisper copies the
    /// style of its prompt: a bare word list made it drop full stops (measured 13 Nov 2026).
    pub fn recogniser_hint(&self, max_chars: usize) -> String {
        const LEAD: &str = "Glossary: ";
        let mut list = String::new();
        for t in &self.terms {
            let extra = if list.is_empty() {
                t.len()
            } else {
                t.len() + 2
            };
            if LEAD.len() + list.len() + extra + 1 > max_chars {
                break;
            }
            if !list.is_empty() {
                list.push_str(", ");
            }
            list.push_str(t);
        }
        if list.is_empty() {
            String::new()
        } else {
            format!("{LEAD}{list}.")
        }
    }

    /// Writes every listed term its way. Words match ignoring case and must be separated only
    /// by spaces or hyphens; at each word the longest listed phrase wins; replaced text is not
    /// matched again.
    pub fn apply(&self, text: &str, changes: &mut Vec<Change>) -> String {
        if self.phrases.is_empty() {
            return text.to_string();
        }
        let spans = word_spans(text);
        let mut out = String::with_capacity(text.len());
        let (mut copied, mut i) = (0, 0);
        while i < spans.len() {
            let mut found = None;
            let longest = self.max_words.min(spans.len() - i);
            for n in (1..=longest).rev() {
                let joined_ok = (i + 1..i + n).all(|k| {
                    let sep = &text[spans[k - 1].1..spans[k].0];
                    !sep.is_empty() && sep.chars().all(|c| c.is_whitespace() || c == '-')
                });
                if !joined_ok {
                    continue;
                }
                let key = spans[i..i + n]
                    .iter()
                    .map(|&(a, b)| text[a..b].to_lowercase())
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(term) = self.phrases.get(&key) {
                    found = Some((n, term));
                    break;
                }
            }
            match found {
                Some((n, term)) => {
                    let (start, end) = (spans[i].0, spans[i + n - 1].1);
                    out.push_str(&text[copied..start]);
                    let before = &text[start..end];
                    if before != term {
                        changes.push(Change {
                            kind: ChangeKind::Dictionary,
                            before: before.to_string(),
                            after: term.clone(),
                        });
                    }
                    out.push_str(term);
                    copied = end;
                    i += n;
                }
                None => i += 1,
            }
        }
        out.push_str(&text[copied..]);
        out
    }
}

/// The words of a spoken form, lower case (what a phrase is looked up by).
fn phrase_words(spoken: &str) -> Vec<String> {
    spoken
        .split(|c: char| !is_word_char(c))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Byte ranges of the words in a text.
fn word_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (is_word_char(c), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        spans.push((s, text.len()));
    }
    spans
}

/// The dictionary terms worth giving the AI clean-up for this text: terms whose words all occur
/// in it first, then terms with a word that looks like one of its words (same first four
/// letters, e.g. "kaggel" -> "Kaggle"), at most `max`. Every term still corrects spellings
/// locally; this only keeps the AI request small. Measured 20 Nov 2026: sending all 1,564
/// terms made each clean-up request 13-19 KB, and requests timed out or hit Groq's limits.
pub fn relevant_terms(terms: &[String], text: &str, max: usize) -> Vec<String> {
    let split = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let prefix = |w: &str| -> Option<String> {
        (w.chars().count() >= 4).then(|| w.chars().take(4).collect())
    };
    let words: std::collections::HashSet<String> = split(text).into_iter().collect();
    let prefixes: std::collections::HashSet<String> =
        words.iter().filter_map(|w| prefix(w)).collect();
    let (mut exact, mut near) = (Vec::new(), Vec::new());
    for t in terms {
        let tw = split(t);
        if tw.is_empty() {
            continue;
        }
        if tw.iter().all(|w| words.contains(w)) {
            exact.push(t.clone());
        } else if tw
            .iter()
            .any(|w| prefix(w).is_some_and(|p| prefixes.contains(&p)))
        {
            near.push(t.clone());
        }
    }
    exact.into_iter().chain(near).take(max).collect()
}

/// Built-in word packs. The owner's profile enables Finance and Tech by default.
pub fn pack(name: &str) -> Vec<Term> {
    match name {
        "finance" => vec![
            Term::new("CMA"),
            Term::with_variants("FP&A", &["fp and a", "f p and a", "fpa"]),
            Term::new("EBITDA"),
            Term::new("EBIT"),
            Term::new("working capital"),
            Term::new("variance analysis"),
            Term::new("forecasting"),
            Term::new("budgeting"),
            Term::new("actuals"),
            Term::with_variants("Power BI", &["powerbi"]),
            Term::new("SQL"),
            Term::new("Python"),
            Term::new("pandas"),
            Term::with_variants("NumPy", &["num py", "numpy"]),
            Term::hint_only("Excel"),
            Term::with_variants("VLOOKUP", &["v lookup", "v-lookup"]),
            Term::with_variants("XLOOKUP", &["x lookup", "x-lookup"]),
            Term::new("GST"),
            Term::new("TDS"),
            Term::new("ROI"),
            Term::with_variants("YoY", &["y o y", "year on year"]),
            Term::with_variants("MoM", &["m o m"]),
        ],
        "tech" => vec![
            Term::with_variants("Supabase", &["supa base", "super base"]),
            Term::new("Cloudflare"),
            Term::new("Vercel"),
            Term::with_variants("GitHub", &["git hub"]),
            Term::new("Tauri"),
            Term::hint_only("React"),
            Term::with_variants("TypeScript", &["type script"]),
            Term::new("PostgreSQL"),
            Term::hint_only("Claude"),
            Term::with_variants("ChatGPT", &["chat gpt"]),
            Term::hint_only("Cursor"),
            Term::new("Windsurf"),
            Term::new("Replit"),
        ],
        "study" => crate::packs::study(),
        "tools" => crate::packs::tools(),
        "personal" => crate::packs::personal(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_pass_lookup_matches_like_the_old_patterns() {
        let d = Dictionary::new(vec![
            Term::with_variants("Power BI", &["power bee eye"]),
            Term::new("Power BI Desktop"),
            Term::with_variants("XLOOKUP", &["x lookup"]),
            Term::with_variants("mujhe", &["muje"]),
        ]);
        let mut c = Vec::new();
        // Longest phrase wins; case is fixed; hyphen counts as a space; punctuation does not.
        assert_eq!(
            d.apply(
                "open power bi desktop, then power bee eye and X-lookup. Muje x. lookup",
                &mut c
            ),
            "open Power BI Desktop, then Power BI and XLOOKUP. mujhe x. lookup"
        );
        assert_eq!(c.len(), 4);
        // Inside a longer word nothing changes.
        assert_eq!(d.apply("mujhey powerbi", &mut Vec::new()), "mujhey powerbi");
        assert_eq!(d.apply("", &mut Vec::new()), "");
    }

    #[test]
    fn only_terms_that_matter_for_the_text_go_to_the_ai() {
        let mut terms: Vec<String> = (0..2000).map(|i| format!("Term{i} Word{i}")).collect();
        terms.extend(["Power Query", "Kaggle", "ITR", "CIBIL", "Zomato"].map(String::from));
        let text = "I filed my itr and opened power query, then kaggel and the cibil score.";
        let got = relevant_terms(&terms, text, 40);
        assert_eq!(&got[..3], ["Power Query", "ITR", "CIBIL"]);
        assert!(got.contains(&"Kaggle".to_string()), "near match: {got:?}");
        assert!(!got.contains(&"Zomato".to_string()));
        assert!(got.len() <= 40);
        assert!(relevant_terms(&terms, "", 40).is_empty());
    }

    #[test]
    fn packs_fix_spellings_but_leave_everyday_words() {
        let mut terms = pack("finance");
        terms.extend(pack("tech"));
        let d = Dictionary::new(terms);
        let mut changes = Vec::new();
        assert_eq!(
            d.apply(
                "check the yoy numbers in powerbi and the x lookup on git hub",
                &mut changes
            ),
            "check the YoY numbers in Power BI and the XLOOKUP on GitHub"
        );
        // Hint-only words are never forced.
        let s = "Move the cursor and react quickly to excel at work.";
        assert_eq!(d.apply(s, &mut Vec::new()), s);
        assert!(d.terms().iter().any(|t| t == "Cursor"));
    }

    #[test]
    fn recogniser_hint_respects_the_limit_and_order() {
        let d = Dictionary::new(vec![
            Term::new("EBITDA"),
            Term::new("Supabase"),
            Term::new("XLOOKUP"),
        ]);
        assert_eq!(
            d.recogniser_hint(100),
            "Glossary: EBITDA, Supabase, XLOOKUP."
        );
        assert_eq!(d.recogniser_hint(27), "Glossary: EBITDA, Supabase.");
        assert_eq!(d.recogniser_hint(12), "");
    }

    fn dict() -> Dictionary {
        Dictionary::new(vec![
            Term::with_variants("EBITDA", &["e bit da"]),
            Term::new("Power BI"),
            Term::new("CMA"),
        ])
    }

    #[test]
    fn fixes_case_and_variants() {
        let mut changes = Vec::new();
        let out = dict().apply(
            "our ebitda and e bit da are on the power-bi board for the cma exam",
            &mut changes,
        );
        assert_eq!(
            out,
            "our EBITDA and EBITDA are on the Power BI board for the CMA exam"
        );
        assert_eq!(changes.len(), 4);
    }

    #[test]
    fn whole_words_only() {
        let s = "The cinema was packed.";
        assert_eq!(dict().apply(s, &mut Vec::new()), s);
    }

    #[test]
    fn already_correct_text_logs_nothing() {
        let mut changes = Vec::new();
        dict().apply("EBITDA is up.", &mut changes);
        assert!(changes.is_empty());
    }
}

#[cfg(test)]
mod timing {
    use super::*;

    /// Run by hand: cargo test -p app-core --release dictionary::timing -- --ignored --nocapture
    #[test]
    #[ignore]
    fn five_thousand_words() {
        let terms: Vec<Term> = (0..5000)
            .map(|i| Term {
                term: format!("Termword{i}"),
                spoken_variants: vec![format!("term word {i}"), format!("termwurd{i}")],
                auto_replace: true,
            })
            .collect();
        let text = "Today I filed my ITR and checked the dashboard, then I spoke for three minutes about the budget, the forecast and the next steps for the team. ".repeat(6);
        let t0 = std::time::Instant::now();
        let d = Dictionary::new(terms);
        let build = t0.elapsed();
        let t1 = std::time::Instant::now();
        let out = d.apply(&text, &mut Vec::new());
        let apply = t1.elapsed();
        println!(
            "5000 terms: build {build:?}, apply to {} words {apply:?}",
            text.split_whitespace().count()
        );
        assert_eq!(out.len(), text.len());
    }
}
