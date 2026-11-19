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

/// A recognised stretch of speech with its time in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Timed {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn has_devanagari(s: &str) -> bool {
    s.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c))
}

/// Which segments of a Hindi-mode recognition came back in Hindi letters (and so need the
/// "was it really English?" check).
pub fn devanagari_segments(hindi: &[Timed]) -> Vec<usize> {
    hindi
        .iter()
        .enumerate()
        .filter(|(_, s)| has_devanagari(&s.text))
        .map(|(i, _)| i)
        .collect()
}

/// Measured on the owner's own dictations (19 Nov 2026): in Hindi mode the recogniser
/// sometimes turns the English part of a long dictation into invented Hindi-letter text, while
/// English mode translates real Hindi into English. So the Hindi-mode result is kept, except
/// for the segments the language check found to be English (`english[i]` true): those take
/// the English-mode words recognised in the same seconds. Segments with no English
/// counterpart are kept as they were.
pub fn rescue_english(hindi: &[Timed], english_spoken: &[bool], english: &[Timed]) -> String {
    let mut used = vec![false; english.len()];
    let mut out: Vec<String> = Vec::new();
    for (i, h) in hindi.iter().enumerate() {
        if english_spoken.get(i).copied().unwrap_or(false) {
            let mut picked: Vec<String> = Vec::new();
            for (j, e) in english.iter().enumerate() {
                let mid = (e.start + e.end) / 2.0;
                if !used[j] && mid >= h.start - 0.5 && mid <= h.end + 0.5 {
                    used[j] = true;
                    picked.push(e.text.trim().to_string());
                }
            }
            if !picked.is_empty() {
                out.push(picked.join(" "));
                continue;
            }
        }
        out.push(h.text.trim().to_string());
    }
    out.retain(|s| !s.is_empty());
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tt(start: f64, end: f64, text: &str) -> Timed {
        Timed {
            start,
            end,
            text: text.into(),
        }
    }

    #[test]
    fn english_said_but_written_in_hindi_letters_is_replaced_hinglish_kept() {
        // The owner's dictation of 19 Nov: English throughout, but the end came back as
        // invented Hindi-letter text; a real Hindi sentence in the middle must stay.
        let hindi = vec![
            tt(
                0.0,
                6.0,
                "So, when I click Ctrl plus Windows plus H, it should start.",
            ),
            tt(6.0, 9.0, "आपने ये चीज बहुत अच्छी की है"),
            tt(9.0, 14.0, "अगर मैं एक्शब लगे बढ़ा सकूर मेरे वर्षों में"),
        ];
        let english = vec![
            tt(
                0.0,
                6.1,
                "So when I click Ctrl plus Windows plus H, it should start.",
            ),
            tt(6.1, 9.0, "You have done this very well."),
            tt(9.0, 14.2, "and it should recognise my words instantly."),
        ];
        assert_eq!(devanagari_segments(&hindi), [1, 2]);
        // The language check: segment 1 is Hindi, segment 2 is English.
        let mask = [false, false, true];
        assert_eq!(
            rescue_english(&hindi, &mask, &english),
            "So, when I click Ctrl plus Windows plus H, it should start. आपने ये चीज बहुत अच्छी की है and it should recognise my words instantly."
        );
        // Nothing to rescue: the Hindi-mode text stays exactly as it was.
        assert_eq!(
            rescue_english(&hindi, &[false, false, false], &english),
            "So, when I click Ctrl plus Windows plus H, it should start. आपने ये चीज बहुत अच्छी की है अगर मैं एक्शब लगे बढ़ा सकूर मेरे वर्षों में"
        );
        // No English segment in that time: keep the original.
        assert_eq!(
            rescue_english(&hindi[2..], &[true], &english[..1]),
            "अगर मैं एक्शब लगे बढ़ा सकूर मेरे वर्षों में"
        );
    }

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
