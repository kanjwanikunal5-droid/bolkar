//! Personal dictionary: write a term the way the user wants it spelled.
//!
//! Stage 0 matches the term itself and its listed spoken variants, ignoring case. It never
//! replaces a different word just because it sounds similar.

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::pipeline::tokens::phrase_regex;
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
    rules: Vec<(Regex, String)>,
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
        // Longest spoken form first, so "power bi desktop" wins over "power bi".
        pairs.sort_by_key(|p| std::cmp::Reverse(p.0.len()));
        let rules = pairs
            .into_iter()
            .filter_map(|(spoken, term)| phrase_regex(&spoken, r"\s\-").map(|re| (re, term)))
            .collect();
        Self { rules, terms: all }
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
    pub fn recogniser_hint(&self, max_chars: usize) -> String {
        let mut out = String::new();
        for t in &self.terms {
            let extra = if out.is_empty() { t.len() } else { t.len() + 2 };
            if out.len() + extra > max_chars {
                break;
            }
            if !out.is_empty() {
                out.push_str(", ");
            }
            out.push_str(t);
        }
        out
    }

    pub fn apply(&self, text: &str, changes: &mut Vec<Change>) -> String {
        let mut out = text.to_string();
        for (re, term) in &self.rules {
            out = re
                .replace_all(&out, |caps: &Captures| {
                    if &caps[0] != term {
                        changes.push(Change {
                            kind: ChangeKind::Dictionary,
                            before: caps[0].to_string(),
                            after: term.clone(),
                        });
                    }
                    term.clone()
                })
                .into_owned();
        }
        out
    }
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
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(d.recogniser_hint(100), "EBITDA, Supabase, XLOOKUP");
        assert_eq!(d.recogniser_hint(16), "EBITDA, Supabase");
        assert_eq!(d.recogniser_hint(3), "");
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
