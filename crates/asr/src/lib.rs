//! Local speech recognition. Stage 1 ships one engine: whisper.cpp.

use std::cell::RefCell;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState,
};

pub mod audio;
pub mod guard;

pub const SAMPLE_RATE: u32 = 16_000;

/// In "English + Hinglish" mode, audio counts as English when Whisper's English probability
/// is at least this. Measured 13 Nov 2026 on synthetic clips: English (US and Indian voices,
/// normal volume) 0.998-1.000; Hinglish 0.015-0.95.
pub const ENGLISH_SURE: f32 = 0.97;

/// ...and Hindi must be at least this likely to switch to Hinglish. Quiet Indian-accented
/// English drops the English score (down to 0.47) but its Hindi score stays at or below 0.001,
/// while every normal-volume Hinglish clip scored 0.015 or more.
pub const HINDI_POSSIBLE: f32 = 0.01;

/// The English-or-Hinglish decision for "English + Hinglish" mode.
pub fn is_hinglish(p_en: f32, p_hi: f32) -> bool {
    p_en < ENGLISH_SURE && p_hi >= HINDI_POSSIBLE
}

#[cfg(test)]
mod routing_tests {
    use super::is_hinglish;

    #[test]
    fn measured_cases_route_correctly() {
        // English, normal and quiet (Indian voices): English.
        for (en, hi) in [
            (1.0, 0.0),
            (0.998, 0.0),
            (0.818, 0.001),
            (0.536, 0.0),
            (0.470, 0.001),
            (0.953, 0.0),
        ] {
            assert!(!is_hinglish(en, hi), "en={en} hi={hi}");
        }
        // Hinglish at normal volume: Hinglish (lowest Hindi score seen: 0.015).
        for (en, hi) in [
            (0.015, 0.864),
            (0.433, 0.368),
            (0.838, 0.081),
            (0.763, 0.090),
            (0.920, 0.015),
        ] {
            assert!(is_hinglish(en, hi), "en={en} hi={hi}");
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub text: String,
    pub no_speech_prob: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    /// Text after the hallucination guard. Empty when nothing was said.
    pub text: String,
    pub segments: Vec<Segment>,
    /// Segments the guard removed (kept for the log, never shown as speech).
    pub dropped: Vec<String>,
    pub audio_ms: u64,
    pub elapsed_ms: u64,
}

impl Transcript {
    /// Processing time divided by audio length. Below 1.0 is faster than real time.
    pub fn real_time_factor(&self) -> Option<f64> {
        (self.audio_ms > 0).then(|| self.elapsed_ms as f64 / self.audio_ms as f64)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// ISO code such as "en" or "hi". `None` lets the model detect the language.
    pub language: Option<String>,
    /// Vocabulary hints (dictionary terms). Whisper reads only the last ~224 tokens.
    pub initial_prompt: Option<String>,
    pub threads: Option<i32>,
    /// Size the model's audio window to the clip instead of the full 30 s. Much faster for
    /// short dictations; can reduce accuracy, so it is measured before being turned on.
    pub fit_window: bool,
    /// Beam search width. None = greedy (fastest).
    pub beam_size: Option<i32>,
}

/// The graphics device to run on: a dedicated card if there is one.
///
/// Measured on the owner's laptop (13 Nov 2026), Vulkan lists the built-in AMD Radeon 740M
/// first; running there took 2.4 s for an 8 s clip, the RTX 4050 took 0.67 s.
pub fn best_gpu() -> Option<(i32, String)> {
    #[cfg(feature = "vulkan")]
    {
        let devices = whisper_rs::vulkan::list_devices();
        let score = |name: &str| {
            let n = name.to_lowercase();
            let dedicated = n.contains("nvidia")
                || n.contains("geforce")
                || n.contains("rtx")
                || n.contains("quadro")
                || n.contains("radeon rx")
                || n.contains("radeon pro")
                || n.contains("arc");
            let software = n.contains("llvmpipe")
                || n.contains("swiftshader")
                || n.contains("microsoft basic");
            (if software {
                0
            } else if dedicated {
                2
            } else {
                1
            }) as u8
        };
        return devices
            .iter()
            .max_by_key(|d| (score(&d.name), d.vram.total))
            .filter(|d| score(&d.name) > 0)
            .map(|d| (d.id, d.name.clone()));
    }
    #[allow(unreachable_code)]
    None
}

/// Encoder frames for a clip: 50 per second, plus a margin, capped at Whisper's 1500.
pub fn fitted_audio_ctx(samples: usize) -> i32 {
    let frames = samples as f64 / f64::from(SAMPLE_RATE) * 50.0;
    ((frames * 1.1) as i32 + 128).clamp(256, 1500)
}

pub struct WhisperEngine {
    /// Kept alive for as long as the state that was created from it.
    #[allow(dead_code)]
    ctx: WhisperContext,
    pub gpu: bool,
    state: RefCell<WhisperState>,
    /// Separate state for language detection. Detecting on the transcription state left a
    /// stale "no speech" score on the first 30 s window, and the guard then dropped real
    /// speech (measured 14 Nov 2026: large-v3 lost the first 28 s of two voice notes).
    detect_state: RefCell<WhisperState>,
}

impl WhisperEngine {
    /// Loads a GGML model file. With `use_gpu` the Vulkan backend is used when this build
    /// has it; a build without the `vulkan` feature always runs on the processor.
    pub fn load(model: &Path, use_gpu: bool) -> Result<Self, String> {
        whisper_rs::install_logging_hooks();
        let gpu = use_gpu && cfg!(feature = "vulkan");
        let mut params = WhisperContextParameters::default();
        params.use_gpu(gpu);
        if gpu {
            if let Some((id, _)) = best_gpu() {
                params.gpu_device(id);
            }
        }
        // Flash attention: same results, less work on the graphics card.
        params.flash_attn(gpu);
        let path = model.to_str().ok_or("model path is not valid UTF-8")?;
        let ctx = WhisperContext::new_with_params(path, params)
            .map_err(|e| format!("could not load model: {e}"))?;
        // One working state, reused by every dictation (creating it allocates GPU memory).
        let state = ctx
            .create_state()
            .map_err(|e| format!("could not start recogniser: {e}"))?;
        let detect_state = ctx
            .create_state()
            .map_err(|e| format!("could not start recogniser: {e}"))?;
        Ok(Self {
            ctx,
            gpu,
            state: RefCell::new(state),
            detect_state: RefCell::new(detect_state),
        })
    }

    /// Runs the model once on a short quiet tone so the graphics card prepares its programs
    /// now, not during the user's first dictation. The output is discarded.
    pub fn warm_up(&self) {
        let tone: Vec<f32> = (0..(SAMPLE_RATE as usize * 3 / 2))
            .map(|i| (i as f32 * 220.0 * std::f32::consts::TAU / SAMPLE_RATE as f32).sin() * 0.01)
            .collect();
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("en"));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        let _ = self.state.borrow_mut().full(params, &tone);
    }

    /// How likely each of `langs` (e.g. ["en", "hi"]) is for this audio, from Whisper's own
    /// language detector on the first 30 s.
    pub fn language_probabilities(
        &self,
        samples: &[f32],
        langs: &[&str],
    ) -> Result<Vec<f32>, String> {
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let mut padded;
        let samples = if samples.len() < SAMPLE_RATE as usize + 1600 {
            padded = samples.to_vec();
            padded.resize(SAMPLE_RATE as usize + 1600, 0.0);
            &padded[..]
        } else {
            samples
        };
        let mut state = self.detect_state.borrow_mut();
        state
            .pcm_to_mel(samples, threads)
            .map_err(|e| format!("language detection failed: {e}"))?;
        let (_, probs) = state
            .lang_detect(0, threads)
            .map_err(|e| format!("language detection failed: {e}"))?;
        Ok(langs
            .iter()
            .map(|l| {
                whisper_rs::get_lang_id(l)
                    .and_then(|id| probs.get(id as usize).copied())
                    .unwrap_or(0.0)
            })
            .collect())
    }

    /// Transcribes 16 kHz mono audio.
    pub fn transcribe(&self, samples: &[f32], options: &Options) -> Result<Transcript, String> {
        let audio_ms = samples.len() as u64 * 1000 / u64::from(SAMPLE_RATE);
        // Too short or silent: do not even ask the model, it would invent something.
        if samples.len() < (SAMPLE_RATE as usize) / 4 || !audio::has_signal(samples) {
            return Ok(Transcript {
                text: String::new(),
                segments: vec![],
                dropped: vec![],
                audio_ms,
                elapsed_ms: 0,
            });
        }
        // whisper.cpp needs at least one second of audio.
        let mut padded;
        let samples = if samples.len() < SAMPLE_RATE as usize + 1600 {
            padded = samples.to_vec();
            padded.resize(SAMPLE_RATE as usize + 1600, 0.0);
            &padded[..]
        } else {
            samples
        };

        let started = Instant::now();
        let mut state = self.state.borrow_mut();

        let strategy = match options.beam_size.filter(|b| *b > 1) {
            Some(beam_size) => SamplingStrategy::BeamSearch {
                beam_size,
                patience: -1.0,
            },
            None => SamplingStrategy::Greedy { best_of: 1 },
        };
        let mut params = FullParams::new(strategy);
        params.set_language(options.language.as_deref());
        params.set_translate(false);
        params.set_no_context(true); // condition_on_previous_text = false
        params.set_temperature(0.0);
        params.set_temperature_inc(0.2);
        params.set_no_speech_thold(0.6);
        params.set_logprob_thold(-1.0);
        params.set_entropy_thold(2.4);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        if let Some(prompt) = options.initial_prompt.as_deref().filter(|p| !p.is_empty()) {
            params.set_initial_prompt(prompt);
        }
        if let Some(t) = options.threads {
            params.set_n_threads(t);
        }
        if options.fit_window {
            params.set_audio_ctx(fitted_audio_ctx(samples.len()));
        }

        state
            .full(params, samples)
            .map_err(|e| format!("recognition failed: {e}"))?;

        let mut segments = Vec::new();
        for seg in state.as_iter() {
            segments.push(Segment {
                start_ms: seg.start_timestamp() * 10,
                end_ms: seg.end_timestamp() * 10,
                text: seg
                    .to_str_lossy()
                    .map(|c| c.into_owned())
                    .unwrap_or_default(),
                no_speech_prob: seg.no_speech_probability(),
            });
        }
        let elapsed: Duration = started.elapsed();
        let (text, dropped) = guard::filter(&segments);
        Ok(Transcript {
            text,
            segments,
            dropped,
            audio_ms,
            elapsed_ms: elapsed.as_millis() as u64,
        })
    }
}
