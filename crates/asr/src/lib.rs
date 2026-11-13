//! Local speech recognition. Stage 1 ships one engine: whisper.cpp.

use std::path::Path;
use std::time::{Duration, Instant};

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub mod audio;
pub mod guard;

pub const SAMPLE_RATE: u32 = 16_000;

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
}

pub struct WhisperEngine {
    ctx: WhisperContext,
    pub gpu: bool,
}

impl WhisperEngine {
    /// Loads a GGML model file. With `use_gpu` the Vulkan backend is used when this build
    /// has it; a build without the `vulkan` feature always runs on the processor.
    pub fn load(model: &Path, use_gpu: bool) -> Result<Self, String> {
        whisper_rs::install_logging_hooks();
        let gpu = use_gpu && cfg!(feature = "vulkan");
        let mut params = WhisperContextParameters::default();
        params.use_gpu(gpu);
        let path = model.to_str().ok_or("model path is not valid UTF-8")?;
        let ctx = WhisperContext::new_with_params(path, params)
            .map_err(|e| format!("could not load model: {e}"))?;
        Ok(Self { ctx, gpu })
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
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("could not start recogniser: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
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
