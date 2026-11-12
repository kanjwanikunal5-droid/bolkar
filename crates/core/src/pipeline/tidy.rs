//! Final spacing and capitalisation pass. Never lower-cases anything and never adds
//! punctuation that was not already there.

use std::sync::LazyLock;

use regex::Regex;

use super::tokens::{detokenize, tokenize, Tok};

static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").unwrap());
static SPACE_AROUND_NEWLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]*\n[ \t]*").unwrap());
static MANY_NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static SPACE_BEFORE_PUNCT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" +([,.;:?!])").unwrap());
static REPEATED_COMMAS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",(?:\s*,)+").unwrap());
static COMMA_BEFORE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",\s*([.?!])").unwrap());
static LEADING_PUNCT_IN_SENTENCE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([.?!][ \t]+|\n)[,;:]+[ \t]*").unwrap());

/// Words after which a full stop does not end the sentence.
const ABBREVIATIONS: &[&str] = &[
    "mr", "mrs", "ms", "dr", "prof", "vs", "etc", "approx", "inc", "ltd", "st", "sr", "jr", "no",
];

pub fn tidy(text: &str) -> String {
    let mut s = SPACES.replace_all(text, " ").into_owned();
    s = SPACE_AROUND_NEWLINE.replace_all(&s, "\n").into_owned();
    s = MANY_NEWLINES.replace_all(&s, "\n\n").into_owned();
    s = SPACE_BEFORE_PUNCT.replace_all(&s, "$1").into_owned();
    s = REPEATED_COMMAS.replace_all(&s, ",").into_owned();
    s = COMMA_BEFORE_END.replace_all(&s, "$1").into_owned();
    s = LEADING_PUNCT_IN_SENTENCE.replace_all(&s, "$1").into_owned();
    let s = s.trim_matches(|c: char| c.is_whitespace());
    let s = s.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':'));
    capitalise(s).trim().to_string()
}

/// Capitalises the first word of the text, of each line and of each sentence, and the
/// English pronoun "I".
fn capitalise(text: &str) -> String {
    let mut toks = tokenize(text);
    let mut cap_next = true;
    let mut pending_cap = false; // saw . ? ! and now waiting for a space
    let mut last_word = String::new();

    for tok in toks.iter_mut() {
        match tok {
            Tok::Word(w) => {
                if cap_next {
                    if let Some(first) = w.chars().next() {
                        if first.is_lowercase() {
                            let rest: String = w.chars().skip(1).collect();
                            *w = first.to_uppercase().collect::<String>() + &rest;
                        }
                    }
                }
                let lower = w.to_lowercase();
                let is_pronoun_i =
                    lower == "i" || lower.starts_with("i'") || lower.starts_with("i\u{2019}");
                if is_pronoun_i && w.starts_with('i') {
                    *w = format!("I{}", &w[1..]);
                }
                last_word = lower;
                cap_next = false;
                pending_cap = false;
            }
            Tok::Punct(p) => {
                if matches!(p.as_str(), "." | "?" | "!" | "\u{0964}") {
                    let single_letter = last_word.chars().count() == 1
                        && last_word.chars().all(|c| c.is_ascii_alphabetic())
                        && last_word != "i";
                    let abbreviation =
                        p == "." && (single_letter || ABBREVIATIONS.contains(&last_word.as_str()));
                    pending_cap = !abbreviation;
                }
                // Other punctuation (quotes, brackets, list dashes) changes nothing.
            }
            Tok::Space(sp) => {
                if sp.contains('\n') || pending_cap {
                    cap_next = true;
                }
                pending_cap = false;
            }
        }
    }
    detokenize(&toks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixes_spacing_and_capitals() {
        assert_eq!(
            tidy(" the numbers look good .  we ship monday"),
            "The numbers look good. We ship monday"
        );
    }

    #[test]
    fn leaves_urls_emails_and_abbreviations_alone() {
        let s = "Mail me at a.b@example.com by 3 p.m. tomorrow, e.g. at lunch.";
        assert_eq!(tidy(s), s);
    }

    #[test]
    fn capitalises_list_items_and_pronoun_i() {
        assert_eq!(
            tidy("agenda:\n- budget\n1. hiring\nthen i'm done"),
            "Agenda:\n- Budget\n1. Hiring\nThen I'm done"
        );
    }

    #[test]
    fn does_not_add_a_full_stop() {
        assert_eq!(
            tidy("I have two points to make"),
            "I have two points to make"
        );
    }

    #[test]
    fn devanagari_is_untouched() {
        let s = "कल मीटिंग है। फिर मिलते हैं।";
        assert_eq!(tidy(s), s);
    }
}
