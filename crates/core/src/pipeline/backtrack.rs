//! Spoken self-corrections ("backtracking").
//!
//! "Let's meet at 2 PM... actually make that 3 PM" -> "Let's meet at 3 PM."
//!
//! The rules are deliberately conservative. A correction is applied only when the new
//! words clearly replace something of the same kind (a time for a time, a percentage for
//! a percentage) or restate the previous sentence. Otherwise BOTH parts are kept and the
//! span is reported as uncertain, because wrongly deleting what someone said is worse than
//! leaving a correction for them to tidy.

use std::sync::LazyLock;

use regex::Regex;

use super::tokens::capitalise_first;
use super::{Change, ChangeKind};

static CUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(scratch that|no,? wait|nahi,? nahi|no,? no|i mean|sorry|correction|actually|matlab)\b").unwrap()
});
static MAKE_THAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:let'?s\s+)?make\s+(?:that|it)\s+").unwrap());
static LEAD_IN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:it'?s|it is|it was|that'?s|i meant|rather)\s+").unwrap());
static PRONOUN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:it|that|this)\s+").unwrap());
static TRAILING_FLUFF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\s,]*\b(?:instead|please|rather)$").unwrap());
static PREPOSITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:at|by|on|to|for|in|around|about|is|was)\s+").unwrap());
static SENTENCE_BREAK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[.?!]+(?:\s+|$)|\n+").unwrap());

/// Characters that can separate a correction from what it corrects.
const BOUNDARY: &str = ".,;:!?-\u{2014}\n";

struct Entity {
    search: Regex,
    anchored: Regex,
}

/// Kinds of value that can be swapped like-for-like, most specific first.
static ENTITIES: LazyLock<Vec<Entity>> = LazyLock::new(|| {
    const MONTHS: &str =
        "january|february|march|april|may|june|july|august|september|october|november|december";
    let patterns = [
        // time: 3 PM, 3:30 p.m., 15:45, 4 o'clock, 4 baje
        // (the final dot of "p.m." is left out on purpose so it survives a swap)
        r"\b\d{1,2}(?:[:.]\d{2})?\s*(?:[ap]\.m|[ap]m)\b|\b\d{1,2}:\d{2}\b|\b\d{1,2}\s*o'?clock\b|\b\d{1,2}\s*baje\b"
            .to_string(),
        // percentage
        r"\b\d+(?:[.,]\d+)*\s*(?:percent\b|per cent\b|%)".to_string(),
        // money
        r"(?:₹|\brs\.?\s|\$|\binr\s|\busd\s)\s*\d[\d,]*(?:\.\d+)?(?:\s*(?:lakhs?|crores?|thousand|million|billion|k)\b)?|\b\d[\d,]*(?:\.\d+)?\s*(?:rupees?|dollars?|lakhs?|crores?|thousand|million|billion)\b"
            .to_string(),
        // calendar date
        format!(r"\b(?:\d{{1,2}}(?:st|nd|rd|th)?\s+(?:of\s+)?)?(?:{MONTHS})(?:\s+\d{{1,2}}(?:st|nd|rd|th)?\b)?"),
        // day
        r"\b(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday|today|tomorrow|tonight)\b".to_string(),
        // plain number
        r"\b\d[\d,]*(?:\.\d+)?\b|\b(?:two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|fifteen|twenty|thirty|forty|fifty|hundred)\b"
            .to_string(),
    ];
    patterns
        .iter()
        .map(|p| Entity {
            search: Regex::new(&format!("(?i)(?:{p})")).unwrap(),
            anchored: Regex::new(&format!("(?i)^(?:{p})")).unwrap(),
        })
        .collect()
});

enum Fix {
    Applied {
        text: String,
        resume: usize,
        before: String,
        after: String,
    },
    Uncertain(String),
    NotACue,
}

fn is_lowercase_start(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_lowercase())
}

/// Start of the last sentence in `s`. "3 p.m. on Monday" is not split: a break only
/// counts when the next word does not start with a lower-case letter.
fn last_sentence_start(s: &str) -> usize {
    SENTENCE_BREAK
        .find_iter(s)
        .filter(|m| m.end() < s.len() && !is_lowercase_start(&s[m.end()..]))
        .last()
        .map_or(0, |m| m.end())
}

/// First real sentence end in `s`: (start of terminator, end of terminator, start of next).
fn first_sentence_end(s: &str) -> Option<(usize, usize, usize)> {
    SENTENCE_BREAK
        .find_iter(s)
        .find(|m| !is_lowercase_start(&s[m.end()..]))
        .map(|m| {
            let punct_len = m.as_str().trim_end().len();
            (m.start(), m.start() + punct_len, m.end())
        })
}

fn norm_eq(a: &str, b: &str) -> bool {
    let norm = |s: &str| {
        s.to_lowercase()
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect::<Vec<_>>()
    };
    let (na, nb) = (norm(a), norm(b));
    !na.is_empty() && na == nb
}

/// Replaces like with like: every value at the start of `r` must have a value of the same
/// kind in `p`. Anything left over in `r` must either start with a comma (it is appended)
/// or repeat what already follows the replaced value in `p`.
fn typed_replace(p: &str, r: &str) -> Option<String> {
    let mut rest = r.trim();
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut remainder = "";

    loop {
        let candidate = PREPOSITION.find(rest).map_or(rest, |m| &rest[m.end()..]);
        let hit = ENTITIES
            .iter()
            .enumerate()
            .find_map(|(i, e)| e.anchored.find(candidate).map(|m| (i, m.end())));
        match hit {
            Some((class, end)) => {
                found.push((class, candidate[..end].to_string()));
                rest = candidate[end..].trim_start();
                if rest.is_empty() {
                    break;
                }
            }
            None => {
                remainder = rest;
                break;
            }
        }
    }
    if found.is_empty() {
        return None;
    }

    let mut new_p = p.to_string();
    let mut replaced_end = 0usize;
    for (class, value) in &found {
        let m = ENTITIES[*class].search.find_iter(&new_p).last()?;
        let (start, end) = (m.start(), m.end());
        new_p.replace_range(start..end, value);
        replaced_end = start + value.len();
    }

    if remainder.is_empty() {
        Some(new_p)
    } else if remainder.starts_with(',') {
        new_p.push_str(remainder);
        Some(new_p)
    } else if norm_eq(&new_p[replaced_end..], remainder) {
        Some(new_p)
    } else {
        None
    }
}

/// "Revenue increased by 15 percent. Sorry, it increased by 12 percent."
/// The correction restarts the sentence part-way: find where its first two words occur in
/// `p` and continue from there. Both versions must contain a value of the same kind.
fn align_replace(p: &str, r: &str) -> Option<String> {
    let r1 = PRONOUN.replace(r, "");
    let r_words: Vec<&str> = r1.split_whitespace().collect();
    if r_words.len() < 2 {
        return None;
    }
    let key = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let (k0, k1) = (key(r_words[0]), key(r_words[1]));
    if k0.is_empty() || k1.is_empty() {
        return None;
    }

    let p_words: Vec<(usize, &str)> = p
        .split_whitespace()
        .map(|w| (w.as_ptr() as usize - p.as_ptr() as usize, w))
        .collect();
    let idx = (0..p_words.len().saturating_sub(1))
        .rev()
        .find(|&i| key(p_words[i].1) == k0 && key(p_words[i + 1].1) == k1)?;
    let cut = p_words[idx].0;

    let comparable = ENTITIES
        .iter()
        .any(|e| e.search.is_match(&p[cut..]) && e.search.is_match(&r1));
    if !comparable {
        return None;
    }
    Some(format!("{}{}", &p[..cut], r1))
}

fn try_fix(hay: &str, cue_start: usize, cue_end: usize) -> Fix {
    let before = &hay[..cue_start];

    // The cue must open a clause: "I actually like it" is not a correction.
    let before_trimmed = before.trim_end_matches([' ', '\t']);
    match before_trimmed.chars().last() {
        Some(c) if BOUNDARY.contains(c) => {}
        _ => return Fix::NotACue,
    }
    // What is being corrected: the sentence just before the cue.
    let b = before.trim_end_matches(|c: char| c.is_whitespace() || BOUNDARY.contains(c));
    if b.is_empty() {
        return Fix::NotACue;
    }
    let p_start = last_sentence_start(b);
    let head = &hay[..p_start];
    let p = &b[p_start..];

    let cue = hay[cue_start..cue_end].to_lowercase().replace(',', "");
    let scratch = cue == "scratch that";
    let strong = matches!(
        cue.as_str(),
        "sorry" | "no wait" | "nahi nahi" | "no no" | "i mean" | "correction"
    );

    // The correction itself: from the cue to the end of its sentence, or to the next cue.
    let mut rest = hay[cue_end..].trim_start_matches([' ', '\t', ',', ':', '.']);
    if let Some(m) = MAKE_THAT.find(rest) {
        rest = &rest[m.end()..];
    }

    let sentence_end = first_sentence_end(rest);
    let next_cue = CUE.find_iter(rest).find(|m| {
        m.start() > 0
            && rest[..m.start()]
                .trim_end_matches([' ', '\t'])
                .ends_with(|c: char| BOUNDARY.contains(c))
    });
    let chained = match (&next_cue, sentence_end) {
        (Some(c), Some((punct_start, _, _))) => c.start() < punct_start,
        (Some(_), None) => true,
        _ => false,
    };

    let (r_raw, terminator, tail): (&str, String, String) = if chained {
        let c = next_cue.unwrap();
        (
            &rest[..c.start()],
            String::new(),
            format!(", {}", &rest[c.start()..]),
        )
    } else if let Some((punct_start, punct_end, next_start)) = sentence_end {
        let punct = &rest[punct_start..punct_end];
        let term = match punct.chars().next() {
            Some(c) => c.to_string(),
            None => ".\n".to_string(), // the sentence ended at a line break
        };
        (&rest[..punct_start], term, rest[next_start..].to_string())
    } else {
        (rest, String::new(), String::new())
    };

    let r = r_raw.trim().trim_end_matches([',', ';', ':', ' ']);
    let r = TRAILING_FLUFF.replace(r, "");
    let r = r.as_ref();

    let new_p = if scratch {
        Some(capitalise_first(r))
    } else if r.is_empty() {
        return Fix::NotACue;
    } else {
        let without_lead_in = LEAD_IN.replace(r, "");
        typed_replace(p, &without_lead_in)
            .or_else(|| if strong { align_replace(p, r) } else { None })
    };

    let Some(new_p) = new_p else {
        return Fix::Uncertain(
            format!("{} {}", hay[cue_start..cue_end].trim(), r)
                .trim()
                .to_string(),
        );
    };

    let mut out = String::with_capacity(hay.len());
    out.push_str(head);
    out.push_str(&new_p);
    if tail.trim().is_empty() {
        if !new_p.is_empty() {
            out.push_str(if terminator.is_empty() {
                "."
            } else {
                terminator.trim_end()
            });
        }
    } else if chained {
        out.push_str(&tail);
    } else {
        if !new_p.is_empty() {
            out.push_str(if terminator.is_empty() {
                "."
            } else {
                &terminator
            });
            out.push(' ');
        }
        out.push_str(&tail);
    }

    Fix::Applied {
        text: out,
        resume: head.len(),
        before: format!("{} | {} {}", p, hay[cue_start..cue_end].trim(), r)
            .trim()
            .to_string(),
        after: new_p,
    }
}

pub fn apply(text: &str, changes: &mut Vec<Change>, uncertain: &mut Vec<String>) -> String {
    let mut s = text.to_string();
    let mut from = 0usize;

    // Each applied correction rewrites the text, so rescan after every one.
    'rescan: for _ in 0..32 {
        let hay = s.clone();
        for m in CUE.find_iter(&hay) {
            if m.start() < from {
                continue;
            }
            match try_fix(&hay, m.start(), m.end()) {
                Fix::Applied {
                    text,
                    resume,
                    before,
                    after,
                } => {
                    changes.push(Change {
                        kind: ChangeKind::Backtrack,
                        before,
                        after,
                    });
                    s = text;
                    from = resume;
                    continue 'rescan;
                }
                Fix::Uncertain(span) => {
                    if !uncertain.contains(&span) {
                        uncertain.push(span);
                    }
                }
                Fix::NotACue => {}
            }
        }
        break;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::tidy::tidy;
    use super::*;

    fn run(s: &str) -> String {
        tidy(&apply(s, &mut Vec::new(), &mut Vec::new()))
    }

    #[test]
    fn golden_examples() {
        assert_eq!(
            run("Let's meet at 2 PM... actually make that 3 PM"),
            "Let's meet at 3 PM."
        );
        assert_eq!(
            run("Schedule the meeting for Monday at 3 PM. Actually, make that 4 PM."),
            "Schedule the meeting for Monday at 4 PM."
        );
        assert_eq!(
            run("Revenue increased by 15 percent. Sorry, it increased by 12 percent."),
            "Revenue increased by 12 percent."
        );
    }

    #[test]
    fn chained_corrections() {
        assert_eq!(
            run("Let's meet at 2 PM, actually 3 PM, no wait, 4 PM."),
            "Let's meet at 4 PM."
        );
    }

    #[test]
    fn only_the_last_sentence_is_touched() {
        assert_eq!(
            run("Hi team. The total is 5,000 rupees. No wait, 6,000 rupees. Thanks."),
            "Hi team. The total is 6,000 rupees. Thanks."
        );
    }

    #[test]
    fn scratch_that() {
        assert_eq!(
            run("Send the report to Rahul. Scratch that, send it to Priya."),
            "Send it to Priya."
        );
    }

    #[test]
    fn hinglish_time() {
        assert_eq!(
            run("Meeting 3 baje hai, nahi nahi, 4 baje hai."),
            "Meeting 4 baje hai."
        );
    }

    #[test]
    fn dotted_pm_is_not_a_sentence_end() {
        assert_eq!(
            run("Call me at 2 p.m. on Monday, actually make that 3 p.m."),
            "Call me at 3 p.m. on Monday."
        );
    }

    #[test]
    fn uncertain_corrections_keep_both_parts() {
        for s in [
            "We will ship on Monday. Sorry, we will need more time.",
            "I actually like the new design.",
            "Sorry, I'm late.",
            "It costs 5 dollars. Actually, 3 people came yesterday.",
            "I think, actually, we should go.",
            "I'm sorry about the delay at 3 PM.",
        ] {
            assert_eq!(run(s), s, "should be unchanged");
        }
    }

    #[test]
    fn uncertain_spans_are_reported() {
        let mut uncertain = Vec::new();
        apply(
            "We will ship on Monday. Sorry, we will need more time.",
            &mut Vec::new(),
            &mut uncertain,
        );
        assert_eq!(uncertain.len(), 1);
    }
}
