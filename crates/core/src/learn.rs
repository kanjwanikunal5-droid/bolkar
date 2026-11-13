//! Learning dictionary words from the user's own corrections.
//!
//! When a dictation is fixed by hand, the word-level differences become suggestions such as
//! "ebit da" -> "EBITDA". Nothing is added without the user's confirmation, and only short
//! replacements (1 to 3 words) are suggested; rewritten sentences are ignored.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    /// How it was written before (what the recogniser heard), lower case.
    pub heard: String,
    /// How the user wants it written.
    pub term: String,
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '&' && c != '+' && c != '#')
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word-level longest-common-subsequence alignment; returns the changed blocks.
fn changed_blocks(a: &[String], b: &[String]) -> Vec<(Vec<String>, Vec<String>)> {
    let key = |w: &String| w.to_lowercase();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut blocks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut da, mut db) = (Vec::new(), Vec::new());
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            if !da.is_empty() || !db.is_empty() {
                blocks.push((std::mem::take(&mut da), std::mem::take(&mut db)));
            }
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            db.push(b[j].clone());
            j += 1;
        } else {
            da.push(a[i].clone());
            i += 1;
        }
    }
    if !da.is_empty() || !db.is_empty() {
        blocks.push((da, db));
    }
    // Case-only differences count as changes too ("ebitda" -> "EBITDA"), which the exact
    // comparison above already caught; drop blocks that are identical ignoring case AND
    // spelling only differs in case for ordinary lower-case words.
    blocks
        .into_iter()
        .filter(|(x, y)| {
            let same = x.len() == y.len() && x.iter().zip(y).all(|(p, q)| key(p) == key(q));
            // A pure capitalisation change is only worth learning when the new form is not
            // simply "first letter upper case" (sentence starts).
            !same || y.iter().any(|w| w.chars().skip(1).any(char::is_uppercase))
        })
        .collect()
}

/// Suggests dictionary entries from an edit of `before` into `after`.
pub fn suggest(before: &str, after: &str) -> Vec<Suggestion> {
    let (a, b) = (words(before), words(after));
    let mut out: Vec<Suggestion> = Vec::new();
    for (heard, fixed) in changed_blocks(&a, &b) {
        if heard.is_empty() || fixed.is_empty() || heard.len() > 3 || fixed.len() > 3 {
            continue; // insertions, deletions and rewrites are not vocabulary
        }
        let term = fixed.join(" ");
        if term
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
        {
            continue; // numbers are facts, not vocabulary
        }
        let s = Suggestion {
            heard: heard.join(" ").to_lowercase(),
            term,
        };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learns_replaced_words() {
        let s = suggest(
            "The ebit da margin and the super base project are ready.",
            "The EBITDA margin and the Supabase project are ready.",
        );
        assert_eq!(
            s,
            vec![
                Suggestion {
                    heard: "ebit da".into(),
                    term: "EBITDA".into()
                },
                Suggestion {
                    heard: "super base".into(),
                    term: "Supabase".into()
                },
            ]
        );
    }

    #[test]
    fn learns_capitalisation_of_acronyms_only() {
        assert_eq!(
            suggest("check the ebitda", "check the EBITDA"),
            vec![Suggestion {
                heard: "ebitda".into(),
                term: "EBITDA".into()
            }]
        );
        // Plain sentence-case fixes are not vocabulary.
        assert!(suggest("hello there", "Hello there").is_empty());
    }

    #[test]
    fn ignores_rewrites_numbers_and_insertions() {
        assert!(suggest("we will meet at 3", "we will meet at 4").is_empty());
        assert!(suggest("send it", "please send it today").is_empty());
        assert!(suggest(
            "this sentence is completely different from that one",
            "nothing here matches anything at all ok"
        )
        .is_empty());
    }
}
