//! Command Mode: the user selects text, speaks an instruction ("make this a bulleted list"),
//! and the selection is replaced.
//!
//! Common instructions are done here, offline and exactly: lists, letter case, clean-up.
//! Anything else ("make it more formal", "translate to English") goes to the user's AI with
//! [`command_prompt`], and the answer is checked by [`accept_ai_edit`].

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::dictionary::Dictionary;
use crate::pipeline::{self, Level, Settings};
use crate::snippets::SnippetSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Command {
    BulletList,
    NumberedList,
    Uppercase,
    Lowercase,
    TitleCase,
    SentenceCase,
    /// The normal dictation clean-up (fillers, repeated words, punctuation).
    CleanUp,
    /// One paragraph: list markers and line breaks removed.
    OneParagraph,
    /// Free-form instruction for the AI.
    Ai(String),
}

/// What the app pastes. `html` is set for lists so Word and Google Docs make a real list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edit {
    pub text: String,
    pub html: Option<String>,
}

static PATTERNS: LazyLock<Vec<(Regex, Command)>> = LazyLock::new(|| {
    let p = |re: &str| Regex::new(&format!("(?i){re}")).unwrap();
    vec![
        (
            p(
                r"\b(numbered|number|numbers|numeric)\b.*\b(list|points?)\b|\bnumber (it|them|these|this)\b|\b(1|one),? ?(2|two),? ?(3|three)\b|\bginti\b|\bnumber\s*wali\b",
            ),
            Command::NumberedList,
        ),
        (
            p(
                r"\bbullet(ed|s)?\b|\bpoints?\s*(mein|me|bana|banao|bana\s*do)\b|\blist\s*(bana|banao|bana\s*do)\b|\b(make|turn|convert|put)\b.*\b(a|into a|in a|as a)?\s*list\b|^\s*list\s*$",
            ),
            Command::BulletList,
        ),
        (
            p(r"\b(upper\s*case|all\s*caps|capital\s*letters?|caps\s*lock|bade\s*akshar)\b"),
            Command::Uppercase,
        ),
        (
            p(r"\b(lower\s*case|small\s*letters?|chhote\s*akshar|no\s*caps)\b"),
            Command::Lowercase,
        ),
        (
            p(r"\btitle\s*case\b|\bcapitali[sz]e\s+(each|every)\s+word\b"),
            Command::TitleCase,
        ),
        (p(r"\bsentence\s*case\b"), Command::SentenceCase),
        (
            p(r"\b(one|single)\s+paragraph\b|\bremove\s+(the\s+)?(bullets|list|line\s*breaks)\b"),
            Command::OneParagraph,
        ),
        (
            p(
                r"^\s*(clean\s*(it|this)?\s*up|tidy\s*(it|this)?\s*up|fix\s+(the\s+)?(punctuation|grammar)|remove\s+(the\s+)?(filler|fillers|ums))\b",
            ),
            Command::CleanUp,
        ),
    ]
});

/// Reads a spoken instruction. Anything not recognised is left for the AI.
pub fn parse(spoken: &str) -> Command {
    let s = spoken.trim().trim_end_matches(['.', '!', '?']);
    for (re, cmd) in PATTERNS.iter() {
        if re.is_match(s) {
            return cmd.clone();
        }
    }
    Command::Ai(s.to_string())
}

static MARKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:[-*\u{2022}\u{25CF}\u{25E6}]|\d{1,3}[.)]|[a-zA-Z][.)])\s+").unwrap()
});
static SENTENCE_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([.?!])\s+").unwrap());

/// Splits a selection into list items: by line when it has several lines, otherwise by
/// sentence, otherwise a comma list ("A, B and C") when there are at least three parts.
pub fn items(text: &str) -> Vec<String> {
    let lines: Vec<String> = text
        .lines()
        .map(|l| MARKER.replace(l, "").trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() > 1 {
        return lines;
    }
    let one = lines.into_iter().next().unwrap_or_default();
    let sentences: Vec<String> = SENTENCE_END
        .replace_all(&one, "$1\n")
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if sentences.len() > 1 {
        return sentences;
    }
    // "A, B and C" (optionally after "...: ").
    let body = one.rsplit_once(": ").map_or(one.as_str(), |(_, b)| b);
    let body = body.trim_end_matches(['.', '!', '?']);
    let mut parts: Vec<String> = body.split(", ").map(str::to_string).collect();
    if let Some(last) = parts.pop() {
        match last.split_once(" and ") {
            Some((a, b)) if !parts.is_empty() => {
                parts.push(a.to_string());
                parts.push(b.to_string());
            }
            _ => parts.push(last),
        }
    }
    let parts: Vec<String> = parts
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() >= 3 && parts.iter().all(|p| p.split_whitespace().count() <= 8) {
        return parts.into_iter().map(|p| capitalise_first(&p)).collect();
    }
    vec![one]
}

fn capitalise_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn list(text: &str, numbered: bool) -> Edit {
    let items = items(text);
    let plain: Vec<String> = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            if numbered {
                format!("{}. {it}", i + 1)
            } else {
                format!("\u{2022} {it}")
            }
        })
        .collect();
    let tag = if numbered { "ol" } else { "ul" };
    let html = format!(
        "<{tag}>{}</{tag}>",
        items
            .iter()
            .map(|it| format!("<li>{}</li>", escape_html(it)))
            .collect::<String>()
    );
    Edit {
        text: plain.join("\n"),
        html: Some(html),
    }
}

fn title_case(text: &str) -> String {
    const SMALL: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "of", "in", "on", "at", "to", "for", "by",
    ];
    text.split(' ')
        .enumerate()
        .map(|(i, w)| {
            let lower = w.to_lowercase();
            if i > 0 && SMALL.contains(&lower.as_str()) {
                lower
            } else if w.chars().skip(1).any(char::is_uppercase) {
                w.to_string() // keep acronyms and names like "iPhone", "EBITDA"
            } else {
                capitalise_first(&lower)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentence_case(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut start = true;
    for c in lower.chars() {
        if start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.push(c);
        }
        if matches!(c, '.' | '?' | '!' | '\n') {
            start = true;
        }
    }
    out.replace(" i ", " I ")
}

/// Applies a built-in command. `None` for [`Command::Ai`] (the caller asks the AI).
pub fn apply(command: &Command, selection: &str) -> Option<Edit> {
    let plain = |text: String| Some(Edit { text, html: None });
    match command {
        Command::BulletList => Some(list(selection, false)),
        Command::NumberedList => Some(list(selection, true)),
        Command::Uppercase => plain(selection.to_uppercase()),
        Command::Lowercase => plain(selection.to_lowercase()),
        Command::TitleCase => plain(
            selection
                .lines()
                .map(title_case)
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Command::SentenceCase => plain(sentence_case(selection)),
        Command::OneParagraph => plain(items(selection).join(" ")),
        Command::CleanUp => plain(
            pipeline::run(
                selection,
                &Settings {
                    level: Level::Light,
                    spoken_commands: false,
                },
                &Dictionary::default(),
                &SnippetSet::default(),
            )
            .text,
        ),
        Command::Ai(_) => None,
    }
}

/// System prompt for free-form edits (see `spec/prompts/command.md`).
pub fn command_prompt() -> String {
    const SPEC: &str = include_str!("../../../spec/prompts/command.md");
    let after = SPEC.split_once("## System prompt").map_or("", |(_, r)| r);
    let body = after.split_once("```\n").map_or("", |(_, r)| r);
    body.split_once("\n```")
        .map_or("", |(b, _)| b)
        .replace('\r', "")
}

/// The user message for an AI edit.
pub fn command_request(instruction: &str, selection: &str) -> String {
    serde_json::json!({ "instruction": instruction, "text": selection }).to_string()
}

static NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(?:[.,]\d+)*").unwrap());

/// Checks an AI edit: valid JSON {"text"}, not empty, not wildly longer than asked, and no
/// number that is neither in the selection nor in the instruction (an AI must not invent
/// amounts or dates).
pub fn accept_ai_edit(answer: &str, instruction: &str, selection: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Reply {
        text: String,
    }
    let (s, e) = (answer.find('{'), answer.rfind('}'));
    let reply: Reply = match (s, e) {
        (Some(s), Some(e)) if e > s => {
            serde_json::from_str(&answer[s..=e]).map_err(|e| format!("invalid JSON: {e}"))?
        }
        _ => return Err("no JSON in the answer".into()),
    };
    let out = reply.text.trim().to_string();
    if out.is_empty() {
        return Err("empty answer".into());
    }
    let wants_longer = Regex::new(r"(?i)\b(longer|expand|elaborate|detail|lamba|write)\b")
        .unwrap()
        .is_match(instruction);
    if !wants_longer && out.chars().count() > selection.chars().count() * 3 + 200 {
        return Err("answer far longer than the selection".into());
    }
    let known: Vec<String> = NUM
        .find_iter(&format!("{selection} {instruction}"))
        .map(|m| m.as_str().replace(',', ""))
        .collect();
    if let Some(n) = NUM
        .find_iter(&out)
        .map(|m| m.as_str().replace(',', ""))
        .find(|n| !known.contains(n) && !is_list_number(&out, n))
    {
        return Err(format!("new number {n} not in the selection"));
    }
    Ok(out)
}

/// "1." "2." at the start of a line are list numbers, not invented amounts.
fn is_list_number(text: &str, n: &str) -> bool {
    text.lines().any(|l| {
        l.trim_start().starts_with(&format!("{n}.")) || l.trim_start().starts_with(&format!("{n})"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_instructions_are_recognised() {
        for s in [
            "make this a bulleted list",
            "Make it bullet points.",
            "convert to a list",
            "bullet points bana do",
            "isko list bana do",
        ] {
            assert_eq!(parse(s), Command::BulletList, "{s}");
        }
        for s in [
            "make this a numbered list",
            "number them",
            "numbered points",
        ] {
            assert_eq!(parse(s), Command::NumberedList, "{s}");
        }
        assert_eq!(parse("all caps"), Command::Uppercase);
        assert_eq!(parse("make it lowercase"), Command::Lowercase);
        assert_eq!(parse("title case"), Command::TitleCase);
        assert_eq!(parse("clean this up"), Command::CleanUp);
        assert_eq!(parse("make it one paragraph"), Command::OneParagraph);
        assert_eq!(
            parse("make it more formal"),
            Command::Ai("make it more formal".into())
        );
    }

    #[test]
    fn lists_from_sentences_lines_and_commas() {
        let e = apply(
            &Command::BulletList,
            "Check the invoice. Call the client. Send the report.",
        )
        .unwrap();
        assert_eq!(
            e.text,
            "\u{2022} Check the invoice.\n\u{2022} Call the client.\n\u{2022} Send the report."
        );
        assert_eq!(
            e.html.as_deref(),
            Some("<ul><li>Check the invoice.</li><li>Call the client.</li><li>Send the report.</li></ul>")
        );
        let n = apply(&Command::NumberedList, "- milk\n- eggs\n- bread").unwrap();
        assert_eq!(n.text, "1. milk\n2. eggs\n3. bread");
        let c = apply(
            &Command::BulletList,
            "We need three things: budget, timeline and team.",
        )
        .unwrap();
        assert_eq!(c.text, "\u{2022} Budget\n\u{2022} Timeline\n\u{2022} Team");
        // A single sentence without a list stays one item.
        let one = apply(&Command::BulletList, "Please call me back tomorrow.").unwrap();
        assert_eq!(one.text, "\u{2022} Please call me back tomorrow.");
    }

    #[test]
    fn case_commands_keep_acronyms() {
        assert_eq!(
            apply(&Command::TitleCase, "review of the EBITDA margin")
                .unwrap()
                .text,
            "Review of the EBITDA Margin"
        );
        assert_eq!(
            apply(&Command::SentenceCase, "HELLO THERE. HOW ARE YOU?")
                .unwrap()
                .text,
            "Hello there. How are you?"
        );
        assert_eq!(
            apply(&Command::OneParagraph, "1. First point.\n2. Second point.")
                .unwrap()
                .text,
            "First point. Second point."
        );
    }

    #[test]
    fn ai_edits_may_not_invent_numbers() {
        let sel = "Revenue grew 12% this quarter.";
        assert_eq!(
            accept_ai_edit(
                r#"{"text":"Revenue rose by 12% this quarter."}"#,
                "more formal",
                sel
            )
            .unwrap(),
            "Revenue rose by 12% this quarter."
        );
        assert!(accept_ai_edit(
            r#"{"text":"Revenue grew 15% this quarter."}"#,
            "more formal",
            sel
        )
        .is_err());
        assert!(accept_ai_edit(r#"{"text":"1. Revenue grew 12%"}"#, "make a list", sel).is_ok());
        assert!(accept_ai_edit("sorry", "more formal", sel).is_err());
        assert!(command_prompt().contains("instruction"));
    }
}
