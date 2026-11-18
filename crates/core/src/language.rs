//! The product's language modes and how each maps to a speech-recognition language.
//!
//! The Settings value ("en", "en-hinglish", "hinglish") is a product choice, not a Whisper
//! language code. Only [`RecogniserLanguage::code`] may produce the string handed to the
//! recogniser, and it can only produce "en", "hi" or nothing (detect).

/// What the user chose in Settings > Speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageMode {
    /// English only.
    English,
    /// English or Hinglish, decided for every piece of speech (the default).
    EnglishHinglish,
    /// Always Hinglish (typed in Roman letters).
    Hinglish,
}

impl LanguageMode {
    /// Unknown or old values (for example "auto" from an early version) fall back to the
    /// default mode instead of being passed on.
    pub fn from_setting(value: &str) -> Self {
        match value.trim() {
            "en" => Self::English,
            "hinglish" => Self::Hinglish,
            _ => Self::EnglishHinglish,
        }
    }

    pub fn setting(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::EnglishHinglish => "en-hinglish",
            Self::Hinglish => "hinglish",
        }
    }

    /// True when Hinglish output (Roman letters) is possible in this mode.
    pub fn allows_hinglish(self) -> bool {
        !matches!(self, Self::English)
    }
}

/// The language one piece of audio is recognised in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecogniserLanguage {
    English,
    /// Hindi recognition, steered to Roman letters by an example prompt.
    HindiRoman,
}

impl RecogniserLanguage {
    /// The only language codes ever given to Whisper.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::HindiRoman => "hi",
        }
    }
}

/// How a mode is recognised: one fixed language, or a decision per piece of audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognitionPlan {
    Fixed(RecogniserLanguage),
    /// Ask the recogniser's language detector for each piece, then pick English or Hinglish.
    DetectPerPiece,
}

pub fn plan(mode: LanguageMode) -> RecognitionPlan {
    match mode {
        LanguageMode::English => RecognitionPlan::Fixed(RecogniserLanguage::English),
        LanguageMode::Hinglish => RecognitionPlan::Fixed(RecogniserLanguage::HindiRoman),
        LanguageMode::EnglishHinglish => RecognitionPlan::DetectPerPiece,
    }
}

/// The recogniser language for one piece. `hinglish_detected` is the language detector's
/// verdict for that piece (None when detection was not possible: English is used, as it is
/// the safe choice that never translates).
pub fn choose(plan: RecognitionPlan, hinglish_detected: Option<bool>) -> RecogniserLanguage {
    match plan {
        RecognitionPlan::Fixed(lang) => lang,
        RecognitionPlan::DetectPerPiece => {
            if hinglish_detected == Some(true) {
                RecogniserLanguage::HindiRoman
            } else {
                RecogniserLanguage::English
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_unknown_values_fall_back() {
        for mode in [
            LanguageMode::English,
            LanguageMode::EnglishHinglish,
            LanguageMode::Hinglish,
        ] {
            assert_eq!(LanguageMode::from_setting(mode.setting()), mode);
        }
        assert_eq!(
            LanguageMode::from_setting("auto"),
            LanguageMode::EnglishHinglish
        );
        assert_eq!(
            LanguageMode::from_setting(""),
            LanguageMode::EnglishHinglish
        );
    }

    #[test]
    fn product_modes_never_reach_the_recogniser_as_language_codes() {
        for setting in ["en", "en-hinglish", "hinglish", "auto", "garbage"] {
            let p = plan(LanguageMode::from_setting(setting));
            for detected in [None, Some(false), Some(true)] {
                let code = choose(p, detected).code();
                assert!(
                    code == "en" || code == "hi",
                    "{setting} gave language code {code}"
                );
                assert_ne!(code, "en-hinglish");
                assert_ne!(code, "hinglish");
            }
        }
    }

    #[test]
    fn each_mode_maps_to_the_expected_language() {
        assert_eq!(
            choose(plan(LanguageMode::English), Some(true)),
            RecogniserLanguage::English,
            "English only stays English even if Hindi is detected"
        );
        assert_eq!(
            choose(plan(LanguageMode::Hinglish), Some(false)),
            RecogniserLanguage::HindiRoman
        );
        assert_eq!(
            choose(plan(LanguageMode::EnglishHinglish), Some(true)),
            RecogniserLanguage::HindiRoman
        );
        assert_eq!(
            choose(plan(LanguageMode::EnglishHinglish), Some(false)),
            RecogniserLanguage::English
        );
        assert_eq!(
            choose(plan(LanguageMode::EnglishHinglish), None),
            RecogniserLanguage::English
        );
    }
}
