//! Web bindings: the same deterministic clean-up and AI-answer checks as the desktop app.
//! Inputs and outputs are JSON strings to keep the boundary simple.

use app_core::dictionary::{Dictionary, Term};
use app_core::llm::{self, AppCategory};
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::{Snippet, SnippetSet};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Deserialize, Default)]
#[serde(default)]
struct Vocabulary {
    terms: Vec<Term>,
    snippets: Vec<Snippet>,
}

fn settings(level: &str, spoken_commands: bool) -> Settings {
    Settings {
        level: match level {
            "LITERAL" => Level::Literal,
            "PROFESSIONAL" => Level::Professional,
            _ => Level::Light,
        },
        spoken_commands,
    }
}

fn vocabulary(json: &str) -> (Dictionary, SnippetSet) {
    let v: Vocabulary = serde_json::from_str(json).unwrap_or_default();
    (Dictionary::new(v.terms), SnippetSet::new(v.snippets))
}

/// Rule-based clean-up. Returns the cleaned text.
#[wasm_bindgen]
pub fn clean(raw: &str, level: &str, spoken_commands: bool, vocabulary_json: &str) -> String {
    let (d, s) = vocabulary(vocabulary_json);
    pipeline::run(raw, &settings(level, spoken_commands), &d, &s).text
}

/// Builds the AI clean-up request. Returns JSON {"system": ..., "user": ...}.
#[wasm_bindgen]
pub fn cleanup_request(
    raw: &str,
    level: &str,
    spoken_commands: bool,
    vocabulary_json: &str,
    language: &str,
) -> String {
    let (d, s) = vocabulary(vocabulary_json);
    let st = settings(level, spoken_commands);
    let prepared = pipeline::prepare(raw, &st, &d, &s);
    let req = llm::build_cleanup(&prepared, st.level, AppCategory::Other, language, d.terms());
    serde_json::json!({ "system": req.system, "user": req.user }).to_string()
}

/// Checks an AI answer. Returns JSON {"ok": true, "text": ...} or {"ok": false, "why": ..., "text": rules}.
#[wasm_bindgen]
pub fn accept_cleanup(
    answer: &str,
    raw: &str,
    level: &str,
    spoken_commands: bool,
    vocabulary_json: &str,
) -> String {
    let (d, s) = vocabulary(vocabulary_json);
    let st = settings(level, spoken_commands);
    let prepared = pipeline::prepare(raw, &st, &d, &s);
    let rules = pipeline::finish(&prepared, &st).text;
    let verdict = llm::parse_reply(answer)
        .and_then(|r| llm::accept(&r, &prepared, raw, d.terms(), st.level, &rules));
    match verdict {
        Ok(text) => serde_json::json!({ "ok": true, "text": text }).to_string(),
        Err(why) => {
            serde_json::json!({ "ok": false, "why": why.to_string(), "text": rules }).to_string()
        }
    }
}

#[wasm_bindgen]
pub fn has_devanagari(text: &str) -> bool {
    llm::has_devanagari(text)
}

#[wasm_bindgen]
pub fn transliterate_prompt() -> String {
    llm::transliterate_prompt()
}

/// Checks a Devanagari -> Roman conversion. Returns the Roman text or "" when rejected.
#[wasm_bindgen]
pub fn accept_transliteration(answer: &str, source: &str) -> String {
    llm::parse_transliteration(answer, source).unwrap_or_default()
}

#[wasm_bindgen]
pub fn polish_prompt() -> String {
    llm::hinglish_polish_prompt()
}

/// Checks a Hinglish spelling polish. Returns the polished text or "" when rejected.
#[wasm_bindgen]
pub fn accept_polish(answer: &str, source: &str) -> String {
    llm::parse_polish(answer, source).unwrap_or_default()
}

#[wasm_bindgen]
pub fn roman_hinglish_example() -> String {
    llm::ROMAN_HINGLISH_EXAMPLE.to_string()
}

/// The Finance and Tech word packs, as JSON terms.
#[wasm_bindgen]
pub fn default_terms() -> String {
    let mut t = app_core::dictionary::pack("finance");
    t.extend(app_core::dictionary::pack("tech"));
    serde_json::to_string(&t).unwrap_or_else(|_| "[]".into())
}

/// Vocabulary hint for the recogniser ("Glossary: ...").
#[wasm_bindgen]
pub fn recogniser_hint(vocabulary_json: &str) -> String {
    vocabulary(vocabulary_json).0.recogniser_hint(400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_results_as_the_desktop_pipeline() {
        assert_eq!(
            clean(
                "Let's meet at 2 PM... actually make that 3 PM",
                "LIGHT",
                true,
                "{}"
            ),
            "Let's meet at 3 PM."
        );
        let v = format!("{{\"terms\": {}}}", default_terms());
        assert_eq!(
            clean("our ebitda grew", "LIGHT", true, &v),
            "Our EBITDA grew"
        );
        assert!(recogniser_hint(&v).starts_with("Glossary: CMA"));
        let bad = accept_cleanup(
            r#"{"text":"Let's meet at 4 PM."}"#,
            "lets meet at 3 PM",
            "LIGHT",
            true,
            "{}",
        );
        assert!(bad.contains("\"ok\":false"));
    }
}
