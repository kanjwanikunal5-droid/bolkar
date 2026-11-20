//! The dictation text pipeline. Identical on every platform.
//!
//! ```text
//! raw transcript
//!   -> normalise -> snippets -> dictionary -> spoken commands
//!   -> (LIGHT / PROFESSIONAL) backtracking -> fillers -> stutters -> lists
//!   -> tidy (spacing, capitals)
//! ```
//!
//! This is the DeterministicRules implementation: it never calls a model and never adds
//! words that were not spoken. When a rule is not sure, it leaves the text alone and
//! records the span in `uncertain_spans`.

use serde::{Deserialize, Serialize};

use crate::dictionary::Dictionary;
use crate::snippets::SnippetSet;

mod backtrack;
mod commands;
mod fillers;
mod lists;
mod normalise;
mod structure;
mod tidy;
pub mod tokens;

pub use normalise::normalise;
pub use structure::{hands_free_structure, looks_like_independent_items, sentences, ListFormat};

/// Recognisers sometimes answer in capitals only ("KAL HAMNE SALES DISCUSS KI."), seen with
/// Groq Whisper on Hinglish. Such text (four or more words, no lowercase letter at all) gets
/// normal sentence capitals; the words themselves are not changed. Anything with a lowercase
/// letter is returned as it is, so acronyms in normal text stay.
pub fn fix_all_caps(text: &str) -> String {
    let words = text
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphabetic))
        .count();
    if words < 4 || text.chars().any(char::is_lowercase) || !text.chars().any(char::is_uppercase) {
        return text.to_string();
    }
    let lower = text.to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut start = true;
    for (i, c) in lower.char_indices() {
        if start && c.is_alphabetic() {
            out.extend(c.to_uppercase());
            start = false;
        } else {
            out.push(c);
        }
        if matches!(c, '.' | '?' | '!' | '\n') {
            start = true;
        }
        // "i" on its own is "I".
        if c == 'i'
            && lower[..i].ends_with(' ')
            && lower[i + 1..].starts_with([' ', '\'', ',', '.'])
        {
            out.pop();
            out.push('I');
        }
    }
    out
}

/// Spoken enumerations ("first thing ..., second thing ...", "pehli baat ...") as a numbered
/// list; text without one is returned unchanged. Safe to run on already-formatted text.
pub fn format_spoken_lists(text: &str) -> String {
    lists::format_lists(text, &mut Vec::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    /// Spacing and capitals only. Fillers are kept.
    Literal,
    /// Remove fillers and stutters, apply self-corrections, format lists.
    #[default]
    Light,
    /// LIGHT plus grammar polish. Grammar polish needs an LLM, so the deterministic
    /// pipeline treats this the same as LIGHT.
    Professional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Snippet,
    Dictionary,
    Command,
    Backtrack,
    Filler,
    Stutter,
    List,
}

/// One edit the pipeline made, kept so the UI can show "what changed".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub kind: ChangeKind,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub level: Level,
    /// "new line", "full stop", "bullet point" and friends.
    pub spoken_commands: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            level: Level::Light,
            spoken_commands: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub text: String,
    pub changes: Vec<Change>,
    /// Places where a self-correction was suspected but not applied.
    pub uncertain_spans: Vec<String>,
}

/// The text after the steps every path shares (normalise, snippets, dictionary, spoken
/// commands). Expanded snippet bodies are still parked behind placeholders.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub text: String,
    pub protected: crate::snippets::Protected,
    pub changes: Vec<Change>,
}

pub fn prepare(
    raw: &str,
    settings: &Settings,
    dictionary: &Dictionary,
    snippets: &SnippetSet,
) -> Prepared {
    let mut changes = Vec::new();
    let text = normalise::normalise(raw);
    let (mut text, protected) = snippets.expand(&text, &mut changes);
    text = dictionary.apply(&text, &mut changes);
    if settings.spoken_commands {
        text = commands::apply(&text, &mut changes);
    }
    Prepared {
        text,
        protected,
        changes,
    }
}

/// The rule-based clean-up of prepared text.
pub fn finish(prepared: &Prepared, settings: &Settings) -> Output {
    let mut changes = prepared.changes.clone();
    let mut uncertain = Vec::new();
    let mut text = prepared.text.clone();
    if settings.level != Level::Literal {
        text = backtrack::apply(&text, &mut changes, &mut uncertain);
        text = fillers::remove_fillers(&text, &mut changes);
        text = fillers::collapse_stutters(&text, &mut changes);
        text = lists::format_lists(&text, &mut changes);
    }
    text = tidy::tidy(&text);
    text = prepared.protected.restore(&text);

    Output {
        text,
        changes,
        uncertain_spans: uncertain,
    }
}

/// Runs the whole deterministic pipeline on one raw transcript.
pub fn run(
    raw: &str,
    settings: &Settings,
    dictionary: &Dictionary,
    snippets: &SnippetSet,
) -> Output {
    finish(&prepare(raw, settings, dictionary, snippets), settings)
}

#[cfg(test)]
mod formatting_regressions {
    //! The audit's acceptance cases (17 Nov 2026): prose stays prose, real lists become lists.
    use super::*;

    fn dictate(raw: &str, hands_free: Option<ListFormat>) -> String {
        let out = run(
            raw,
            &Settings::default(),
            &Dictionary::default(),
            &SnippetSet::default(),
        );
        let text = format_spoken_lists(&out.text);
        match hands_free {
            Some(f) => hands_free_structure(&text, f),
            None => text,
        }
    }

    #[test]
    fn a_connected_prose_is_one_paragraph() {
        let raw = "Today we discussed the company's financial performance. Revenue increased by 15% compared to last quarter, mainly because of higher sales volume. However, operating expenses also increased, which affected profitability. We need to investigate the expense increase before the next review.";
        for mode in [None, Some(ListFormat::Natural), Some(ListFormat::Auto)] {
            let out = dictate(raw, mode);
            assert!(!out.contains('\n'), "{mode:?}: {out}");
            assert!(!out.contains('•') && !out.contains("- "), "{out}");
            assert!(out.contains("15%"), "numbers kept: {out}");
        }
    }

    #[test]
    fn b_spoken_enumeration_is_a_numbered_list_after_its_lead_in() {
        let out = dictate(
            "We have three priorities. First, finish the forecast. Second, review hiring costs. Third, send the report by Friday.",
            Some(ListFormat::Natural),
        );
        assert_eq!(
            out,
            "We have three priorities.\n1. Finish the forecast\n2. Review hiring costs\n3. Send the report by Friday"
        );
    }

    #[test]
    fn c_bullet_commands_make_two_bullets() {
        let out = dictate(
            "Bullet point: update the forecast. Next point: check cash flow.",
            Some(ListFormat::Natural),
        );
        let bullets: Vec<&str> = out.lines().filter(|l| l.starts_with("\u{2022} ")).collect();
        assert_eq!(bullets.len(), 2, "{out}");
        assert!(
            bullets[0].to_lowercase().contains("update the forecast"),
            "{out}"
        );
        assert!(
            bullets[1].to_lowercase().contains("check cash flow"),
            "{out}"
        );
    }

    #[test]
    fn capitals_only_answers_get_sentence_case_and_nothing_else_changes() {
        assert_eq!(
            fix_all_caps(
                "KAL HAMNE SALES PERFORMANCE DISCUSS KI. REVENUE 15% INCREASE HUA, I THINK."
            ),
            "Kal hamne sales performance discuss ki. Revenue 15% increase hua, I think."
        );
        for unchanged in [
            "The EBITDA margin is up.",
            "OK GST",
            "Sales are up 12%.",
            "",
        ] {
            assert_eq!(fix_all_caps(unchanged), unchanged);
        }
    }

    #[test]
    fn d_connected_hinglish_stays_a_roman_paragraph() {
        let raw = "Kal humne sales performance discuss ki. Revenue 15 percent increase hua, mainly because sales volume better tha. Lekin operating expenses bhi increase hue, jis wajah se profitability impact hui.";
        for mode in [Some(ListFormat::Natural), Some(ListFormat::Auto)] {
            let out = dictate(raw, mode);
            assert!(!out.contains('\n'), "{out}");
            assert!(!out.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c)));
        }
    }
}
