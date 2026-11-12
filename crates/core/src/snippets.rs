//! Voice snippets: a short spoken trigger expands to a saved block of text.

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use crate::pipeline::tokens::{phrase_regex, PROTECT_CLOSE, PROTECT_DIGIT_BASE, PROTECT_OPEN};
use crate::pipeline::{Change, ChangeKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snippet {
    /// What the user says, e.g. "insert my calendar link".
    pub trigger: String,
    /// What gets written.
    pub body: String,
}

#[derive(Debug, Clone, Default)]
pub struct SnippetSet {
    items: Vec<(Regex, Snippet)>,
}

/// Expanded snippet bodies, parked behind placeholders so later cleanup rules cannot
/// alter them (a snippet body is inserted exactly as saved).
#[derive(Debug, Clone, Default)]
pub struct Protected {
    bodies: Vec<String>,
}

fn placeholder(index: usize) -> String {
    let mut s = String::new();
    s.push(PROTECT_OPEN);
    for d in index.to_string().chars() {
        let digit = d.to_digit(10).unwrap_or(0);
        s.push(char::from_u32(PROTECT_DIGIT_BASE + digit).unwrap_or(PROTECT_OPEN));
    }
    s.push(PROTECT_CLOSE);
    s
}

impl Protected {
    pub fn restore(&self, text: &str) -> String {
        let mut out = text.to_string();
        // Highest index first so "1" never matches inside "10".
        for (i, body) in self.bodies.iter().enumerate().rev() {
            out = out.replace(&placeholder(i), body);
        }
        out
    }
}

impl SnippetSet {
    pub fn new(mut snippets: Vec<Snippet>) -> Self {
        // Longest trigger first, so "insert my work address" wins over "insert my work".
        snippets.sort_by_key(|s| std::cmp::Reverse(s.trigger.len()));
        let items = snippets
            .into_iter()
            .filter_map(|s| phrase_regex(&s.trigger, r"\s,").map(|re| (re, s)))
            .collect();
        Self { items }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn expand(&self, text: &str, changes: &mut Vec<Change>) -> (String, Protected) {
        let mut protected = Protected::default();
        let mut out = text.to_string();
        for (re, snippet) in &self.items {
            out = re
                .replace_all(&out, |caps: &Captures| {
                    let index = protected.bodies.len();
                    protected.bodies.push(snippet.body.clone());
                    changes.push(Change {
                        kind: ChangeKind::Snippet,
                        before: caps[0].to_string(),
                        after: snippet.body.clone(),
                    });
                    placeholder(index)
                })
                .into_owned();
        }
        (out, protected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> SnippetSet {
        SnippetSet::new(vec![
            Snippet {
                trigger: "insert my calendar link".into(),
                body: "https://cal.example/kunal".into(),
            },
            Snippet {
                trigger: "insert my sign off".into(),
                body: "Regards,\nKunal".into(),
            },
        ])
    }

    #[test]
    fn expands_and_restores() {
        let (text, protected) = set().expand("Book here: Insert my calendar link", &mut Vec::new());
        assert!(!text.contains("calendar"));
        assert_eq!(
            protected.restore(&text),
            "Book here: https://cal.example/kunal"
        );
    }

    #[test]
    fn trigger_may_contain_commas_from_the_recogniser() {
        let (text, protected) = set().expand("insert, my sign off", &mut Vec::new());
        assert_eq!(protected.restore(&text), "Regards,\nKunal");
    }

    #[test]
    fn many_snippets_keep_their_order() {
        let snippets: Vec<Snippet> = (0..12)
            .map(|i| Snippet {
                trigger: format!("code word{i}x"),
                body: format!("<{i}>"),
            })
            .collect();
        let spoken: Vec<String> = (0..12).map(|i| format!("code word{i}x")).collect();
        let (text, protected) =
            SnippetSet::new(snippets).expand(&spoken.join(" and "), &mut Vec::new());
        let expected: Vec<String> = (0..12).map(|i| format!("<{i}>")).collect();
        assert_eq!(protected.restore(&text), expected.join(" and "));
    }
}
