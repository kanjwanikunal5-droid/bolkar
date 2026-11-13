//! Transcribes every WAV in a folder with chosen settings and prints "id<TAB>text" lines,
//! ready for `wer --ref refs.tsv --hyp out.tsv`.
//!
//!   batch <model.bin> <folder> [--lang hi] [--prompt "..."] [--beam 5] [--cpu]

use std::{env, path::Path};

use app_asr::{audio, Options, WhisperEngine};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let model = &args[0];
    let dir = Path::new(&args[1]);
    let engine =
        WhisperEngine::load(Path::new(model), !args.iter().any(|a| a == "--cpu")).expect("model");
    let options = Options {
        language: flag("--lang"),
        initial_prompt: flag("--prompt"),
        beam_size: flag("--beam").and_then(|b| b.parse().ok()),
        ..Default::default()
    };
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    files.sort();
    let mut total_ms = 0;
    for f in &files {
        let samples = audio::read_wav_16k_mono(f).expect("wav");
        if args.iter().any(|a| a == "--detect") {
            let p = engine
                .language_probabilities(&samples, &["en", "hi"])
                .expect("detect");
            eprintln!(
                "{} en={:.3} hi={:.3}",
                f.file_stem().unwrap().to_string_lossy(),
                p[0],
                p[1]
            );
        }
        // --auto: English if Whisper is sure it is English, otherwise the Hinglish route
        // (Hindi recognition, script converted later), mirroring the app.
        let options = if args.iter().any(|a| a == "--auto") {
            let p = engine
                .language_probabilities(&samples, &["en", "hi"])
                .expect("detect");
            if !app_asr::is_hinglish(p[0], p[1]) {
                options.clone()
            } else {
                Options {
                    language: Some("hi".into()),
                    initial_prompt: None,
                    ..options.clone()
                }
            }
        } else {
            options.clone()
        };
        let t = engine.transcribe(&samples, &options).expect("transcribe");
        total_ms += t.elapsed_ms;
        println!(
            "{}\t{}",
            f.file_stem().unwrap().to_string_lossy(),
            t.text.replace(['\t', '\n'], " ")
        );
    }
    eprintln!("files {} | total recognition {} ms", files.len(), total_ms);
}
