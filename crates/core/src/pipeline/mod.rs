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
mod tidy;
pub mod tokens;

pub use normalise::normalise;

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
