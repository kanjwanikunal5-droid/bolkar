//! AI clean-up support: prompt building, answer parsing and validation.
//!
//! No network code lives here. The desktop app sends the request and hands the answer back
//! to [`accept`]. An answer is used only if it passes every check; otherwise the caller falls
//! back to the rule-based clean-up. The checks are the "never invent" guarantee: a number,
//! name, e-mail address or link that the speaker did not say can never reach the text.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::pipeline::{Level, Prepared};

/// The versioned prompt file, compiled in so the app and the spec can never drift apart.
const CLEANUP_SPEC: &str = include_str!("../../../spec/prompts/cleanup.md");

/// Kind of app being dictated into; decides the writing style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppCategory {
    Email,
    Chat,
    Docs,
    Code,
    AiChat,
    Terminal,
    Other,
}

impl AppCategory {
    pub fn parse(s: &str) -> Self {
        match s {
            "Email" => Self::Email,
            "Chat" => Self::Chat,
            "Docs" => Self::Docs,
            "Code" => Self::Code,
            "AiChat" => Self::AiChat,
            "Terminal" => Self::Terminal,
            _ => Self::Other,
        }
    }

    pub fn style_description(self) -> &'static str {
        match self {
            Self::Email => {
                "Formal: complete sentences and a professional tone, as in a work e-mail"
            }
            Self::Chat => "Casual: short and natural, as in a chat message; no formal phrasing",
            Self::Docs => "Professional: clear complete sentences and paragraphs",
            Self::Code => {
                "Technical/Code: keep identifiers, file names, commands and casing exactly"
            }
            Self::AiChat => "AI-prompt: clear and structured; keep lists and every detail",
            Self::Terminal => "Verbatim: change as little as possible",
            Self::Other => "Professional: clear complete sentences",
        }
    }
}

/// The cleanup system prompt template: the first fenced block after "## System prompt".
pub fn cleanup_template() -> &'static str {
    static TEMPLATE: LazyLock<String> = LazyLock::new(|| {
        let after = CLEANUP_SPEC
            .split_once("## System prompt")
            .map_or("", |(_, rest)| rest);
        let body = after.split_once("```\n").map_or("", |(_, rest)| rest);
        let body = body.split_once("\n```").map_or("", |(b, _)| b);
        body.replace('\r', "")
    });
    &TEMPLATE
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupRequest {
    pub system: String,
    /// JSON user message, as specified in `spec/prompts/cleanup.md`.
    pub user: String,
}

pub fn build_cleanup(
    prepared: &Prepared,
    level: Level,
    category: AppCategory,
    language: &str,
    dictionary_terms: &[String],
) -> CleanupRequest {
    let level_name = match level {
        Level::Professional => "PROFESSIONAL",
        _ => "LIGHT",
    };
    let terms = if dictionary_terms.is_empty() {
        "(none)".to_string()
    } else {
        dictionary_terms.join(", ")
    };
    let system = cleanup_template()
        .replace("{style_description}", category.style_description())
        .replace("{level}", level_name)
        .replace("{dictionary_terms}", &terms)
        .replace(
            "{script_instruction}",
            "keep the script the speaker's words are already written in",
        );
    let raw = prepared.protected.to_tokens(&prepared.text);
    let mut user = serde_json::json!({
        "raw": raw,
        "app_category": format!("{category:?}"),
        "language": language,
        "settings": { "level": level_name },
    });
    if !prepared.protected.bodies().is_empty() {
        user["settings"]["keep_tokens"] = serde_json::json!(
            "Tokens like [[S1]] are saved text blocks: copy each one exactly once, unchanged."
        );
    }
    CleanupRequest {
        system,
        user: user.to_string(),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CleanupReply {
    pub text: String,
    #[serde(default)]
    pub uncertain_spans: Vec<String>,
}

/// Why an AI answer was not used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Rejection {
    NotJson(String),
    Empty,
    TooLong { output: usize, limit: usize },
    NewNumber(String),
    NewName(String),
    NewEmailOrLink(String),
    TooManyNewWords { new: usize, of: usize },
    Snippet(String),
}

impl std::fmt::Display for Rejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotJson(e) => write!(f, "answer was not valid JSON ({e})"),
            Self::Empty => write!(f, "answer was empty"),
            Self::TooLong { output, limit } => {
                write!(f, "answer too long ({output} characters, limit {limit})")
            }
            Self::NewNumber(n) => write!(f, "answer added a number that was not spoken: {n}"),
            Self::NewName(n) => write!(f, "answer added a name that was not spoken: {n}"),
            Self::NewEmailOrLink(n) => {
                write!(f, "answer added an address that was not spoken: {n}")
            }
            Self::TooManyNewWords { new, of } => {
                write!(
                    f,
                    "answer rewrote too much ({new} of {of} words were not spoken)"
                )
            }
            Self::Snippet(e) => write!(f, "answer damaged a saved snippet ({e})"),
        }
    }
}

/// Extracts the JSON object from a model answer (tolerates ```json fences and chatter).
pub fn parse_reply(content: &str) -> Result<CleanupReply, Rejection> {
    let start = content.find('{');
    let end = content.rfind('}');
    let json = match (start, end) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => return Err(Rejection::NotJson("no JSON object".into())),
    };
    serde_json::from_str(json).map_err(|e| Rejection::NotJson(e.to_string()))
}

static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d[\d,]*(?:\.\d+)?").unwrap());
static ADDRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b[\w.+-]+@[\w-]+(?:\.[\w-]+)+\b|\bhttps?://\S+|\bwww\.\S+").unwrap()
});
static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{M}][\p{L}\p{M}'\u{2019}]*").unwrap());
static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[S\d+\]\]").unwrap());

fn number_key(s: &str) -> String {
    let t = s.replace(',', "");
    let t = t.trim_end_matches('.');
    match t.split_once('.') {
        Some((int, frac)) if frac.chars().all(|c| c == '0') => int.to_string(),
        _ => t.to_string(),
    }
}

fn small_number(word: &str) -> Option<u64> {
    const UNITS: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 8] = [
        "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    if let Some(i) = UNITS.iter().position(|u| *u == word) {
        return Some(i as u64);
    }
    TENS.iter()
        .position(|t| *t == word)
        .map(|i| (i as u64 + 2) * 10)
}

/// Every number the speaker said: digits as written, plus spelled-out numbers such as
/// "fifty thousand" (50000) or "twenty five" (25).
fn spoken_numbers(text: &str) -> HashSet<String> {
    let mut found: HashSet<String> = NUMBER
        .find_iter(text)
        .map(|m| number_key(m.as_str()))
        .collect();
    let lower = text.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let mut total: u64 = 0;
    let mut group: u64 = 0;
    let mut active = false;
    fn flush(found: &mut HashSet<String>, total: &mut u64, group: &mut u64, active: &mut bool) {
        if *active {
            found.insert((*total + *group).to_string());
            if *total > 0 && *group > 0 {
                found.insert(group.to_string());
            }
        }
        *total = 0;
        *group = 0;
        *active = false;
    }
    for w in words {
        if let Some(n) = small_number(w) {
            group += n;
            found.insert(n.to_string());
            active = true;
        } else if w == "hundred" && active {
            group *= 100;
        } else if matches!(
            w,
            "thousand" | "lakh" | "lakhs" | "million" | "crore" | "crores"
        ) {
            let scale = match w {
                "thousand" => 1_000,
                "lakh" | "lakhs" => 100_000,
                "million" => 1_000_000,
                _ => 10_000_000,
            };
            if active {
                total += group.max(1) * scale;
                group = 0;
            }
        } else if w == "and" && active {
            // "one hundred and five"
        } else {
            flush(&mut found, &mut total, &mut group, &mut active);
        }
    }
    flush(&mut found, &mut total, &mut group, &mut active);
    found
}

fn word_set(text: &str) -> HashSet<String> {
    WORD.find_iter(text)
        .map(|m| m.as_str().to_lowercase().replace('\u{2019}', "'"))
        .collect()
}

/// Capitalised words that do not start a sentence, line or list item.
fn mid_sentence_capitals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for m in WORD.find_iter(text) {
        let w = m.as_str();
        if !w.chars().next().is_some_and(char::is_uppercase) {
            continue;
        }
        let lower = w.to_lowercase();
        if lower == "i" || lower.starts_with("i'") {
            continue;
        }
        let before = text[..m.start()].trim_end_matches([' ', '\t', '"', '\'', '(', '*']);
        let at_start = before.is_empty()
            || before.ends_with(['.', '?', '!', ':', '\n', '-', '\u{2022}'])
            || before.ends_with(|c: char| c.is_ascii_digit()) // "1. Budget" (after "1.")
            || before.ends_with("1.");
        if !at_start {
            out.push(w.to_string());
        }
    }
    out
}

/// Checks an AI answer against what was actually said. `raw` is the recogniser's text,
/// `allowed` are dictionary terms. Returns the final text (snippets restored) if accepted.
pub fn accept(
    reply: &CleanupReply,
    prepared: &Prepared,
    raw: &str,
    allowed: &[String],
    level: Level,
) -> Result<String, Rejection> {
    let out = reply.text.trim();
    if out.is_empty() {
        return Err(Rejection::Empty);
    }
    let sent = prepared.protected.to_tokens(&prepared.text);
    let sent_chars = sent.chars().count();
    let limit = (sent_chars * 13 / 10).max(sent_chars + 20);
    let out_chars = out.chars().count();
    if out_chars > limit {
        return Err(Rejection::TooLong {
            output: out_chars,
            limit,
        });
    }

    // Snippet bodies are checked by token, not by content.
    let checked = TOKEN.replace_all(out, " ").into_owned();
    let source = format!("{raw}\n{sent}\n{}", allowed.join("\n"));

    let numbers = spoken_numbers(&source);
    for m in NUMBER.find_iter(&checked) {
        let key = number_key(m.as_str());
        if !numbers.contains(&key) {
            return Err(Rejection::NewNumber(m.as_str().to_string()));
        }
    }
    let source_lower = source.to_lowercase();
    for m in ADDRESS.find_iter(&checked) {
        let a = m.as_str().trim_end_matches(['.', ',', ')']).to_lowercase();
        if !source_lower.contains(&a) {
            return Err(Rejection::NewEmailOrLink(m.as_str().to_string()));
        }
    }
    let source_words = word_set(&source);
    for name in mid_sentence_capitals(&checked) {
        if !source_words.contains(&name.to_lowercase().replace('\u{2019}', "'")) {
            return Err(Rejection::NewName(name));
        }
    }

    // The model must clean, not write: most of its words must have been spoken.
    let out_words: Vec<String> = WORD
        .find_iter(&checked)
        .map(|m| m.as_str().to_lowercase().replace('\u{2019}', "'"))
        .collect();
    if out_words.len() >= 4 {
        let new = out_words
            .iter()
            .filter(|w| !source_words.contains(w.as_str()))
            .count();
        let allowed_share = if level == Level::Professional { 35 } else { 20 };
        if new * 100 > out_words.len() * allowed_share {
            return Err(Rejection::TooManyNewWords {
                new,
                of: out_words.len(),
            });
        }
    }

    prepared
        .protected
        .restore_tokens(out)
        .map_err(Rejection::Snippet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::Dictionary;
    use crate::pipeline::{prepare, Settings};
    use crate::snippets::{Snippet, SnippetSet};

    fn prep(raw: &str) -> Prepared {
        prepare(
            raw,
            &Settings::default(),
            &Dictionary::default(),
            &SnippetSet::default(),
        )
    }

    fn reply(text: &str) -> CleanupReply {
        CleanupReply {
            text: text.into(),
            uncertain_spans: vec![],
        }
    }

    fn check(raw: &str, answer: &str) -> Result<String, Rejection> {
        accept(&reply(answer), &prep(raw), raw, &[], Level::Light)
    }

    #[test]
    fn template_is_loaded_from_the_spec_file() {
        let t = cleanup_template();
        assert!(t.starts_with("You are a dictation post-processor."));
        for p in [
            "{style_description}",
            "{level}",
            "{dictionary_terms}",
            "{script_instruction}",
        ] {
            assert!(t.contains(p), "missing placeholder {p}");
        }
        let req = build_cleanup(
            &prep("um hello"),
            Level::Light,
            AppCategory::Email,
            "en",
            &[],
        );
        assert!(!req.system.contains("{level}"));
        assert!(req.system.contains("Formal"));
        let user: serde_json::Value = serde_json::from_str(&req.user).unwrap();
        assert_eq!(user["raw"], "um hello");
        assert_eq!(user["app_category"], "Email");
    }

    #[test]
    fn good_cleanups_are_accepted() {
        assert_eq!(
            check(
                "um so lets meet at 2 PM actually make that 3 PM on monday with rahul",
                "So, let's meet at 3 PM on Monday with Rahul."
            )
            .unwrap(),
            "So, let's meet at 3 PM on Monday with Rahul."
        );
        // Spelled-out numbers may become digits.
        assert!(check(
            "the marketing team needs another fifty thousand rupees",
            "The marketing team needs another 50,000 rupees."
        )
        .is_ok());
        assert!(check("sales grew by twenty five percent", "Sales grew by 25%.").is_ok());
    }

    #[test]
    fn invented_facts_are_rejected() {
        assert_eq!(
            check("lets meet on monday", "Let's meet on Monday at 4 PM."),
            Err(Rejection::NewNumber("4".into()))
        );
        assert_eq!(
            check(
                "send the report to the team",
                "Send the report to Priya and the team."
            ),
            Err(Rejection::NewName("Priya".into()))
        );
        assert!(matches!(
            check(
                "please mail the final version of the quarterly report to me at my work address",
                "Please mail the final version of the quarterly report to me at kunal@example.com."
            ),
            Err(Rejection::NewEmailOrLink(_))
        ));
    }

    #[test]
    fn answering_instead_of_cleaning_is_rejected() {
        let r = check(
            "what is the capital of france",
            "The capital of France is Paris, a city known for art and culture.",
        );
        assert!(matches!(
            r,
            Err(Rejection::NewName(_))
                | Err(Rejection::TooManyNewWords { .. })
                | Err(Rejection::TooLong { .. })
        ));
        let r = check(
            "can you help me with this",
            "Sure! I would be happy to assist you with whatever you need today.",
        );
        assert!(r.is_err());
    }

    #[test]
    fn json_is_parsed_from_fenced_answers() {
        let r = parse_reply("```json\n{\"text\": \"Hello.\", \"changes\": []}\n```").unwrap();
        assert_eq!(r.text, "Hello.");
        assert!(matches!(
            parse_reply("Sorry, I cannot."),
            Err(Rejection::NotJson(_))
        ));
    }

    #[test]
    fn snippets_must_be_copied_exactly_once() {
        let snippets = SnippetSet::new(vec![Snippet {
            trigger: "insert my link".into(),
            body: "https://cal.example/k".into(),
        }]);
        let raw = "um book here insert my link";
        let p = prepare(raw, &Settings::default(), &Dictionary::default(), &snippets);
        let req = build_cleanup(&p, Level::Light, AppCategory::Chat, "en", &[]);
        assert!(req.user.contains("[[S1]]"));
        let ok = accept(&reply("Book here: [[S1]]"), &p, raw, &[], Level::Light).unwrap();
        assert_eq!(ok, "Book here: https://cal.example/k");
        assert!(matches!(
            accept(&reply("Book here."), &p, raw, &[], Level::Light),
            Err(Rejection::Snippet(_))
        ));
    }

    #[test]
    fn dictionary_terms_count_as_spoken() {
        let raw = "the ebitda margin improved";
        let allowed = vec!["EBITDA".to_string()];
        assert!(accept(
            &reply("The EBITDA margin improved."),
            &prep(raw),
            raw,
            &allowed,
            Level::Light
        )
        .is_ok());
    }

    #[test]
    fn too_long_answers_are_rejected() {
        let r = check(
            "ok",
            "Okay, that sounds perfectly fine to me and I agree completely.",
        );
        assert!(matches!(r, Err(Rejection::TooLong { .. })));
    }
}
