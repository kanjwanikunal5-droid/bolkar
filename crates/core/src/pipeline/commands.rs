//! Spoken formatting commands: "new line", "full stop", "bullet point" and friends.
//!
//! A phrase is treated as a command only when it is not part of an ordinary sentence
//! ("a new line of products", "the trial period" stay as they are).

use std::sync::LazyLock;

use regex::Regex;

use super::tokens::{capitalise_first, is_word_char};
use super::{Change, ChangeKind};

/// A word right before the phrase that shows it is being used as a normal noun.
const DETERMINERS: &[&str] = &[
    "a", "an", "the", "this", "that", "these", "those", "our", "my", "your", "his", "her", "their",
    "its", "another", "brand", "whole", "entire", "one", "each", "every", "no", "some", "any",
    "first", "second", "last", "same", "of",
];

static FULL_STOP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bfull[ -]?stop\b").unwrap());
static QUESTION_MARK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bquestion mark\b").unwrap());
static EXCLAMATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bexclamation (?:mark|point)\b").unwrap());
static COMMA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcomma\b").unwrap());
static COLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bcolon\b").unwrap());
static PERIOD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bperiod\b").unwrap());
static NEW_PARAGRAPH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:new|next) paragraph\b").unwrap());
static NEW_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:new|next) line\b|\bnewline\b").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:bullet point|next point|next bullet|new bullet)\b").unwrap()
});
static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^heading[,:]?\s+(\S.*?)\s*\.?$").unwrap());

/// "Heading to the office" is a sentence, not a heading command.
const HEADING_BLOCK: &[&str] = &[
    "to", "towards", "toward", "into", "out", "over", "back", "home", "up", "down", "for", "there",
    "off", "in", "north", "south", "east", "west",
];

struct Rep {
    text: &'static str,
    /// Punctuation directly before the phrase that the command replaces.
    eat_before: &'static str,
    /// One punctuation character directly after the phrase that the command replaces.
    eat_after: &'static str,
    eat_after_space: bool,
    space_after: bool,
}

fn prev_adjacent_word(before: &str) -> Option<String> {
    let t = before.trim_end_matches([' ', '\t']);
    let rev: Vec<char> = t
        .chars()
        .rev()
        .take_while(|c| is_word_char(*c) || *c == '\'')
        .collect();
    if rev.is_empty() {
        None
    } else {
        Some(rev.into_iter().rev().collect::<String>().to_lowercase())
    }
}

fn next_adjacent_word(after: &str) -> Option<String> {
    let t = after.trim_start_matches([' ', '\t']);
    let w: String = t.chars().take_while(|c| is_word_char(*c)).collect();
    if w.is_empty() {
        None
    } else {
        Some(w.to_lowercase())
    }
}

fn used_as_noun(before: &str, extra_prev: &[&str]) -> bool {
    prev_adjacent_word(before)
        .is_some_and(|w| DETERMINERS.contains(&w.as_str()) || extra_prev.contains(&w.as_str()))
}

fn rewrite(
    text: &str,
    re: &Regex,
    changes: &mut Vec<Change>,
    decide: &dyn Fn(&str, &str) -> Option<Rep>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for m in re.find_iter(text) {
        if m.start() < last {
            continue;
        }
        let before = &text[..m.start()];
        let after = &text[m.end()..];
        let Some(rep) = decide(before, after) else {
            continue;
        };

        out.push_str(&text[last..m.start()]);
        while let Some(c) = out.chars().last() {
            if c == ' ' || c == '\t' || rep.eat_before.contains(c) {
                out.pop();
            } else {
                break;
            }
        }
        out.push_str(rep.text);

        let mut rest = after;
        if let Some(c) = rest.chars().next() {
            if rep.eat_after.contains(c) {
                rest = &rest[c.len_utf8()..];
            }
        }
        if rep.eat_after_space {
            rest = rest.trim_start_matches([' ', '\t']);
        }
        last = text.len() - rest.len();
        if rep.space_after && !rest.is_empty() && !rest.starts_with([' ', '\t', '\n']) {
            out.push(' ');
        }
        changes.push(Change {
            kind: ChangeKind::Command,
            before: m.as_str().to_string(),
            after: rep.text.to_string(),
        });
    }
    out.push_str(&text[last..]);
    out
}

fn punct(text: &'static str) -> Rep {
    Rep {
        text,
        eat_before: ",.;:",
        eat_after: ",.;:?!",
        eat_after_space: false,
        space_after: true,
    }
}

pub fn apply(text: &str, changes: &mut Vec<Change>) -> String {
    let mut s = text.to_string();

    // Punctuation words.
    s = rewrite(&s, &FULL_STOP, changes, &|b, _| {
        (!used_as_noun(b, &[])).then(|| punct("."))
    });
    s = rewrite(&s, &QUESTION_MARK, changes, &|b, _| {
        (!used_as_noun(b, &[])).then(|| punct("?"))
    });
    s = rewrite(&s, &EXCLAMATION, changes, &|b, _| {
        (!used_as_noun(b, &[])).then(|| punct("!"))
    });
    s = rewrite(&s, &COMMA, changes, &|b, a| {
        let noun = used_as_noun(b, &["oxford", "serial", "missing", "extra"]);
        let compound = next_adjacent_word(a).is_some_and(|w| {
            matches!(w.as_str(), "separated" | "delimited" | "splice" | "splices")
        });
        (!noun && !compound).then_some(Rep {
            text: ",",
            eat_before: ",.;:",
            eat_after: ",.",
            eat_after_space: false,
            space_after: true,
        })
    });
    s = rewrite(&s, &COLON, changes, &|b, a| {
        let noun = used_as_noun(b, &["semi"]);
        let medical = next_adjacent_word(a).is_some_and(|w| {
            matches!(
                w.as_str(),
                "cancer" | "cleanse" | "surgery" | "health" | "infection" | "polyps"
            )
        });
        (!noun && !medical).then(|| punct(":"))
    });
    // "period" is an everyday word, so it only counts when the recogniser already set it
    // apart: "... by Friday. Period." or "... by Friday, period."
    s = rewrite(&s, &PERIOD, changes, &|b, a| {
        let b = b.trim_end();
        let a = a.trim_start();
        let set_apart = !b.is_empty() && b.ends_with(['.', '?', '!', ',']);
        let closes = a.is_empty() || a.starts_with(['.', ',', '?', '!']);
        (set_apart && closes).then(|| punct("."))
    });

    // Line structure.
    let line_break = |text: &'static str| Rep {
        text,
        eat_before: "",
        eat_after: ".,;:",
        eat_after_space: true,
        space_after: false,
    };
    s = rewrite(&s, &NEW_PARAGRAPH, changes, &|b, a| {
        let phrase_of = next_adjacent_word(a).is_some_and(|w| w == "of");
        (!used_as_noun(b, &[]) && !phrase_of).then(|| line_break("\n\n"))
    });
    s = rewrite(&s, &NEW_LINE, changes, &|b, a| {
        let phrase_of = next_adjacent_word(a).is_some_and(|w| w == "of");
        (!used_as_noun(b, &[]) && !phrase_of).then(|| line_break("\n"))
    });
    s = rewrite(&s, &BULLET, changes, &|b, a| {
        let sentence = next_adjacent_word(a)
            .is_some_and(|w| matches!(w.as_str(), "is" | "was" | "of" | "about"));
        (!used_as_noun(b, &[]) && !sentence).then(|| line_break("\n\u{2022} "))
    });

    // "Heading <title>" at the start of a line.
    if s.to_lowercase().contains("heading") {
        let lines: Vec<String> = s
            .split('\n')
            .map(|line| {
                let Some(caps) = HEADING.captures(line.trim()) else {
                    return line.to_string();
                };
                let title = caps.get(1).map_or("", |m| m.as_str());
                let first = next_adjacent_word(title).unwrap_or_default();
                if HEADING_BLOCK.contains(&first.as_str()) {
                    return line.to_string();
                }
                changes.push(Change {
                    kind: ChangeKind::Command,
                    before: "heading".into(),
                    after: String::new(),
                });
                capitalise_first(title)
            })
            .collect();
        s = lines.join("\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::super::tidy::tidy;
    use super::*;

    fn run(s: &str) -> String {
        tidy(&apply(s, &mut Vec::new()))
    }

    #[test]
    fn new_line_and_full_stop() {
        assert_eq!(
            run("Hi Rahul, new line I wanted to share the update full stop"),
            "Hi Rahul,\nI wanted to share the update."
        );
    }

    #[test]
    fn ordinary_sentences_are_untouched() {
        for s in [
            "We launched a new line of products this period.",
            "I have two points to make",
            "The next point is about hiring.",
            "Put a comma after the name.",
            "Heading to the office now.",
            "The trial period ends on Friday.",
        ] {
            assert_eq!(run(s), s, "should be unchanged");
        }
    }

    #[test]
    fn bullets() {
        assert_eq!(
            run("Agenda for today. Bullet point, budget review. Bullet point, hiring plan."),
            "Agenda for today.\n\u{2022} Budget review.\n\u{2022} Hiring plan."
        );
    }

    #[test]
    fn spoken_punctuation() {
        assert_eq!(run("Are you coming question mark"), "Are you coming?");
        assert_eq!(run("Hi John comma how are you"), "Hi John, how are you");
        assert_eq!(run("Send it by Friday. Period."), "Send it by Friday.");
    }

    #[test]
    fn heading_and_paragraph() {
        assert_eq!(
            run("Heading budget review new paragraph the numbers are final."),
            "Budget review\n\nThe numbers are final."
        );
    }
}
