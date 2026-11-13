//! Hallucination guard. Whisper is known to invent text on silence
//! ("Thank you for watching") and to loop. This removes those before anything is typed.

use std::sync::LazyLock;

use regex::Regex;

use crate::Segment;

/// Phrases Whisper produces on silence. A segment is dropped only when it consists of
/// nothing but one of these AND the model itself thought the audio might be silence.
const SILENCE_PHRASES: &[&str] = &[
    "thank you for watching",
    "thanks for watching",
    "thank you so much for watching",
    "thank you",
    "thanks",
    "please subscribe",
    "subscribe to my channel",
    "like and subscribe",
    "subtitles by the amara org community",
    "subtitles by",
    "you",
    "bye",
    "देखने के लिए धन्यवाद",
    "धन्यवाद",
    "सब्सक्राइब करें",
];

/// Above this the model itself believes the segment is not speech.
const NO_SPEECH_DROP: f32 = 0.6;
/// A silence phrase is suspicious from this level up.
const NO_SPEECH_SUSPICIOUS: f32 = 0.3;

static BRACKETED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\[(]\s*(?:music|silence|blank[ _]audio|applause|noise|inaudible|laughter|no audio)\s*[\])]|♪+").unwrap()
});
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

fn bare(s: &str) -> String {
    let lower = s.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| {
            if c.is_alphanumeric()
                || c.is_whitespace()
                || (('\u{0900}'..='\u{097F}').contains(&c) && c != '\u{0964}' && c != '\u{0965}')
            {
                c
            } else {
                ' '
            }
        })
        .collect();
    SPACES.replace_all(cleaned.trim(), " ").into_owned()
}

fn is_silence_phrase(text: &str) -> bool {
    let b = bare(text);
    b.is_empty() || SILENCE_PHRASES.contains(&b.as_str())
}

/// Collapses a phrase of 1-6 words repeated three or more times in a row into one copy.
pub fn collapse_repeats(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let key = |w: &str| {
        w.trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase()
    };
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let mut collapsed = false;
        for n in (1..=6).rev() {
            if i + n * 3 > words.len() {
                continue;
            }
            let base: Vec<String> = words[i..i + n].iter().map(|w| key(w)).collect();
            if base.iter().all(|w| w.is_empty()) {
                continue;
            }
            let mut reps = 1;
            while i + (reps + 1) * n <= words.len()
                && words[i + reps * n..i + (reps + 1) * n]
                    .iter()
                    .map(|w| key(w))
                    .eq(base.iter().cloned())
            {
                reps += 1;
            }
            if reps >= 3 {
                out.extend_from_slice(&words[i + (reps - 1) * n..i + reps * n]);
                i += reps * n;
                collapsed = true;
                break;
            }
        }
        if !collapsed {
            out.push(words[i]);
            i += 1;
        }
    }
    out.join(" ")
}

/// Guard for text from a cloud recogniser, which returns no per-segment confidence.
/// `quiet` = the recording was very quiet overall, which is when Whisper invents phrases.
pub fn filter_text(text: &str, quiet: bool) -> String {
    let cleaned = BRACKETED.replace_all(text, " ");
    let cleaned = SPACES.replace_all(cleaned.trim(), " ").into_owned();
    if cleaned.is_empty() || (quiet && is_silence_phrase(&cleaned)) {
        return String::new();
    }
    collapse_repeats(&cleaned)
}

/// Returns the kept text and the list of dropped segment texts.
pub fn filter(segments: &[Segment]) -> (String, Vec<String>) {
    let mut kept: Vec<String> = Vec::new();
    let mut dropped = Vec::new();
    for seg in segments {
        let text = BRACKETED.replace_all(&seg.text, " ");
        let text = SPACES.replace_all(text.trim(), " ").into_owned();
        if text.is_empty() {
            continue;
        }
        let drop = seg.no_speech_prob > NO_SPEECH_DROP
            || (seg.no_speech_prob > NO_SPEECH_SUSPICIOUS && is_silence_phrase(&text));
        if drop {
            dropped.push(text);
        } else {
            kept.push(text);
        }
    }
    // The whole result being one silence phrase is suspicious even at low no-speech
    // probability only when the model was unsure; a real "Thank you." must survive.
    let joined = collapse_repeats(&kept.join(" "));
    (joined, dropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(text: &str, p: f32) -> Segment {
        Segment {
            start_ms: 0,
            end_ms: 1000,
            text: text.into(),
            no_speech_prob: p,
        }
    }

    #[test]
    fn drops_silence_hallucinations() {
        let (text, dropped) = filter(&[
            seg(" Thank you for watching!", 0.7),
            seg(" [BLANK_AUDIO]", 0.1),
            seg(" धन्यवाद।", 0.5),
        ]);
        assert_eq!(text, "");
        assert_eq!(dropped.len(), 2);
    }

    #[test]
    fn a_real_thank_you_survives() {
        let (text, dropped) = filter(&[seg(" Thank you.", 0.02)]);
        assert_eq!(text, "Thank you.");
        assert!(dropped.is_empty());
    }

    #[test]
    fn keeps_normal_speech_and_joins_segments() {
        let (text, _) = filter(&[
            seg(" Please send the report", 0.01),
            seg(" by Friday.", 0.02),
        ]);
        assert_eq!(text, "Please send the report by Friday.");
    }

    #[test]
    fn cloud_text_guard() {
        assert_eq!(filter_text(" Thank you for watching! ", true), "");
        assert_eq!(filter_text("Thank you.", false), "Thank you.");
        assert_eq!(
            filter_text("[Music] Send it today.", false),
            "Send it today."
        );
    }

    #[test]
    fn collapses_loops_but_not_emphasis() {
        assert_eq!(
            collapse_repeats("I think I think I think we should go"),
            "I think we should go"
        );
        assert_eq!(collapse_repeats("the the the report"), "the report");
        assert_eq!(collapse_repeats("very very good"), "very very good");
        assert_eq!(
            collapse_repeats("no change here at all"),
            "no change here at all"
        );
    }
}
