//! A small tokenizer shared by the cleanup rules.
//!
//! Text is split into words, punctuation and whitespace so that rules can look at
//! "what is to the left / right of this word" without fragile string surgery.

use regex::Regex;
use unicode_normalization::char::is_combining_mark;

/// Start / end markers of a protected span (an expanded snippet). Private-use characters,
/// so no rule can ever match inside them.
pub const PROTECT_OPEN: char = '\u{E000}';
pub const PROTECT_CLOSE: char = '\u{E001}';
/// Digits of a protected span's index are encoded as `PROTECT_DIGIT_BASE + digit`.
pub const PROTECT_DIGIT_BASE: u32 = 0xE100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Word(String),
    Punct(String),
    Space(String),
}

fn is_private_use(c: char) -> bool {
    ('\u{E000}'..='\u{F8FF}').contains(&c)
}

/// Letters, digits, Indic vowel signs and joiners, and protected-span characters.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '_'
        || is_combining_mark(c)
        || c == '\u{200C}'
        || c == '\u{200D}'
        || is_private_use(c)
}

pub fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_word_char(c) {
            let start = i;
            i += 1;
            while i < chars.len() {
                let d = chars[i];
                if is_word_char(d) {
                    i += 1;
                    continue;
                }
                let next_is_word = i + 1 < chars.len() && is_word_char(chars[i + 1]);
                // let's, can't, well-known
                if next_is_word && matches!(d, '\'' | '\u{2019}' | '-') {
                    i += 1;
                    continue;
                }
                // 3.5, 50,000, 3:30
                if next_is_word
                    && matches!(d, '.' | ',' | ':')
                    && chars[i - 1].is_ascii_digit()
                    && chars[i + 1].is_ascii_digit()
                {
                    i += 1;
                    continue;
                }
                break;
            }
            out.push(Tok::Word(chars[start..i].iter().collect()));
        } else if c.is_whitespace() {
            let start = i;
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            out.push(Tok::Space(chars[start..i].iter().collect()));
        } else if c == '.' && i + 2 < chars.len() && chars[i + 1] == '.' && chars[i + 2] == '.' {
            while i < chars.len() && chars[i] == '.' {
                i += 1;
            }
            out.push(Tok::Punct("...".to_string()));
        } else {
            out.push(Tok::Punct(c.to_string()));
            i += 1;
        }
    }
    out
}

pub fn detokenize(toks: &[Tok]) -> String {
    let mut s = String::new();
    for t in toks {
        match t {
            Tok::Word(w) | Tok::Punct(w) | Tok::Space(w) => s.push_str(w),
        }
    }
    s
}

pub fn is_punct(t: &Tok, p: &str) -> bool {
    matches!(t, Tok::Punct(x) if x == p)
}

/// `.`, `?` or `!` (and the Devanagari danda).
pub fn is_sentence_end(t: &Tok) -> bool {
    matches!(t, Tok::Punct(x) if matches!(x.as_str(), "." | "?" | "!" | "\u{0964}"))
}

/// Upper-cases the first letter, leaving everything else untouched.
pub fn capitalise_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_lowercase() => c.to_uppercase().collect::<String>() + chars.as_str(),
        _ => s.to_string(),
    }
}

/// Builds a case-insensitive whole-phrase regex. The words of `phrase` may be separated in
/// the text by any run of characters from `sep_class` (a regex character class body).
pub fn phrase_regex(phrase: &str, sep_class: &str) -> Option<Regex> {
    let words: Vec<String> = phrase
        .split(|c: char| !is_word_char(c))
        .filter(|w| !w.is_empty())
        .map(regex::escape)
        .collect();
    if words.is_empty() {
        return None;
    }
    let body = words.join(&format!("[{sep_class}]+"));
    Regex::new(&format!(r"(?i)\b{body}\b")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_contractions_stay_whole() {
        let toks = tokenize("Let's pay 50,000 at 3:30, ok?");
        let words: Vec<&str> = toks
            .iter()
            .filter_map(|t| {
                if let Tok::Word(w) = t {
                    Some(w.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(words, ["Let's", "pay", "50,000", "at", "3:30", "ok"]);
    }

    #[test]
    fn devanagari_words_keep_their_vowel_signs() {
        let toks = tokenize("मीटिंग कल है।");
        assert_eq!(toks[0], Tok::Word("मीटिंग".to_string()));
        assert!(is_sentence_end(toks.last().unwrap()));
    }

    #[test]
    fn round_trips() {
        let s = "Wait... what?  Yes - really.\nNew line";
        assert_eq!(detokenize(&tokenize(s)), s);
    }
}
