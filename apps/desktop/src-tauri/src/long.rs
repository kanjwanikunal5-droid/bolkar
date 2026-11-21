//! Long dictations (more than ~40 s): recognised in pieces, all at the same time.
//!
//! Measured 21 Nov 2026 on the owner's own recordings: sending a whole 50-135 s dictation to
//! Whisper in one request silently skipped whole stretches (46 words in a row in one 80 s
//! dictation, 20 in another); the same audio in pieces of at most ~29 s, cut at quiet moments,
//! lost nothing and came back faster (all pieces in parallel). This module holds the helpers;
//! the decisions per piece are in `controller.rs`.

use std::time::Duration;

use crate::providers::{self, CallError, Config, TimedText};

/// One piece's answer: text, sentence times, time taken.
pub type PieceAnswer = Result<(String, Vec<TimedText>, Duration), CallError>;

/// Dictations longer than this are recognised in pieces.
pub const PIECES_FROM_SECS: usize = 40;
/// At most this many requests in flight at once (Groq's free plan allows about 20 per minute).
pub const MAX_IN_FLIGHT: usize = 6;

/// Seconds of the recording that carry speech-like sound (30 ms frames clearly above the
/// quietest frames). Used to judge whether a piece came back with too few words.
pub fn voiced_seconds(samples: &[f32]) -> f64 {
    let frame = 480; // 30 ms at 16 kHz
    let levels: Vec<f32> = samples
        .chunks(frame)
        .map(|c| (c.iter().map(|s| s * s).sum::<f32>() / c.len().max(1) as f32).sqrt())
        .collect();
    if levels.is_empty() {
        return 0.0;
    }
    let mut sorted = levels.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let floor = sorted[sorted.len() / 10];
    let threshold = (floor * 3.0).max(0.01);
    levels.iter().filter(|l| **l > threshold).count() as f64 * 0.03
}

/// True when a piece's text is far shorter than its speech: people say about 2-3 words per
/// second of voiced speech; under one word per voiced second (with at least 6 s of speech) a
/// stretch was almost certainly skipped.
pub fn looks_incomplete(words: usize, voiced_secs: f64) -> bool {
    voiced_secs >= 6.0 && (words as f64) < voiced_secs * 1.0
}

/// One piece to Groq, with time stamps, tried twice (the free plan allows about 20 requests a
/// minute; a piece still refused is recognised on this computer by the caller, not waited for). Also returns the upload size for the privacy log.
pub fn piece_request(
    cfg: &Config,
    samples: &[f32],
    language: Option<&str>,
    prompt: Option<&str>,
    timeout: Duration,
) -> (PieceAnswer, usize) {
    let body =
        app_asr::opus::encode_ogg_opus(samples).unwrap_or_else(|_| providers::wav_bytes(samples));
    let bytes = body.len();
    let mut last = Err(CallError::RateLimited);
    for wait in [0u64, 2] {
        if wait > 0 {
            std::thread::sleep(Duration::from_secs(wait));
        }
        last = providers::transcribe_timed(cfg, body.clone(), language, prompt, timeout);
        match &last {
            Err(CallError::RateLimited) | Err(CallError::Timeout) | Err(CallError::Network(_)) => {
                continue
            }
            _ => break,
        }
    }
    (last, bytes)
}

/// Runs `f` on every item with at most `max` running at the same time (a queue: each worker
/// takes the next item as soon as it is free); results in order.
pub fn parallel<T: Sync, R: Send>(items: &[T], max: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<R>>> = Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..max.max(1).min(items.len().max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(item) = items.get(i) else { break };
                let r = f(item);
                if let Ok(mut all) = results.lock() {
                    all[i] = Some(r);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.expect("every item ran"))
        .collect()
}

/// A piece cut in two at its quietest 100 ms between 40% and 60% of its length (for a second
/// try when a piece came back with too few words). Nothing is lost: the halves cover it all.
pub fn halves(samples: &[f32]) -> Vec<&[f32]> {
    let window = 1600;
    let (from, to) = (samples.len() * 2 / 5, samples.len() * 3 / 5);
    let mut best = samples.len() / 2;
    let mut best_level = f32::MAX;
    let mut pos = from;
    while pos + window <= to {
        let w = &samples[pos..pos + window];
        let level = w.iter().map(|x| x * x).sum::<f32>();
        if level < best_level {
            best_level = level;
            best = pos + window / 2;
        }
        pos += window / 2;
    }
    vec![&samples[..best], &samples[best..]]
}

/// Joins piece texts in order with single spaces.
pub fn join_pieces(texts: &[String]) -> String {
    texts
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(secs: f64, amp: f32) -> Vec<f32> {
        (0..(secs * 16000.0) as usize)
            .map(|i| (i as f32 * 0.07).sin() * amp)
            .collect()
    }

    #[test]
    fn voiced_time_counts_speech_not_silence() {
        let mut s = tone(10.0, 0.2);
        s.extend(vec![0.0; 16000 * 5]);
        let v = voiced_seconds(&s);
        assert!((9.0..=10.5).contains(&v), "{v}");
        assert_eq!(voiced_seconds(&[]), 0.0);
    }

    #[test]
    fn a_piece_with_far_too_few_words_is_flagged() {
        assert!(looks_incomplete(8, 20.0), "8 words for 20 s of speech");
        assert!(!looks_incomplete(45, 20.0));
        assert!(!looks_incomplete(2, 3.0), "short pieces are not judged");
    }

    #[test]
    fn parallel_keeps_order_and_runs_everything() {
        let items: Vec<usize> = (0..17).collect();
        let out = parallel(&items, 4, |i| i * 2);
        assert_eq!(out, items.iter().map(|i| i * 2).collect::<Vec<_>>());
    }

    #[test]
    fn halves_cover_the_whole_piece_and_cut_in_the_quiet() {
        let mut s = tone(10.0, 0.2);
        for x in &mut s[16000 * 5..16000 * 5 + 3200] {
            *x = 0.0;
        }
        let h = halves(&s);
        assert_eq!(h[0].len() + h[1].len(), s.len());
        let cut = h[0].len();
        assert!((16000 * 5..16000 * 5 + 3200).contains(&cut), "{cut}");
    }

    #[test]
    fn pieces_join_in_order_without_gaps() {
        let t = [
            "One two.".to_string(),
            " ".to_string(),
            "Three four.".to_string(),
        ];
        assert_eq!(join_pieces(&t), "One two. Three four.");
    }
}
