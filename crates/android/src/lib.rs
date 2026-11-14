//! Android bindings: the same deterministic clean-up and AI-answer checks as the desktop and
//! web apps, exposed to Kotlin (`app.voice.Core`) through JNI. Every value crosses the
//! boundary as a string (JSON where it is structured) to keep the glue small.
//!
//! The logic lives in [`api`], plain Rust that the unit tests call directly; the `Java_*`
//! functions only convert strings.

pub mod api {
    use app_core::dictionary::{Dictionary, Term};
    use app_core::llm::{self, AppCategory};
    use app_core::pipeline::{self, Level, Settings};
    use app_core::snippets::{Snippet, SnippetSet};
    use serde::Deserialize;

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

    /// Rule-based clean-up.
    pub fn clean(raw: &str, level: &str, spoken_commands: bool, vocabulary_json: &str) -> String {
        let (d, s) = vocabulary(vocabulary_json);
        pipeline::run(raw, &settings(level, spoken_commands), &d, &s).text
    }

    /// The AI clean-up request as JSON {"system": ..., "user": ...}.
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

    /// Checks an AI clean-up answer: JSON {"ok": true, "text"} or {"ok": false, "why", "text": rules}.
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
        match llm::parse_reply(answer)
            .and_then(|r| llm::accept(&r, &prepared, raw, d.terms(), st.level, &rules))
        {
            Ok(text) => serde_json::json!({ "ok": true, "text": text }).to_string(),
            Err(why) => serde_json::json!({ "ok": false, "why": why.to_string(), "text": rules })
                .to_string(),
        }
    }

    pub fn has_devanagari(text: &str) -> bool {
        llm::has_devanagari(text)
    }

    pub fn transliterate_prompt() -> String {
        llm::transliterate_prompt()
    }

    /// The Roman text, or "" when the conversion is rejected.
    pub fn accept_transliteration(answer: &str, source: &str) -> String {
        llm::parse_transliteration(answer, source).unwrap_or_default()
    }

    pub fn polish_prompt() -> String {
        llm::hinglish_polish_prompt()
    }

    /// The polished text, or "" when the polish is rejected (a rewrite, changed numbers...).
    pub fn accept_polish(answer: &str, source: &str) -> String {
        llm::parse_polish(answer, source).unwrap_or_default()
    }

    pub fn roman_hinglish_example() -> String {
        llm::ROMAN_HINGLISH_EXAMPLE.to_string()
    }

    /// Vocabulary hint for the recogniser ("Glossary: ...").
    pub fn recogniser_hint(vocabulary_json: &str) -> String {
        vocabulary(vocabulary_json).0.recogniser_hint(400)
    }

    /// The Finance and Tech word packs as JSON terms.
    pub fn default_terms() -> String {
        let mut t = app_core::dictionary::pack("finance");
        t.extend(app_core::dictionary::pack("tech"));
        serde_json::to_string(&t).unwrap_or_else(|_| "[]".into())
    }

    pub fn app_name() -> String {
        app_core::brand::APP_NAME.to_string()
    }
}

mod jni_glue {
    use jni::objects::{JClass, JString};
    use jni::sys::{jboolean, jstring, JNI_FALSE, JNI_TRUE};
    use jni::JNIEnv;

    use crate::api;

    fn arg(env: &mut JNIEnv, s: &JString) -> String {
        env.get_string(s).map(Into::into).unwrap_or_default()
    }

    fn ret(env: &mut JNIEnv, s: String) -> jstring {
        env.new_string(s)
            .map(|j| j.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_clean<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        raw: JString<'l>,
        level: JString<'l>,
        spoken: jboolean,
        vocabulary: JString<'l>,
    ) -> jstring {
        let out = api::clean(
            &arg(&mut env, &raw),
            &arg(&mut env, &level),
            spoken != JNI_FALSE,
            &arg(&mut env, &vocabulary),
        );
        ret(&mut env, out)
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_cleanupRequest<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        raw: JString<'l>,
        level: JString<'l>,
        spoken: jboolean,
        vocabulary: JString<'l>,
        language: JString<'l>,
    ) -> jstring {
        let out = api::cleanup_request(
            &arg(&mut env, &raw),
            &arg(&mut env, &level),
            spoken != JNI_FALSE,
            &arg(&mut env, &vocabulary),
            &arg(&mut env, &language),
        );
        ret(&mut env, out)
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_acceptCleanup<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        answer: JString<'l>,
        raw: JString<'l>,
        level: JString<'l>,
        spoken: jboolean,
        vocabulary: JString<'l>,
    ) -> jstring {
        let out = api::accept_cleanup(
            &arg(&mut env, &answer),
            &arg(&mut env, &raw),
            &arg(&mut env, &level),
            spoken != JNI_FALSE,
            &arg(&mut env, &vocabulary),
        );
        ret(&mut env, out)
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_hasDevanagari<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        text: JString<'l>,
    ) -> jboolean {
        if api::has_devanagari(&arg(&mut env, &text)) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_acceptTransliteration<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        answer: JString<'l>,
        source: JString<'l>,
    ) -> jstring {
        let out = api::accept_transliteration(&arg(&mut env, &answer), &arg(&mut env, &source));
        ret(&mut env, out)
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_acceptPolish<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        answer: JString<'l>,
        source: JString<'l>,
    ) -> jstring {
        let out = api::accept_polish(&arg(&mut env, &answer), &arg(&mut env, &source));
        ret(&mut env, out)
    }

    #[no_mangle]
    pub extern "system" fn Java_app_voice_Core_recogniserHint<'l>(
        mut env: JNIEnv<'l>,
        _c: JClass<'l>,
        vocabulary: JString<'l>,
    ) -> jstring {
        let out = api::recogniser_hint(&arg(&mut env, &vocabulary));
        ret(&mut env, out)
    }

    macro_rules! no_arg {
        ($name:ident, $f:path) => {
            #[no_mangle]
            pub extern "system" fn $name<'l>(mut env: JNIEnv<'l>, _c: JClass<'l>) -> jstring {
                ret(&mut env, $f())
            }
        };
    }
    no_arg!(
        Java_app_voice_Core_transliteratePrompt,
        api::transliterate_prompt
    );
    no_arg!(Java_app_voice_Core_polishPrompt, api::polish_prompt);
    no_arg!(
        Java_app_voice_Core_romanHinglishExample,
        api::roman_hinglish_example
    );
    no_arg!(Java_app_voice_Core_defaultTerms, api::default_terms);
    no_arg!(Java_app_voice_Core_appName, api::app_name);
}

#[cfg(test)]
mod tests {
    use super::api::*;

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

    #[test]
    fn hinglish_helpers() {
        assert!(roman_hinglish_example().starts_with("Haan bhai"));
        assert!(has_devanagari("मैं"));
        assert!(!has_devanagari("main"));
        assert_eq!(
            accept_polish(
                r#"{"text":"Main bahut busy hoon."}"#,
                "Main baut busy hoon."
            ),
            "Main bahut busy hoon."
        );
        assert_eq!(
            accept_polish(r#"{"text":"I am very busy."}"#, "Main baut busy hoon."),
            ""
        );
        assert_eq!(
            accept_transliteration(r#"{"text":"main busy hoon"}"#, "मैं busy हूँ"),
            "main busy hoon"
        );
        assert!(polish_prompt().contains("WhatsApp"));
        assert!(transliterate_prompt().contains("Devanagari"));
    }
}
