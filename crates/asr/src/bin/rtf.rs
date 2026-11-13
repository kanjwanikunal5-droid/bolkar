//! Measures local recognition speed (real-time factor) for one model on one WAV file.
//!
//!   rtf <model.bin> <audio.wav> [--cpu] [--lang en] [--runs 3]

use std::{env, path::Path, process::ExitCode};

use app_asr::{audio, Options, WhisperEngine};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let flag_value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if positional.len() < 2 {
        eprintln!("usage: rtf <model.bin> <audio.wav> [--cpu] [--lang en] [--runs 3]");
        return ExitCode::FAILURE;
    }
    // --lang and --runs values are positional-looking; strip them.
    let lang = flag_value("--lang");
    let runs: usize = flag_value("--runs")
        .and_then(|r| r.parse().ok())
        .unwrap_or(3);
    let use_gpu = !args.iter().any(|a| a == "--cpu");
    let (model, wav) = (positional[0], positional[1]);

    let samples = match audio::read_wav_16k_mono(Path::new(wav)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let load_started = std::time::Instant::now();
    let engine = match WhisperEngine::load(Path::new(model), use_gpu) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let load_ms = load_started.elapsed().as_millis();
    if engine.gpu {
        println!("device: {:?}", app_asr::best_gpu());
    }
    let options = Options {
        language: lang,
        fit_window: args.iter().any(|a| a == "--fit"),
        initial_prompt: flag_value("--prompt"),
        ..Default::default()
    };

    let mut times = Vec::new();
    let mut text = String::new();
    for _ in 0..runs {
        match engine.transcribe(&samples, &options) {
            Ok(t) => {
                times.push(t.elapsed_ms);
                text = t.text;
            }
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    }
    let audio_ms = samples.len() as u64 * 1000 / 16_000;
    times.sort_unstable();
    let median = times[times.len() / 2];
    println!(
        "backend={} load_ms={} audio_ms={} first_ms={} median_ms={} rtf={:.3}",
        if engine.gpu { "vulkan" } else { "cpu" },
        load_ms,
        audio_ms,
        times.first().copied().unwrap_or(0),
        median,
        median as f64 / audio_ms.max(1) as f64
    );
    println!("text: {text}");
    ExitCode::SUCCESS
}
