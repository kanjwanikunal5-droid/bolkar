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
}

#[derive(Debug, Clone, Default)]
pub struct Dictionary {
    rules: Vec<(Regex, String)>,
}

impl Dictionary {
    pub fn new(terms: Vec<Term>) -> Self {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for t in &terms {
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
        Self { rules }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dict() -> Dictionary {
        Dictionary::new(vec![
            Term {
                term: "EBITDA".into(),
                spoken_variants: vec!["e bit da".into()],
            },
            Term {
                term: "Power BI".into(),
                spoken_variants: vec![],
            },
            Term {
                term: "CMA".into(),
                spoken_variants: vec![],
            },
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
