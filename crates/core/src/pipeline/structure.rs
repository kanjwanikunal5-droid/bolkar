//! Paragraphs or points? Natural prose is the default: connected sentences stay together.
//! A list is made only when the speaker clearly enumerates ("first ..., second ..."), says
//! "bullet point" / "next point", or (in the opt-in "auto" mode) when the sentences are short,
//! independent requests. A full stop on its own never starts a new bullet.

use std::sync::LazyLock;

use regex::Regex;

/// Settings > Writing > Lists in hands-free dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListFormat {
    /// Natural rules, plus the AI layout (paragraphs, bullets, numbered steps, labels) for
    /// longer dictations when AI clean-up is on (`crate::layout`). The default since 0.4.3.
    #[default]
    Smart,
    /// Paragraphs; lists only from spoken enumerations and "bullet point" commands.
    Natural,
    /// Also bullets when hands-free speech is several short, independent requests.
    Auto,
    /// Every hands-free dictation of two or more sentences becomes bullets (the old behaviour).
    Always,
}

impl ListFormat {
    /// Unknown or empty values mean Smart (the default); "natural" keeps the rules only.
    pub fn from_setting(value: &str) -> Self {
        match value.trim() {
            "natural" => Self::Natural,
            "auto" => Self::Auto,
            "always" => Self::Always,
            _ => Self::Smart,
        }
    }
}

static SENTENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^.?!]+[.?!]*").unwrap());

/// Words that tie a sentence to the one before it: the text is an explanation, not a list.
const LINKING_STARTS: &[&str] = &[
    "however",
    "but",
    "so",
    "because",
    "therefore",
    "although",
    "though",
    "which",
    "this",
    "that",
    "it",
    "they",
    "these",
    "those",
    "then",
    "thus",
    "hence",
    "meanwhile",
    "otherwise",
    "lekin",
    "par",
    "magar",
    "kyunki",
    "isliye",
    "toh",
    "to",
    "matlab",
    "phir",
    "fir",
    "jo",
    "ye",
    "yeh",
    "woh",
    "wo",
];
/// Connectives anywhere in a sentence that make it part of an argument.
const LINKING_ANYWHERE: &[&str] = &[
    "because",
    "since",
    "therefore",
    "however",
    "although",
    "whereas",
    "which affected",
    "kyunki",
    "isliye",
    "jis wajah se",
    "jiski wajah se",
    "lekin",
    "mainly",
];
/// A sentence that starts with a narrating subject reads as a story, not a to-do item.
const NARRATING_SUBJECTS: &[&str] = &[
    "i",
    "we",
    "he",
    "she",
    "you",
    "our",
    "my",
    "main",
    "hum",
    "humne",
    "maine",
    "mera",
    "hamara",
    "today",
    "yesterday",
    "kal",
    "aaj",
];

fn first_word(s: &str) -> String {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .find(|w| !w.is_empty())
        .unwrap_or("")
        .to_lowercase()
}

/// The sentences of a one-paragraph text.
pub fn sentences(text: &str) -> Vec<String> {
    SENTENCE
        .find_iter(text)
        .map(|m| m.as_str().trim().to_string())
        .filter(|s| s.chars().any(char::is_alphanumeric))
        .collect()
}

/// True only for several short, independent sentences, such as hands-free requests:
/// "Please book the room for Monday. Also send the agenda. And remind me to call the bank."
/// Connected explanations ("Revenue rose. However, costs also rose ...") are never items.
pub fn looks_like_independent_items(text: &str) -> bool {
    if text.contains('\n') {
        return false;
    }
    let s = sentences(text);
    if s.len() < 3 {
        return false;
    }
    let lower: Vec<String> = s
        .iter()
        .map(|x| format!(" {} ", x.to_lowercase()))
        .collect();
    let short = s.iter().all(|x| {
        let n = x.split_whitespace().count();
        (2..=14).contains(&n)
    });
    let linked = s
        .iter()
        .any(|x| LINKING_STARTS.contains(&first_word(x).as_str()))
        || lower.iter().any(|x| {
            LINKING_ANYWHERE
                .iter()
                .any(|w| x.contains(&format!(" {w} ")) || x.contains(&format!(" {w},")))
        });
    let narrated = s
        .iter()
        .filter(|x| NARRATING_SUBJECTS.contains(&first_word(x).as_str()))
        .count();
    short && !linked && narrated * 2 < s.len()
}

/// Applies the hands-free list setting to finished text. Text that already has line breaks
/// (a spoken list, "new line") is left alone.
pub fn hands_free_structure(text: &str, format: ListFormat) -> String {
    if text.contains('\n') {
        return text.to_string();
    }
    let make_bullets = match format {
        ListFormat::Smart | ListFormat::Natural => false,
        ListFormat::Auto => looks_like_independent_items(text),
        ListFormat::Always => crate::command::items(text).len() >= 2,
    };
    if make_bullets {
        if let Some(e) = crate::command::apply(&crate::command::Command::BulletList, text) {
            return e.text;
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROSE_A: &str = "Today we discussed the company's financial performance. Revenue increased by 15% compared to last quarter, mainly because of higher sales volume. However, operating expenses also increased, which affected profitability. We need to investigate the expense increase before the next review.";
    const HINGLISH_D: &str = "Kal humne sales performance discuss ki. Revenue 15 percent increase hua, mainly because sales volume better tha. Lekin operating expenses bhi increase hue, jis wajah se profitability impact hui.";

    #[test]
    fn connected_prose_stays_a_paragraph_in_every_mode_but_always() {
        for text in [PROSE_A, HINGLISH_D] {
            assert!(!looks_like_independent_items(text), "{text}");
            assert_eq!(hands_free_structure(text, ListFormat::Natural), text);
            assert_eq!(hands_free_structure(text, ListFormat::Auto), text);
        }
        // The opt-in "always" mode still does what it says.
        assert!(hands_free_structure(PROSE_A, ListFormat::Always).starts_with("• "));
    }

    #[test]
    fn short_independent_requests_are_items_only_in_auto() {
        let t = "Please book the meeting room for Monday. Also, send the agenda to the whole team. And remind me to call the bank tomorrow.";
        assert!(looks_like_independent_items(t));
        assert_eq!(hands_free_structure(t, ListFormat::Natural), t);
        let auto = hands_free_structure(t, ListFormat::Auto);
        assert_eq!(auto.lines().count(), 3, "{auto}");
    }

    #[test]
    fn narration_and_two_sentences_are_not_items() {
        for t in [
            "We met the client. We showed the demo. We agreed on the price.",
            "Send the report. Call the bank.",
            "The weather is nice.",
        ] {
            assert!(!looks_like_independent_items(t), "{t}");
        }
    }

    #[test]
    fn existing_line_breaks_are_kept() {
        let t = "1. Update the forecast\n2. Check cash flow";
        assert_eq!(hands_free_structure(t, ListFormat::Always), t);
    }

    #[test]
    fn settings_values() {
        assert_eq!(ListFormat::from_setting("auto"), ListFormat::Auto);
        assert_eq!(ListFormat::from_setting("always"), ListFormat::Always);
        assert_eq!(ListFormat::from_setting("natural"), ListFormat::Natural);
        assert_eq!(ListFormat::from_setting("on"), ListFormat::Smart);
        assert_eq!(ListFormat::from_setting(""), ListFormat::Smart);
    }
}
