//! Filler-word and stutter removal.
//!
//! Hesitation sounds ("um", "uh") are always removed. Phrases that can also carry meaning
//! ("you know", "like", Hindi "matlab" / "toh") are removed only when the punctuation
//! around them shows they were said as a filler.

use std::sync::LazyLock;

use regex::Regex;

use super::tokens::{detokenize, is_punct, is_sentence_end, tokenize, Tok};
use super::{Change, ChangeKind};

static HESITATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:u+m+|u+h+m*|u+m+h+|e+r+m*|h+m+|m+h*m+|a+h+)$").unwrap());

/// Removed only when there is a comma on both sides.
const BOTH_COMMAS: &[&str] = &[
    "like",
    "matlab",
    "toh",
    "na",
    "haan",
    "मतलब",
    "तो",
    "ना",
    "हाँ",
    "हां",
];

/// Function words that are safe to de-duplicate ("I I think", "the the"). Deliberately a
/// closed English list: Hindi reduplication ("dheere dheere") is real grammar.
const STUTTER_WORDS: &[&str] = &[
    "i", "we", "you", "he", "she", "it", "they", "the", "a", "an", "to", "of", "in", "on", "at",
    "and", "but", "is", "was", "are", "for", "with", "this", "my", "our", "your", "if", "can",
    "will", "would", "should", "could", "do", "did", "have", "has",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Hesitation,
    /// "you know", "I mean": a comma or sentence edge on each side.
    Parenthetical,
    BothCommas,
}

fn word_lower(t: Option<&Tok>) -> Option<String> {
    match t {
        Some(Tok::Word(w)) => Some(w.to_lowercase()),
        _ => None,
    }
}

/// Index of the previous token that is neither whitespace nor already deleted.
fn prev_sig(toks: &[Tok], del: &[bool], i: usize) -> Option<usize> {
    (0..i)
        .rev()
        .find(|&j| !del[j] && !matches!(toks[j], Tok::Space(_)))
}

fn next_sig(toks: &[Tok], del: &[bool], i: usize) -> Option<usize> {
    (i + 1..toks.len()).find(|&j| !del[j] && !matches!(toks[j], Tok::Space(_)))
}

fn newline_between(toks: &[Tok], a: usize, b: usize) -> bool {
    toks[a + 1..b]
        .iter()
        .any(|t| matches!(t, Tok::Space(s) if s.contains('\n')))
}

/// Returns the index of the filler's last token and its kind.
fn match_filler(toks: &[Tok], del: &[bool], i: usize) -> Option<(usize, Kind)> {
    let lw = word_lower(toks.get(i))?;

    if let (Some(Tok::Space(sp)), Some(l2)) = (toks.get(i + 1), word_lower(toks.get(i + 2))) {
        if !sp.contains('\n') && ((lw == "you" && l2 == "know") || (lw == "i" && l2 == "mean")) {
            return Some((i + 2, Kind::Parenthetical));
        }
    }
    if HESITATION.is_match(&lw) {
        // "5 mm" is millimetres, not a hesitation.
        let after_number = prev_sig(toks, del, i)
            .and_then(|j| word_lower(toks.get(j)))
            .is_some_and(|w| w.starts_with(|c: char| c.is_ascii_digit()));
        if !after_number {
            return Some((i, Kind::Hesitation));
        }
    }
    if BOTH_COMMAS.contains(&lw.as_str()) {
        return Some((i, Kind::BothCommas));
    }
    None
}

/// True when exactly one word stands between the start of the sentence and this comma
/// ("So, um, I think"): that comma belongs to the opening word and is kept.
///
/// A short greeting is treated the same way: "Hi Rahul, um, I wanted" keeps its comma.
fn single_word_before(toks: &[Tok], del: &[bool], comma: usize) -> bool {
    const GREETINGS: &[&str] = &[
        "hi", "hello", "hey", "dear", "thanks", "thank", "okay", "ok", "namaste", "good",
    ];
    let mut words: Vec<String> = Vec::new();
    let mut at = comma;
    while let Some(w) = prev_sig(toks, del, at) {
        // A line break starts a new sentence, whatever comes before it.
        if newline_between(toks, w, at) {
            break;
        }
        match &toks[w] {
            Tok::Word(word) => words.push(word.to_lowercase()),
            t if is_sentence_end(t) => break,
            _ => return false, // another comma or symbol: this is mid-sentence
        }
        at = w;
        if words.len() > 3 {
            return false;
        }
    }
    match words.len() {
        1 => true,
        2 | 3 => words
            .last()
            .is_some_and(|first| GREETINGS.contains(&first.as_str())),
        _ => false,
    }
}

pub fn remove_fillers(text: &str, changes: &mut Vec<Change>) -> String {
    let toks = tokenize(text);
    let n = toks.len();
    let mut del = vec![false; n];

    let mut i = 0;
    while i < n {
        if del[i] {
            i += 1;
            continue;
        }
        let Some((end, kind)) = match_filler(&toks, &del, i) else {
            i += 1;
            continue;
        };

        let left = prev_sig(&toks, &del, i);
        let right = next_sig(&toks, &del, end);

        let l_comma = left.is_some_and(|j| is_punct(&toks[j], ","));
        let l_start = match left {
            None => true,
            Some(j) => is_sentence_end(&toks[j]) || newline_between(&toks, j, i),
        };
        let l_soft = left.is_some_and(|j| matches!(&toks[j], Tok::Punct(p) if matches!(p.as_str(), "..." | ":" | ";" | "-" | "\u{2014}")));
        let r_ellipsis = right.is_some_and(|k| is_punct(&toks[k], "..."));
        let r_comma = right.is_some_and(|k| is_punct(&toks[k], ","))
            || (kind == Kind::Hesitation && r_ellipsis);
        let r_end = match right {
            None => true,
            Some(k) => is_sentence_end(&toks[k]),
        };

        let eligible = match kind {
            Kind::Hesitation => true,
            Kind::Parenthetical => {
                (l_comma || l_start || l_soft) && (r_comma || r_end) && !(l_start && r_end)
            }
            Kind::BothCommas => l_comma && r_comma,
        };
        if !eligible {
            i = end + 1;
            continue;
        }

        let filler_text = detokenize(&toks[i..=end]);
        for d in del.iter_mut().take(end + 1).skip(i) {
            *d = true;
        }
        if r_comma {
            if let Some(k) = right {
                del[k] = true;
            }
        }
        if l_comma {
            let keep = r_comma && left.is_some_and(|j| single_word_before(&toks, &del, j));
            if !keep {
                if let Some(j) = left {
                    del[j] = true;
                }
            }
        } else if l_start && r_end && !r_comma && kind == Kind::Hesitation {
            // The whole "sentence" was just "Um." - drop its full stop too.
            if let Some(k) = right {
                del[k] = true;
            }
        }
        changes.push(Change {
            kind: ChangeKind::Filler,
            before: filler_text,
            after: String::new(),
        });
        i = end + 1;
    }

    let kept: Vec<Tok> = toks
        .into_iter()
        .zip(del)
        .filter(|(_, d)| !d)
        .map(|(t, _)| t)
        .collect();
    detokenize(&kept)
}

/// "I I think" -> "I think", "th- the" -> "the".
pub fn collapse_stutters(text: &str, changes: &mut Vec<Change>) -> String {
    let toks = tokenize(text);
    let n = toks.len();
    let mut del = vec![false; n];

    for i in 0..n {
        let Some(w1) = word_lower(toks.get(i)) else {
            continue;
        };

        // word [,] space same-word
        let (gap_end, comma) = match (toks.get(i + 1), toks.get(i + 2)) {
            (Some(Tok::Space(s)), _) if !s.contains('\n') => (i + 1, None),
            (Some(t), Some(Tok::Space(s))) if is_punct(t, ",") && !s.contains('\n') => {
                (i + 2, Some(i + 1))
            }
            _ => (0, None),
        };
        if gap_end != 0
            && STUTTER_WORDS.contains(&w1.as_str())
            && word_lower(toks.get(gap_end + 1)).as_deref() == Some(&w1)
        {
            del[i] = true;
            del[gap_end] = true;
            if let Some(c) = comma {
                del[c] = true;
            }
            changes.push(Change {
                kind: ChangeKind::Stutter,
                before: format!("{w1} {w1}"),
                after: w1.clone(),
            });
            continue;
        }

        // cut-off word: "th- the"
        if w1.chars().count() <= 4
            && toks.get(i + 1).is_some_and(|t| is_punct(t, "-"))
            && matches!(toks.get(i + 2), Some(Tok::Space(s)) if !s.contains('\n'))
        {
            if let Some(w2) = word_lower(toks.get(i + 3)) {
                if w2.starts_with(&w1) {
                    del[i] = true;
                    del[i + 1] = true;
                    del[i + 2] = true;
                    changes.push(Change {
                        kind: ChangeKind::Stutter,
                        before: format!("{w1}- {w2}"),
                        after: w2,
                    });
                }
            }
        }
    }

    let kept: Vec<Tok> = toks
        .into_iter()
        .zip(del)
        .filter(|(_, d)| !d)
        .map(|(t, _)| t)
        .collect();
    detokenize(&kept)
}

#[cfg(test)]
mod tests {
    use super::super::tidy::tidy;
    use super::*;

    fn clean(s: &str) -> String {
        let mut c = Vec::new();
        let t = remove_fillers(s, &mut c);
        tidy(&collapse_stutters(&t, &mut c))
    }

    #[test]
    fn golden_filler_sentence() {
        assert_eq!(
            clean("So, um, I think, you know, we should probably review the budget again."),
            "So, I think we should probably review the budget again."
        );
    }

    #[test]
    fn keeps_meaningful_like_and_you_know() {
        let s = "I like the new dashboard and you know the team.";
        assert_eq!(clean(s), s);
    }

    #[test]
    fn hesitation_without_commas() {
        assert_eq!(clean("I um think we uh should go"), "I think we should go");
    }

    #[test]
    fn millimetres_are_not_a_hesitation() {
        let s = "The gap is 5 mm wide.";
        assert_eq!(clean(s), s);
    }

    #[test]
    fn hindi_reduplication_is_kept() {
        let s = "Dheere dheere kaam ho raha hai.";
        assert_eq!(clean(s), s);
    }

    #[test]
    fn cut_off_word() {
        assert_eq!(clean("Send th- the report."), "Send the report.");
    }
}
